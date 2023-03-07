use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, container, image, text, Column, Row};
use iced::{theme, Alignment};
use iced::{Element, Length};

use crate::component::*;
use crate::comps;
use crate::model::{EpisodeId, RemoteSeasonId, SeasonNumber, SeriesId, Watched};
use crate::params::{
    centered, GAP, GAP2, SCREENCAP_HEIGHT, SCREENCAP_HINT, SMALL, SPACE, SUBTITLE_SIZE,
};
use crate::style;

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeasonId),
    RemoveLastWatch(usize, comps::confirm::Message),
    RemoveWatch(usize, usize, comps::confirm::Message),
    Watch(usize, comps::watch::Message),
    SelectPending(SeriesId, EpisodeId),
    ClearPending(EpisodeId),
    SeasonInfo(comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

struct EpisodeState {
    episode_id: EpisodeId,
    watch: comps::Watch,
    remove_last_watch: Option<comps::Confirm>,
    remove_watches: Vec<comps::Confirm>,
}

impl<'a, I> Component<(SeriesId, EpisodeId, I)> for EpisodeState
where
    I: DoubleEndedIterator<Item = &'a Watched> + Clone,
{
    #[inline]
    fn new((series_id, episode_id, watched): (SeriesId, EpisodeId, I)) -> Self {
        Self {
            episode_id,
            watch: comps::Watch::new(comps::watch::Props::new(episode_id)),
            remove_last_watch: watched.clone().next_back().map(move |w| {
                comps::Confirm::new(comps::confirm::Props::new(
                    comps::confirm::Kind::RemoveWatch {
                        series_id,
                        episode_id,
                        watch_id: w.id,
                    },
                ))
            }),
            remove_watches: watched
                .map(move |w| {
                    comps::Confirm::new(
                        comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                            series_id,
                            episode_id,
                            watch_id: w.id,
                        })
                        .with_ordering(comps::ordering::Ordering::Left),
                    )
                })
                .collect(),
        }
    }

    #[inline]
    fn changed(&mut self, (series_id, episode_id, watched): (SeriesId, EpisodeId, I)) {
        self.episode_id = episode_id;
        self.watch.changed(comps::watch::Props::new(episode_id));
        self.remove_last_watch
            .init_from_iter(watched.clone().next_back().map(move |w| {
                comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                    series_id,
                    episode_id,
                    watch_id: w.id,
                })
            }));
        self.remove_watches.init_from_iter(watched.map(|w| {
            comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                series_id,
                episode_id,
                watch_id: w.id,
            })
            .with_ordering(comps::ordering::Ordering::Left)
        }));
    }
}

pub(crate) struct Season {
    series_id: SeriesId,
    season: SeasonNumber,
    episodes: Vec<EpisodeState>,
    season_info: comps::SeasonInfo,
    banner: comps::SeriesBanner,
}

