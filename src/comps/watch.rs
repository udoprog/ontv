use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, text, Row};
use iced::{theme, Element, Length};

use crate::component::Component;
use crate::comps::ordering::Ordering;
use crate::model::EpisodeId;
use crate::params::SMALL;
use crate::service::RemainingSeason;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RightNow,
    AirDate,
    Cancel,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Props {
    ordering: Ordering,
    episode_id: EpisodeId,
}

impl Props {
    #[inline]
    pub(crate) fn new(episode_id: EpisodeId) -> Self {
        Self {
            ordering: Ordering::Right,
            episode_id,
        }
    }
}

pub(crate) struct Watch {
    props: Props,
    confirm: bool,
}

impl Component<Props> for Watch {
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

impl Watch {
    pub(crate) fn is_confirm(&self) -> bool {
        self.confirm
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::RightNow => {
                self.confirm = false;
                let now = Utc::now();
                s.service
                    .watch(&now, &self.props.episode_id, RemainingSeason::Aired);
            }
            Message::AirDate => {
                self.confirm = false;
                let now = Utc::now();
                s.service
                    .watch(&now, &self.props.episode_id, RemainingSeason::AirDate);
            }
            Message::Cancel => {
                self.confirm = false;
            }
            Message::Start => {
                self.confirm = true;
            }
        }
    }

    pub(crate) fn view(
        &self,
        title: &str,
        right_now: theme::Button,
        air_date: theme::Button,
        width: Length,
        alignment: Horizontal,
        reminder: bool,
    ) -> Element<'static, Message> {
        let mut row = Row::new().width(width);

        if self.confirm {
            let buttons = [
                button(text("Now").size(SMALL))
                    .style(right_now)
                    .on_press(Message::RightNow),
                button(text("Air date").size(SMALL))
                    .style(air_date)
                    .on_press(Message::AirDate),
                button(
                    text("Cancel")
                        .horizontal_alignment(Horizontal::Center)
                        .size(SMALL),
                )
                .style(theme::Button::Secondary)
                .width(width)
                .on_press(Message::Cancel),
            ];

            let head = if reminder {
                Some(button(text(title).size(SMALL)).style(theme::Button::Secondary))
            } else {
                None
            };

            let buttons = head.into_iter().chain(buttons);

            match self.props.ordering {
                Ordering::Right => {
                    for b in buttons {
                        row = row.push(b);
                    }
                }
                Ordering::Left => {
                    for b in buttons.rev() {
                        row = row.push(b);
                    }
                }
            }
        } else {
            row = row.push(
                button(
                    text(title)
                        .size(SMALL)
                        .width(Length::Fill)
                        .horizontal_alignment(alignment),
                )
                .style(right_now)
                .width(width)
                .on_press(Message::Start),
            );
        }

        row.into()
    }
}
