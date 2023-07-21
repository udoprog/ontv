use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    pub(crate) id: MovieId,
}

pub(crate) fn page(id: MovieId) -> Page {
    Page::Movie(State { id })
}

#[derive(Debug, Clone)]
pub(crate) enum Message {}

pub(crate) struct Movie;

impl Movie {
    #[inline]
    pub(crate) fn new(_: &State) -> Self {
        Self
    }

    pub(crate) fn view(&self, state: &State) -> Element<'static, Message> {
        let id = w::text(state.id);
        w::Column::new().push(id).into()
    }
}
