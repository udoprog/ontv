use crate::component::{Component, ComponentInitExt};
use crate::comps;
use crate::model::{EpisodeId, Watched};
use crate::params::{GAP, SCREENCAP_HEIGHT, SCREENCAP_HINT, SMALL, SPACE};
use crate::prelude::*;
use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RemoveLastWatch(comps::confirm::Message),
    RemoveWatch(usize, comps::confirm::Message),
    Watch(comps::watch::Message),
    SelectPending(EpisodeId),
    ClearPending(EpisodeId),
    Navigate(Page),
}

#[derive(PartialEq, Eq)]
pub(crate) struct Props<I> {
    pub(crate) include_series: bool,
    pub(crate) episode_id: EpisodeId,
    pub(crate) watched: I,
}

pub(crate) struct Episode {
    include_series: bool,
    episode_id: EpisodeId,
    watch: comps::Watch,
    remove_last_watch: Option<comps::Confirm>,
    remove_watches: Vec<comps::Confirm>,
}

impl Episode {
    pub(crate) fn episode_id(&self) -> &EpisodeId {
        &self.episode_id
    }
}

impl<'a, I> Component<Props<I>> for Episode
where
    I: DoubleEndedIterator<Item = &'a Watched> + Clone,
{
    #[inline]
    fn new(props: Props<I>) -> Self {
        Self {
            include_series: props.include_series,
            episode_id: props.episode_id,
            watch: comps::Watch::new(comps::watch::Props::new(props.episode_id)),
            remove_last_watch: props.watched.clone().next_back().map(move |w| {
                comps::Confirm::new(comps::confirm::Props::new(
                    comps::confirm::Kind::RemoveWatch {
                        episode_id: props.episode_id,
                        watch_id: w.id,
                    },
                ))
            }),
            remove_watches: props
                .watched
                .map(move |w| {
                    comps::Confirm::new(
                        comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                            episode_id: props.episode_id,
                            watch_id: w.id,
                        })
                        .with_ordering(comps::ordering::Ordering::Left),
                    )
                })
                .collect(),
        }
    }

    #[inline]
    fn changed(&mut self, props: Props<I>) {
        self.include_series = props.include_series;
        self.episode_id = props.episode_id;
        self.watch
            .changed(comps::watch::Props::new(props.episode_id));
        self.remove_last_watch
            .init_from_iter(props.watched.clone().next_back().map(move |w| {
                comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                    episode_id: props.episode_id,
                    watch_id: w.id,
                })
            }));
        self.remove_watches.init_from_iter(props.watched.map(|w| {
            comps::confirm::Props::new(comps::confirm::Kind::RemoveWatch {
                episode_id: props.episode_id,
                watch_id: w.id,
            })
            .with_ordering(comps::ordering::Ordering::Left)
        }));
    }
}

