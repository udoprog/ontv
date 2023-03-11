use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    pub(crate) series_id: SeriesId,
    pub(crate) season: SeasonNumber,
}

pub(crate) fn page(series_id: SeriesId, season: SeasonNumber) -> Page {
    Page::Season(State { series_id, season })
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeasonId),
    Episode(usize, comps::episode::Message),
    SeasonInfo(comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

pub(crate) struct Season {
    episodes: Vec<comps::Episode>,
    season_info: comps::SeasonInfo,
    banner: comps::SeriesBanner,
}

impl Season {
    #[inline]
    pub(crate) fn new(state: &State) -> Self {
        Self {
            episodes: Vec::new(),
            season_info: comps::SeasonInfo::new((state.series_id, state.season)),
            banner: comps::SeriesBanner::default(),
        }
    }

    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, state: &State) {
        self.episodes.init_from_iter(
            cx.service
                .episodes(&state.series_id)
                .filter(|e| e.season == state.season)
                .map(|e| comps::episode::Props {
                    include_series: false,
                    episode_id: e.id,
                    watched: cx.service.watched(&e.id),
                }),
        );

        for e in &mut self.episodes {
            e.prepare(cx);
        }

        self.banner.prepare(cx, &state.series_id);
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::OpenRemote(remote) => {
                let url = remote.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::SeasonInfo(message) => {
                self.season_info.update(cx, message);
            }
            Message::SeriesBanner(message) => {
                self.banner.update(cx, message);
            }
            Message::Episode(index, m) => {
                if let Some(c) = self.episodes.get_mut(index) {
                    c.update(cx, m);
                }
            }
        }
    }

    /// Render season view.
    pub(crate) fn view(
        &self,
        cx: &mut CtxtRef<'_>,
        state: &State,
    ) -> Result<Element<'static, Message>> {
        let Some(series) = cx.service.series(&state.series_id) else {
            bail!("missing series {}", state.series_id);
        };

        let Some(season) = cx.service.season(&series.id, &state.season) else {
            bail!("missing series {} season {}", series.id, state.season);
        };

        let mut episodes = w::Column::new();

        let pending = cx.service.get_pending(&series.id).map(|p| &p.episode);

        for (index, episode) in self.episodes.iter().enumerate() {
            episodes = episodes.push(
                centered(
                    episode
                        .view(cx, pending == Some(episode.episode_id()))?
                        .map(move |m| Message::Episode(index, m)),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let season_title = w::text(season.number).size(SUBTITLE_SIZE);

        let mut banner = w::Column::new()
            .push(self.banner.view(cx, series).map(Message::SeriesBanner))
            .push(season_title)
            .align_items(Alignment::Center)
            .spacing(GAP);

        let mut remote_ids = cx
            .service
            .remotes_by_series(&series.id)
            .flat_map(|remote_id| remote_id.into_season(season.number))
            .peekable();

        if remote_ids.peek().is_some() {
            let mut remotes = w::Row::new();

            for remote_id in remote_ids {
                remotes = remotes.push(
                    w::button(w::text(&remote_id).size(SMALL))
                        .style(theme::Button::Primary)
                        .on_press(Message::OpenRemote(remote_id)),
                );
            }

            banner = banner.push(remotes.spacing(GAP));
        }

        let top = w::Column::new()
            .push(banner)
            .push(self.season_info.view(cx).map(Message::SeasonInfo))
            .spacing(GAP)
            .width(Length::Fill);

        let header = centered(top, None).padding(GAP);

        Ok(w::Column::new()
            .push(header)
            .push(episodes.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into())
    }
}
