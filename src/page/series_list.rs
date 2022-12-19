use iced::widget::{button, column, image, row, scrollable, text};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};

use crate::message::{Message, Page};
use crate::params::{GAP, SPACE};
use crate::service::Service;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SeriesList;

impl SeriesList {
    pub(crate) fn view(&self, service: &Service) -> Element<'static, Message> {
        let mut series = column![].spacing(GAP);

        for s in service.list_series() {
            let handle = match service.get_image(&s.poster) {
                Some(handle) => handle,
                None => service.missing_banner(),
            };

            let graphic = column![image(handle).height(Length::Units(200)), text(&s.title)]
                .spacing(GAP)
                .align_items(Alignment::Center);

            let episodes = service.episodes(s.id);

            let actions = row![button("Seasons").on_press(Message::Navigate(Page::Series(s.id)))]
                .spacing(SPACE);

            let content =
                column![text(format!("{} episode(s)", episodes.count())), actions].spacing(GAP);

            series = series.push(row![graphic, content].spacing(GAP));
        }

        scrollable(
            column![series]
                .width(Length::Fill)
                .spacing(GAP)
                .padding(GAP),
        )
        .into()
    }
}
