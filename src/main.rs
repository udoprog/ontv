mod message;
mod model;
mod page;
mod params;
mod service;
mod thetvdb;
mod utils;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Error, Result};
use iced::theme::{self, Theme};
use iced::widget::{button, column, container, row, text};
use iced::{Application, Command, Element, Length, Settings};

use crate::message::{Message, Page, ThemeType};
use crate::params::{GAP2, SPACE};
use crate::service::Service;
use crate::utils::Timeout;

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let service = Service::new()?;
    Main::run(Settings::with_flags(Flags { service }))?;
    Ok(())
}

struct Main {
    service: Service,
    page: Page,
    dashboard: page::dashboard::State,
    settings: page::settings::State,
    settings_error: Option<Arc<Error>>,
    search: page::search::State,
    loading: bool,
    save_timeout: Timeout,
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
        let this = Main {
            service: flags.service,
            page: Page::Dashboard,
            loading: true,
            dashboard: page::dashboard::State::default(),
            settings: page::settings::State::default(),
            settings_error: None,
            search: page::search::State::default(),
            save_timeout: Timeout::default(),
        };

        let command = Command::perform(this.service.setup(), |out| out);
        (this, command)
    }

    fn title(&self) -> String {
        String::from("Styling - Iced")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::Noop => {}
            Message::Setup((settings, error)) => {
                self.settings = settings;
                self.settings_error = error;
                self.loading = false;
            }
            Message::SaveConfig => {
                self.loading = true;
                return Command::perform(self.service.save_config(self.settings.clone()), |m| m);
            }
            Message::SavedConfig(..) => {
                self.loading = false;
            }
            Message::Navigate(page) => {
                self.page = page;
            }
            Message::Settings(message) => {
                if page::settings::update(&mut self.settings, message) {
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
                return page::dashboard::update(&mut self.dashboard, message);
            }
            Message::Search(message) => {
                return page::search::update(
                    &self.service,
                    &mut self.search,
                    &self.settings,
                    message,
                );
            }
            Message::Error(error) => {
                log::error!("error: {error}");
            }
            Message::ImagesLoaded => {}
            Message::Track(id) => {
                log::debug!("track: {id}");
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
            menu_item(&self.page, "Settings", Page::Settings),
            menu_item(&self.page, "Search", Page::Search),
        ]
        .spacing(SPACE)
        .max_width(140);

        let mut content = row![menu,]
            .spacing(GAP2)
            .padding(GAP2)
            .width(Length::Fill)
            .height(Length::Fill);

        let content = match &self.page {
            Page::Dashboard => content.push(page::dashboard::view(&self.dashboard)),
            Page::Search => content.push(page::search::view(&self.service, &self.search)),
            Page::Settings => content.push(page::settings::view(&self.settings)),
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
