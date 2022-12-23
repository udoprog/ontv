use chrono::Utc;
use iced::widget::{button, text, Column, Row};
use iced::{theme, Command, Element, Length};

use crate::component::Component;
use crate::model::{SeasonNumber, SeriesId};
use crate::params::{ACTION_SIZE, GAP, SPACE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Weatch the remainder of all unwatched episodes in the specified season.
    WatchRemainingSeason,
    /// Remove all matching season watches.
    RemoveSeasonWatches,
}

#[derive(Clone)]
pub(crate) struct SeasonInfo {
    series_id: SeriesId,
    season: SeasonNumber,
}

impl Component<(SeriesId, SeasonNumber)> for SeasonInfo {
    #[inline]
    fn new((series_id, season): (SeriesId, SeasonNumber)) -> Self {
        Self { series_id, season }
    }

    #[inline]
    fn init(&mut self, (series_id, season): (SeriesId, SeasonNumber)) {
        self.series_id = series_id;
        self.season = season;
    }
}

impl SeasonInfo {
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::WatchRemainingSeason => {
                let now = Utc::now();
                s.service
                    .watch_remaining_season(&self.series_id, &self.season, now);
                Command::none()
            }
            Message::RemoveSeasonWatches => {
                let now = Utc::now();
                s.service
                    .remove_season_watches(&self.series_id, &self.season, now);
                Command::none()
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let (watched, total) = s.service.season_watched(&self.series_id, &self.season);
        let mut actions = Row::new().spacing(SPACE);

        if watched < total {
            actions = actions.push(
                button(text("Watch remaining").size(ACTION_SIZE))
                    .style(theme::Button::Primary)
                    .on_press(Message::WatchRemainingSeason),
            );
        }

        if watched != 0 {
            actions = actions.push(
                button(text("Remove watches").size(ACTION_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::RemoveSeasonWatches),
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
