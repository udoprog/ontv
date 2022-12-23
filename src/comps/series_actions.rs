use iced::widget::{button, text, Row};
use iced::{theme, Command, Element};

use crate::message::ErrorMessage;
use crate::model::{RemoteSeriesId, Series, SeriesId};
use crate::params::{ACTION_SIZE, SPACE};
use crate::service::NewSeries;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Untrack(SeriesId),
    Track(SeriesId),
    RefreshSeries(SeriesId),
    RemoveSeries(SeriesId),
    SeriesDownloadToTrack(Option<SeriesId>, RemoteSeriesId, NewSeries),
    SeriesDownloadFailed(Option<SeriesId>, RemoteSeriesId, ErrorMessage),
}

#[derive(Default, Debug, Clone)]
pub(crate) struct SeriesActions {
    _confirm: bool,
}

impl SeriesActions {
    /// Update message.
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Untrack(series_id) => {
                s.service.untrack(&series_id);
                Command::none()
            }
            Message::Track(series_id) => {
                s.service.track(&series_id);
                Command::none()
            }
            Message::RefreshSeries(series_id) => {
                if let Some(future) = s.refresh_series(&series_id) {
                    Command::perform(future, |(id, remote_id, result)| match result {
                        Ok(new_data) => Message::SeriesDownloadToTrack(id, remote_id, new_data),
                        Err(e) => Message::SeriesDownloadFailed(id, remote_id, e.into()),
                    })
                } else {
                    Command::none()
                }
            }
            Message::RemoveSeries(series_id) => {
                s.remove_series(&series_id);
                Command::none()
            }
            Message::SeriesDownloadToTrack(id, remote_id, data) => {
                s.download_complete(id, remote_id);
                s.service.insert_new_series(data);
                Command::none()
            }
            Message::SeriesDownloadFailed(id, remote_id, error) => {
                s.download_complete(id, remote_id);
                s.handle_error(error);
                Command::none()
            }
        }
    }

    /// Generate buttons which perform actions on the given series.
    pub(crate) fn view(&self, s: &State, series: &Series) -> Element<'static, Message> {
        let mut row = Row::new();

        if series.tracked {
            row = row.push(
                button(text("Untrack").size(ACTION_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Untrack(series.id)),
            );
        } else {
            row = row.push(
                button(text("Track").size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::Track(series.id)),
            );
        }

        if s.is_downloading_id(&series.id) {
            row = row.push(
                button(text("Downloading...").size(ACTION_SIZE)).style(theme::Button::Primary),
            );
        } else {
            row = row.push(
                button(text("Refresh").size(ACTION_SIZE))
                    .style(theme::Button::Positive)
                    .on_press(Message::RefreshSeries(series.id)),
            );

            row = row.push(
                button(text("Remove").size(ACTION_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::RemoveSeries(series.id)),
            );
        }

        row.spacing(SPACE).into()
    }
}
