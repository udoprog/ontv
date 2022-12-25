use iced::widget::{button, image, text, Column, Row};
use iced::{theme, Command, Element};
use iced::{Alignment, Length};

use crate::component::*;
use crate::comps;
use crate::message::Page;
use crate::model::{RemoteSeriesId, SeriesId};
use crate::params::{centered, GAP, GAP2, IMAGE_HEIGHT, POSTER_HINT, SPACE, SUBTITLE_SIZE};
use crate::state::State;
use crate::style;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeriesId),
    SeriesActions(comps::series_actions::Message),
    Navigate(Page),
    SeasonInfo(usize, comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

pub(crate) struct Series {
    series_id: SeriesId,
    series: comps::SeriesActions,
    seasons: Vec<comps::SeasonInfo>,
    banner: comps::SeriesBanner,
}

impl Series {
    #[inline]
    pub(crate) fn new(series_id: SeriesId) -> Self {
        Self {
            series_id,
            series: comps::SeriesActions::new(series_id),
            seasons: Vec::new(),
            banner: comps::SeriesBanner::default(),
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        self.seasons.init_from_iter(
            s.service
                .seasons(&self.series_id)
                .iter()
                .map(|s| (self.series_id, s.number)),
        );

        self.banner.prepare(s, &self.series_id);

        if let Some(series) = s.service.series(&self.series_id) {
            s.assets.mark_with_hint(
                s.service
                    .seasons(&self.series_id)
                    .iter()
                    .flat_map(|season| season.poster.or(series.poster)),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::OpenRemote(remote_id) => {
                let url = remote_id.url();
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
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let Some(series) = s.service.series(&self.series_id) else {
            return Column::new().into();
        };

        let mut top = Column::new().push(self.banner.view(s, series).map(Message::SeriesBanner));

        if !series.remote_ids.is_empty() {
            let mut remotes = Row::new();

            for remote_id in &series.remote_ids {
                remotes = remotes.push(
                    button(text(remote_id.to_string()))
                        .style(theme::Button::Text)
                        .on_press(Message::OpenRemote(*remote_id)),
                );
            }

            top = top.push(remotes.spacing(SPACE));
        }

        let mut cols = Column::new();

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
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(poster) => poster,
                None => s.missing_poster(),
            };

            let graphic = button(image(poster).height(Length::Units(IMAGE_HEIGHT)))
                .on_press(Message::Navigate(Page::Season(series.id, season.number)))
                .style(theme::Button::Text)
                .padding(0);

            let title = button(text(season.number).size(SUBTITLE_SIZE))
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
                                .push(c.view(s).map(move |m| Message::SeasonInfo(index, m)))
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
            .push(top.align_items(Alignment::Center).spacing(GAP))
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
