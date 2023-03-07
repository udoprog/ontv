use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Episode(usize, comps::episode::Message),
}

pub(crate) struct WatchNext {
    episodes: Vec<comps::Episode>,
}

impl WatchNext {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            episodes: Vec::new(),
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        self.episodes
            .init_from_iter(s.service.pending(s.service.now()).rev().map(|p| {
                comps::episode::Props {
                    include_series: true,
                    episode_id: p.episode.id,
                    watched: s.service.watched(&p.episode.id),
                }
            }));

        for e in &mut self.episodes {
            e.prepare(s);
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
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

        w::Column::new()
            .push(list.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP2)
            .into()
    }
}
