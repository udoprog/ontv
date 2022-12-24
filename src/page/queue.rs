use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, horizontal_rule, text, vertical_space, Column, Row};
use iced::{theme, Command, Element, Length};
use serde::{Deserialize, Serialize};

use crate::message::Page;
use crate::model::TaskKind;
use crate::params::{default_container, duration_display, GAP, HALF_GAP, SPACE, TITLE_SIZE};
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Navigate to the given page.
    Navigate(Page),
}

/// The state for the settings page.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Queue;

impl Queue {
    pub(crate) fn prepare(&mut self, _: &mut State) {}

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Navigate(page) => {
                s.push_history(page);
            }
        }

        Command::none()
    }

    pub(crate) fn view(&self, state: &State) -> Element<'static, Message> {
        let mut page = Column::new();

        let tasks = state.service.tasks();

        if tasks.is_empty() {
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
                        text(format!("Queue ({})", tasks.len()))
                            .size(TITLE_SIZE)
                            .width(Length::Fill)
                            .horizontal_alignment(Horizontal::Center),
                    )
                    .padding(GAP),
            );

            let mut it = tasks.iter().peekable();

            let now = Utc::now();

            while let Some(task) = it.next() {
                let mut row = Row::new();

                match &task.kind {
                    TaskKind::CheckForUpdates {
                        series_id,
                        remote_id,
                    } => {
                        let mut update = Row::new();

                        update = update.push(text("Check for updates"));

                        if let Some(series) = state.service.series(series_id) {
                            update = update
                                .push(
                                    button(text(&series.title))
                                        .style(theme::Button::Text)
                                        .padding(0)
                                        .on_press(Message::Navigate(Page::Series(*series_id))),
                                )
                                .push(text(remote_id));
                        } else {
                            update = update.push(text(format!("{series_id}")));
                        }

                        row = row.push(update.spacing(SPACE).width(Length::Fill));
                    }
                }

                let duration = now.signed_duration_since(task.scheduled);
                let when = duration_display(duration);

                page = page.push(row.push(when).spacing(GAP));

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
