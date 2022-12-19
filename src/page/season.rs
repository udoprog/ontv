use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, container, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::assets::Assets;
use crate::message::Message;
use crate::model::SeasonNumber;
use crate::params::{ACTION_SIZE, GAP, GAP2, SPACE, SUBTITLE_SIZE, WARNING_COLOR};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct State;

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
            crate::page::series::prepare_banner(assets, s);

            for e in service.episodes(id).filter(|e| e.season == season) {
                assets.mark(e.filename);
            }
        }
    }

    /// Render season view.
    pub(crate) fn view(
        &self,
        service: &Service,
        assets: &Assets,
        id: Uuid,
        season: SeasonNumber,
    ) -> Column<'static, Message> {
        let Some(series) = service.series(id) else {
            return column![text("no such series")];
        };

        let Some(season) = service.seasons(id).find(|s| s.number == season) else {
            return column![text("no such season")];
        };

        let top = crate::page::series::season_info(service, series, season);

        let mut episodes = column![];

        for episode in service
            .episodes(series.id)
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

            let watched = service
                .watched()
                .filter(|w| w.episode == episode.id)
                .collect::<Vec<_>>();

            let mut actions = row![];

            let watch_text = match &watched[..] {
                [] => text("First watch"),
                _ => text("Watch again"),
            };

            actions = actions.push(
                button(watch_text.size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Watch(id, episode.id)),
            );

            let mut info = column![name].spacing(GAP);

            let mut show_info = row![].spacing(GAP);

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

            info = info.push(actions).push(show_info).push(overview);

            episodes = episodes.push(
                row![
                    container(image(screencap))
                        .width(Length::Units(140))
                        .max_height(140)
                        .align_x(Horizontal::Center),
                    info,
                ]
                .spacing(GAP),
            );
        }

        let season_title = season.title().size(SUBTITLE_SIZE);

        let banner = crate::page::series::banner(assets, series, [season_title]);

        column![banner, top, episodes.spacing(GAP2)]
            .spacing(GAP)
            .padding(GAP)
    }
}
