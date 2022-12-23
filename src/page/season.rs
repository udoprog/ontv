use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, container, image, text, Column, Row};
use iced::{theme, Alignment, Command};
use iced::{Element, Length};

use crate::cache::ImageHint;
use crate::component::*;
use crate::comps;
use crate::model::{EpisodeId, SeasonNumber, SeriesId, Watched};
use crate::params::{centered, ACTION_SIZE, GAP, GAP2, SCREENCAP_HEIGHT, SPACE, SUBTITLE_SIZE};
use crate::style;

use crate::state::State;

// Force a 16:9 aspect ratio
const SCREENCAP_HINT: ImageHint = ImageHint::Fill(480, SCREENCAP_HEIGHT as u32);

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RemoveLastWatch(usize, comps::confirm::Message),
    RemoveWatch(usize, usize, comps::confirm::Message),
    Watch(SeriesId, EpisodeId),
    SelectPending(SeriesId, EpisodeId),
    SeasonInfo(comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

struct EpisodeState {
    remove_last_watch: Option<comps::Confirm>,
    remove_watches: Vec<comps::Confirm>,
}

impl Component<(SeriesId, EpisodeId, &[Watched])> for EpisodeState {
    #[inline]
    fn new((series_id, episode_id, watched): (SeriesId, EpisodeId, &[Watched])) -> Self {
        Self {
            remove_last_watch: watched.last().map(move |w| {
                comps::Confirm::new(comps::confirm::Props::new(
                    comps::confirm::Kind::RemoveWatch {
                        series_id,
                        episode_id,
                        watch_id: w.id,
                    },
                ))
            }),
            remove_watches: watched
                .iter()
                .map(move |w| {
                    comps::Confirm::new(
                        comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                            series_id,
                            episode_id,
                            watch_id: w.id,
                        })
                        .with_ordering(comps::confirm::Ordering::Left),
                    )
                })
                .collect(),
        }
    }

    #[inline]
    fn changed(&mut self, (series_id, episode_id, watched): (SeriesId, EpisodeId, &[Watched])) {
        self.remove_last_watch
            .init_from_iter(watched.last().map(move |w| {
                comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                    series_id,
                    episode_id,
                    watch_id: w.id,
                })
            }));
        self.remove_watches.init_from_iter(watched.iter().map(|w| {
            comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                series_id,
                episode_id,
                watch_id: w.id,
            })
            .with_ordering(comps::confirm::Ordering::Left)
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
                .iter()
                .filter(|e| e.season == self.season)
                .map(|e| (self.series_id, e.id, s.service.watched(&e.id))),
        );

        self.banner.prepare(s, &self.series_id);

        for e in s
            .service
            .episodes(&self.series_id)
            .iter()
            .filter(|e| e.season == self.season)
        {
            s.assets.mark_with_hint(e.filename, SCREENCAP_HINT);
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::RemoveLastWatch(index, message) => {
                if let Some(c) = self
                    .episodes
                    .get_mut(index)
                    .and_then(|data| data.remove_last_watch.as_mut())
                {
                    c.update(s, message)
                        .map(move |m| Message::RemoveLastWatch(index, m))
                } else {
                    Command::none()
                }
            }
            Message::RemoveWatch(index, n, message) => {
                if let Some(c) = self
                    .episodes
                    .get_mut(index)
                    .and_then(|data| data.remove_watches.get_mut(n))
                {
                    c.update(s, message)
                        .map(move |m| Message::RemoveWatch(index, n, m))
                } else {
                    Command::none()
                }
            }
            Message::Watch(series, episode) => {
                let now = Utc::now();
                s.service.watch(&series, &episode, now);
                Command::none()
            }
            Message::SelectPending(series, episode) => {
                let now = Utc::now();
                s.service.select_pending(&series, &episode, now);
                Command::none()
            }
            Message::SeasonInfo(message) => {
                self.season_info.update(s, message).map(Message::SeasonInfo)
            }
            Message::SeriesBanner(message) => {
                self.banner.update(s, message).map(Message::SeriesBanner)
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

        for (index, (episode, data)) in s
            .service
            .episodes(&series.id)
            .iter()
            .filter(|e| e.season == season.number)
            .zip(&self.episodes)
            .enumerate()
        {
            let screencap = match episode
                .filename
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

            let overview = text(episode.overview.as_deref().unwrap_or_default());

            let watched = s.service.watched(&episode.id);

            let mut actions = Row::new().spacing(SPACE);

            let watch_text = match watched {
                [] => text("First watch"),
                _ => text("Watch again"),
            };

            actions = actions.push(
                button(watch_text.size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Watch(series.id, episode.id)),
            );

            let remove_last = match (watched, &data.remove_last_watch) {
                ([_], Some(c)) => Some(("Remove watch", c)),
                ([.., _], Some(c)) => Some(("Remove last watch", c)),
                _ => None,
            };

            if let Some((watch_text, c)) = remove_last {
                actions = actions.push(
                    c.view(watch_text, theme::Button::Destructive)
                        .map(move |m| Message::RemoveLastWatch(index, m)),
                );
            }

            if pending != Some(episode.id) {
                actions = actions.push(
                    button(text("Make next episode").size(ACTION_SIZE))
                        .style(theme::Button::Secondary)
                        .on_press(Message::SelectPending(series.id, episode.id)),
                );
            } else {
                actions = actions.push(
                    button(text("Next episode").size(ACTION_SIZE)).style(theme::Button::Secondary),
                );
            }

            let mut show_info = Column::new();

            if let Some(air_date) = episode.aired {
                show_info = show_info.push(text(format!("Aired: {air_date}")).size(ACTION_SIZE));
            }

            let watched_text = match watched {
                [] => text("Never watched").style(s.warning_text()),
                [once] => text(format!("Watched once on {}", once.timestamp.date_naive())),
                all @ [.., last] => text(format!(
                    "Watched {} times, last on {}",
                    all.len(),
                    last.timestamp.date_naive()
                )),
            };

            show_info = show_info.push(watched_text.size(ACTION_SIZE));

            let info_top = Column::new()
                .push(name)
                .push(actions)
                .push(show_info.spacing(SPACE))
                .spacing(SPACE);

            let mut info = Column::new().push(info_top).push(overview);

            if !watched.is_empty() {
                let mut history = Column::new();

                history = history.push(text("Watch history"));

                for ((n, watch), c) in watched.iter().enumerate().zip(&data.remove_watches) {
                    let mut row = Row::new();

                    row = row.push(
                        text(format!("#{}", n + 1))
                            .size(ACTION_SIZE)
                            .width(Length::Units(24))
                            .horizontal_alignment(Horizontal::Left),
                    );

                    row = row.push(
                        text(watch.timestamp.date_naive())
                            .size(ACTION_SIZE)
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
                .max_height(SCREENCAP_HEIGHT as u32)
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

        let season_title = season.number.title().size(SUBTITLE_SIZE);

        let banner = Column::new()
            .push(self.banner.view(s, series).map(Message::SeriesBanner))
            .push(season_title)
            .align_items(Alignment::Center)
            .spacing(GAP);

        let top = self.season_info.view(s).map(Message::SeasonInfo);

        let header = centered(Column::new().push(banner).push(top).spacing(GAP), None).padding(GAP);

        Column::new()
            .push(header)
            .push(episodes.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into()
    }
}
