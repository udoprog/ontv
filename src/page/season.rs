use iced::alignment::Horizontal;
use iced::widget::{button, column, container, image, row, text, Column, Row};
use iced::Length;
use iced::{theme, Command};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::assets::Assets;
use crate::message::Message;
use crate::model::SeasonNumber;
use crate::page::series::{prepare_series_banner, season_info, series_banner};
use crate::params::{centered, style, ACTION_SIZE, GAP, GAP2, SPACE, SUBTITLE_SIZE, WARNING_COLOR};
use crate::service::Service;

#[derive(Debug, Clone)]
pub(crate) enum M {
    RemoveWatch(Uuid, Uuid),
    RemoveLastWatch(Uuid, Uuid),
    CancelRemoveWatch,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct State {
    remove_watch: Option<(Uuid, Uuid)>,
}

impl State {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(
        &mut self,
        service: &Service,
        assets: &mut Assets,
        id: Uuid,
        season: SeasonNumber,
    ) {
        if let Some(s) = service.series(id) {
            prepare_series_banner(assets, s);

            for e in service.episodes(id).iter().filter(|e| e.season == season) {
                assets.mark(e.filename);
            }
        }
    }

    /// Handle series messages.
    pub(crate) fn update(&mut self, service: &mut Service, message: M) -> Command<Message> {
        match message {
            M::RemoveWatch(season_id, episode_id) => {
                self.remove_watch = Some((season_id, episode_id));
            }
            M::RemoveLastWatch(series_id, episode_id) => {
                self.remove_watch = None;
                service.remove_last_episode_watch(series_id, episode_id);
            }
            M::CancelRemoveWatch => {
                self.remove_watch = None;
            }
        }

        Command::none()
    }

    /// Render season view.
    pub(crate) fn view(
        &self,
        service: &Service,
        assets: &Assets,
        series_id: Uuid,
        season: SeasonNumber,
    ) -> Column<'static, Message> {
        let Some(series) = service.series(series_id) else {
            return column![text("no such series")];
        };

        let Some(season) = service.seasons(series_id).find(|s| s.number == season) else {
            return column![text("no such season")];
        };

        let mut episodes = column![];

        let pending = service.get_pending(series_id).map(|p| p.episode);

        for episode in service
            .episodes(series.id)
            .iter()
            .filter(|e| e.season == season.number)
        {
            let screencap = match episode.filename.and_then(|image| assets.image(&image)) {
                Some(handle) => handle,
                None => assets.missing_screencap(),
            };

            let mut name = row![].spacing(SPACE);

            name = name.push(text(format!("{}", episode.number)));

            if let Some(string) = &episode.name {
                name = name.push(text(string));
            }

            let overview = text(episode.overview.as_deref().unwrap_or_default());

            let watched = service.watched(episode.id);

            let mut actions = row![].spacing(SPACE);

            let watch_text = match &watched[..] {
                [] => text("First watch"),
                _ => text("Watch again"),
            };

            actions = actions.push(
                button(watch_text.size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Watch(series_id, episode.id)),
            );

            if !watched.is_empty() {
                match self.remove_watch {
                    Some((series_id, episode_id)) if episode_id == episode.id => {
                        let mut prompt = Row::new();

                        prompt = prompt.push(
                            button(text("remove").size(ACTION_SIZE))
                                .style(theme::Button::Destructive)
                                .on_press(Message::Season(M::RemoveLastWatch(
                                    series_id, episode_id,
                                ))),
                        );

                        prompt = prompt.push(
                            button(text("cancel").size(ACTION_SIZE))
                                .style(theme::Button::Secondary)
                                .on_press(Message::Season(M::CancelRemoveWatch)),
                        );

                        actions = actions.push(prompt);
                    }
                    _ => {
                        let remove_watch_text = match &watched[..] {
                            [_] => text("Remove watch"),
                            _ => text("Remove last watch"),
                        };

                        actions = actions.push(
                            button(remove_watch_text.size(ACTION_SIZE))
                                .style(theme::Button::Primary)
                                .on_press(Message::Season(M::RemoveWatch(series_id, episode.id))),
                        );
                    }
                }
            }

            if pending != Some(episode.id) {
                actions = actions.push(
                    button(text("Make next episode").size(ACTION_SIZE))
                        .style(theme::Button::Secondary)
                        .on_press(Message::SelectPending(series_id, episode.id)),
                );
            } else {
                actions = actions.push(
                    button(text("Next episode").size(ACTION_SIZE)).style(theme::Button::Secondary),
                );
            }

            let mut show_info = row![].spacing(SPACE);

            if let Some(air_date) = episode.aired {
                show_info = show_info.push(text(format!("Aired: {}", air_date)).size(ACTION_SIZE));
            }

            let watched = match &watched[..] {
                &[] => text("Never watched").style(theme::Text::Color(WARNING_COLOR)),
                &[once] => text(format!("Watched once on {}", once.timestamp.date_naive())),
                all @ &[.., last] => text(format!(
                    "Watched {} times, last on {}",
                    all.len(),
                    last.timestamp.date_naive()
                )),
            };

            show_info = show_info.push(watched.size(ACTION_SIZE));

            let info_top = column![name, actions, show_info].spacing(SPACE);
            let info = column![info_top, overview];

            let image = container(image(screencap))
                .max_width(200)
                .max_height(200)
                .align_x(Horizontal::Center);

            let image = column![image,];

            episodes = episodes.push(
                centered(
                    row![image, info.width(Length::Fill).spacing(GAP)].spacing(GAP),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let season_title = season.number.title().size(SUBTITLE_SIZE);

        let banner = series_banner(assets, series)
            .push(season_title)
            .spacing(GAP);

        let top = season_info(service, series, season).spacing(GAP);

        let header = centered(column![banner, top].spacing(GAP), None).padding(GAP);

        column![header, episodes.spacing(GAP2)].spacing(GAP)
    }
}
