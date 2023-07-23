use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::queue::TaskKind;
use crate::utils::{TimedOut, Timeout};

const LIMIT: usize = 8;
const UPDATE_TIMER: u64 = 10;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum State {
    #[default]
    Default,
    Running,
    Pending,
    Completed,
}

enum Temporal {
    Past,
    Now,
    Future,
}

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
            Message::Tick(timed_out) => {
                if matches!(timed_out, TimedOut::TimedOut) {
                    let future = self.timeout.set(Duration::from_secs(UPDATE_TIMER));
                    commands.perform(future, Message::Tick);
                }
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

    pub(crate) fn view(&self, cx: &CtxtRef<'_>, state: &State) -> Element<'static, Message> {
        let now = Utc::now();

        let queue = {
            let mut running = cx.service.running_tasks();
            let mut completed = cx.service.completed_tasks();
            let mut pending = cx.service.pending_tasks();

            let mut list = w::Column::new();

            macro_rules! peek {
                () => {
                    running.len() > 0 || completed.len() > 0 || pending.len() > 0
                };
            }

            macro_rules! more {
                ($iter:ident, $toggle:ident) => {
                    if matches!(state, State::Default) {
                        if $iter.len() > 0 {
                            list = list.push(
                                link(w::text(format!("{} more", $iter.len())).size(SMALL))
                                    .on_press(Message::Navigate(Page::Queue(State::$toggle))),
                            );

                            if peek!() {
                                list = list.push(w::horizontal_rule(1));
                            }
                        }
                    }
                };
            }

            macro_rules! title {
                ($iter:ident, $title:expr, $empty:expr) => {
                    list = list
                        .push(w::text($title).size(SUBTITLE_SIZE))
                        .push(w::horizontal_rule(1));

                    if $iter.len() == 0 {
                        list = list.push(w::text($empty).size(SMALL));

                        if peek!() {
                            list = list.push(w::horizontal_rule(1));
                        }
                    }
                };
            }

            if matches!(state, State::Default | State::Running) {
                title!(running, "Running", "No running tasks");

                for _ in 0..matches!(state, State::Running)
                    .then_some(usize::MAX)
                    .unwrap_or(LIMIT)
                {
                    let Some(task) = running.next() else {
                        break;
                    };

                    let row = build_task_row(cx, &task.kind, Temporal::Now);
                    list = list.push(row.width(Length::Fill).spacing(GAP));

                    if peek!() {
                        list = list.push(w::horizontal_rule(1));
                    }
                }

                more!(running, Running);
            }

            if matches!(state, State::Default | State::Pending) {
                title!(pending, "Pending", "No pending tasks");

                for _ in 0..matches!(state, State::Pending)
                    .then_some(usize::MAX)
                    .unwrap_or(LIMIT)
                {
                    let Some(task) = pending.next() else {
                        break;
                    };

                    let duration = task
                        .scheduled
                        .as_ref()
                        .map(|s| now.signed_duration_since(*s))
                        .unwrap_or_else(chrono::Duration::zero);

                    let mut row = build_task_row(cx, &task.kind, Temporal::Future);
                    row = row.push(duration_display(duration).size(SMALL));
                    list = list.push(row.width(Length::Fill).spacing(GAP));

                    if peek!() {
                        list = list.push(w::horizontal_rule(1));
                    }
                }

                more!(pending, Pending);
            }

            if matches!(state, State::Default | State::Completed) {
                title!(completed, "Completed", "No completed tasks");

                for _ in 0..matches!(state, State::Completed)
                    .then_some(usize::MAX)
                    .unwrap_or(LIMIT)
                {
                    let Some(c) = completed.next() else {
                        break;
                    };

                    let mut row = build_task_row(cx, &c.task.kind, Temporal::Past);
                    row = row.push(duration_display(now.signed_duration_since(c.at)).size(SMALL));
                    list = list.push(row.width(Length::Fill).spacing(GAP));

                    if peek!() {
                        list = list.push(w::horizontal_rule(1));
                    }
                }

                more!(completed, Completed);
            }

            list.spacing(SPACE)
        };

        default_container(
            w::Column::new()
                .push(queue)
                .push(w::vertical_space(Length::Shrink))
                .padding(GAP)
                .spacing(GAP),
        )
        .into()
    }
}

fn build_task_row<'a>(cx: &CtxtRef<'_>, kind: &TaskKind, t: Temporal) -> w::Row<'a, Message> {
    let mut update = w::Row::new();

    match kind {
        TaskKind::CheckForUpdates {
            series_id,
            remote_id,
            ..
        } => {
            let text = match t {
                Temporal::Past => "Updated",
                Temporal::Now => "Updating",
                Temporal::Future => "Update",
            };

            update = update.push(w::text(text).size(SMALL));
            update = decorate_series(cx, *series_id, *remote_id, update);
        }
        TaskKind::DownloadSeries {
            series_id,
            remote_id,
            ..
        } => {
            let text = match t {
                Temporal::Past => "Downloaded series",
                Temporal::Now => "Updating series",
                Temporal::Future => "Update series",
            };

            update = update.push(w::text(text).size(SMALL));
            update = decorate_series(cx, *series_id, *remote_id, update);
        }
        TaskKind::DownloadSeriesByRemoteId { remote_id, .. } => {
            let text = match t {
                Temporal::Past => "Downloaded series",
                Temporal::Now => "Updating series",
                Temporal::Future => "Update series",
            };

            update = update.push(w::text(text).size(SMALL).width(Length::Fill));
            update = update.push(
                link(w::text(remote_id).size(SMALL))
                    .width(Length::Fill)
                    .on_press(Message::OpenRemoteSeries(*remote_id)),
            );
        }
        TaskKind::DownloadMovieByRemoteId { remote_id } => {
            let text = match t {
                Temporal::Past => "Downloaded movie",
                Temporal::Now => "Updating movie",
                Temporal::Future => "Update movie",
            };

            update = update.push(w::text(text).size(SMALL).width(Length::Fill));
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
    series_id: SeriesId,
    remote_id: RemoteSeriesId,
    mut row: w::Row<'a, Message>,
) -> w::Row<'a, Message> {
    row = row
        .push(link(w::text(remote_id).size(SMALL)).on_press(Message::OpenRemoteSeries(remote_id)));

    let text = if let Some(series) = cx.service.series(&series_id) {
        w::text(&series.title)
    } else {
        w::text(format!("{series_id}"))
    };

    row.push(
        link(text.size(SMALL))
            .width(Length::Fill)
            .on_press(Message::Navigate(page::series::page(series_id))),
    )
}
