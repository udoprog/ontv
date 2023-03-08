use crate::prelude::*;

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

    pub(crate) fn view(&self) -> Element<'static, Message> {
        let id = w::text(self.movie_id.to_string());
        w::Column::new().push(id).into()
    }
}
