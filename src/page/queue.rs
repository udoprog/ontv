use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{column, horizontal_rule, row, text, vertical_space, Column};
use iced::Length;
use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::params::{default_container, duration_display, GAP, HALF_GAP, TITLE_SIZE};

use crate::state::State;

/// The state for the settings page.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Queue;

impl Queue {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, _: &mut State) {}

    /// Render the current download queue.
    pub(crate) fn view(&self, state: &State) -> Column<'static, Message> {
        let mut page = column![];

        let queue = state.service.queue();

        if queue.is_empty() {
            page = page.push(
                row![text("Queue is empty")
                    .size(TITLE_SIZE)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center)]
                .padding(GAP),
            );
        } else {
            page = page.push(
                row![text(format!("Queue ({})", queue.len()))
                    .size(TITLE_SIZE)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center)]
                .padding(GAP),
            );

            let mut it = queue.iter().peekable();

            let now = Utc::now();

            while let Some(d) = it.next() {
                let Some(series) = state.service.series(&d.series_id) else {
                    page = page.push(text(format!("{:?} (no series)", d)));
                    continue;
                };

                let duration = now.signed_duration_since(d.scheduled);
                let when = duration_display(duration);

                page = page.push(
                    row![
                        text(&series.title),
                        text(d.remote_id.to_string()).width(Length::Fill),
                        when,
                    ]
                    .spacing(GAP),
                );

                if it.peek().is_some() {
                    page = page.push(horizontal_rule(1));
                }
            }
        }

        default_container(
            column![page.spacing(HALF_GAP), vertical_space(Length::Shrink),]
                .padding(GAP)
                .spacing(GAP),
        )
    }
}
