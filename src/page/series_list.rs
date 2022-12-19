use iced::theme;
use iced::widget::{button, column, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::params::{ACTION_SIZE, GAP, SUBTITLE_SIZE};
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
        let mut series = column![].spacing(GAP);

        for s in service.all_series() {
            let handle = match assets.image(&s.poster) {
                Some(handle) => handle,
                None => assets.missing_banner(),
            };

            let graphic = image(handle).height(Length::Units(200));

            let episodes = service.episodes(s.id);

            let actions = crate::page::series::actions(s)
                .push(
                    button(text("Seasons").size(ACTION_SIZE))
                        .on_press(Message::Navigate(Page::Series(s.id))),
                )
                .push(
                    button(text("Remove").size(ACTION_SIZE))
                        .style(theme::Button::Destructive)
                        .on_press(Message::RemoveSeries(s.id)),
                );

            let content = column![
                text(&s.title).size(SUBTITLE_SIZE),
                text(format!("{} episode(s)", episodes.count())),
                actions
            ]
            .width(Length::Fill)
            .spacing(GAP);

            series = series.push(row![graphic, content].width(Length::Fill).spacing(GAP));
        }

        column![series].spacing(GAP).padding(GAP)
    }
}
