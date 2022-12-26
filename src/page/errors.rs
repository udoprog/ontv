use iced::widget::{text, Column};
use iced::Element;

use crate::error::ErrorId;
use crate::params::{default_container, GAP, SMALL, SPACE, SUBTITLE_SIZE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {}

#[derive(Default)]
pub(crate) struct Errors;

impl Errors {
    pub(crate) fn prepare(&self, _: &mut State) {}

    pub(crate) fn update(&mut self, _: &mut State, _: Message) {}

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut page = Column::new();

        for e in s.errors().rev() {
            let mut error = Column::new();

            match e.id {
                Some(ErrorId::Search(..)) => {
                    error = error.push(
                        text("Search error")
                            .size(SUBTITLE_SIZE)
                            .style(s.warning_text()),
                    );
                }
                None => {
                    error = error.push(text("Error").size(SUBTITLE_SIZE).style(s.warning_text()));
                }
            }

            error = error.push(text(format!("At: {}", e.timestamp)).size(SMALL));
            error = error.push(text(&e.message));

            for cause in &e.causes {
                error = error.push(text(format!("Caused by: {cause}")));
            }

            page = page.push(error.spacing(SPACE));
        }

        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}
