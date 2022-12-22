use chrono::Utc;
use iced::widget::{button, text, Column, Row};
use iced::{theme, Command, Element, Length};
use uuid::Uuid;

use crate::model::{Season, SeasonNumber, Series};
use crate::params::{ACTION_SIZE, GAP, SPACE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Weatch the remainder of all unwatched episodes in the specified season.
    WatchRemainingSeason(Uuid, SeasonNumber),
    /// Remove all matching season watches.
    RemoveSeasonWatches(Uuid, SeasonNumber),
}

#[derive(Default, Clone)]
pub(crate) struct SeasonInfo {}

impl SeasonInfo {
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::WatchRemainingSeason(series, season) => {
                let now = Utc::now();
                s.service.watch_remaining_season(series, season, now);
                Command::none()
            }
            Message::RemoveSeasonWatches(series, season) => {
                let now = Utc::now();
                s.service.remove_season_watches(series, season, now);
                Command::none()
            }
        }
    }

    pub(crate) fn view(
        &self,
        s: &State,
        series: &Series,
        season: &Season,
    ) -> Element<'static, Message> {
        let (watched, total) = s.service.season_watched(series.id, season.number);
        let mut actions = Row::new().spacing(SPACE);

        if watched < total {
            actions = actions.push(
                button(text("Watch remaining").size(ACTION_SIZE))
                    .style(theme::Button::Primary)
                    .on_press(Message::WatchRemainingSeason(series.id, season.number)),
            );
        }

        if watched != 0 {
            actions = actions.push(
                button(text("Remove watches").size(ACTION_SIZE))
                    .style(theme::Button::Destructive)
                    .on_press(Message::RemoveSeasonWatches(series.id, season.number)),
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
