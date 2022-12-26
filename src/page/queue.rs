use std::time::Duration;

use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, horizontal_rule, text, vertical_space, Column, Row};
use iced::{theme, Commands, Element, Length};

use crate::model::{RemoteSeriesId, SeriesId, TaskKind};
use crate::params::{default_container, duration_display, GAP, HALF_GAP, TITLE_SIZE};
use crate::state::{Page, State};
use crate::utils::{TimedOut, Timeout};

const REMOTE_COLUMN: Length = Length::Units(200);
const DURATION_COLUMN: Length = Length::Units(100);
const UPDATE_TIMER: u64 = 10;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Navigate to the given page.
    Navigate(Page),
    /// Tick the page.
    Tick(TimedOut),
    /// Open the given remote URL.
    OpenRemote(RemoteSeriesId),
}

/// The state for the settings page.
pub(crate) struct Queue {
    timeout: Timeout,
}

impl Queue {
    pub(crate) fn new(mut commands: impl Commands<Message>) -> Self {
        let mut this = Queue {
            timeout: Timeout::default(),
        };

        let future = this.timeout.set(Duration::from_secs(UPDATE_TIMER));
        commands.perform(future, Message::Tick);
        this
    }

    pub(crate) fn prepare(&mut self, _: &mut State) {}

    pub(crate) fn update(
        &mut self,
        s: &mut State,
        message: Message,
        mut commands: impl Commands<Message>,
    ) {
        match message {
            Message::Navigate(page) => {
                s.push_history(page);
            }
            Message::Tick(..) => {
                let future = self.timeout.set(Duration::from_secs(UPDATE_TIMER));
                commands.perform(future, Message::Tick);
            }
            Message::OpenRemote(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open(&url);
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut page = Column::new();

        let tasks = s.service.tasks();

        if tasks.len() == 0 {
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

            let mut it = tasks.peekable();

            let now = Utc::now();

            while let Some(task) = it.next() {
                let mut row = Row::new();
                let mut update = Row::new();

                match &task.kind {
                    TaskKind::CheckForUpdates {
                        series_id,
                        remote_id,
                    } => {
                        update = update.push(text("Check series for updates"));
                        update = decorate_series(s, series_id, Some(remote_id), update);
                    }
                    TaskKind::DownloadSeriesById { series_id, .. } => {
                        update = update.push(text("Update series"));
                        update = decorate_series(s, series_id, None, update);
                    }
                    TaskKind::DownloadSeriesByRemoteId { remote_id, .. } => {
                        update = update.push(text("Download series").width(Length::Fill));

                        update = update.push(
                            button(text(remote_id))
                                .style(theme::Button::Text)
                                .padding(0)
                                .width(REMOTE_COLUMN)
                                .on_press(Message::OpenRemote(*remote_id)),
                        );
                    }
                }

                row = row.push(update.width(Length::Fill).spacing(GAP));

                let duration = now.signed_duration_since(task.scheduled);
                let when = duration_display(duration);

                page = page.push(
                    row.push(
                        when.horizontal_alignment(Horizontal::Right)
                            .width(DURATION_COLUMN),
                    )
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

fn decorate_series<'a>(
    state: &State,
    series_id: &SeriesId,
    remote_id: Option<&RemoteSeriesId>,
    mut update: Row<'a, Message>,
) -> Row<'a, Message> {
    if let Some(series) = state.service.series(series_id) {
        update = update.push(
            button(text(&series.title).width(Length::Fill))
                .style(theme::Button::Text)
                .padding(0)
                .width(Length::Fill)
                .on_press(Message::Navigate(Page::Series(*series_id))),
        );

        if let Some(remote_id) = remote_id.or(series.remote_id.as_ref()) {
            update = update.push(
                button(text(remote_id))
                    .style(theme::Button::Text)
                    .padding(0)
                    .width(REMOTE_COLUMN)
                    .on_press(Message::OpenRemote(*remote_id)),
            );
        }
    } else {
        update = update.push(text(format!("{series_id} (missing data)")).width(Length::Fill));
    }

    update
}
