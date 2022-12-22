use iced::widget::{button, column, image, row, text, Column, Row};
use iced::Length;
use iced::{theme, Command, Element};

use crate::comps;
use crate::message::Page;
use crate::model::{RemoteSeriesId, SeriesId};
use crate::params::{centered, style, GAP, GAP2, POSTER_HEIGHT, SPACE, SUBTITLE_SIZE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeriesId),
    SeriesActions(comps::series_actions::Message),
    Navigate(Page),
    SeasonInfo(usize, comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

#[derive(Default)]
pub(crate) struct Series {
    series: comps::SeriesActions,
    seasons: Vec<comps::SeasonInfo>,
    banner: comps::SeriesBanner,
}

impl Series {
    pub(crate) fn prepare(&mut self, s: &mut State, series_id: &SeriesId) {
        let len = s.service.seasons(series_id).len();

        if self.seasons.len() != len {
            self.seasons.resize(len, comps::SeasonInfo::default());
        }

        self.banner.prepare(s, series_id);

        if let Some(series) = s.service.series(series_id) {
            s.assets.mark(
                s.service
                    .seasons(&series.id)
                    .iter()
                    .flat_map(|season| season.poster.or(series.poster)),
            );
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::OpenRemote(remote_id) => {
                let url = match remote_id {
                    RemoteSeriesId::Tvdb { id } => {
                        format!("https://thetvdb.com/search?query={id}")
                    }
                    RemoteSeriesId::Tmdb { id } => {
                        format!("https://www.themoviedb.org/tv/{id}")
                    }
                    RemoteSeriesId::Imdb { id } => {
                        format!("https://www.imdb.com/title/{id}/")
                    }
                };

                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
                Command::none()
            }
            Message::SeriesActions(message) => {
                self.series.update(s, message).map(Message::SeriesActions)
            }
            Message::Navigate(page) => {
                s.push_history(page);
                Command::none()
            }
            Message::SeasonInfo(index, message) => {
                if let Some(season_info) = self.seasons.get_mut(index) {
                    season_info
                        .update(s, message)
                        .map(move |m| Message::SeasonInfo(index, m))
                } else {
                    Command::none()
                }
            }
            Message::SeriesBanner(message) => {
                self.banner.update(s, message).map(Message::SeriesBanner)
            }
        }
    }

    /// Render view of series.
    pub(crate) fn view(&self, s: &State, series_id: &SeriesId) -> Element<'static, Message> {
        let Some(series) = s.service.series(series_id) else {
            return column![text("no series")].into();
        };

        let mut top = Column::new().push(self.banner.view(s, series).map(Message::SeriesBanner));

        if !series.remote_ids.is_empty() {
            let mut remotes = row![];

            for remote_id in &series.remote_ids {
                remotes = remotes.push(
                    button(text(remote_id.to_string()))
                        .style(theme::Button::Text)
                        .on_press(Message::OpenRemote(*remote_id)),
                );
            }

            top = top.push(remotes.spacing(SPACE));
        }

        let mut cols = column![];

        for (index, (season, c)) in s
            .service
            .seasons(&series.id)
            .iter()
            .zip(&self.seasons)
            .enumerate()
        {
            let poster = match season
                .poster
                .or(series.poster)
                .and_then(|i| s.assets.image(&i))
            {
                Some(poster) => poster,
                None => s.assets.missing_poster(),
            };

            let graphic = button(image(poster).height(Length::Units(POSTER_HEIGHT)))
                .on_press(Message::Navigate(Page::Season(series.id, season.number)))
                .style(theme::Button::Text)
                .padding(0);

            let title = button(season.number.title().size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Season(series.id, season.number)));

            cols = cols.push(
                centered(
                    Row::new()
                        .push(graphic)
                        .push(
                            Column::new()
                                .push(title)
                                .push(
                                    c.view(s, series, season)
                                        .map(move |m| Message::SeasonInfo(index, m)),
                                )
                                .spacing(SPACE),
                        )
                        .spacing(GAP),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let info = match s.service.episodes(&series.id).len() {
            0 => text("No episodes"),
            1 => text("One episode"),
            count => text(format!("{count} episodes")),
        };

        let mut header = Column::new()
            .push(top.spacing(GAP))
            .push(self.series.view(s, series).map(Message::SeriesActions))
            .push(info);

        if let Some(overview) = &series.overview {
            header = header.push(text(overview));
        }

        let header = centered(header.spacing(GAP), None).padding(GAP);

        Column::new()
            .push(header)
            .push(cols.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP2)
            .into()
    }
}
