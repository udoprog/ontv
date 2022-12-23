use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{horizontal_rule, text, vertical_space, Column, Row};
use iced::{Command, Element, Length};
use serde::{Deserialize, Serialize};

use crate::params::{default_container, duration_display, GAP, HALF_GAP, TITLE_SIZE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {}

/// The state for the settings page.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Queue;

impl Queue {
    pub(crate) fn prepare(&mut self, _: &mut State) {}

    pub(crate) fn update(&mut self, _: &mut State, message: Message) -> Command<Message> {
        match message {}
    }

    pub(crate) fn view(&self, state: &State) -> Element<'static, Message> {
        let mut page = Column::new();

        let queue = state.service.queue();

        if queue.is_empty() {
            page = page.push(
                Row::new()
                    .push(
                        text("Queue is empty")
                            .size(TITLE_SIZE)
                            .width(Length::Fill)
                            .horizontal_alignment(Horizontal::Center),
                    )
                    .padding(GAP),
            );
        } else {
            page = page.push(
                Row::new()
                    .push(
                        text(format!("Queue ({})", queue.len()))
                            .size(TITLE_SIZE)
                            .width(Length::Fill)
                            .horizontal_alignment(Horizontal::Center),
                    )
                    .padding(GAP),
            );

            let mut it = queue.iter().peekable();

            let now = Utc::now();

            while let Some(d) = it.next() {
                let Some(series) = state.service.series(&d.series_id) else {
                    page = page.push(text(format!("{d:?} (no series)")));
                    continue;
                };

                let duration = now.signed_duration_since(d.scheduled);
                let when = duration_display(duration);

                page = page.push(
                    Row::new()
                        .push(text(&series.title))
                        .push(text(d.remote_id.to_string()).width(Length::Fill))
                        .push(when)
                        .spacing(GAP),
                );

                if it.peek().is_some() {
                    page = page.push(horizontal_rule(1));
                }
            }
        }

        default_container(
            Column::new()
                .push(page.spacing(HALF_GAP))
                .push(vertical_space(Length::Shrink))
                .padding(GAP)
                .spacing(GAP),
        )
        .into()
    }
}
