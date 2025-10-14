use iced::Theme;

use crate::comps::ordering::Ordering;
use crate::prelude::*;
use crate::service::RemainingSeason;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RightNow,
    AirDate,
    Cancel,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Episode(EpisodeId),
    Movie(MovieId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Props {
    ordering: Ordering,
    kind: Kind,
}

impl Props {
    #[inline]
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            ordering: Ordering::Right,
            kind,
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

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::RightNow => {
                self.confirm = false;
                let now = Utc::now();

                match &self.props.kind {
                    Kind::Episode(id) => {
                        cx.service.watch(&now, id, RemainingSeason::Aired);
                    }
                    Kind::Movie(id) => {
                        cx.service.watch_movie(&now, id, RemainingSeason::Aired);
                    }
                }
            }
            Message::AirDate => {
                self.confirm = false;
                let now = Utc::now();

                match &self.props.kind {
                    Kind::Episode(id) => {
                        cx.service.watch(&now, id, RemainingSeason::AirDate);
                    }
                    Kind::Movie(id) => {
                        cx.service.watch_movie(&now, id, RemainingSeason::AirDate);
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
    }

    pub(crate) fn view<'a>(
        &self,
        title: &'a str,
        right_now: fn(&Theme, w::button::Status) -> w::button::Style,
        air_date: fn(&Theme, w::button::Status) -> w::button::Style,
        width: Length,
        alignment: Horizontal,
        reminder: bool,
    ) -> Element<'a, Message> {
        let mut row = w::Row::new().width(width);

        if self.confirm {
            let buttons = [
                w::button(w::text("Now").size(SMALL_SIZE))
                    .style(right_now)
                    .on_press(Message::RightNow),
                w::button(w::text("Air date").size(SMALL_SIZE))
                    .style(air_date)
                    .on_press(Message::AirDate),
                w::button(
                    w::text("Cancel")
                        .align_x(Horizontal::Center)
                        .size(SMALL_SIZE),
                )
                .style(w::button::secondary)
                .width(Length::Fill)
                .on_press(Message::Cancel),
            ];

            let head = if reminder {
                Some(w::button(w::text(title).size(SMALL_SIZE)).style(w::button::secondary))
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
                w::button(w::text(title).size(SMALL_SIZE).align_x(alignment))
                    .style(right_now)
                    .on_press(Message::Start),
            );
        }

        row.into()
    }
}