impl Season {
    #[inline]
    pub(crate) fn new(series_id: SeriesId, season: SeasonNumber) -> Self {
        Self {
            series_id,
            season,
            episodes: Vec::new(),
            season_info: comps::SeasonInfo::new((series_id, season)),
            banner: comps::SeriesBanner::default(),
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        self.episodes.init_from_iter(
            s.service
                .episodes(&self.series_id)
                .filter(|e| e.season == self.season)
                .map(|e| (self.series_id, e.id, s.service.watched(&e.id))),
        );

        self.banner.prepare(s, &self.series_id);

        for e in s
            .service
            .episodes(&self.series_id)
            .filter(|e| e.season == self.season)
        {
            s.assets.mark_with_hint(e.filename(), SCREENCAP_HINT);
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::OpenRemote(remote) => {
                let url = remote.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::RemoveLastWatch(index, message) => {
                if let Some(c) = self
                    .episodes
                    .get_mut(index)
                    .and_then(|data| data.remove_last_watch.as_mut())
                {
                    c.update(s, message);
                }
            }
            Message::RemoveWatch(index, n, message) => {
                if let Some(c) = self
                    .episodes
                    .get_mut(index)
                    .and_then(|data| data.remove_watches.get_mut(n))
                {
                    c.update(s, message);
                }
            }
            Message::Watch(index, message) => {
                if let Some(c) = self.episodes.get_mut(index).map(|data| &mut data.watch) {
                    c.update(s, message);
                }
            }
            Message::SelectPending(series, episode) => {
                let now = Utc::now();
                s.service.select_pending(&now, &series, &episode);
            }
            Message::ClearPending(episode) => {
                s.service.clear_pending(&episode);
            }
            Message::SeasonInfo(message) => {
                self.season_info.update(s, message);
            }
            Message::SeriesBanner(message) => {
                self.banner.update(s, message);
            }
        }
    }

    /// Render season view.
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let Some(series) = s.service.series(&self.series_id) else {
            return Column::new().into();
        };

        let Some(season) = s.service.seasons(&series.id).iter().find(|s| s.number == self.season) else {
            return Column::new().into();
        };

        let mut episodes = Column::new();

        let pending = s.service.get_pending(&series.id).map(|p| p.episode);

        for (index, data) in self.episodes.iter().enumerate() {
            let Some(episode) = s.service.episode(&data.episode_id) else {
                continue;
            };

            let screencap = match episode
                .filename()
                .and_then(|image| s.assets.image_with_hint(&image, SCREENCAP_HINT))
            {
                Some(handle) => handle,
                None => s.assets.missing_screencap(),
            };

            let mut name = Row::new().spacing(SPACE);

            name = name.push(text(format!("{}", episode.number)));

            if let Some(string) = &episode.name {
                name = name.push(text(string));
            }

            let overview = text(&episode.overview);

            let watched = s.service.watched(&episode.id);

            let mut actions = Row::new().spacing(SPACE);

            let any_confirm = data.watch.is_confirm()
                || data
                    .remove_last_watch
                    .as_ref()
                    .map(comps::Confirm::is_confirm)
                    .unwrap_or_default();

            let watch_text = match watched.len() {
                0 => "First watch",
                _ => "Watch again",
            };

            if !any_confirm || data.watch.is_confirm() {
                actions = actions.push(
                    data.watch
                        .view(
                            watch_text,
                            theme::Button::Positive,
                            theme::Button::Positive,
                            Length::Shrink,
                            Horizontal::Center,
                            true,
                        )
                        .map(move |m| Message::Watch(index, m)),
                );
            }

            if let Some(remove_last_watch) = &data.remove_last_watch {
                if !any_confirm || remove_last_watch.is_confirm() {
                    let watch_text = match watched.len() {
                        1 => "Remove watch",
                        _ => "Remove last watch",
                    };

                    actions = actions.push(
                        remove_last_watch
                            .view(watch_text, theme::Button::Destructive)
                            .map(move |m| Message::RemoveLastWatch(index, m)),
                    );
                }
            }

            if !any_confirm {
                if pending != Some(episode.id) {
                    actions = actions.push(
                        button(text("Make next episode").size(SMALL))
                            .style(theme::Button::Secondary)
                            .on_press(Message::SelectPending(series.id, episode.id)),
                    );
                } else {
                    actions = actions.push(
                        button(text("Clear next episode").size(SMALL))
                            .style(theme::Button::Destructive)
                            .on_press(Message::ClearPending(episode.id)),
                    );
                }
            }

            let mut show_info = Column::new();

            if let Some(air_date) = &episode.aired {
                if air_date > s.today() {
                    show_info = show_info.push(text(format!("Airs: {air_date}")).size(SMALL));
                } else {
                    show_info = show_info.push(text(format!("Aired: {air_date}")).size(SMALL));
                }
            }

            let watched_text = {
                let mut it = watched.clone();
                let len = it.len();

                match (len, it.next(), it.next_back()) {
                    (1, Some(once), _) => {
                        text(format!("Watched once on {}", once.timestamp.date_naive()))
                    }
                    (len, _, Some(last)) if len > 0 => text(format!(
                        "Watched {} times, last on {}",
                        len,
                        last.timestamp.date_naive()
                    )),
                    _ => text("Never watched").style(s.warning_text()),
                }
            };

            show_info = show_info.push(watched_text.size(SMALL));

            let info_top = Column::new()
                .push(name)
                .push(actions)
                .push(show_info.spacing(SPACE))
                .spacing(SPACE);

            let mut info = Column::new().push(info_top).push(overview);

            if watched.len() > 0 {
                let mut history = Column::new();

                history = history.push(text("Watch history"));

                for ((n, watch), c) in watched.enumerate().zip(&data.remove_watches) {
                    let mut row = Row::new();

                    row = row.push(
                        text(format!("#{}", n + 1))
                            .size(SMALL)
                            .width(24.0)
                            .horizontal_alignment(Horizontal::Left),
                    );

                    row = row.push(
                        text(watch.timestamp.date_naive())
                            .size(SMALL)
                            .width(Length::Fill),
                    );

                    row = row.push(
                        c.view("Remove", theme::Button::Destructive)
                            .map(move |m| Message::RemoveWatch(index, n, m)),
                    );

                    history = history.push(row.width(Length::Fill).spacing(SPACE));
                }

                info = info.push(history.width(Length::Fill).spacing(SPACE));
            }

            let image = container(image(screencap))
                .max_height(SCREENCAP_HEIGHT)
                .align_x(Horizontal::Center);

            episodes = episodes.push(
                centered(
                    Row::new()
                        .push(image)
                        .push(info.width(Length::Fill).spacing(GAP))
                        .spacing(GAP),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let season_title = text(season.number).size(SUBTITLE_SIZE);

        let mut banner = Column::new()
            .push(self.banner.view(s, series).map(Message::SeriesBanner))
            .push(season_title)
            .align_items(Alignment::Center)
            .spacing(GAP);

        let mut remote_ids = s
            .service
            .remotes_by_series(&series.id)
            .flat_map(|remote_id| remote_id.into_season(season.number))
            .peekable();

        if remote_ids.peek().is_some() {
            let mut remotes = Row::new();

            for remote_season in remote_ids {
                remotes = remotes.push(
                    button(text(&remote_season))
                        .style(theme::Button::Text)
                        .on_press(Message::OpenRemote(remote_season)),
                );
            }

            banner = banner.push(remotes.spacing(SPACE));
        }

        let top = Column::new()
            .push(banner)
            .push(self.season_info.view(s).map(Message::SeasonInfo))
            .spacing(GAP)
            .width(Length::Fill);

        let header = centered(top, None).padding(GAP);

        Column::new()
            .push(header)
            .push(episodes.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into()
    }
}
