use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    future: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Future(usize, comps::episode_or_movie::Message),
    Episode(usize, comps::episode_or_movie::Message),
    ToggleFuture(bool),
}

#[derive(Default)]
pub(crate) struct WatchNext {
    future: Vec<comps::EpisodeOrMovie>,
    episodes: Vec<comps::EpisodeOrMovie>,
}

impl WatchNext {
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, state: &State) {
        let today = cx.state.today();

        if state.future {
            let future = cx.service.pending().rev().filter(|p| p.will_air(today));

            self.future.init_from_iter(future.map(|p| match p {
                crate::service::PendingRef::Episode { episode, .. } => {
                    comps::episode_or_movie::Props::Episode(comps::episode::Props {
                        include_series: true,
                        episode_id: episode.id,
                        watched: cx.service.watched_by_episode(&episode.id),
                    })
                }
                crate::service::PendingRef::Movie { movie } => {
                    comps::episode_or_movie::Props::Movie(comps::movie_item::Props {
                        movie_id: movie.id,
                        watched: cx.service.watched_by_movie(&movie.id),
                    })
                }
            }));
        } else {
            self.future.clear();
        }

        let episodes = cx.service.pending().rev().filter(|p| p.has_aired(today));

        self.episodes.init_from_iter(episodes.map(|p| match p {
            crate::service::PendingRef::Episode { episode, .. } => {
                comps::episode_or_movie::Props::Episode(comps::episode::Props {
                    include_series: true,
                    episode_id: episode.id,
                    watched: cx.service.watched_by_episode(&episode.id),
                })
            }
            crate::service::PendingRef::Movie { movie } => {
                comps::episode_or_movie::Props::Movie(comps::movie_item::Props {
                    movie_id: movie.id,
                    watched: cx.service.watched_by_movie(&movie.id),
                })
            }
        }));

        for e in self.future.iter_mut().chain(&mut self.episodes) {
            e.prepare(cx);
        }
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, state: &mut State, message: Message) {
        match message {
            Message::Future(index, m) => {
                if let Some(c) = self.future.get_mut(index) {
                    c.update(cx, m);
                }
            }
            Message::Episode(index, m) => {
                if let Some(c) = self.episodes.get_mut(index) {
                    c.update(cx, m);
                }
            }
            Message::ToggleFuture(value) => {
                state.future = value;
            }
        }
    }

    pub(crate) fn view<'a>(&self, cx: &CtxtRef<'a>, state: &State) -> Result<Element<'a, Message>> {
        let mut list = w::Column::new();

        list = list.push(w::vertical_space().height(Length::Shrink));

        list = list.push(centered(
            w::text("Watch next").size(TITLE_SIZE).width(Length::Fill),
        ));

        let mut options = w::Row::new();

        options = options.push(centered(
            w::Row::new()
                .push(
                    w::checkbox("Show future episodes", state.future)
                        .on_toggle(Message::ToggleFuture),
                )
                .width(Length::Fill),
        ));

        list = list.push(options.width(Length::Fill));

        if !self.future.is_empty() {
            list = list.push(centered(
                w::text("Future episodes:")
                    .size(SUBTITLE_SIZE)
                    .width(Length::Fill),
            ));

            for (index, episode) in self.future.iter().enumerate() {
                list = list.push(
                    centered(
                        episode
                            .view(cx, true)?
                            .map(move |m| Message::Future(index, m)),
                    )
                    .padding(GAP),
                );
            }
        }

        if !self.episodes.is_empty() {
            list = list.push(centered(
                w::text("Available episodes:")
                    .size(SUBTITLE_SIZE)
                    .width(Length::Fill),
            ));

            for (index, episode) in self.episodes.iter().enumerate() {
                list = list.push(
                    centered(
                        episode
                            .view(cx, true)?
                            .map(move |m| Message::Episode(index, m)),
                    )
                    .padding(GAP),
                );
            }
        }

        Ok(w::Column::new()
            .push(list.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP2)
            .into())
    }
}
