use crate::prelude::*;
use crate::queue::{TaskKind, TaskRef, TaskStatus};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Untrack,
    Track,
    RefreshSeries(RemoteSeriesId),
    RemoveSeries,
}

#[derive(Debug, Clone)]
pub(crate) struct SeriesActions {
    series_id: SeriesId,
    confirm: bool,
}

impl Component<SeriesId> for SeriesActions {
    #[inline]
    fn new(series_id: SeriesId) -> Self {
        Self {
            series_id,
            confirm: false,
        }
    }

    #[inline]
    fn changed(&mut self, series_id: SeriesId) {
        if self.series_id != series_id {
            self.series_id = series_id;
            self.confirm = false;
        }
    }
}

impl SeriesActions {
    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::Untrack => {
                cx.service.untrack(&self.series_id);
            }
            Message::Track => {
                cx.service.track(&self.series_id);
            }
            Message::RefreshSeries(remote_id) => {
                cx.service
                    .push_task_without_delay(TaskKind::DownloadSeries {
                        series_id: self.series_id,
                        remote_id,
                        last_modified: None,
                        force: true,
                    });
            }
            Message::RemoveSeries => {
                cx.remove_series(&self.series_id);
            }
        }
    }

    pub(crate) fn view(&self, cx: &CtxtRef<'_>, series: &Series) -> Element<'static, Message> {
        let mut row = w::Row::new();

        if series.tracked {
            row = row.push(
                w::button(w::text("Untrack").size(SMALL_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Untrack),
            );
        } else {
            row = row.push(
                w::button(w::text("Track").size(SMALL_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Track),
            );
        }

        let status = cx.service.task_status(TaskRef::Series {
            series_id: series.id,
        });

        match status {
            Some(TaskStatus::Pending) => {
                row = row.push(
                    w::button(w::text("Refresh").size(SMALL_SIZE)).style(theme::Button::Positive),
                );
            }
            Some(TaskStatus::Running) => {
                row = row.push(
                    w::button(w::text("Downloading...").size(SMALL_SIZE))
                        .style(theme::Button::Primary),
                );
            }
            None => {
                if let Some(remote_id) = series.remote_id {
                    row = row.push(
                        w::button(w::text("Refresh").size(SMALL_SIZE))
                            .style(theme::Button::Positive)
                            .on_press(Message::RefreshSeries(remote_id)),
                    );
                }
            }
        }

        row = row.push(
            w::button(w::text("Remove").size(SMALL_SIZE))
                .style(theme::Button::Destructive)
                .on_press(Message::RemoveSeries),
        );

        row.spacing(SPACE).into()
    }
}
