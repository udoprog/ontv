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
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, Space, Text,
};
use iced::{Alignment, Application, Command, Element, Length, Settings};
use iced_native::image::Handle;
use params::ACTION_SIZE;
use utils::Singleton;

use crate::assets::Assets;
use crate::message::{Message, Page, ThemeType};
use crate::model::Image;
use crate::params::{GAP, GAP2, SPACE, SPACE2, SUB_MENU_SIZE};
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
    database_timeout: Timeout,
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
            database_timeout: Timeout::default(),
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
        log::trace!("{message:?}");

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
            Message::SaveDatabase(timed_out) => {
                // To avoid a cancellation loop we need to return here.
                if !timed_out {
                    return Command::none();
                }

                self.loading = true;

                Command::perform(self.service.save_changes(), |result| match result {
                    Ok(()) => Message::SavedDatabase,
                    Err(error) => Message::error(error),
                })
            }
            Message::SavedDatabase => {
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
                self.service.insert_new_series(data);
                Command::none()
            }
            Message::RefreshSeries(id) => {
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
                self.service.remove_series(id);
                Command::none()
            }
            Message::AddSeriesByRemote(id) => {
                if self.service.set_tracked_by_remote(id) {
                    Command::none()
                } else {
                    let translate = |result| match result {
                        Ok(data) => Message::SeriesDownloadToTrack(data),
                        Err(e) => Message::error(e),
                    };

                    Command::perform(self.service.download_series_by_remote(id), translate)
                }
            }
            Message::Watch(series, episode) => {
                let timestamp = chrono::Utc::now();
                self.service.watch(series, episode, timestamp);
                Command::none()
            }
            Message::Skip(series, episode) => {
                let timestamp = chrono::Utc::now();
                self.service.skip(series, episode, timestamp);
                Command::none()
            }
            Message::SelectPending(series, episode) => {
                let timestamp = chrono::Utc::now();
                self.service.select_pending(series, episode, timestamp);
                Command::none()
            }
            Message::RemoveEpisodeWatches(series, episode) => {
                let timestamp = chrono::Utc::now();
                self.service
                    .remove_episode_watches(series, episode, timestamp);
                Command::none()
            }
            Message::RemoveSeasonWatches(series, season) => {
                let timestamp = chrono::Utc::now();
                self.service
                    .remove_season_watches(series, season, timestamp);
                Command::none()
            }
            Message::WatchRemainingSeason(series, season) => {
                let timestamp = chrono::Utc::now();
                self.service
                    .watch_remaining_season(series, season, timestamp);
                Command::none()
            }
            Message::Track(id) => {
                self.service.track(id);
                Command::none()
            }
            Message::Untrack(id) => {
                self.service.untrack(id);
                Command::none()
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

        let save_database = if self.service.has_changes() {
            Command::single(
                self.database_timeout
                    .set(Duration::from_secs(5), Message::SaveDatabase),
            )
        } else {
            Command::none()
        };

        self.prepare();
        Command::batch([self.handle_image_loading(), save_database, command])
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

        let content = content.push(scrollable(page.width(Length::Fill)));

        let mut window = column![];

        window = window.push(content);

        if self.loading {
            window = window.push(horizontal_rule(1));
            window = window.push(
                row![text("Saving...")
                    .width(Length::Fill)
                    .size(ACTION_SIZE)
                    .horizontal_alignment(Horizontal::Left)]
                .width(Length::Fill)
                .height(Length::Shrink)
                .align_items(Alignment::Center)
                .padding(GAP),
            );
        }

        container(window)
            .width(Length::Fill)
            .height(Length::Fill)
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
