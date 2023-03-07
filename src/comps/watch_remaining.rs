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
pub(crate) struct Props {
    ordering: Ordering,
    series_id: SeriesId,
    season: SeasonNumber,
}

impl Props {
    #[inline]
    pub(crate) fn new(series_id: SeriesId, season: SeasonNumber) -> Self {
        Self {
            ordering: Ordering::Right,
            series_id,
            season,
        }
    }
}

pub(crate) struct WatchRemaining {
    props: Props,
    confirm: bool,
}

impl Component<Props> for WatchRemaining {
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

impl WatchRemaining {
    pub(crate) fn is_confirm(&self) -> bool {
        self.confirm
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::RightNow => {
                self.confirm = false;
                let now = Utc::now();
                s.service.watch_remaining_season(
                    &now,
                    &self.props.series_id,
                    &self.props.season,
                    RemainingSeason::Aired,
                );
            }
            Message::AirDate => {
                self.confirm = false;
                let now = Utc::now();
                s.service.watch_remaining_season(
                    &now,
                    &self.props.series_id,
                    &self.props.season,
                    RemainingSeason::AirDate,
                );
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
    ) -> Element<'static, Message> {
        let mut row = w::Row::new();

        if self.confirm {
            let buttons = [
                w::button(w::text(title).size(SMALL)).style(theme::Button::Secondary),
                w::button(w::text("Right now").size(SMALL))
                    .style(right_now)
                    .on_press(Message::RightNow),
                w::button(w::text("Air date").size(SMALL))
                    .style(air_date)
                    .on_press(Message::AirDate),
                w::button(w::text("Cancel").size(SMALL))
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
                w::button(w::text(title).size(SMALL))
                    .style(right_now)
                    .on_press(Message::Start),
            );
        }

        row.into()
    }
}
