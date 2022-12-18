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
use iced::widget::{button, column, container, row, text};
use iced::{Application, Command, Element, Length, Settings};
use iced_native::image::Handle;

use crate::message::{Message, Page, ThemeType};
use crate::model::Image;
use crate::params::{GAP2, SPACE};
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
    dashboard: page::dashboard::State,
    settings: page::settings::State,
    search: page::search::State,
    loading: bool,
    save_timeout: Timeout,
}

struct Flags {
    service: Service,
    settings: page::settings::State,
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
            dashboard: page::dashboard::State::default(),
            settings: flags.settings,
            search: page::search::State::default(),
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
            Message::SeriesTracked(id, series, loaded) => {
                self.service.insert_loaded_images(loaded);
                let command = self
                    .service
                    .track(id, series)
                    .map(|f| Command::perform(f, Message::from));
                return Command::batch(command);
            }
            Message::Track(id) => {
                let translate = |result| match result {
                    Ok((id, series, output)) => Message::SeriesTracked(id, series, output),
                    Err(e) => Message::error(e),
                };

                return Command::perform(self.service.track_thetvdb(id), translate);
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
            menu_item(&self.page, "Settings", Page::Settings),
        ]
        .spacing(SPACE)
        .max_width(140);

        let content = row![menu,]
            .spacing(GAP2)
            .padding(GAP2)
            .width(Length::Fill)
            .height(Length::Fill);

        let content = match &self.page {
            Page::Dashboard => content.push(self.dashboard.view(&self.service)),
            Page::Search => content.push(self.search.view(&self.service)),
            Page::Settings => content.push(self.settings.view()),
        };

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
