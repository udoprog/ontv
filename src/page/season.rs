use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, container, image, row, scrollable, text};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::message::{Message, Page};
use crate::params::{ACTION_BUTTON_SIZE, GAP, SPACE, SUBTITLE_SIZE};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Season;

impl Season {
    pub(crate) fn view(
        &self,
        service: &Service,
        id: Uuid,
        season: Option<u32>,
    ) -> Element<'static, Message> {
        let Some(s) = service.series(id) else {
            return text("no such series").into();
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
                button(text("mark watched").size(ACTION_BUTTON_SIZE))
                    .style(theme::Button::Destructive),
            );

            episodes = episodes.push(
                row![
                    container(image(screencap))
                        .width(Length::Units(140))
                        .max_height(140)
                        .align_x(Horizontal::Center),
                    column![name, overview, actions.spacing(GAP)].spacing(GAP)
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

        scrollable(
            column![banner, back, episodes.spacing(GAP).width(Length::Fill)]
                .spacing(GAP)
                .padding(GAP),
        )
        .into()
    }
}
