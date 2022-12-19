use iced::widget::{button, column, image, row, text, Column};
use iced::Length;
use serde::{Deserialize, Serialize};

use crate::message::{Message, Page};
use crate::params::{ACTION_SIZE, GAP, SUBTITLE_SIZE};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SeriesList;

impl SeriesList {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, service: &mut Service) {
        let images = service.all_series().map(|s| s.poster).collect::<Vec<_>>();
        service.mark_images(images);
    }

    pub(crate) fn view(&self, service: &Service) -> Column<'static, Message> {
        let mut series = column![].spacing(GAP);

        for s in service.all_series() {
            let handle = match service.get_image(&s.poster) {
                Some(handle) => handle,
                None => service.missing_banner(),
            };

            let graphic = image(handle).height(Length::Units(200));

            let episodes = service.episodes(s.id);

            let actions = crate::page::series::actions(s).push(
                button(text("Seasons").size(ACTION_SIZE))
                    .on_press(Message::Navigate(Page::Series(s.id))),
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
