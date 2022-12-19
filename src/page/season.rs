use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, container, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::message::{Message, Page};
use crate::params::{ACTION_SIZE, GAP, GAP2, SPACE, SUBTITLE_SIZE};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Season;

impl Season {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, service: &mut Service, id: Uuid, season: Option<u32>) {}

    pub(crate) fn view(
        &self,
        service: &Service,
        id: Uuid,
        season: Option<u32>,
    ) -> Column<'static, Message> {
        let Some(s) = service.series(id) else {
            return column![text("no such series")];
        };

        let mut episodes = column![];

        for e in service.episodes(id).filter(|e| e.season == season) {
            let screencap = match service.get_image(&e.filename.unwrap_or(s.poster)) {
                Some(handle) => handle,
                None => service.missing_screencap(),
            };

            let mut name = row![].spacing(SPACE);

            name = name.push(text(format!("{}", e.number)));

            if let Some(string) = &e.name {
                name = name.push(text(string));
            }

            let overview = text(e.overview.as_deref().unwrap_or_default());

            let mut actions = row![];

            actions = actions.push(
                button(text("mark watched").size(ACTION_SIZE)).style(theme::Button::Destructive),
            );

            let mut info = column![name].spacing(GAP).push(actions);

            if let Some(air_date) = e.aired {
                info = info.push(text(format!("Aired: {}", air_date)).size(ACTION_SIZE));
            }

            info = info.push(overview);

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

        let banner = crate::page::series::banner(service, s, [season]);

        let back = button("back").on_press(Message::Navigate(Page::Series(s.id)));

        column![banner, back, episodes.spacing(GAP2)]
            .spacing(GAP)
            .padding(GAP)
    }
}
