mod message;
mod model;
mod page;
mod params;
mod service;
mod thetvdb;
mod utils;

use std::collections::VecDeque;
use std::time::Duration;

use anyhow::Result;
use iced::theme::{self, Theme};
use iced::widget::{button, column, container, row, text};
use iced::{Application, Command, Element, Length, Settings};

use crate::message::{Message, Page, ThemeType};
use crate::model::Image;
use crate::page::search::SearchMessage;
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
    /// Image IDs to load.
    image_ids: VecDeque<Image>,
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
        let this = Main {
            service: flags.service,
            page: Page::Dashboard,
            loading: false,
            dashboard: page::dashboard::State::default(),
            settings: flags.settings,
            search: page::search::State::default(),
            save_timeout: Timeout::default(),
            image_ids: VecDeque::new(),
        };

        let command = Command::perform(this.service.setup(), |out| out);
        (this, command)
    }

    #[inline]
    fn title(&self) -> String {
        String::from("Styling - Iced")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::Noop => {}
            Message::Error(error) => {
                log::error!("error: {error}");
            }
            Message::SaveConfig => {
                self.loading = true;
                return Command::perform(self.service.save_config(self.settings.clone()), |m| m);
            }
            Message::SavedConfig(..) => {
                self.loading = false;
            }
            Message::Navigate(page) => {
                if !matches!(page, Page::Search) {
                    self.image_ids.clear();
                }

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
                let image_loading = match &message {
                    SearchMessage::Result(series) => {
                        self.image_ids.clear();
                        self.handle_image_loading(series.iter().map(|s| s.poster))
                    }
                    _ => None,
                };

                let command = page::search::update(&self.service, &mut self.search, message);
                return Command::batch([command].into_iter().chain(image_loading));
            }
            Message::SeriesTracked => {}
            Message::ImageLoaded => {
                let command = self.handle_image_loading([]);
                return Command::batch(command);
            }
            Message::Track(id) => {
                return Command::perform(self.service.track_thetvdb(id), |m| m);
            }
            Message::Untrack(id) => {
                return Command::perform(self.service.untrack(id), |m| m);
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
            Page::Dashboard => content.push(page::dashboard::view(&self.service, &self.dashboard)),
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

impl Main {
    fn handle_image_loading<I>(&mut self, iter: I) -> Option<Command<Message>>
    where
        I: IntoIterator<Item = Image>,
    {
        self.image_ids.extend(iter);
        let id = self.image_ids.pop_front()?;
        Some(Command::perform(self.service.load_image(id), |m| m))
    }
}
