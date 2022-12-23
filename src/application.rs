use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use iced::theme::{self, Theme};
use iced::widget::{button, horizontal_rule, scrollable, text, Button, Column, Row};
use iced::{Alignment, Command, Element, Length};
use iced_native::image::Handle;

use crate::assets::{Assets, ImageKey};
use crate::message::{ErrorMessage, Page};
use crate::page;
use crate::params::{ACTION_SIZE, GAP, HALF_GAP, SPACE, SUB_MENU_SIZE};
use crate::service::{Queued, Service};
use crate::state;
use crate::state::State;
use crate::utils::{Singleton, TimedOut, Timeout};

// Check for remote updates every 60 seconds.
const UPDATE_TIMEOUT: u64 = 60;
// Number of images to process in parallel.
const IMAGE_BATCH: usize = 10;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Platform-specific events.
    CloseRequested,
    Settings(page::settings::Message),
    Dashboard(page::dashboard::Message),
    Search(page::search::Message),
    SeriesList(page::series_list::Message),
    Series(page::series::Message),
    Season(page::season::Message),
    Queue(page::queue::Message),
    /// Save application changes.
    Save(TimedOut),
    /// Application state was saved.
    Saved(Result<(), ErrorMessage>),
    /// Check for updates.
    CheckForUpdates(TimedOut),
    /// Update download queue with the given items.
    UpdateDownloadQueue(Result<Vec<Queued>, ErrorMessage>),
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Navigate history by the specified stride.
    History(isize),
    /// A scroll happened.
    Scroll(f32),
    /// Images have been loaded in the background.
    ImagesLoaded(Result<Vec<(ImageKey, Handle)>, ErrorMessage>),
}

/// Main application.
pub(crate) struct Application {
    state: state::State,
    dashboard: page::Dashboard,
    settings: page::Settings,
    search: page::Search,
    series: page::Series,
    series_list: page::SeriesList,
    season: page::Season,
    queue: page::Queue,
    // Timeout before database changes are saved to the filesystem.
    database_timeout: Timeout,
    // Timeout to populate the update queue.
    update_timeout: Timeout,
    /// Image loader future being run.
    image_loader: Singleton,
    // Exit after save has been completed.
    exit_after_save: bool,
    // Should exit.
    should_exit: bool,
    // Images to load.
    images: Vec<ImageKey>,
    /// The identifier used for the main scrollable.
    scrollable_id: scrollable::Id,
}

pub(crate) struct Flags {
    pub(crate) service: Service,
}

impl iced::Application for Application {
    type Executor = iced_futures::backend::native::tokio::Executor;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut this = Application {
            state: State::new(flags.service, Assets::new()),
            dashboard: page::dashboard::Dashboard::default(),
            settings: page::settings::Settings::default(),
            search: page::search::Search::default(),
            series: page::series::Series::default(),
            series_list: page::series_list::SeriesList::default(),
            season: page::season::Season::default(),
            queue: page::queue::Queue::default(),
            database_timeout: Timeout::default(),
            update_timeout: Timeout::default(),
            image_loader: Singleton::default(),
            exit_after_save: false,
            should_exit: false,
            images: Vec::new(),
            scrollable_id: scrollable::Id::unique(),
        };

