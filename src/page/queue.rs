use std::time::Duration;

use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, horizontal_rule, text, vertical_space, Column, Row};
use iced::{theme, Commands, Element, Length};

use crate::model::{RemoteMovieId, RemoteSeriesId, SeriesId, TaskKind};
use crate::params::{default_container, duration_display, GAP, GAP2, SMALL, SPACE};
use crate::state::{Page, State};
use crate::utils::{TimedOut, Timeout};

const UPDATE_TIMER: u64 = 10;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Navigate to the given page.
    Navigate(Page),
    /// Tick the page.
    Tick(TimedOut),
    /// Open the given remote series.
    OpenRemoteSeries(RemoteSeriesId),
    /// Open the given remote movie.
    OpenRemoteMovie(RemoteMovieId),
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
            Message::OpenRemoteSeries(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open(&url);
            }
            Message::OpenRemoteMovie(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open(&url);
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let now = Utc::now();

        let mut running_col = Column::new();

        let mut running = s.service.running_tasks().peekable();

        running_col = running_col.push(
            Row::new()
                .push(
                    text(format!("Running ({})", running.len()))
                        .width(Length::Fill)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(GAP),
        );

        if running.len() == 0 {
            running_col = running_col.push(
                text("Empty")
                    .size(SMALL)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            );
        }

        let mut list = Column::new();

        while let Some(task) = running.next() {
            let mut row = Row::new();
            let update = build_task_row(s, &task.kind);
            row = row.push(update.width(Length::Fill).spacing(GAP));

            list = list.push(row.width(Length::Fill).spacing(GAP));

            if running.peek().is_some() {
                list = list.push(horizontal_rule(1));
            }
        }

        running_col = running_col.push(list.spacing(SPACE));

        let mut tasks_col = Column::new();

        let mut tasks = s.service.tasks().peekable();

        tasks_col = tasks_col.push(
            Row::new()
                .push(
                    text(format!("Queue ({})", tasks.len()))
                        .width(Length::Fill)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(GAP),
        );

        if tasks.len() == 0 {
            tasks_col = tasks_col.push(
                text("Empty")
                    .size(SMALL)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            );
        }

        let mut list = Column::new();

        while let Some(task) = tasks.next() {
            let mut row = build_task_row(s, &task.kind);

            let duration = now.signed_duration_since(task.scheduled);
            let when = duration_display(duration);

            row = row.push(when.size(SMALL));

            list = list.push(row.width(Length::Fill).spacing(GAP));

            if tasks.peek().is_some() {
                list = list.push(horizontal_rule(1));
            }
        }

        tasks_col = tasks_col.push(list.spacing(SPACE));

        let page = Row::new()
            .push(tasks_col.width(Length::FillPortion(1)).spacing(GAP))
            .push(running_col.width(Length::FillPortion(1)).spacing(GAP));

        default_container(
            Column::new()
                .push(page.spacing(GAP2))
                .push(vertical_space(Length::Shrink))
                .padding(GAP)
                .spacing(GAP),
        )
        .into()
    }
}

fn build_task_row<'a>(s: &State, kind: &TaskKind) -> Row<'a, Message> {
    let mut update = Row::new();

    match kind {
        TaskKind::CheckForUpdates {
            series_id,
            remote_id,
        } => {
            update = update.push(text("Updates").size(SMALL));
            update = decorate_series(s, series_id, Some(remote_id), update);
        }
        TaskKind::DownloadSeriesById { series_id, .. } => {
            update = update.push(text("Downloading").size(SMALL));
            update = decorate_series(s, series_id, None, update);
        }
        TaskKind::DownloadSeriesByRemoteId { remote_id, .. } => {
            update = update.push(text("Downloading").size(SMALL).width(Length::Fill));

            update = update.push(
                button(text(remote_id).size(SMALL))
                    .width(Length::Fill)
                    .style(theme::Button::Text)
                    .padding(0)
                    .on_press(Message::OpenRemoteSeries(*remote_id)),
            );
        }
        TaskKind::DownloadMovieByRemoteId { remote_id } => {
            update = update.push(text("Downloading").size(SMALL).width(Length::Fill));

            update = update.push(
                button(text(remote_id).size(SMALL))
                    .width(Length::Fill)
                    .style(theme::Button::Text)
                    .padding(0)
                    .on_press(Message::OpenRemoteMovie(*remote_id)),
            );
        }
    }

    update
}

fn decorate_series<'a>(
    state: &State,
    series_id: &SeriesId,
    remote_id: Option<&RemoteSeriesId>,
    mut row: Row<'a, Message>,
) -> Row<'a, Message> {
    let remote_id = if let Some(series) = state.service.series(series_id) {
        row = row.push(
            button(text(&series.title).size(SMALL))
                .style(theme::Button::Text)
                .padding(0)
                .on_press(Message::Navigate(Page::Series(*series_id))),
        );

        remote_id.or(series.remote_id.as_ref())
    } else {
        row = row.push(text(format!("{series_id}")).size(SMALL));
        remote_id
    };

    if let Some(remote_id) = remote_id {
        row = row.push(
            button(text(remote_id).size(SMALL))
                .width(Length::Fill)
                .style(theme::Button::Text)
                .padding(0)
                .on_press(Message::OpenRemoteSeries(*remote_id)),
        );
    }

    row
}
