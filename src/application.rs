use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use iced::theme::{self, Theme};
use iced::widget::{button, horizontal_rule, scrollable, text, Button, Column, Row, Space};
use iced::{window, Alignment, Commands, Element, Length};
use iced_native::image::Handle;
use uuid::Uuid;

use crate::assets::{Assets, ImageKey};
use crate::error::ErrorInfo;
use crate::model::{Task, TaskFinished, TaskKind};
use crate::page;
use crate::params::{GAP, SMALL, SPACE, SUB_MENU_SIZE};
use crate::service::{NewSeries, Service};
use crate::state::{Page, State};
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
    Errors(page::errors::Message),
    /// Save application changes.
    Save(TimedOut),
    /// Application state was saved.
    Saved(Result<(), ErrorInfo>),
    /// Check for updates.
    CheckForUpdates(TimedOut),
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Navigate history by the specified stride.
    History(isize),
    /// A scroll happened.
    Scroll(f32),
    /// Images have been loaded in the background.
    ImagesLoaded(Result<Vec<(ImageKey, Handle)>, ErrorInfo>),
    /// Update download queue with the given items.
    TaskUpdateDownloadQueue(Result<Option<(TaskKind, TaskFinished)>, ErrorInfo>, Task),
    /// Task output of add series by remote.
    TaskSeriesDownloaded(Result<Option<NewSeries>, ErrorInfo>, Task),
    /// Queue processing.
    ProcessQueue(TimedOut, Uuid),
}

/// Current page state.
enum Current {
    Dashboard(page::Dashboard),
    Settings(page::Settings),
    Search(page::Search),
    Series(page::Series),
    SeriesList(page::SeriesList),
    Season(page::Season),
    Queue(page::Queue),
    Errors(page::Errors),
}

/// Main application.
pub(crate) struct Application {
    /// Application state.
    state: State,
    /// Current page state.
    current: Current,
    // Timeout before database changes are saved to the filesystem.
    database_timeout: Timeout,
    // Timeout to populate the update queue.
    update_timeout: Timeout,
    // Timeout until the next queue should wakeup.
    queue_timeout: Timeout,
    /// Image loader future being run.
    image_loader: Singleton,
    // Exit after save has been completed.
    exit_after_save: bool,
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

