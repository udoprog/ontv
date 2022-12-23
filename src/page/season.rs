use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, container, image, text, Column, Row};
use iced::{theme, Alignment, Command};
use iced::{Element, Length};

use crate::cache::ImageHint;
use crate::comps;
use crate::model::{EpisodeId, SeasonNumber, SeriesId};
use crate::params::{
    centered, style, ACTION_SIZE, GAP, GAP2, IMAGE_HEIGHT, SPACE, SUBTITLE_SIZE, WARNING_COLOR,
};

use crate::state::State;

const SCREENCAP_HINT: ImageHint = ImageHint::Max(IMAGE_HEIGHT as u32);

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RemoveWatch(SeriesId, EpisodeId),
    RemoveLastWatch(SeriesId, EpisodeId),
    CancelRemoveWatch,
    Watch(SeriesId, EpisodeId),
    SelectPending(SeriesId, EpisodeId),
    SeasonInfo(comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

#[derive(Default)]
pub(crate) struct Season {
    remove_watch: Option<(SeriesId, EpisodeId)>,
    season_info: comps::SeasonInfo,
    banner: comps::SeriesBanner,
}

impl Season {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, s: &mut State, series_id: &SeriesId, season: SeasonNumber) {
        self.banner.prepare(s, series_id);

        for e in s
            .service
            .episodes(series_id)
            .iter()
            .filter(|e| e.season == season)
        {
            s.assets.mark_with_hint(e.filename, SCREENCAP_HINT);
        }
    }

    /// Handle series messages.
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::RemoveWatch(season_id, episode_id) => {
                self.remove_watch = Some((season_id, episode_id));
                Command::none()
            }
            Message::RemoveLastWatch(series_id, episode_id) => {
                self.remove_watch = None;
                s.service.remove_last_episode_watch(&series_id, &episode_id);
                Command::none()
            }
            Message::CancelRemoveWatch => {
                self.remove_watch = None;
                Command::none()
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
    pub(crate) fn view(
        &self,
        s: &State,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) -> Element<'static, Message> {
        let Some(series) = s.service.series(series_id) else {
            return Column::new().into();
        };

        let Some(season) = s.service.seasons(series_id).iter().find(|s| s.number == *season) else {
            return Column::new().into();
        };

        let mut episodes = Column::new();

        let pending = s.service.get_pending(series_id).map(|p| p.episode);

        for episode in s
            .service
            .episodes(&series.id)
            .iter()
            .filter(|e| e.season == season.number)
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
                    .on_press(Message::Watch(*series_id, episode.id)),
            );

            if !watched.is_empty() {
                match self.remove_watch {
                    Some((series_id, episode_id)) if episode_id == episode.id => {
                        let mut prompt = Row::new();

                        prompt = prompt.push(
                            button(text("remove").size(ACTION_SIZE))
                                .style(theme::Button::Destructive)
                                .on_press(Message::RemoveLastWatch(series_id, episode_id)),
                        );

                        prompt = prompt.push(
                            button(text("cancel").size(ACTION_SIZE))
                                .style(theme::Button::Secondary)
                                .on_press(Message::CancelRemoveWatch),
                        );

                        actions = actions.push(prompt);
                    }
                    _ => {
                        let remove_watch_text = match watched {
                            [_] => text("Remove watch"),
                            _ => text("Remove last watch"),
                        };

                        actions = actions.push(
                            button(remove_watch_text.size(ACTION_SIZE))
                                .style(theme::Button::Primary)
                                .on_press(Message::RemoveWatch(*series_id, episode.id)),
                        );
                    }
                }
            }

            if pending != Some(episode.id) {
                actions = actions.push(
                    button(text("Make next episode").size(ACTION_SIZE))
                        .style(theme::Button::Secondary)
                        .on_press(Message::SelectPending(*series_id, episode.id)),
                );
            } else {
                actions = actions.push(
                    button(text("Next episode").size(ACTION_SIZE)).style(theme::Button::Secondary),
                );
            }

            let mut show_info = Row::new().spacing(SPACE);

            if let Some(air_date) = episode.aired {
                show_info = show_info.push(text(format!("Aired: {air_date}")).size(ACTION_SIZE));
            }

            let watched = match watched {
                [] => text("Never watched").style(theme::Text::Color(WARNING_COLOR)),
                [once] => text(format!("Watched once on {}", once.timestamp.date_naive())),
                all @ [.., last] => text(format!(
                    "Watched {} times, last on {}",
                    all.len(),
                    last.timestamp.date_naive()
                )),
            };

            show_info = show_info.push(watched.size(ACTION_SIZE));

            let info_top = Column::new()
                .push(name)
                .push(actions)
                .push(show_info)
                .spacing(SPACE);
            let info = Column::new().push(info_top).push(overview);

            let image = container(image(screencap))
                .max_width(IMAGE_HEIGHT as u32)
                .max_height(IMAGE_HEIGHT as u32)
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

        let top = self
            .season_info
            .view(s, series, season)
            .map(Message::SeasonInfo);

        let header = centered(Column::new().push(banner).push(top).spacing(GAP), None).padding(GAP);

        Column::new()
            .push(header)
            .push(episodes.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into()
    }
}
