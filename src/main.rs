#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

mod api;
mod assets;
mod cache;
mod comps;
mod import;
mod message;
mod model;
mod page;
mod params;
mod search;
mod service;
mod state;
mod utils;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use iced::theme::{self, Theme};
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, Button, Space,
};
use iced::{Alignment, Application, Command, Element, Length, Settings};
use iced_native::image::Handle;

use params::{ACTION_SIZE, WARNING_COLOR};
use utils::Singleton;

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::model::{Image, ThemeType};
use crate::params::{GAP, GAP2, HALF_GAP, SPACE, SUB_MENU_SIZE};
use crate::service::Service;
use crate::state::State;
use crate::utils::{TimedOut, Timeout};

// Check for remote updates every 60 seconds.
const UPDATE_TIMEOUT: u64 = 60;

#[derive(Parser)]
struct Opts {
    /// Import watch history from trakt.
    #[arg(long, name = "path")]
    import_trakt_watched: Option<PathBuf>,
    /// Only import a show matching the given filter.
    #[arg(long, name = "string")]
    import_filter: Option<String>,
    /// Override any existing watch history.
    #[arg(long)]
    import_remove: bool,
    /// Import any missing shows encountered.
    #[arg(long)]
    import_missing: bool,
    /// Ensure that import history is saved.
    #[arg(long)]
    import_test: bool,
    /// Don't save anything.
    #[arg(long)]
    test: bool,
}

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let mut service = Service::new()?;

    let opts = Opts::try_parse()?;

    if opts.test {
        service.do_not_save();
    }

    if let Some(path) = opts.import_trakt_watched {
        crate::import::import_trakt_watched(
            &mut service,
            &path,
            opts.import_filter.as_deref(),
            opts.import_remove,
            opts.import_missing,
        )?;
    }

    let mut settings = Settings::with_flags(Flags { service });
    settings.exit_on_close_request = false;
    Main::run(settings)?;
    Ok(())
}

struct Main {
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
}

struct Flags {
    service: Service,
}

impl Application for Main {
    type Executor = iced_futures::backend::native::tokio::Executor;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut this = Main {
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
        };