        this.prepare();
        let a = this.handle_image_loading();
        let b = Command::perform(async { TimedOut::TimedOut }, Message::CheckForUpdates);
        (this, Command::batch([a, b]))
    }

    #[inline]
    fn title(&self) -> String {
        const BASE: &str = "OnTV";

        if let Some(page) = self.state.page() {
            match page {
                Page::Dashboard => {
                    return format!("{BASE} - Dashboard");
                }
                Page::Search => {
                    return format!("{BASE} - Search");
                }
                Page::SeriesList => {
                    return format!("{BASE} - Series overview");
                }
                Page::Series(id) => {
                    if let Some(series) = self.state.service.series(&id) {
                        return format!("{BASE} - {}", series.title);
                    }
                }
                Page::Settings => {
                    return format!("{BASE} - Settings");
                }
                Page::Season(series, season) => {
                    if let Some(series) = self.state.service.series(&series) {
                        return format!("{BASE} - {} - {season}", series.title);
                    }
                }
                Page::Queue => {
                    return format!("{BASE} - Queue");
                }
            }
        }

        BASE.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        log::trace!("{message:?}");

        let command = match message {
            Message::CloseRequested => {
                self.exit_after_save = true;

                if self.database_timeout.is_set() {
                    self.database_timeout.clear();
                } else {
                    self.should_exit = true;
                }

                return Command::none();
            }
            Message::Settings(message) => self
                .settings
                .update(&mut self.state, message)
                .map(Message::Settings),
            Message::Dashboard(message) => self
                .dashboard
                .update(&mut self.state, message)
                .map(Message::Dashboard),
            Message::Search(message) => self
                .search
                .update(&mut self.state, message)
                .map(Message::Search),
            Message::SeriesList(message) => self
                .series_list
                .update(&mut self.state, message)
                .map(Message::SeriesList),
            Message::Series(message) => self
                .series
                .update(&mut self.state, message)
                .map(Message::Series),
            Message::Season(message) => self
                .season
                .update(&mut self.state, message)
                .map(Message::Season),
            Message::Queue(message) => self
                .queue
                .update(&mut self.state, message)
                .map(Message::Queue),
            Message::Save(timed_out) => {
                // To avoid a cancellation loop we need to return here.
                if !matches!(timed_out, TimedOut::TimedOut) && !self.exit_after_save {
                    return Command::none();
                }

                self.database_timeout.clear();
                self.state.set_saving(true);

                Command::perform(self.state.service.save_changes(), |result| match result {
                    Ok(()) => Message::Saved(Ok(())),
                    Err(error) => Message::Saved(Err(error.into())),
                })
            }
            Message::Saved(result) => {
                if let Err(error) = result {
                    self.state.handle_error(error);
                }

                if self.exit_after_save {
                    self.should_exit = true;
                }

                self.state.set_saving(false);
                Command::none()
            }
            Message::CheckForUpdates(timed_out) => {
                match timed_out {
                    TimedOut::TimedOut => {
                        let now = Utc::now();

                        let a = Command::perform(self.state.service.find_updates(now), |output| {
                            match output {
                                Ok(update) => Message::UpdateDownloadQueue(Ok(update)),
                                Err(error) => Message::UpdateDownloadQueue(Err(error.into())),
                            }
                        });

                        // Schedule next update.
                        let b = Command::perform(
                            self.update_timeout.set(Duration::from_secs(UPDATE_TIMEOUT)),
                            Message::CheckForUpdates,
                        );

                        Command::batch([a, b])
                    }
                    TimedOut::Cancelled => {
                        // Someone else has already scheduled the next update, so do nothing.
                        Command::none()
                    }
                }
            }
            Message::UpdateDownloadQueue(result) => {
                match result {
                    Ok(queue) => {
                        self.state.service.add_to_queue(queue);
                    }
                    Err(error) => {
                        self.state.handle_error(error);
                    }
                }

                Command::none()
            }
            Message::Navigate(page) => {
                self.state.push_history(page);
                Command::none()
            }
            Message::History(relative) => {
                self.state.history(relative);
                Command::none()
            }
            Message::Scroll(offset) => {
                self.state.history_scroll(offset);
                Command::none()
            }
            Message::ImagesLoaded(loaded) => {
                match loaded {
                    Ok(loaded) => {
                        self.state.assets.insert_images(loaded);
                    }
                    Err(error) => {
                        log::error!("error loading images: {error}");
                    }
                }

                self.image_loader.clear();
                return self.handle_image_loading();
            }
        };

        let save_database = if self.state.service.has_changes() && !self.exit_after_save {
            Command::perform(
                self.database_timeout.set(Duration::from_secs(5)),
                Message::Save,
            )
        } else {
            Command::none()
        };

        self.prepare();

        let scroll = if let Some(scroll) = self.state.take_history_scroll() {
            scrollable::snap_to(self.scrollable_id.clone(), scroll)
        } else {
            Command::none()
        };

        Command::batch([self.handle_image_loading(), save_database, command, scroll])
    }

    #[inline]
    fn subscription(&self) -> iced::Subscription<Self::Message> {
        use iced::{event, mouse, window, Event};
        return iced_native::subscription::events_with(handle_event);

        fn handle_event(event: Event, status: event::Status) -> Option<Message> {
            let event::Status::Ignored = status else {
                return None;
            };

            match event {
                Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested),
                Event::Mouse(mouse::Event::ButtonPressed(button)) => match button {
                    mouse::Button::Other(32) => Some(Message::History(-1)),
                    mouse::Button::Other(64) => Some(Message::History(1)),
                    _ => None,
                },
                _ => None,
            }
        }
    }

    #[inline]
    fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn view(&self) -> Element<Message> {
        let mut top_menu = Row::new().spacing(HALF_GAP).align_items(Alignment::Center);

        let Some(&page) = self.state.page() else {
            return text("missing history entry").into();
        };

        top_menu = top_menu.push(menu_item(&page, text("Dashboard"), Page::Dashboard));
        top_menu = top_menu.push(menu_item(&page, text("Search"), Page::Search));
        top_menu = top_menu.push(menu_item(&page, text("Series"), Page::SeriesList));
        top_menu = top_menu.push(menu_item(&page, text("Settings"), Page::Settings));

        // Build queue element.
        {
            let count = self.state.service.queue().len();

            let text = match count {
                0 => text("Queue"),
                n => text(format!("Queue ({n})")),
            };

            top_menu = top_menu.push(menu_item(&page, text, Page::Queue));
        }

        let mut menu = Column::new().push(top_menu);

        if let Page::Series(series_id) | Page::Season(series_id, _) = page {
            let mut sub_menu = Row::new();

            if let Some(series) = self.state.service.series(&series_id) {
                sub_menu = sub_menu.push(menu_item(
                    &page,
                    text(&series.title).size(SUB_MENU_SIZE),
                    Page::Series(series_id),
                ));
            }

            for season in self.state.service.seasons(&series_id) {
                let title = season.number.title();

                let (watched, total) = self
                    .state
                    .service
                    .season_watched(&series_id, &season.number);

                let mut title = Row::new().push(title.size(SUB_MENU_SIZE));

                if let Some(p) = watched.saturating_mul(100).checked_div(total) {
                    title = title.push(text(format!(" ({p}%)")).size(SUB_MENU_SIZE));
                }

                sub_menu = sub_menu.push(menu_item(
                    &page,
                    title,
                    Page::Season(series_id, season.number),
                ));
            }

            menu = menu.push(sub_menu.spacing(GAP));
        }

        let mut window = Column::new();

        window = window.push(
            menu.align_items(Alignment::Center)
                .spacing(GAP)
                .padding(GAP),
        );

        let page: Element<'static, Message> = match page {
            Page::Dashboard => self.dashboard.view(&self.state).map(Message::Dashboard),
            Page::Search => self.search.view(&self.state).map(Message::Search),
            Page::SeriesList => self.series_list.view(&self.state).map(Message::SeriesList),
            Page::Series(series_id) => self
                .series
                .view(&self.state, &series_id)
                .map(Message::Series),
            Page::Settings => self.settings.view(&self.state).map(Message::Settings),
            Page::Season(series_id, season) => self
                .season
                .view(&self.state, &series_id, &season)
                .map(Message::Season),
            Page::Queue => self.queue.view(&self.state).map(Message::Queue),
        };

        window = window.push(horizontal_rule(1));
        window = window.push(
            scrollable(page)
                .id(self.scrollable_id.clone())
                .on_scroll(Message::Scroll)
                .height(Length::Fill),
        );

        let mut status_bar = Row::new();
        let mut any = false;

        if self.state.is_saving() {
            status_bar = status_bar.push(
                Row::new().push(text("saving... ").size(ACTION_SIZE)).push(
                    text("please wait")
                        .style(self.state.warning_text())
                        .size(ACTION_SIZE),
                ),
            );
            any = true;
        }

        for error in self.state.errors() {
            status_bar = status_bar.push(
                text(&error.message)
                    .size(ACTION_SIZE)
                    .style(self.state.warning_text()),
            );
            any = true;
        }

        window = window.push(horizontal_rule(1));

        if any {
            window = window.push(
                status_bar
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .align_items(Alignment::Start)
                    .spacing(GAP)
                    .padding(SPACE),
            );
        }

        window
            .align_items(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    #[inline]
    fn theme(&self) -> Theme {
        self.state.service.theme().clone()
    }
}

