use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeasonId),
    Episode(usize, comps::episode::Message),
    SeasonInfo(comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

pub(crate) struct Season {
    series_id: SeriesId,
    season: SeasonNumber,
    episodes: Vec<comps::Episode>,
    season_info: comps::SeasonInfo,
    banner: comps::SeriesBanner,
}

impl Season {
    #[inline]
    pub(crate) fn new(series_id: SeriesId, season: SeasonNumber) -> Self {
        Self {
            series_id,
            season,
            episodes: Vec::new(),
            season_info: comps::SeasonInfo::new((series_id, season)),
            banner: comps::SeriesBanner::default(),
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        self.episodes.init_from_iter(
            s.service
                .episodes(&self.series_id)
                .filter(|e| e.season == self.season)
                .map(|e| comps::episode::Props {
                    include_series: false,
                    episode_id: e.id,
                    watched: s.service.watched(&e.id),
                }),
        );

        for e in &mut self.episodes {
            e.prepare(s);
        }

        self.banner.prepare(s, &self.series_id);
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::OpenRemote(remote) => {
                let url = remote.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::SeasonInfo(message) => {
                self.season_info.update(s, message);
            }
            Message::SeriesBanner(message) => {
                self.banner.update(s, message);
            }
            Message::Episode(index, m) => {
                if let Some(c) = self.episodes.get_mut(index) {
                    c.update(s, m);
                }
            }
        }
    }

    /// Render season view.
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let Some(series) = s.service.series(&self.series_id) else {
            return w::Column::new().into();
        };

        let Some(season) = s.service.seasons(&series.id).iter().find(|s| s.number == self.season) else {
            return w::Column::new().into();
        };

        let mut episodes = w::Column::new();

        let pending = s.service.get_pending(&series.id).map(|p| &p.episode);

        for (index, episode) in self.episodes.iter().enumerate() {
            episodes = episodes.push(
                centered(
                    episode
                        .view(s, pending == Some(episode.episode_id()))
                        .map(move |m| Message::Episode(index, m)),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let season_title = w::text(season.number).size(SUBTITLE_SIZE);

        let mut banner = w::Column::new()
            .push(self.banner.view(s, series).map(Message::SeriesBanner))
            .push(season_title)
            .align_items(Alignment::Center)
            .spacing(GAP);

        let mut remote_ids = s
            .service
            .remotes_by_series(&series.id)
            .flat_map(|remote_id| remote_id.into_season(season.number))
            .peekable();

        if remote_ids.peek().is_some() {
            let mut remotes = w::Row::new();

            for remote_season in remote_ids {
                remotes = remotes.push(
                    w::button(w::text(&remote_season))
                        .style(theme::Button::Text)
                        .on_press(Message::OpenRemote(remote_season)),
                );
            }

            banner = banner.push(remotes.spacing(SPACE));
        }

        let top = w::Column::new()
            .push(banner)
            .push(self.season_info.view(s).map(Message::SeasonInfo))
            .spacing(GAP)
            .width(Length::Fill);

        let header = centered(top, None).padding(GAP);

        w::Column::new()
            .push(header)
            .push(episodes.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into()
    }
}
