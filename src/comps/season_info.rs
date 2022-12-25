use iced::widget::{text, Column, Row};
use iced::{theme, Element, Length};

use crate::component::Component;
use crate::comps;
use crate::model::{SeasonNumber, SeriesId};
use crate::params::{GAP, SPACE};
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Weatch the remainder of all unwatched episodes in the specified season.
    WatchRemaining(comps::confirm::Message),
    /// Remove all matching season watches.
    RemoveWatches(comps::confirm::Message),
}

pub(crate) struct SeasonInfo {
    series_id: SeriesId,
    season: SeasonNumber,
    watch_remaining: comps::confirm::Confirm,
    remove_watches: comps::confirm::Confirm,
}

impl Component<(SeriesId, SeasonNumber)> for SeasonInfo {
    #[inline]
    fn new((series_id, season): (SeriesId, SeasonNumber)) -> Self {
        Self {
            series_id,
            season,
            watch_remaining: comps::confirm::Confirm::new(comps::confirm::Props::new(
                comps::confirm::Kind::WatchRemaining { series_id, season },
            )),
            remove_watches: comps::confirm::Confirm::new(comps::confirm::Props::new(
                comps::confirm::Kind::RemoveSeason { series_id, season },
            )),
        }
    }

    #[inline]
    fn changed(&mut self, (series_id, season): (SeriesId, SeasonNumber)) {
        self.series_id = series_id;
        self.season = season;
        self.watch_remaining.changed(comps::confirm::Props::new(
            comps::confirm::Kind::WatchRemaining { series_id, season },
        ));
        self.remove_watches.changed(comps::confirm::Props::new(
            comps::confirm::Kind::RemoveSeason { series_id, season },
        ));
    }
}

impl SeasonInfo {
    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::WatchRemaining(m) => {
                self.watch_remaining.update(s, m);
            }
            Message::RemoveWatches(m) => {
                self.remove_watches.update(s, m);
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let (watched, total) = s.service.season_watched(&self.series_id, &self.season);
        let mut actions = Row::new().spacing(SPACE);

        if watched < total {
            actions = actions.push(
                self.watch_remaining
                    .view("Watch remaining", theme::Button::Positive)
                    .map(Message::WatchRemaining),
            );
        }

        if watched != 0 {
            actions = actions.push(
                self.remove_watches
                    .view("Remove watches", theme::Button::Destructive)
                    .map(Message::RemoveWatches),
            );
        }

        let plural = match total {
            1 => "episode",
            _ => "episodes",
        };

        let percentage = if let Some(p) = (watched * 100).checked_div(total) {
            format!("{p}%")
        } else {
            String::from("0%")
        };

        let info = text(format!(
            "Watched {watched} out of {total} {plural} ({percentage})"
        ));

        Column::new()
            .push(actions)
            .push(info)
            .spacing(GAP)
            .width(Length::Fill)
            .into()
    }
}