impl Application {
    // Call prepare on the appropriate components to prepare asset loading.
    fn prepare(&mut self) {
        let Some(page) = self.state.page() else {
            return;
        };

        match *page {
            Page::Dashboard => {
                self.dashboard.prepare(&mut self.state);
            }
            Page::Search => {
                self.search.prepare(&mut self.state);
            }
            Page::SeriesList => {
                self.series_list.prepare(&mut self.state);
            }
            Page::Series(series_id) => {
                self.series.prepare(&mut self.state, &series_id);
            }
            Page::Settings => {
                self.settings.prepare(&mut self.state);
            }
            Page::Season(series_id, season) => {
                self.season.prepare(&mut self.state, &series_id, season);
            }
            Page::Queue => {
                self.queue.prepare(&mut self.state);
            }
        }

        if self.state.assets.is_cleared() {
            self.image_loader.clear();
        }

        self.state.assets.commit();
    }

    fn handle_image_loading(&mut self) -> Command<Message> {
        fn translate(value: Option<Result<Vec<(ImageKey, Handle)>>>) -> Message {
            match value {
                Some(Ok(value)) => Message::ImagesLoaded(Ok(value)),
                None => Message::ImagesLoaded(Ok(Vec::new())),
                Some(Err(e)) => Message::ImagesLoaded(Err(e.into())),
            }
        }

        if self.image_loader.is_set() {
            return Command::none();
        }

        self.images.clear();

        while self.images.len() < IMAGE_BATCH {
            let Some(key) = self.state.assets.next_image() else {
                break;
            };

            self.images.push(key);
        }

        if self.images.is_empty() {
            return Command::none();
        }

        let future = self
            .image_loader
            .set(self.state.service.load_images(&self.images));
        Command::perform(future, translate)
    }
}

/// Helper for building menu items.
fn menu_item<E>(at: &Page, element: E, page: Page) -> Button<'static, Message>
where
    Element<'static, Message>: From<E>,
{
    let current = button(element)
        .padding(0)
        .style(theme::Button::Text)
        .width(Length::Fill);

    let current = if *at == page {
        current
    } else {
        current.on_press(Message::Navigate(page))
    };

    current.width(Length::Shrink)
}
