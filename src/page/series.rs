use iced::theme;
use iced::widget::{button, column, container, image, row, text, Column, Row};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::message::{Message, Page};
use crate::model;
use crate::params::{ACTION_SIZE, GAP, GAP2, SUBTITLE_SIZE, TITLE_SIZE};
use crate::service::Service;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct Series;

impl Series {
    pub(crate) fn view(&self, service: &Service, id: Uuid) -> Column<'static, Message> {
        let Some(s) = service.series(id) else {
            return column![text("no series")];
        };

        let top = banner::<[Element<'static, Message>; 0]>(service, s, []);

        let episodes = service.episodes(s.id);

        let mut seasons = column![].spacing(GAP2);

        for season in service.seasons(s.id) {
            let title = if let Some(number) = season.number {
                text(format!("Season {}", number)).size(SUBTITLE_SIZE)
            } else {
                text("Specials").size(SUBTITLE_SIZE)
            };

            let episodes = service
                .episodes(s.id)
                .filter(|e| e.season == season.number)
                .count();

            seasons = seasons.push(
                column![
                    title,
                    button(text(format!("{} Episode(s)", episodes)))
                        .style(theme::Button::Primary)
                        .on_press(Message::Navigate(Page::Season(s.id, season.number)))
                ]
                .spacing(GAP),
            );
        }

        let info = text(format!("{} episode(s)", episodes.count()));
        let content = column![top, actions(s), info, seasons].spacing(GAP);
        content.spacing(GAP).padding(GAP)
    }
}

/// Generate buttons which perform actions on the given series.
pub(crate) fn actions(s: &model::Series) -> Row<'static, Message> {
    let mut row = row![].spacing(GAP);

    if s.tracked {
        row = row.push(
            button(text("Untrack").size(ACTION_SIZE))
                .style(theme::Button::Destructive)
                .on_press(Message::Untrack(s.id)),
        );
    } else {
        row = row.push(
            button(text("Track").size(ACTION_SIZE))
                .style(theme::Button::Positive)
                .on_press(Message::Track(s.id)),
        );
    }

    row
}

/// Render a banner for the series.
pub(crate) fn banner<I>(
    service: &Service,
    s: &crate::model::Series,
    extra: I,
) -> Column<'static, Message>
where
    I: IntoIterator,
    I::Item: Into<Element<'static, Message>>,
{
    let handle = match service.get_image(&s.banner.unwrap_or(s.poster)) {
        Some(handle) => handle,
        None => service.missing_banner(),
    };

    let banner = container(image(handle)).max_height(100);
    let title = text(&s.title).size(TITLE_SIZE);

    let mut column = column![banner, title];

    for e in extra {
        column = column.push(e);
    }

    column
        .width(Length::Fill)
        .spacing(GAP)
        .align_items(Alignment::Center)
}
