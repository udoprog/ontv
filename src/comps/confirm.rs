use chrono::Utc;
use iced::widget::{button, text, Row};
use iced::{theme, Command, Element};
use uuid::Uuid;

use crate::component::Component;
use crate::model::SeriesId;
use crate::model::{EpisodeId, SeasonNumber};
use crate::params::ACTION_SIZE;
use crate::state::State;

/// Indicates which side of the button confirmation will show up on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ordering {
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    RemoveWatch {
        series_id: SeriesId,
        episode_id: EpisodeId,
        watch_id: Uuid,
    },
    RemoveSeason {
        series_id: SeriesId,
        season: SeasonNumber,
    },
    WatchRemaining {
        series_id: SeriesId,
        season: SeasonNumber,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Confirm,
    Cancel,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Props {
    pub(crate) kind: Kind,
    ordering: Ordering,
}

impl Props {
    #[inline]
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            ordering: Ordering::Right,
        }
    }

    #[inline]
    pub(crate) fn with_ordering(self, ordering: Ordering) -> Self {
        Self { ordering, ..self }
    }
}

pub(crate) struct Confirm {
    props: Props,
    confirm: bool,
}

impl Component<Props> for Confirm {
    #[inline]
    fn new(props: Props) -> Self {
        Self {
            props,
            confirm: false,
        }
    }

    #[inline]
    fn changed(&mut self, props: Props) {
        if self.props != props {
            self.props = props;
            self.confirm = false;
        }
    }
}

impl Confirm {
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Confirm => {
                self.confirm = false;

                match &self.props.kind {
                    Kind::RemoveWatch {
                        series_id,
                        episode_id,
                        watch_id,
                    } => {
                        s.service
                            .remove_episode_watch(series_id, episode_id, watch_id);
                    }
                    Kind::RemoveSeason { series_id, season } => {
                        let now = Utc::now();
                        s.service.remove_season_watches(series_id, season, now);
                    }
                    Kind::WatchRemaining { series_id, season } => {
                        let now = Utc::now();
                        s.service.watch_remaining_season(series_id, season, now);
                    }
                }
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

    pub(crate) fn view(
        &self,
        title: &str,
        initial_theme: theme::Button,
    ) -> Element<'static, Message> {
        let mut row = Row::new();

        if self.confirm {
            let buttons = [
                button(text(title).size(ACTION_SIZE)).style(theme::Button::Secondary),
                button(text("Confirm").size(ACTION_SIZE))
                    .style(initial_theme)
                    .on_press(Message::Confirm),
                button(text("Cancel").size(ACTION_SIZE))
                    .style(theme::Button::Secondary)
                    .on_press(Message::Cancel),
            ];

            match self.props.ordering {
                Ordering::Right => {
                    for b in buttons {
                        row = row.push(b);
                    }
                }
                Ordering::Left => {
                    for b in buttons.into_iter().rev() {
                        row = row.push(b);
                    }
                }
            }
        } else {
            row = row.push(
                button(text(title).size(ACTION_SIZE))
                    .style(initial_theme)
                    .on_press(Message::Start),
            );
        }

        row.into()
    }
}