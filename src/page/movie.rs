use iced::widget::{text, Column};
use iced::Element;

use crate::model::MovieId;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {}

pub(crate) struct Movie {
    movie_id: MovieId,
}

impl Movie {
    #[inline]
    pub(crate) fn new(movie_id: MovieId) -> Self {
        Self { movie_id }
    }

    pub(crate) fn prepare(&mut self, _: &mut State) {}

    #[allow(unused)]
    pub(crate) fn update(&mut self, _: &mut State, message: Message) {
        match message {}
    }

    pub(crate) fn view(&self, _: &State) -> Element<'static, Message> {
        let id = text(self.movie_id.to_string());

        Column::new().push(id).into()
    }
}
