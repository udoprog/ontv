mod assets;
mod message;
mod model;
mod page;
mod params;
mod service;
mod thetvdb;
mod utils;

use std::time::Duration;

use anyhow::Result;
use iced::alignment::Horizontal;
use iced::theme::{self, Theme};
use iced::widget::{button, column, container, row, scrollable, text, Space, Text};
use iced::{Alignment, Application, Command, Element, Length, Settings};
use iced_native::image::Handle;
use params::WARNING_COLOR;
use utils::Singleton;

use crate::assets::Assets;
use crate::message::{Message, Page, ThemeType};
use crate::model::Image;
use crate::params::{CONTAINER_WIDTH, GAP, GAP2, NOTICE_SIZE, SPACE, SPACE2, SUB_MENU_SIZE};
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
    series: page::series::State,
    series_list: page::series_list::SeriesList,
    season: page::season::State,
    loading: bool,
    save_timeout: Timeout,
    /// Image loader future being run.
    image_loader: Singleton,
    assets: Assets,
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
            series: page::series::State::default(),
            series_list: page::series_list::SeriesList::default(),
            season: page::season::State::default(),
            save_timeout: Timeout::default(),
            image_loader: Singleton::default(),
            assets: Assets::new(),
        };

        this.prepare();
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
                self.loading = false;
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
                self.assets.clear();
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
            Message::Search(message) => {
                self.search
                    .update(&mut self.service, &mut self.assets, message)
            }
            Message::SeriesDownloadToTrack(data) => {
                self.loading = false;
                let command = self
                    .service
                    .insert_new_series(data)
                    .map(|f| Command::perform(f, Message::from));
                Command::batch(command)
            }
            Message::SeriesEdited => {
                self.loading = false;
                Command::none()
            }
            Message::RefreshSeries(id) => {
                self.loading = true;

                if let Some(future) = self.service.refresh_series(id) {
                    Command::perform(future, |result| match result {
                        Ok(new_data) => Message::SeriesDownloadToTrack(new_data),
                        Err(e) => Message::error(e),
                    })
                } else {
                    Command::none()
                }
            }
            Message::RemoveSeries(id) => {
                self.loading = true;

                Command::perform(self.service.remove_series(id), |result| match result {
                    Ok(()) => Message::SeriesEdited,
                    Err(e) => Message::error(e),
                })
            }
            Message::AddSeriesByRemote(id) => {
                if let Some(action) = self.service.set_tracked_by_remote(id) {
                    let translate = |result| match result {
                        Ok(()) => Message::Noop,
                        Err(e) => Message::error(e),
                    };

                    Command::perform(action, translate)
                } else {
                    self.loading = true;

                    let translate = |result| match result {
                        Ok(data) => Message::SeriesDownloadToTrack(data),
                        Err(e) => Message::error(e),
                    };

                    Command::perform(self.service.add_series_by_remote(id), translate)
                }
            }
            Message::WatchRemainingSeason(series, season) => {
                let timestamp = chrono::Utc::now();

                match self
                    .service
                    .watch_remaining_season(series, season, timestamp)
                {
                    Some(future) => Command::perform(future, Message::from),
                    None => Command::none(),
                }
            }
            Message::Watch(series, episode) => {
                let timestamp = chrono::Utc::now();
                Command::perform(
                    self.service.watch(series, episode, timestamp),
                    Message::from,
                )
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
                match loaded {
                    Ok(loaded) => {
                        self.assets.insert_images(loaded);
                    }
                    Err(error) => {
                        log::error!("error loading images: {error}");
                    }
                }

                self.image_loader.clear();
                return self.handle_image_loading();
            }
        };

        self.prepare();
        Command::batch([self.handle_image_loading(), command])
    }

    fn view(&self) -> Element<Message> {
        let menu_item = |at: &Page, title: Text<'static>, page: Page| {
            let current = button(title)
                .padding(0)
                .style(theme::Button::Text)
                .width(Length::Fill);

            if *at == page {
                current
            } else {
                current.on_press(Message::Navigate(page))
            }
        };

        let mut menu = column![].spacing(SPACE).padding(GAP).max_width(140);

        menu = menu.push(menu_item(&self.page, text("Dashboard"), Page::Dashboard));
        menu = menu.push(menu_item(&self.page, text("Search"), Page::Search));
        menu = menu.push(menu_item(&self.page, text("Series"), Page::SeriesList));

        if let Page::Series(id) | Page::Season(id, _) = self.page {
            if let Some(series) = self.service.series(id) {
                menu = menu.push(row![
                    Space::new(Length::Units(SPACE), Length::Shrink),
                    menu_item(
                        &self.page,
                        text(&series.title).size(SUB_MENU_SIZE),
                        Page::Series(id)
                    )
                ]);
            }

            for season in self.service.seasons(id) {
                let title = season.title();

                menu = menu.push(row![
                    Space::new(Length::Units(SPACE2), Length::Shrink),
                    menu_item(
                        &self.page,
                        title.size(SUB_MENU_SIZE),
                        Page::Season(id, season.number),
                    )
                ]);
            }
        }

        menu = menu.push(menu_item(&self.page, text("Settings"), Page::Settings));

        let content = row![menu]
            .spacing(GAP2)
            .width(Length::Fill)
            .height(Length::Fill);

        let page = match self.page {
            Page::Dashboard => self.dashboard.view(&self.service, &self.assets),
            Page::Search => self.search.view(&self.service, &self.assets),
            Page::SeriesList => self.series_list.view(&self.service, &self.assets),
            Page::Series(id) => self.series.view(&self.service, &self.assets, id),
            Page::Settings => self.settings.view(&self.assets),
            Page::Season(id, season) => self.season.view(&self.service, &self.assets, id, season),
        };

        let content = content.push(scrollable(
            column![container(page.width(Length::Fill)).max_width(CONTAINER_WIDTH)]
                .width(Length::Fill)
                .align_items(Alignment::Center),
        ));

        let mut window = column![];

        if self.loading {
            window = window.push(
                row![text("Loading")
                    .width(Length::Fill)
                    .size(NOTICE_SIZE)
                    .horizontal_alignment(Horizontal::Center)
                    .style(theme::Text::Color(WARNING_COLOR))]
                .width(Length::Fill)
                .align_items(Alignment::Center)
                .padding(GAP),
            );
        }

        window = window.push(content);

        container(window)
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
    // Call prepare on the appropriate components to prepare asset loading.
    fn prepare(&mut self) {
        match self.page {
            Page::Dashboard => {
                self.dashboard.prepare(&self.service, &mut self.assets);
            }
            Page::Search => {
                self.search.prepare(&self.service, &mut self.assets);
            }
            Page::SeriesList => {
                self.series_list.prepare(&self.service, &mut self.assets);
            }
            Page::Series(id) => {
                self.series.prepare(&self.service, &mut self.assets, id);
            }
            Page::Settings => {
                self.settings.prepare(&self.service, &mut self.assets);
            }
            Page::Season(id, season) => {
                self.season
                    .prepare(&self.service, &mut self.assets, id, season);
            }
        }

        if self.assets.is_cleared() {
            self.image_loader.clear();
        }

        self.assets.commit();
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

        let Some(id) = self.assets.image_ids.pop_front() else {
            return Command::none();
        };

        let future = self.image_loader.set(self.service.load_image(id));
        Command::perform(future, translate)
    }
}
