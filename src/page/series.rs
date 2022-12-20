use iced::widget::{button, column, image, row, text, Column, Row};
use iced::{theme, Command};
use iced::{Alignment, Length};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::model::{RemoteSeriesId, Season, Series};
use crate::params::{centered, style, ACTION_SIZE, GAP, GAP2, SPACE, SUBTITLE_SIZE, TITLE_SIZE};
use crate::service::Service;

#[derive(Debug, Clone)]
pub(crate) enum M {
    OpenRemote(RemoteSeriesId),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct State;

impl State {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, service: &Service, assets: &mut Assets, id: Uuid) {
        if let Some(s) = service.series(id) {
            prepare_series_banner(assets, s);
        }
    }

    /// Handle series messages.
    pub(crate) fn update(&mut self, message: M) -> Command<Message> {
        match message {
            M::OpenRemote(remote_id) => {
                let url = match remote_id {
                    RemoteSeriesId::TheTvDb { id } => {
                        format!("https://thetvdb.com/search?query={id}")
                    }
                    RemoteSeriesId::Imdb { id } => {
                        format!("https://www.imdb.com/title/{id}/")
                    }
                };

                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
        }

        Command::none()
    }

    /// Render view of series.
    pub(crate) fn view(
        &self,
        service: &Service,
        assets: &Assets,
        id: Uuid,
    ) -> Column<'static, Message> {
        let Some(series) = service.series(id) else {
            return column![text("no series")];
        };

        let mut top = series_banner(assets, series);

        if !series.remote_ids.is_empty() {
            let mut remotes = row![];

            for remote_id in &series.remote_ids {
                remotes = remotes.push(
                    button(text(remote_id.to_string()))
                        .style(theme::Button::Text)
                        .on_press(Message::Series(M::OpenRemote(*remote_id))),
                );
            }

            top = top.push(remotes.spacing(SPACE));
        }

        let mut seasons = column![];

        for season in service.seasons(series.id) {
            let title = button(season.number.title().size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Season(series.id, season.number)));

            seasons = seasons.push(
                centered(
                    column![
                        title,
                        season_info(service, series, season)
                            .spacing(GAP)
                            .width(Length::Fill)
                    ]
                    .spacing(SPACE),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let info = match service.episodes(series.id).count() {
            0 => text(format!("No episodes")),
            1 => text(format!("One episode")),
            count => text(format!("{count} episodes")),
        };

        let mut header = column![top.spacing(GAP), actions(series).spacing(SPACE), info,];

        if let Some(overview) = &series.overview {
            header = header.push(text(overview));
        }

        let header = centered(header.spacing(GAP), None).padding(GAP);
        column![header, seasons.spacing(GAP2)].spacing(GAP2)
    }
}

/// Generate buttons which perform actions on the given series.
pub(crate) fn actions(s: &Series) -> Row<'static, Message> {
    let mut row = row![];

    if s.tracked {
        row = row.push(
            button(text("Untrack").size(ACTION_SIZE))
                .style(theme::Button::Destructive)
                .on_press(Message::Untrack(s.id)),
        );
    } else {
        row = row.push(
            button(text("Track").size(ACTION_SIZE))
                .style(theme::Button::Positive)
                .on_press(Message::Track(s.id)),
        );
    }

    row = row.push(
        button(text("Refresh").size(ACTION_SIZE))
            .style(theme::Button::Positive)
            .on_press(Message::RefreshSeries(s.id)),
    );

    row = row.push(
        button(text("Remove").size(ACTION_SIZE))
            .style(theme::Button::Destructive)
            .on_press(Message::RemoveSeries(s.id)),
    );

    row
}

/// Render season banner.
pub(crate) fn season_info(
    service: &Service,
    series: &Series,
    season: &Season,
) -> Column<'static, Message> {
    let (watched, total) = service.season_watched(series.id, season.number);
    let mut actions = row![].spacing(SPACE);

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

    column![actions, info]
}

/// Prepare assets needed for banner.
pub(crate) fn prepare_series_banner(assets: &mut Assets, s: &Series) {
    assets.mark([s.banner.unwrap_or(s.poster)]);
}

/// Render a banner for the series.
pub(crate) fn series_banner(assets: &Assets, series: &Series) -> Column<'static, Message> {
    let handle = match assets.image(&series.banner.unwrap_or(series.poster)) {
        Some(handle) => handle,
        None => assets.missing_banner(),
    };

    let banner = image(handle);

    let title = button(text(&series.title).size(TITLE_SIZE))
        .padding(0)
        .style(theme::Button::Text)
        .on_press(Message::Navigate(Page::Series(series.id)));

    column![banner, title]
        .width(Length::Fill)
        .align_items(Alignment::Center)
}