impl Episode {
    pub(crate) fn prepare(&mut self, s: &mut State) {
        if let Some(e) = s.service.episode(&self.episode_id) {
            if self.include_series {
                if let Some(series) = s.service.series(e.series()) {
                    s.assets.mark_with_hint(series.poster(), POSTER_HINT);
                }
            } else {
                s.assets.mark_with_hint(e.filename(), SCREENCAP_HINT);
            }
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, m: Message) {
        match m {
            Message::RemoveLastWatch(message) => {
                if let Some(c) = &mut self.remove_last_watch {
                    c.update(s, message);
                }
            }
            Message::RemoveWatch(index, message) => {
                if let Some(c) = self.remove_watches.get_mut(index) {
                    c.update(s, message);
                }
            }
            Message::Watch(message) => {
                self.watch.update(s, message);
            }
            Message::SelectPending(episode) => {
                let now = Utc::now();
                s.service.select_pending(&now, &episode);
            }
            Message::ClearPending(episode) => {
                s.service.clear_pending(&episode);
            }
            Message::Navigate(page) => {
                s.push_history(page);
            }
        }
    }

    pub(crate) fn view(&self, s: &State, pending: bool) -> Element<'static, Message> {
        let Some(e) = s.service.episode(&self.episode_id) else {
            return w::Column::new().into();
        };

        let series = if self.include_series {
            s.service.series(e.series())
        } else {
            None
        };

        let image = if let Some(series) = series {
            let poster = match series
                .poster()
                .and_then(|image| s.assets.image_with_hint(&image, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.missing_poster(),
            };

            w::container(w::image(poster))
                .max_height(SCREENCAP_HEIGHT)
                .align_x(Horizontal::Center)
        } else {
            let screencap = match e
                .filename()
                .and_then(|image| s.assets.image_with_hint(&image, SCREENCAP_HINT))
            {
                Some(handle) => handle,
                None => s.assets.missing_screencap(),
            };

            w::container(w::image(screencap))
                .max_height(SCREENCAP_HEIGHT)
                .align_x(Horizontal::Center)
        };

        let mut name = w::Row::new().spacing(SPACE);

        name = name.push(w::text(e.number));

        if let Some(string) = &e.name {
            name = name.push(w::text(string));
        }

        let watched = s.service.watched(&e.id);

        let mut actions = w::Row::new().spacing(SPACE);

        let any_confirm = self.watch.is_confirm()
            || self
                .remove_last_watch
                .as_ref()
                .map(comps::Confirm::is_confirm)
                .unwrap_or_default();

        let watch_text = match watched.len() {
            0 => "First watch",
            _ => "Watch again",
        };

        if !any_confirm || self.watch.is_confirm() {
            actions = actions.push(
                self.watch
                    .view(
                        watch_text,
                        theme::Button::Positive,
                        theme::Button::Positive,
                        Length::Shrink,
                        Horizontal::Center,
                        true,
                    )
                    .map(Message::Watch),
            );
        }

        if let Some(remove_last_watch) = &self.remove_last_watch {
            if !any_confirm || remove_last_watch.is_confirm() {
                let watch_text = match watched.len() {
                    1 => "Remove watch",
                    _ => "Remove last watch",
                };

                actions = actions.push(
                    remove_last_watch
                        .view(watch_text, theme::Button::Destructive)
                        .map(Message::RemoveLastWatch),
                );
            }
        }

        if !any_confirm {
            if !pending {
                actions = actions.push(
                    w::button(w::text("Make next episode").size(SMALL))
                        .style(theme::Button::Secondary)
                        .on_press(Message::SelectPending(e.id)),
                );
            } else {
                actions = actions.push(
                    w::button(w::text("Clear next episode").size(SMALL))
                        .style(theme::Button::Destructive)
                        .on_press(Message::ClearPending(e.id)),
                );
            }
        }

        let mut show_info = w::Column::new();

        if let Some(air_date) = &e.aired {
            if air_date > s.today() {
                show_info = show_info.push(w::text(format_args!("Airs: {air_date}")).size(SMALL));
            } else {
                show_info = show_info.push(w::text(format_args!("Aired: {air_date}")).size(SMALL));
            }
        }

        let watched_text = {
            let mut it = watched.clone();
            let len = it.len();

            match (len, it.next(), it.next_back()) {
                (1, Some(once), _) => w::text(format_args!(
                    "Watched once on {}",
                    once.timestamp.date_naive()
                )),
                (len, _, Some(last)) if len > 0 => w::text(format_args!(
                    "Watched {} times, last on {}",
                    len,
                    last.timestamp.date_naive()
                )),
                _ => w::text("Never watched").style(s.warning_text()),
            }
        };

        show_info = show_info.push(watched_text.size(SMALL));

        let mut info_top = w::Column::new();

        if let Some(series) = series {
            info_top = info_top.push(
                w::button(w::text(&series.title).size(SUBTITLE_SIZE))
                    .padding(0)
                    .style(theme::Button::Text)
                    .on_press(Message::Navigate(Page::Series(series.id))),
            );
        }

        info_top = info_top
            .push(name)
            .push(actions)
            .push(show_info.spacing(SPACE))
            .spacing(SPACE);

        let mut info = w::Column::new().push(info_top).push(w::text(&e.overview));

        if watched.len() > 0 {
            let mut history = w::Column::new();

            history = history.push(w::text("Watch history"));

            for ((n, watch), c) in watched.enumerate().zip(&self.remove_watches) {
                let mut row = w::Row::new();

                row = row.push(
                    w::text(format!("#{}", n + 1))
                        .size(SMALL)
                        .width(24.0)
                        .horizontal_alignment(Horizontal::Left),
                );

                row = row.push(
                    w::text(watch.timestamp.date_naive())
                        .size(SMALL)
                        .width(Length::Fill),
                );

                row = row.push(
                    c.view("Remove", theme::Button::Destructive)
                        .map(move |m| Message::RemoveWatch(n, m)),
                );

                history = history.push(row.width(Length::Fill).spacing(SPACE));
            }

            info = info.push(history.width(Length::Fill).spacing(SPACE));
        }

        w::Row::new()
            .push(image)
            .push(info.width(Length::Fill).spacing(GAP))
            .spacing(GAP)
            .into()
    }
}
