use iced::widget::{button, text, Row};
use iced::{theme, Command, Element};
use uuid::Uuid;

use crate::component::Component;
use crate::model::EpisodeId;
use crate::model::SeriesId;
use crate::params::ACTION_SIZE;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Confirm,
    Cancel,
    Start,
}

pub(crate) struct RemoveWatch {
    series_id: SeriesId,
    episode_id: EpisodeId,
    watch_id: Uuid,
    confirm: bool,
}

impl Component<(SeriesId, EpisodeId, Uuid)> for RemoveWatch {
    #[inline]
    fn new((series_id, episode_id, watch_id): (SeriesId, EpisodeId, Uuid)) -> Self {
        Self {
            series_id,
            episode_id,
            watch_id,
            confirm: false,
        }
    }

    #[inline]
    fn changed(&mut self, (series_id, episode_id, watch_id): (SeriesId, EpisodeId, Uuid)) {
        if self.series_id != series_id || self.episode_id != episode_id || self.watch_id != watch_id
        {
            self.series_id = series_id;
            self.episode_id = episode_id;
            self.watch_id = watch_id;
            self.confirm = false;
        }
    }
}

impl RemoveWatch {
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Confirm => {
                self.confirm = false;
                // Todo: not just remove last.
                s.service
                    .remove_episode_watch(&self.series_id, &self.episode_id, &self.watch_id);
            }
            Message::Cancel => {
                self.confirm = false;
            }
            Message::Start => {
                self.confirm = true;
            }
        }

        Command::none()
    }

    pub(crate) fn view(&self, remove_text: &str) -> Element<'static, Message> {
        let mut row = Row::new();

        if self.confirm {
            row = row.push(
                button(text("Remove").size(ACTION_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::Confirm),
            );

            row = row.push(
                button(text("Cancel").size(ACTION_SIZE))
                    .style(theme::Button::Secondary)
                    .on_press(Message::Cancel),
            );
        } else {
            row = row.push(
                button(text(remove_text).size(ACTION_SIZE))
                    .style(theme::Button::Primary)
                    .on_press(Message::Start),
            );
        }

        row.into()
    }
}
