use crate::prelude::*;
use crate::queue::TaskStatus;

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
    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::Untrack => {
                s.service.untrack(&self.series_id);
            }
            Message::Track => {
                s.service.track(&self.series_id);
            }
            Message::RefreshSeries(remote_id) => {
                s.service
                    .push_task_without_delay(TaskKind::DownloadSeriesById {
                        series_id: self.series_id,
                        remote_id,
                        last_modified: None,
                        force: true,
                    });
            }
            Message::RemoveSeries => {
                s.remove_series(&self.series_id);
            }
        }
    }

    pub(crate) fn view(&self, s: &State, series: &Series) -> Element<'static, Message> {
        let mut row = w::Row::new();

        if series.tracked {
            row = row.push(
                w::button(w::text("Untrack").size(SMALL))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Untrack),
            );
        } else {
            row = row.push(
                w::button(w::text("Track").size(SMALL))
                    .style(theme::Button::Positive)
                    .on_press(Message::Track),
            );
        }

        let status = s.service.task_status(&TaskId::DownloadSeriesById {
            series_id: series.id,
        });

        match status {
            Some(TaskStatus::Pending) => {
                row = row.push(
                    w::button(w::text("Refresh in queue").size(SMALL))
                        .style(theme::Button::Positive),
                );
            }
            Some(TaskStatus::Running) => {
                row = row.push(
                    w::button(w::text("Downloading...").size(SMALL)).style(theme::Button::Primary),
                );
            }
            None => {
                if let Some(remote_id) = series.remote_id {
                    row = row.push(
                        w::button(w::text("Refresh").size(SMALL))
                            .style(theme::Button::Positive)
                            .on_press(Message::RefreshSeries(remote_id)),
                    );
                }
            }
        }

        row = row.push(
            w::button(w::text("Remove").size(SMALL))
                .style(theme::Button::Destructive)
                .on_press(Message::RemoveSeries),
        );

        row.spacing(SPACE).into()
    }
}
