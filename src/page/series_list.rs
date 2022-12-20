use iced::theme;
use iced::widget::{button, column, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::params::{centered, style, GAP, GAP2, SPACE, SUBTITLE_SIZE};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SeriesList;

impl SeriesList {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, service: &Service, assets: &mut Assets) {
        let images = service.all_series().map(|s| s.poster).collect::<Vec<_>>();
        assets.mark(images);
    }

    pub(crate) fn view(&self, service: &Service, assets: &Assets) -> Column<'static, Message> {
        let mut series = column![];

        for s in service.all_series() {
            let handle = match assets.image(&s.poster) {
                Some(handle) => handle,
                None => assets.missing_poster(),
            };

            let graphic = image(handle).height(Length::Units(200));

            let episodes = service.episodes(s.id);

            let actions = crate::page::series::actions(s).spacing(SPACE);

            let title = button(text(&s.title).size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Series(s.id)));

            let mut content = column![].width(Length::Fill);

            content = content.push(
                column![
                    title,
                    text(format!("{} episode(s)", episodes.count())),
                    actions,
                ]
                .spacing(SPACE),
            );

            if let Some(overview) = &s.overview {
                content = content.push(text(overview));
            }

            series = series.push(
                centered(
                    row![graphic, content.spacing(GAP)]
                        .spacing(GAP)
                        .width(Length::Fill),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        series.spacing(GAP2)
    }
}
