use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Future(usize, comps::episode::Message),
    Episode(usize, comps::episode::Message),
}

pub(crate) struct WatchNext {
    future: Vec<comps::Episode>,
    episodes: Vec<comps::Episode>,
}

impl WatchNext {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            future: Vec::new(),
            episodes: Vec::new(),
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        let today = s.today();

        let future = s.service.pending().rev().filter(|p| p.will_air(&today));

        self.future
            .init_from_iter(future.map(|p| comps::episode::Props {
                include_series: true,
                episode_id: p.episode.id,
                watched: s.service.watched(&p.episode.id),
            }));

        let episodes = s.service.pending().rev().filter(|p| p.has_aired(&today));

        self.episodes
            .init_from_iter(episodes.map(|p| comps::episode::Props {
                include_series: true,
                episode_id: p.episode.id,
                watched: s.service.watched(&p.episode.id),
            }));

        for e in self.future.iter_mut().chain(&mut self.episodes) {
            e.prepare(s);
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::Future(index, m) => {
                if let Some(c) = self.future.get_mut(index) {
                    c.update(s, m);
                }
            }
            Message::Episode(index, m) => {
                if let Some(c) = self.episodes.get_mut(index) {
                    c.update(s, m);
                }
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut list = w::Column::new();

        list = list.push(w::vertical_space(Length::Shrink));

        list = list.push(centered(
            w::text("Watch next").size(TITLE_SIZE).width(Length::Fill),
            None,
        ));

        if !self.future.is_empty() {
            list = list.push(centered(
                w::text("Future episodes:")
                    .size(SUBTITLE_SIZE)
                    .width(Length::Fill),
                None,
            ));

            for (index, episode) in self.future.iter().enumerate() {
                list = list.push(
                    centered(
                        episode
                            .view(s, true)
                            .map(move |m| Message::Future(index, m)),
                        Some(style::weak),
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
                None,
            ));

            for (index, episode) in self.episodes.iter().enumerate() {
                list = list.push(
                    centered(
                        episode
                            .view(s, true)
                            .map(move |m| Message::Episode(index, m)),
                        Some(style::weak),
                    )
                    .padding(GAP),
                );
            }
        }

        w::Column::new()
            .push(list.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP2)
            .into()
    }
}
