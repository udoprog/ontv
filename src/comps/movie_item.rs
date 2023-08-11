use crate::component::{Component, ComponentInitExt};
use crate::comps;
use crate::model::{MovieId, Watched};
use crate::params::{GAP, SCREENCAP_HINT, SMALL_SIZE, SPACE};
use crate::prelude::*;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RemoveLastWatch(comps::confirm::Message),
    RemoveWatch(usize, comps::confirm::Message),
    Watch(comps::watch::Message),
    SelectPending(MovieId),
    ClearPending(MovieId),
}

#[derive(PartialEq, Eq)]
pub(crate) struct Props<I> {
    pub(crate) movie_id: MovieId,
    pub(crate) watched: I,
}

pub(crate) struct MovieItem {
    movie_id: MovieId,
    watch: comps::Watch,
    remove_last_watch: Option<comps::Confirm>,
    remove_watches: Vec<comps::Confirm>,
}

impl<'a, I> Component<Props<I>> for MovieItem
where
    I: DoubleEndedIterator<Item = &'a Watched> + Clone,
{
    #[inline]
    fn new(props: Props<I>) -> Self {
        Self {
            movie_id: props.movie_id,
            watch: comps::Watch::new(comps::watch::Props::new(comps::watch::Kind::Movie(
                props.movie_id,
            ))),
            remove_last_watch: props.watched.clone().next_back().map(move |w| {
                comps::Confirm::new(comps::confirm::Props::new(
                    comps::confirm::Kind::RemoveMovieWatch {
                        movie_id: props.movie_id,
                        watch_id: w.id,
                    },
                ))
            }),
            remove_watches: props
                .watched
                .map(move |w| {
                    comps::Confirm::new(
                        comps::confirm::Props::new(comps::confirm::Kind::RemoveMovieWatch {
                            movie_id: props.movie_id,
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
        self.movie_id = props.movie_id;
        self.watch
            .changed(comps::watch::Props::new(comps::watch::Kind::Movie(
                props.movie_id,
            )));
        self.remove_last_watch
            .init_from_iter(props.watched.clone().next_back().map(move |w| {
                comps::confirm::Props::new(comps::confirm::Kind::RemoveMovieWatch {
                    movie_id: props.movie_id,
                    watch_id: w.id,
                })
            }));
        self.remove_watches.init_from_iter(props.watched.map(|w| {
            comps::confirm::Props::new(comps::confirm::Kind::RemoveMovieWatch {
                movie_id: props.movie_id,
                watch_id: w.id,
            })
            .with_ordering(comps::ordering::Ordering::Left)
        }));
    }
}

impl MovieItem {
    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>) {
        if let Some(e) = cx.service.movie(&self.movie_id) {
            cx.assets.mark_with_hint(e.screen_capture(), SCREENCAP_HINT);
        }
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, m: Message) {
        match m {
            Message::RemoveLastWatch(message) => {
                if let Some(c) = &mut self.remove_last_watch {
                    c.update(cx, message);
                }
            }
            Message::RemoveWatch(index, message) => {
                if let Some(c) = self.remove_watches.get_mut(index) {
                    c.update(cx, message);
                }
            }
            Message::Watch(message) => {
                self.watch.update(cx, message);
            }
            Message::SelectPending(movie) => {
                let now = Utc::now();
                cx.service.select_pending_movie(&now, &movie);
            }
            Message::ClearPending(movie) => {
                cx.service.clear_pending_movie(&movie);
            }
        }
    }

    pub(crate) fn view(
        &self,
        cx: &CtxtRef<'_>,
        pending: bool,
    ) -> Result<Element<'static, Message>> {
        let Some(movie) = cx.service.movie(&self.movie_id) else {
            bail!("Missing movie {}", self.movie_id);
        };

        let screen_capture = match movie
            .screen_capture()
            .and_then(|image| cx.assets.image_with_hint(image, SCREENCAP_HINT))
        {
            Some(handle) => handle,
            None => cx.assets.missing_screen_capture(),
        };

        let (image, (image_fill, rest_fill)) = (
            w::container(w::image(screen_capture)).align_x(Horizontal::Center),
            (4, 8),
        );

        let name = w::text(&movie.title).shaping(w::text::Shaping::Advanced);

        let watched = cx.service.watched_by_movie(&movie.id);

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
                    w::button(w::text("Make next movie").size(SMALL_SIZE))
                        .style(theme::Button::Secondary)
                        .on_press(Message::SelectPending(movie.id)),
                );
            } else {
                actions = actions.push(
                    w::button(w::text("Clear next movie").size(SMALL_SIZE))
                        .style(theme::Button::Destructive)
                        .on_press(Message::ClearPending(movie.id)),
                );
            }
        }

        let mut info = w::Column::new();

        info = info.push(name);
        info = info.push(actions);

        if let Some(air_date) = &movie.release_date {
            if air_date > cx.state.today() {
                info = info.push(w::text(format_args!("Releases: {air_date}")).size(SMALL_SIZE));
            } else {
                info = info.push(w::text(format_args!("Released: {air_date}")).size(SMALL_SIZE));
            }
        }

        {
            let mut it = watched.clone();
            let len = it.len();

            let text = match (len, it.next(), it.next_back()) {
                (1, Some(once), _) => w::text(format_args!(
                    "Watched once on {}",
                    once.timestamp.date_naive()
                )),
                (len, _, Some(last)) if len > 0 => w::text(format_args!(
                    "Watched {} times, last on {}",
                    len,
                    last.timestamp.date_naive()
                )),
                _ => w::text("Never watched").style(cx.warning_text()),
            };

            info = info.push(text.size(SMALL_SIZE));
        };

        info = info.push(w::text(&movie.overview).shaping(w::text::Shaping::Advanced));

        if watched.len() > 0 {
            let mut history = w::Column::new();

            history = history.push(w::text("Watch history"));

            for ((n, watch), c) in watched.enumerate().zip(&self.remove_watches) {
                let mut row = w::Row::new();

                row = row.push(
                    w::text(format!("#{}", n + 1))
                        .size(SMALL_SIZE)
                        .width(24.0)
                        .horizontal_alignment(Horizontal::Left),
                );

                row = row.push(
                    w::text(watch.timestamp.date_naive())
                        .size(SMALL_SIZE)
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

        Ok(w::Row::new()
            .push(image.width(Length::FillPortion(image_fill)))
            .push(info.width(Length::FillPortion(rest_fill)).spacing(SPACE))
            .spacing(GAP)
            .into())
    }
}
