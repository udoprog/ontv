mod message;
mod model;
mod page;
mod params;
mod service;
mod thetvdb;
mod utils;

use std::time::Duration;

use anyhow::Result;
use iced::theme::{self, Theme};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Application, Command, Element, Length, Settings};
use iced_native::image::Handle;
use utils::Singleton;

use crate::message::{Message, Page, ThemeType};
use crate::model::Image;
use crate::params::{CONTAINER_WIDTH, GAP, GAP2, SPACE};
use crate::service::Service;
use crate::utils::Timeout;

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let (service, settings) = Service::new()?;
    Main::run(Settings::with_flags(Flags { service, settings }))?;
    Ok(())
}

struct Main {
    service: Service,
    page: Page,
    dashboard: page::dashboard::Dashboard,
    settings: page::settings::Settings,
    search: page::search::Search,
    series: page::series::Series,
    series_list: page::series_list::SeriesList,
    season: page::season::Season,
    loading: bool,
    save_timeout: Timeout,
    /// Image loader future being run.
    image_loader: Singleton,
}

struct Flags {
    service: Service,
    settings: page::settings::Settings,
}

impl Application for Main {
    type Executor = iced_futures::backend::native::tokio::Executor;
    type Message = Message;
    type Theme = Theme;
    type Flags = Flags;

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let mut this = Main {
            service: flags.service,
            page: Page::Dashboard,
            loading: false,
            dashboard: page::dashboard::Dashboard::default(),
            settings: flags.settings,
            search: page::search::Search::default(),
            series: page::series::Series::default(),
            series_list: page::series_list::SeriesList::default(),
            season: page::season::Season::default(),
            save_timeout: Timeout::default(),
            image_loader: Singleton::default(),
        };

        let command = this.handle_image_loading();
        (this, command)
    }

    #[inline]
    fn title(&self) -> String {
        String::from("Styling - Iced")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        let command = match message {
            Message::Noop => {
                return Command::none();
            }
            Message::Error(error) => {
                log::error!("error: {error}");
                Command::none()
            }
            Message::SaveConfig(timed_out) => {
                if timed_out {
                    self.loading = true;
                    Command::perform(self.service.save_config(self.settings.clone()), |m| m)
                } else {
                    Command::none()
                }
            }
            Message::SavedConfig => {
                self.loading = false;
                Command::none()
            }
            Message::Navigate(page) => {
                if !matches!(page, Page::Search) {
                    self.search.image_ids.clear();
                }

                self.page = page;
                Command::none()
            }
            Message::Settings(message) => {
                if self.settings.update(message) {
                    Command::single(
                        self.save_timeout
                            .set(Duration::from_secs(2), Message::SaveConfig),
                    )
                } else {
                    self.loading = true;
                    Command::perform(self.service.save_config(self.settings.clone()), |m| m)
                }
            }
            Message::Dashboard(message) => self.dashboard.update(message),
            Message::Search(message) => self.search.update(&mut self.service, message),
            Message::SeriesDownloadToTrack(data) => {
                let load = self.handle_image_loading();
                let command = self
                    .service
                    .insert_new_series(data)
                    .map(|f| Command::perform(f, Message::from));
                Command::batch(command.into_iter().chain([load]))
            }
            Message::TrackRemote(id) => {
                if let Some(action) = self.service.set_tracked_by_remote(id) {
                    let translate = |result| match result {
                        Ok(()) => Message::Noop,
                        Err(e) => Message::error(e),
                    };

                    Command::perform(action, translate)
                } else {
                    let translate = |result| match result {
                        Ok(data) => Message::SeriesDownloadToTrack(data),
                        Err(e) => Message::error(e),
                    };

                    Command::perform(self.service.track_by_remote(id), translate)
                }
            }
            Message::Track(id) => {
                if let Some(future) = self.service.set_tracked(id) {
                    Command::perform(future, Message::from)
                } else {
                    Command::none()
                }
            }
            Message::Untrack(id) => {
                let command = self
                    .service
                    .untrack(id)
                    .map(|f| Command::perform(f, Message::from));
                Command::batch(command)
            }
            Message::ImagesLoaded(loaded) => {
                self.service.insert_loaded_images(loaded);
                return self.handle_image_loading();
            }
        };

        match self.page {
            Page::Dashboard => {
                self.dashboard.prepare(&mut self.service);
            }
            Page::Search => {
                self.search.prepare(&mut self.service);
            }
            Page::SeriesList => {
                self.series_list.prepare(&mut self.service);
            }
            Page::Series(id) => {
                self.series.prepare(&mut self.service, id);
            }
            Page::Settings => {
                self.settings.prepare(&mut self.service);
            }
            Page::Season(id, season) => {
                self.season.prepare(&mut self.service, id, season);
            }
        }

        Command::batch([self.handle_image_loading(), command])
    }

    fn view(&self) -> Element<Message> {
        if self.loading {
            return row![text("Loading")]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        let menu_item = |at: &Page, title: &'static str, page: Page| {
            let current = button(title).width(Length::Fill).style(theme::Button::Text);

            if *at == page {
                current
            } else {
                current.on_press(Message::Navigate(page))
            }
        };

        let menu = column![
            menu_item(&self.page, "Dashboard", Page::Dashboard),
            menu_item(&self.page, "Search", Page::Search),
            menu_item(&self.page, "Series", Page::SeriesList),
            menu_item(&self.page, "Settings", Page::Settings),
        ]
        .spacing(SPACE)
        .padding(GAP)
        .max_width(140);

        let content = row![menu,]
            .spacing(GAP2)
            .width(Length::Fill)
            .height(Length::Fill);

        let page = match self.page {
            Page::Dashboard => self.dashboard.view(&self.service),
            Page::Search => self.search.view(&self.service),
            Page::SeriesList => self.series_list.view(&self.service),
            Page::Series(id) => self.series.view(&self.service, id),
            Page::Settings => self.settings.view(),
            Page::Season(id, season) => self.season.view(&self.service, id, season),
        };

        let content = content.push(scrollable(
            column![container(page.width(Length::Fill)).max_width(CONTAINER_WIDTH)]
                .width(Length::Fill)
                .align_items(Alignment::Center),
        ));

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn theme(&self) -> Theme {
        match &self.settings.theme {
            ThemeType::Light => Theme::Light,
            ThemeType::Dark => Theme::Dark,
        }
    }
}

impl Main {
    fn handle_image_loading(&mut self) -> Command<Message> {
        fn translate(value: Option<Result<Vec<(Image, Handle)>>>) -> Message {
            match value {
                Some(Ok(value)) => Message::ImagesLoaded(value),
                Some(Err(e)) => Message::error(e),
                None => Message::Noop,
            }
        }

        if !self.service.new_images && !self.image_loader.is_set() {
            return Command::none();
        }

        self.service.new_images = false;

        let Some(id) = self.service.image_ids.pop_front() else {
            self.image_loader.clear();
            return Command::none();
        };

        let future = self.image_loader.set(self.service.load_image(id));
        Command::perform(future, translate)
    }
}