        this.prepare();
        let a = this.handle_image_loading();
        let b = Command::perform(async { TimedOut::TimedOut }, Message::CheckForUpdates);
        (this, Command::batch([a, b]))
    }

    #[inline]
    fn title(&self) -> String {
        String::from("Styling - Iced")
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
            Message::Settings(message) => self.settings.update(&mut self.state, message),
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
            Message::Noop => {
                return Command::none();
            }
            Message::Error(error) => {
                self.state.handle_error(error);
                Command::none()
            }
            Message::Save(timed_out) => {
                // To avoid a cancellation loop we need to return here.
                if !matches!(timed_out, TimedOut::TimedOut) && !self.exit_after_save {
                    return Command::none();
                }

                self.database_timeout.clear();
                self.state.set_loading(true);

                Command::perform(self.state.service.save_changes(), |result| match result {
                    Ok(()) => Message::Saved,
                    Err(error) => Message::error(error),
                })
            }
            Message::Saved => {
                if self.exit_after_save {
                    self.should_exit = true;
                }

                self.state.set_loading(false);
                Command::none()
            }
            Message::CheckForUpdates(timed_out) => {
                match timed_out {
                    TimedOut::TimedOut => {
                        let now = Utc::now();

                        let a = Command::perform(self.state.service.find_updates(now), |output| {
                            match output {
                                Ok(update) => Message::UpdateDownloadQueue(update),
                                Err(error) => Message::error(error),
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
            Message::UpdateDownloadQueue(queue) => {
                self.state.service.add_to_queue(queue);
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
        Command::batch([self.handle_image_loading(), save_database, command])
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
        let mut menu = column![].spacing(HALF_GAP).padding(GAP).max_width(200);

        let Some(&page) = self.state.page() else {
            return text("missing history entry").into();
        };

        menu = menu.push(menu_item(&page, text("Dashboard"), Page::Dashboard));
        menu = menu.push(menu_item(&page, text("Search"), Page::Search));
        menu = menu.push(menu_item(&page, text("Series"), Page::SeriesList));

        if let Page::Series(series_id) | Page::Season(series_id, _) = page {
            let mut sub_menu = column![];

            if let Some(series) = self.state.service.series(&series_id) {
                sub_menu = sub_menu.push(row![
                    Space::new(Length::Units(SPACE), Length::Shrink),
                    menu_item(
                        &page,
                        text(&series.title).size(SUB_MENU_SIZE),
                        Page::Series(series_id)
                    )
                ]);
            }

            for season in self.state.service.seasons(&series_id) {
                let title = season.number.title();
                let (watched, total) = self
                    .state
                    .service
                    .season_watched(&series_id, &season.number);

                let mut title = row![title.size(SUB_MENU_SIZE)];

                if let Some(p) = watched.saturating_mul(100).checked_div(total) {
                    title = title
                        .push(text(format!(" - {p}% ({watched}/{total})")).size(SUB_MENU_SIZE));
                }

                sub_menu = sub_menu.push(row![
                    Space::new(Length::Units(HALF_GAP), Length::Shrink),
                    menu_item(&page, title, Page::Season(series_id, season.number),)
                ]);
            }

            menu = menu.push(sub_menu.spacing(SPACE));
        }

        menu = menu.push(menu_item(&page, text("Settings"), Page::Settings));

        // Build queue element.
        {
            let count = self.state.service.queue().len();

            let text = match count {
                0 => text("Queue"),
                n => text(format!("Queue ({n})")),
            };

            menu = menu.push(menu_item(&page, text, Page::Downloads));
        }

        let content = row![menu]
            .spacing(GAP2)
            .width(Length::Fill)
            .height(Length::Fill);

        let page: Element<'static, Message> = match page {
            Page::Dashboard => self.dashboard.view(&self.state).map(Message::Dashboard),
            Page::Search => self.search.view(&self.state).map(Message::Search),
            Page::SeriesList => self.series_list.view(&self.state).map(Message::SeriesList),
            Page::Series(series_id) => self
                .series
                .view(&self.state, &series_id)
                .map(Message::Series),
            Page::Settings => self.settings.view(&self.state).into(),
            Page::Season(series_id, season) => self
                .season
                .view(&self.state, &series_id, &season)
                .map(Message::Season),
            Page::Downloads => self.queue.view(&self.state).into(),
        };

        let content = content.push(scrollable(page));

        let mut window = column![];

        window = window.push(content);

        let mut status_bar = row![];

        if self.state.is_loading() {
            status_bar = status_bar.push(text("loading...").size(ACTION_SIZE));
        } else {
            status_bar = status_bar.push(text("idle").size(ACTION_SIZE));
        }

        for error in self.state.errors() {
            status_bar = status_bar.push(
                text(&error.message)
                    .size(ACTION_SIZE)
                    .style(theme::Text::Color(WARNING_COLOR)),
            );
        }

        window = window.push(horizontal_rule(1));
        window = window.push(
            status_bar
                .width(Length::Fill)
                .height(Length::Shrink)
                .align_items(Alignment::Start)
                .spacing(GAP)
                .padding(SPACE),
        );

        container(window)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        match self.state.service.config().theme {
            ThemeType::Light => Theme::Light,
            ThemeType::Dark => Theme::Dark,
        }
    }
}

impl Main {
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
            Page::Downloads => {
                self.queue.prepare(&mut self.state);
            }
        }

        if self.state.assets.is_cleared() {
            self.image_loader.clear();
        }

        self.state.assets.commit();
    }

    fn handle_image_loading(&mut self) -> Command<Message> {
        fn translate(value: Option<Result<Vec<(Image, Handle)>>>) -> Message {
            match value {
                Some(Ok(value)) => Message::ImagesLoaded(Ok(value)),
                Some(Err(e)) => Message::ImagesLoaded(Err(e.into())),
                None => Message::Noop,
            }
        }

        if self.image_loader.is_set() {
            return Command::none();
        }

        let Some(id) = self.state.assets.next_image() else {
            return Command::none();
        };

        let future = self.image_loader.set(self.state.service.load_image(id));
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

    if *at == page {
        current
    } else {
        current.on_press(Message::Navigate(page))
    }
}
