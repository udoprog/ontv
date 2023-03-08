use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {}

pub(crate) struct Movie;

impl Movie {
    #[inline]
    pub(crate) fn new(_: &MovieId) -> Self {
        Self
    }

    pub(crate) fn view(&self, movie_id: &MovieId) -> Element<'static, Message> {
        let id = w::text(movie_id);
        w::Column::new().push(id).into()
    }
}