    fn new(flags: Self::Flags, mut commands: impl Commands<Self::Message>) -> Self {
        let today = Utc::now().date_naive();

        let state = State::new(flags.service, Assets::new(), today);
        let current = Current::Dashboard(page::dashboard::Dashboard::new(&state));

        let mut this = Application {
            state,
            current,
            database_timeout: Timeout::default(),
            update_timeout: Timeout::default(),
            queue_timeout: Timeout::default(),
            image_loader: Singleton::default(),
            exit_after_save: false,
            images: Vec::new(),
            scrollable_id: scrollable::Id::unique(),
        };

        this.prepare();
        this.handle_image_loading(commands.by_ref());
        this.handle_process_queue(commands.by_ref(), None);
        commands.perform(async { TimedOut::TimedOut }, Message::CheckForUpdates);
        this
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
                Page::Errors => {
                    let errors = self.state.errors().len();
                    return format!("{BASE} - Errors ({errors})");
                }
            }
        }

        BASE.to_string()
    }

    fn update(&mut self, message: Message, mut commands: impl Commands<Self::Message>) {
        log::trace!("{message:?}");

        match (message, &mut self.current) {
            (Message::Settings(message), Current::Settings(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::Dashboard(message), Current::Dashboard(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::Search(message), Current::Search(page)) => {
                page.update(
                    &mut self.state,
                    message,
                    commands.by_ref().map(Message::Search),
                );
            }
            (Message::SeriesList(message), Current::SeriesList(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::Series(message), Current::Series(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::Season(message), Current::Season(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::Queue(message), Current::Queue(page)) => {
                page.update(
                    &mut self.state,
                    message,
                    commands.by_ref().map(Message::Queue),
                );
            }
            (Message::Errors(message), Current::Errors(page)) => {
                page.update(&mut self.state, message);
            }
            (Message::CloseRequested, _) => {
                log::debug!("Close requested");

                self.exit_after_save = true;

                if self.database_timeout.is_set() {
                    self.database_timeout.clear();
                } else {
                    commands.command(window::close());
                }

                return;
            }
            (Message::Save(timed_out), _) => {
                // To avoid a cancellation loop we need to return here.
                if !matches!(timed_out, TimedOut::TimedOut) && !self.exit_after_save {
                    return;
                }

                self.database_timeout.clear();
                self.state.set_saving(true);

                commands.perform(self.state.service.save_changes(), |result| match result {
                    Ok(()) => Message::Saved(Ok(())),
                    Err(error) => Message::Saved(Err(error.into())),
                })
            }
            (Message::Saved(result), _) => {
                if let Err(error) = result {
                    self.state.handle_error(error);
                }

                if self.exit_after_save {
                    commands.command(window::close());
                }

                self.state.set_saving(false);
            }
            (Message::CheckForUpdates(TimedOut::TimedOut), _) => {
                let now = Utc::now();
                self.state.service.find_updates(&now);

                let today = now.date_naive();

                if *self.state.today() != today {
                    self.state.set_today(today);
                }

                // Schedule next update.
                commands.perform(
                    self.update_timeout.set(Duration::from_secs(UPDATE_TIMEOUT)),
                    Message::CheckForUpdates,
                );
            }
            (Message::TaskUpdateDownloadQueue(result, task), _) => match result {
                Ok(queue) => {
                    self.state.service.push_tasks(
                        queue
                            .into_iter()
                            .map(|(kind, finished)| (kind, Some(finished))),
                    );
                }
                Err(error) => {
                    self.state.handle_error(error);
                    self.state.service.complete_task(task);
                }
            },
            (Message::Navigate(page), _) => {
                self.state.push_history(page);
            }
            (Message::History(relative), _) => {
                self.state.history(relative);
            }
            (Message::Scroll(offset), _) => {
                self.state.history_scroll(offset);
            }
            (Message::ImagesLoaded(loaded), _) => {
                match loaded {
                    Ok(loaded) => {
                        self.state.assets.insert_images(loaded);
                    }
                    Err(error) => {
                        log::error!("error loading images: {error}");
                    }
                }

                self.image_loader.clear();
                self.handle_image_loading(&mut commands);
                return;
            }
            (Message::TaskSeriesDownloaded(result, task), _) => {
                match result {
                    Ok(new_series) => {
                        if let Some(new_series) = new_series {
                            self.state.service.insert_new_series(new_series);
                        }
                    }
                    Err(error) => {
                        self.state.handle_error(error);
                    }
                }

                self.state.service.complete_task(task);
            }
            (Message::ProcessQueue(TimedOut::TimedOut, id), _) => {
                self.handle_process_queue(&mut commands, Some(id));
            }
            _ => {}
        };

        if self.state.service.has_changes() && !self.exit_after_save {
            commands.perform(
                self.database_timeout.set(Duration::from_secs(5)),
                Message::Save,
            );
        }

        if let Some((page, scroll)) = self.state.history_change() {
            self.current = match page {
                Page::Dashboard => Current::Dashboard(page::Dashboard::new(&self.state)),
                Page::Search => Current::Search(page::Search::default()),
                Page::SeriesList => Current::SeriesList(page::SeriesList::default()),
                Page::Series(series_id) => Current::Series(page::Series::new(series_id)),
                Page::Settings => Current::Settings(page::Settings::default()),
                Page::Season(series_id, season) => {
                    Current::Season(page::Season::new(series_id, season))
                }
                Page::Queue => {
                    let page = page::Queue::new(commands.by_ref().map(Message::Queue));
                    Current::Queue(page)
                }
                Page::Errors => Current::Errors(page::Errors::default()),
            };

            commands.command(scrollable::snap_to(self.scrollable_id.clone(), scroll));
        }

        self.prepare();

        self.handle_image_loading(commands.by_ref());
        self.handle_setup_queue(commands.by_ref());
    }

    #[inline]
    fn subscription(&self) -> iced::Subscription<Self::Message> {
        use iced::{event, mouse, Event};
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

    fn view(&self) -> Element<Message> {
        let mut top_menu = Row::new().spacing(GAP).align_items(Alignment::Center);

        let Some(&page) = self.state.page() else {
            return text("missing history entry").into();
        };

        top_menu = top_menu.push(menu_item(&page, text("Dashboard"), Page::Dashboard));
        top_menu = top_menu.push(menu_item(&page, text("Series"), Page::SeriesList));
        top_menu = top_menu.push(menu_item(&page, text("Search"), Page::Search));
        top_menu = top_menu.push(menu_item(&page, text("Settings"), Page::Settings));

        // Build queue element.
        {
            let count = self.state.service.tasks().len();

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
                let title = text(season.number);

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

        let page: Element<'static, Message> = match &self.current {
            Current::Dashboard(page) => page.view(&self.state).map(Message::Dashboard),
            Current::Search(page) => page.view(&self.state).map(Message::Search),
            Current::SeriesList(page) => page.view(&self.state).map(Message::SeriesList),
            Current::Series(page) => page.view(&self.state).map(Message::Series),
            Current::Settings(page) => page.view(&self.state).map(Message::Settings),
            Current::Season(page) => page.view(&self.state).map(Message::Season),
            Current::Queue(page) => page.view(&self.state).map(Message::Queue),
            Current::Errors(page) => page.view(&self.state).map(Message::Errors),
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
            status_bar = status_bar.push(Row::new().push(text("Saving... ").size(SMALL)));
            any = true;
        }

        status_bar = status_bar.push(Space::new(Length::Fill, Length::Shrink));

        let errors = self.state.errors().len();

        if errors != 0 {
            status_bar = status_bar.push(
                button(text(format!("Errors ({errors})")).size(SMALL))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Navigate(Page::Errors)),
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
        match &mut self.current {
            Current::Dashboard(page) => {
                page.prepare(&mut self.state);
            }
            Current::Search(page) => {
                page.prepare(&mut self.state);
            }
            Current::SeriesList(page) => {
                page.prepare(&mut self.state);
            }
            Current::Series(page) => {
                page.prepare(&mut self.state);
            }
            Current::Settings(page) => {
                page.prepare(&mut self.state);
            }
            Current::Season(page) => {
                page.prepare(&mut self.state);
            }
            Current::Queue(page) => {
                page.prepare(&mut self.state);
            }
            Current::Errors(page) => {
                page.prepare(&mut self.state);
            }
        }

        if self.state.assets.is_cleared() {
            self.image_loader.clear();
        }

        self.state.assets.commit();
    }

    /// Handle image loading.
    fn handle_image_loading(&mut self, mut commands: impl Commands<Message>) {
        fn translate(value: Option<Result<Vec<(ImageKey, Handle)>>>) -> Message {
            match value {
                Some(Ok(value)) => Message::ImagesLoaded(Ok(value)),
                None => Message::ImagesLoaded(Ok(Vec::new())),
                Some(Err(e)) => Message::ImagesLoaded(Err(e.into())),
            }
        }

        if self.image_loader.is_set() {
            return;
        }

        self.images.clear();

        while self.images.len() < IMAGE_BATCH {
            let Some(key) = self.state.assets.next_image() else {
                break;
            };

            self.images.push(key);
        }

        if self.images.is_empty() {
            return;
        }

        let future = self
            .image_loader
            .set(self.state.service.load_images(&self.images));
        commands.perform(future, translate);
    }

    /// Setup queue processing.
    fn handle_setup_queue(&mut self, commands: impl Commands<Message>) {
        if self.state.service.take_tasks_modified() {
            self.handle_process_queue(commands, None)
        }
    }

    /// Handle process queue.
    fn handle_process_queue(
        &mut self,
        mut commands: impl Commands<Message>,
        timed_out: Option<Uuid>,
    ) {
        let now = Utc::now();

        while let Some(task) = self.state.service.next_task(&now, timed_out) {
            log::trace!("running task {}", task.id);

            match &task.kind {
                TaskKind::CheckForUpdates {
                    series_id,
                    remote_id,
                } => {
                    if let Some(future) = self.state.service.check_for_updates(series_id, remote_id)
                    {
                        commands.perform(future, move |output| match output {
                            Ok(update) => {
                                Message::TaskUpdateDownloadQueue(Ok(update), task.clone())
                            }
                            Err(error) => {
                                Message::TaskUpdateDownloadQueue(Err(error.into()), task.clone())
                            }
                        });
                    } else {
                        self.state.service.complete_task(task);
                    }
                }
                TaskKind::DownloadSeriesById { series_id } => {
                    if let Some(future) = self.state.refresh_series(&series_id) {
                        commands.perform(future, move |result| match result {
                            Ok(new_series) => {
                                Message::TaskSeriesDownloaded(Ok(new_series), task.clone())
                            }
                            Err(error) => {
                                Message::TaskSeriesDownloaded(Err(error.into()), task.clone())
                            }
                        });
                    } else {
                        self.state.service.complete_task(task);
                    }
                }
                TaskKind::DownloadSeriesByRemoteId { remote_id } => {
                    if self.state.service.set_tracked_by_remote(&remote_id) {
                        self.state.service.complete_task(task);
                    } else {
                        commands.perform(
                            self.state
                                .service
                                .download_series_by_remote(&remote_id, None),
                            move |result| {
                                Message::TaskSeriesDownloaded(
                                    result.map_err(Into::into),
                                    task.clone(),
                                )
                            },
                        );
                    }
                }
            }
        }

        let now = Utc::now();

        if let Some((seconds, id)) = self.state.service.next_task_sleep(&now) {
            log::trace!("next queue sleep: {seconds}s");

            commands.perform(
                self.queue_timeout.set(Duration::from_secs(seconds)),
                move |timed_out| Message::ProcessQueue(timed_out, id),
            );
        }
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
