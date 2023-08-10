use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::queue::{TaskKind, TaskRef};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    pub(crate) id: MovieId,
}

pub(crate) fn page(id: MovieId) -> Page {
    Page::Movie(State { id })
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    OpenRemote(RemoteId),
    MovieActions(comps::movie_actions::Message),
    MovieBanner(comps::movie_banner::Message),
    SwitchMovie(MovieId, RemoteId),
    RemoveWatch(usize, comps::confirm::Message),
}

pub(crate) struct Movie {
    movie: comps::MovieActions,
    banner: comps::MovieBanner,
    remove_watches: Vec<comps::Confirm>,
}

impl Movie {
    #[inline]
    pub(crate) fn new(state: &State) -> Self {
        Self {
            movie: comps::MovieActions::new(state.id),
            banner: comps::MovieBanner,
            remove_watches: Vec::new(),
        }
    }

    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, state: &State) {
        self.banner.prepare(cx, &state.id);

        self.remove_watches
            .init_from_iter(cx.service.watched_by_movie(&state.id).map(|w| {
                comps::confirm::Props::new(comps::confirm::Kind::RemoveMovieWatch {
                    movie_id: state.id,
                    watch_id: w.id,
                })
                .with_ordering(comps::ordering::Ordering::Left)
            }));
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::OpenRemote(remote_id) => {
                let url = remote_id.url();
                let _ = webbrowser::open_browser(webbrowser::Browser::Default, &url);
            }
            Message::MovieActions(message) => {
                self.movie.update(cx, message);
            }
            Message::MovieBanner(message) => {
                self.banner.update(cx, message);
            }
            Message::SwitchMovie(movie_id, remote_id) => {
                cx.service.push_task_without_delay(TaskKind::DownloadMovie {
                    movie_id,
                    remote_id,
                    last_modified: None,
                    force: true,
                });
            }
            Message::RemoveWatch(n, m) => {
                if let Some(remove_watch) = self.remove_watches.get_mut(n) {
                    remove_watch.update(cx, m);
                }
            }
        }
    }

    pub(crate) fn view(
        &self,
        cx: &CtxtRef<'_>,
        state: &State,
    ) -> Result<Element<'static, Message>> {
        let Some(movie) = cx.service.movie(&state.id) else {
            bail!("Missing movie {}", state.id);
        };

        let mut top = w::Column::new().push(self.banner.view(cx, movie).map(Message::MovieBanner));

        let remote_ids = cx.service.remotes_by_movie(&movie.id);

        if remote_ids.len() > 0 {
            let mut remotes = w::Row::new();

            for remote_id in remote_ids {
                let mut row = w::Row::new().push(
                    w::button(w::text(remote_id).size(SMALL_SIZE))
                        .style(theme::Button::Primary)
                        .on_press(Message::OpenRemote(remote_id)),
                );

                if movie.remote_id.as_ref() == Some(&remote_id) {
                    row = row.push(w::button(w::text("Current").size(SMALL_SIZE)));
                } else if remote_id.is_supported() {
                    let button = w::button(w::text("Switch").size(SMALL_SIZE))
                        .style(theme::Button::Positive);

                    let status = cx.service.task_status_any([
                        TaskRef::RemoteMovie { remote_id },
                        TaskRef::Movie { movie_id: movie.id },
                    ]);

                    let button = if status.is_none() {
                        button.on_press(Message::SwitchMovie(movie.id, remote_id))
                    } else {
                        button
                    };

                    row = row.push(button);
                }

                remotes = remotes.push(row);
            }

            top = top.push(remotes.spacing(GAP));
        }

        let mut info = w::Column::new()
            .push(top.align_items(Alignment::Center).spacing(GAP))
            .push(self.movie.view(cx, movie).map(Message::MovieActions));

        if !movie.overview.is_empty() {
            info = info.push(w::text(&movie.overview).shaping(w::text::Shaping::Advanced));
        }

        let watched = cx.service.watched_by_movie(&movie.id);

        {
            let mut it = watched.clone();
            let len = it.len();

            let watched_text = match (len, it.next(), it.next_back()) {
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

            info = info.push(watched_text.size(SMALL_SIZE));
        }

        if watched.len() > 0 {
            let mut history = w::Column::new();

            history = history.push(w::text("Watch history"));

            for (n, (watch, c)) in watched.zip(&self.remove_watches).enumerate() {
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

        let info = centered(info.spacing(GAP), None).padding(GAP);

        Ok(w::Column::new()
            .push(info)
            .width(Length::Fill)
            .spacing(GAP2)
            .into())
    }
}
