use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, container, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::params::{ACTION_SIZE, GAP, GAP2, SPACE, SUBTITLE_SIZE, WARNING_COLOR};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Season;

impl Season {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(
        &mut self,
        service: &Service,
        assets: &mut Assets,
        id: Uuid,
        season: Option<u32>,
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
        season: Option<u32>,
    ) -> Column<'static, Message> {
        let Some(s) = service.series(id) else {
            return column![text("no such series")];
        };

        let mut episodes = column![];

        for e in service.episodes(id).filter(|e| e.season == season) {
            let screencap = match e.filename.and_then(|image| assets.image(&image)) {
                Some(handle) => handle,
                None => assets.missing_screencap(),
            };

            let mut name = row![].spacing(SPACE);

            name = name.push(text(format!("{}", e.number)));

            if let Some(string) = &e.name {
                name = name.push(text(string));
            }

            let overview = text(e.overview.as_deref().unwrap_or_default());

            let watched = service
                .watched()
                .filter(|w| w.episode == e.id)
                .collect::<Vec<_>>();

            let mut actions = row![];

            let watch_text = match &watched[..] {
                [] => "First watch",
                _ => "Watch again",
            };

            actions = actions.push(
                button(text(watch_text).size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Watch(id, e.id)),
            );

            let mut info = column![name].spacing(GAP);

            let mut show_info = row![].spacing(GAP);

            if let Some(air_date) = e.aired {
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

            info = info.push(show_info).push(overview);
            info = info.push(actions);

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

        let season = match season {
            Some(number) => text(format!("Season {}", number)),
            None => text("Specials"),
        }
        .size(SUBTITLE_SIZE);

        let banner = crate::page::series::banner(assets, s, [season]);

        let back = button("back").on_press(Message::Navigate(Page::Series(s.id)));

        column![banner, back, episodes.spacing(GAP2)]
            .spacing(GAP)
            .padding(GAP)
    }
}
