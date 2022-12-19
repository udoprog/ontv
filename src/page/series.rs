use iced::theme;
use iced::widget::{button, column, container, image, scrollable, text, Column};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::message::{Message, Page};
use crate::params::{GAP, SPACE, SUBTITLE_SIZE, TITLE_SIZE};
use crate::service::Service;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct Series;

impl Series {
    pub(crate) fn view(&self, service: &Service, id: Uuid) -> Element<'static, Message> {
        let content = if let Some(s) = service.series(id) {
            let top = banner::<[Element<'static, Message>; 0]>(service, s, []);

            let episodes = service.episodes(s.id);

            let mut seasons = column![].spacing(GAP);

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
                    .spacing(SPACE),
                );
            }

            let info =
                column![text(format!("{} episode(s)", episodes.count())), seasons].spacing(GAP);
            column![top, info].spacing(GAP)
        } else {
            column![text("no series")]
        };

        scrollable(content.width(Length::Fill).spacing(GAP).padding(GAP)).into()
    }
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
