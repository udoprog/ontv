use std::time::Duration;

use crate::prelude::*;
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

    pub(crate) fn update(
        &mut self,
        cx: &mut Ctxt<'_>,
        message: Message,
        mut commands: impl Commands<Message>,
    ) {
        match message {
            Message::Navigate(page) => {
                cx.push_history(page);
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

    pub(crate) fn view(&self, cx: &CtxtRef<'_>) -> Element<'static, Message> {
        let now = Utc::now();

        let mut running_col = w::Column::new();

        let mut running = cx.service.running_tasks().peekable();

        running_col = running_col.push(
            w::Row::new()
                .push(
                    w::text(format!("Running ({})", running.len()))
                        .width(Length::Fill)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(GAP),
        );

        if running.len() == 0 {
            running_col = running_col.push(
                w::text("Empty")
                    .size(SMALL)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            );
        }

        let mut list = w::Column::new();

        while let Some(task) = running.next() {
            let mut row = w::Row::new();
            let update = build_task_row(cx, &task.kind);
            row = row.push(update.width(Length::Fill).spacing(GAP));

            list = list.push(row.width(Length::Fill).spacing(GAP));

            if running.peek().is_some() {
                list = list.push(w::horizontal_rule(1));
            }
        }

        running_col = running_col.push(list.spacing(SPACE));

        let mut tasks_col = w::Column::new();

        let mut tasks = cx.service.tasks().peekable();

        tasks_col = tasks_col.push(
            w::Row::new()
                .push(
                    w::text(format!("Queue ({})", tasks.len()))
                        .width(Length::Fill)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(GAP),
        );

        if tasks.len() == 0 {
            tasks_col = tasks_col.push(
                w::text("Empty")
                    .size(SMALL)
                    .width(Length::Fill)
                    .horizontal_alignment(Horizontal::Center),
            );
        }

        let mut list = w::Column::new();

        while let Some(task) = tasks.next() {
            let mut row = build_task_row(cx, &task.kind.id());

            let duration = match &task.scheduled {
                Some(scheduled) => now.signed_duration_since(*scheduled),
                None => chrono::Duration::zero(),
            };

            let when = duration_display(duration);
            row = row.push(when.size(SMALL));

            list = list.push(row.width(Length::Fill).spacing(GAP));

            if tasks.peek().is_some() {
                list = list.push(w::horizontal_rule(1));
            }
        }

        tasks_col = tasks_col.push(list.spacing(SPACE));

        let page = w::Row::new()
            .push(tasks_col.width(Length::FillPortion(1)).spacing(GAP))
            .push(running_col.width(Length::FillPortion(1)).spacing(GAP));

        default_container(
            w::Column::new()
                .push(page.spacing(GAP2))
                .push(w::vertical_space(Length::Shrink))
                .padding(GAP)
                .spacing(GAP),
        )
        .into()
    }
}

fn build_task_row<'a>(cx: &CtxtRef<'_>, kind: &TaskId) -> w::Row<'a, Message> {
    let mut update = w::Row::new();

    match kind {
        TaskId::CheckForUpdates {
            series_id,
            remote_id,
            ..
        } => {
            update = update.push(w::text("Updates").size(SMALL));
            update = decorate_series(cx, series_id, Some(remote_id), update);
        }
        TaskId::DownloadSeriesById { series_id, .. } => {
            update = update.push(w::text("Downloading").size(SMALL));
            update = decorate_series(cx, series_id, None, update);
        }
        TaskId::DownloadSeriesByRemoteId { remote_id, .. } => {
            update = update.push(w::text("Downloading").size(SMALL).width(Length::Fill));

            update = update.push(
                link(w::text(remote_id).size(SMALL))
                    .width(Length::Fill)
                    .on_press(Message::OpenRemoteSeries(*remote_id)),
            );
        }
        TaskId::DownloadMovieByRemoteId { remote_id } => {
            update = update.push(w::text("Downloading").size(SMALL).width(Length::Fill));

            update = update.push(
                link(w::text(remote_id).size(SMALL))
                    .width(Length::Fill)
                    .on_press(Message::OpenRemoteMovie(*remote_id)),
            );
        }
    }

    update
}

fn decorate_series<'a>(
    cx: &CtxtRef<'_>,
    series_id: &SeriesId,
    remote_id: Option<&RemoteSeriesId>,
    mut row: w::Row<'a, Message>,
) -> w::Row<'a, Message> {
    let remote_id = if let Some(series) = cx.service.series(series_id) {
        row = row.push(
            link(w::text(&series.title).size(SMALL))
                .on_press(Message::Navigate(Page::Series(*series_id))),
        );

        remote_id.or(series.remote_id.as_ref())
    } else {
        row = row.push(w::text(format!("{series_id}")).size(SMALL));
        remote_id
    };

    if let Some(remote_id) = remote_id {
        row = row.push(
            link(w::text(remote_id).size(SMALL))
                .width(Length::Fill)
                .on_press(Message::OpenRemoteSeries(*remote_id)),
        );
    }

    row
}
