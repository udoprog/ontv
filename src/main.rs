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
        fn translate(result: Result<Vec<(Image, Handle)>>) -> Message {
            match result {
                Ok(output) => Message::Loaded(output),
                Err(e) => Message::error(e),
            }
        }

        let this = Main {
            service: flags.service,
            page: Page::Dashboard,
            loading: true,
            dashboard: page::dashboard::Dashboard::default(),
            settings: flags.settings,
            search: page::search::Search::default(),
            series: page::series::Series::default(),
            series_list: page::series_list::SeriesList::default(),
            season: page::season::Season::default(),
            save_timeout: Timeout::default(),
        };

        let command = Command::perform(this.service.setup(), translate);
        (this, command)
    }

    #[inline]
    fn title(&self) -> String {
        String::from("Styling - Iced")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::Noop => {}
            Message::Loaded(loaded) => {
                self.service.insert_loaded_images(loaded);
                self.loading = false;
            }
            Message::Error(error) => {
                log::error!("error: {error}");
            }
            Message::SaveConfig => {
                self.loading = true;
                return Command::perform(self.service.save_config(self.settings.clone()), |m| m);
            }
            Message::SavedConfig => {
                self.loading = false;
            }
            Message::Navigate(page) => {
                if !matches!(page, Page::Search) {
                    self.search.image_ids.clear();
                }

                self.page = page;
            }
            Message::Settings(message) => {
                if self.settings.update(message) {
                    return Command::single(
                        self.save_timeout
                            .set(Duration::from_secs(2), Message::SaveConfig),
                    );
                } else {
                    self.loading = true;
                    return Command::perform(
                        self.service.save_config(self.settings.clone()),
                        |m| m,
                    );
                }
            }
            Message::Dashboard(message) => {
                return self.dashboard.update(message);
            }
            Message::Search(message) => {
                return self.search.update(&mut self.service, message);
            }
            Message::SeriesDownloadToTrack(data, loaded) => {
                self.service.insert_loaded_images(loaded);
                let command = self
                    .service
                    .insert_new_series(data)
                    .map(|f| Command::perform(f, Message::from));
                return Command::batch(command);
            }
            Message::TrackRemote(id) => {
                return if let Some(action) = self.service.set_tracked_by_remote(id) {
                    let translate = |result| match result {
                        Ok(()) => Message::Noop,
                        Err(e) => Message::error(e),
                    };

                    Command::perform(action, translate)
                } else {
                    let translate = |result| match result {
                        Ok((data, output)) => Message::SeriesDownloadToTrack(data, output),
                        Err(e) => Message::error(e),
                    };

                    Command::perform(self.service.track_by_remote(id), translate)
                };
            }
            Message::Track(id) => {
                if let Some(future) = self.service.set_tracked(id) {
                    return Command::perform(future, Message::from);
                }
            }
            Message::Untrack(id) => {
                let command = self
                    .service
                    .untrack(id)
                    .map(|f| Command::perform(f, Message::from));
                return Command::batch(command);
            }
        }

        Command::none()
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
