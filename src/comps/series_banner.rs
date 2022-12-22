use iced::widget::{button, image, text, Column};
use iced::{theme, Alignment, Command, Element, Length};

use crate::message::Page;
use crate::model::{Series, SeriesId};
use crate::params::TITLE_SIZE;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Navigate(Page),
}

#[derive(Default, Debug, Clone)]
pub(crate) struct SeriesBanner {}

impl SeriesBanner {
    /// Prepare assets needed for banner.
    pub(crate) fn prepare(&mut self, s: &mut State, series_id: &SeriesId) {
        if let Some(series) = s.service.series(series_id) {
            s.assets.mark(series.banner);
        }
    }

    /// Update message.
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Navigate(page) => {
                s.push_history(page);
                Command::none()
            }
        }
    }

    /// Generate buttons which perform actions on the given series.
    pub(crate) fn view(&self, s: &State, series: &Series) -> Element<'static, Message> {
        let handle = match series.banner.and_then(|i| s.assets.image(&i)) {
            Some(handle) => handle,
            None => s.assets.missing_banner(),
        };

        let banner = image(handle);

        let title = button(text(&series.title).size(TITLE_SIZE))
            .padding(0)
            .style(theme::Button::Text)
            .on_press(Message::Navigate(Page::Series(series.id)));

        Column::new()
            .push(banner)
            .push(title)
            .width(Length::Fill)
            .align_items(Alignment::Center)
            .into()
    }
}
