use iced::widget::{button, text, Row};
use iced::{theme, Element};

use crate::component::Component;
use crate::model::{Series, SeriesId, TaskData, TaskKind};
use crate::params::{SMALL, SPACE};
use crate::queue::TaskStatus;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Untrack,
    Track,
    RefreshSeries,
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
            Message::RefreshSeries => {
                s.service.push_task_without_delay(
                    TaskKind::DownloadSeriesById {
                        series_id: self.series_id,
                    },
                    TaskData::default(),
                );
            }
            Message::RemoveSeries => {
                s.remove_series(&self.series_id);
            }
        }
    }

    pub(crate) fn view(&self, s: &State, series: &Series) -> Element<'static, Message> {
        let mut row = Row::new();

        if series.tracked {
            row = row.push(
                button(text("Untrack").size(SMALL))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Untrack),
            );
        } else {
            row = row.push(
                button(text("Track").size(SMALL))
                    .style(theme::Button::Positive)
                    .on_press(Message::Track),
            );
        }

        let status = s.service.task_status(&TaskKind::DownloadSeriesById {
            series_id: series.id,
        });

        match status {
            Some(TaskStatus::Pending) => {
                row = row.push(
                    button(text("Refresh in queue").size(SMALL)).style(theme::Button::Positive),
                );
            }
            Some(TaskStatus::Running) => {
                row = row
                    .push(button(text("Downloading...").size(SMALL)).style(theme::Button::Primary));
            }
            None => {
                row = row.push(
                    button(text("Refresh").size(SMALL))
                        .style(theme::Button::Positive)
                        .on_press(Message::RefreshSeries),
                );
            }
        }

        row = row.push(
            button(text("Remove").size(SMALL))
                .style(theme::Button::Destructive)
                .on_press(Message::RemoveSeries),
        );

        row.spacing(SPACE).into()
    }
}
