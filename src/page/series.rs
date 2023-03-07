use crate::prelude::*;

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
                    .flat_map(|season| season.poster().or(series.poster())),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::OpenRemote(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::SeriesActions(message) => {
                self.series.update(s, message);
            }
            Message::Navigate(page) => {
                s.push_history(page);
            }
            Message::SeasonInfo(index, message) => {
                if let Some(season_info) = self.seasons.get_mut(index) {
                    season_info.update(s, message);
                }
            }
            Message::SeriesBanner(message) => {
                self.banner.update(s, message);
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let Some(series) = s.service.series(&self.series_id) else {
            return w::Column::new().into();
        };

        let mut top = w::Column::new().push(self.banner.view(s, series).map(Message::SeriesBanner));

        let remote_ids = s.service.remotes_by_series(&series.id);

        if remote_ids.len() > 0 {
            let mut remotes = w::Row::new();

            for remote_id in remote_ids {
                remotes = remotes.push(
                    w::button(w::text(remote_id.to_string()))
                        .style(theme::Button::Text)
                        .on_press(Message::OpenRemote(remote_id)),
                );
            }

            top = top.push(remotes.spacing(SPACE));
        }

        let mut cols = w::Column::new();

        for (index, (season, c)) in s
            .service
            .seasons(&series.id)
            .iter()
            .zip(&self.seasons)
            .enumerate()
        {
            let poster = match season
                .graphics
                .poster
                .as_ref()
                .or(series.graphics.poster.as_ref())
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(poster) => poster,
                None => s.missing_poster(),
            };

            let graphic = w::button(w::image(poster).height(IMAGE_HEIGHT))
                .on_press(Message::Navigate(Page::Season(series.id, season.number)))
                .style(theme::Button::Text)
                .padding(0);

            let title = w::button(w::text(season.number).size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Season(series.id, season.number)));

            cols = cols.push(
                centered(
                    w::Row::new()
                        .push(graphic)
                        .push(
                            w::Column::new()
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
            0 => w::text("No episodes"),
            1 => w::text("One episode"),
            count => w::text(format!("{count} episodes")),
        };

        let mut header = w::Column::new()
            .push(top.align_items(Alignment::Center).spacing(GAP))
            .push(self.series.view(s, series).map(Message::SeriesActions))
            .push(info);

        if !series.overview.is_empty() {
            header = header.push(w::text(&series.overview));
        }

        let header = centered(header.spacing(GAP), None).padding(GAP);

        w::Column::new()
            .push(header)
            .push(cols.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP2)
            .into()
    }
}
