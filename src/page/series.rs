use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    pub(crate) id: SeriesId,
}

pub(crate) fn page(id: SeriesId) -> Page {
    Page::Series(State { id })
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteSeriesId),
    SeriesActions(comps::series_actions::Message),
    Navigate(Page),
    SeasonInfo(usize, comps::season_info::Message),
    SeriesBanner(comps::series_banner::Message),
}

pub(crate) struct Series {
    series: comps::SeriesActions,
    seasons: Vec<comps::SeasonInfo>,
    banner: comps::SeriesBanner,
}

impl Series {
    #[inline]
    pub(crate) fn new(state: &State) -> Self {
        Self {
            series: comps::SeriesActions::new(state.id),
            seasons: Vec::new(),
            banner: comps::SeriesBanner::default(),
        }
    }

    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, state: &State) {
        self.seasons.init_from_iter(
            cx.service
                .seasons(&state.id)
                .map(|s| (*s.series(), s.number)),
        );

        self.banner.prepare(cx, &state.id);

        if let Some(series) = cx.service.series(&state.id) {
            cx.assets.mark_with_hint(
                cx.service
                    .seasons(&state.id)
                    .flat_map(|season| season.into_season().poster().or(series.poster())),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::OpenRemote(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::SeriesActions(message) => {
                self.series.update(cx, message);
            }
            Message::Navigate(page) => {
                cx.push_history(page);
            }
            Message::SeasonInfo(index, message) => {
                if let Some(season_info) = self.seasons.get_mut(index) {
                    season_info.update(cx, message);
                }
            }
            Message::SeriesBanner(message) => {
                self.banner.update(cx, message);
            }
        }
    }

    pub(crate) fn view(&self, cx: &CtxtRef<'_>, state: &State) -> Element<'static, Message> {
        let Some(series) = cx.service.series(&state.id) else {
            return w::Column::new().into();
        };

        let mut top =
            w::Column::new().push(self.banner.view(cx, series).map(Message::SeriesBanner));

        let remote_ids = cx.service.remotes_by_series(&series.id);

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

        for (index, (season, c)) in cx
            .service
            .seasons(&series.id)
            .zip(&self.seasons)
            .enumerate()
        {
            let poster = match season
                .graphics
                .poster
                .as_ref()
                .or(series.graphics.poster.as_ref())
                .and_then(|i| cx.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(poster) => poster,
                None => cx.missing_poster(),
            };

            let graphic = link(w::image(poster).height(IMAGE_HEIGHT)).on_press(Message::Navigate(
                page::season::page(series.id, season.number),
            ));

            let title = link(w::text(season.number).size(SUBTITLE_SIZE)).on_press(
                Message::Navigate(page::season::page(series.id, season.number)),
            );

            cols = cols.push(
                centered(
                    w::Row::new()
                        .push(graphic)
                        .push(
                            w::Column::new()
                                .push(title)
                                .push(c.view(cx).map(move |m| Message::SeasonInfo(index, m)))
                                .spacing(SPACE),
                        )
                        .spacing(GAP),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let info = match cx.service.episodes(&series.id).len() {
            0 => w::text("No episodes"),
            1 => w::text("One episode"),
            count => w::text(format!("{count} episodes")),
        };

        let mut header = w::Column::new()
            .push(top.align_items(Alignment::Center).spacing(GAP))
            .push(self.series.view(cx, series).map(Message::SeriesActions))
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
