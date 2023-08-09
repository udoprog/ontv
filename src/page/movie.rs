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
}

pub(crate) struct Movie {
    movie: comps::MovieActions,
    banner: comps::MovieBanner,
}

impl Movie {
    #[inline]
    pub(crate) fn new(state: &State) -> Self {
        Self {
            movie: comps::MovieActions::new(state.id),
            banner: comps::MovieBanner,
        }
    }

    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>, state: &State) {
        self.banner.prepare(cx, &state.id);
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

        let mut header = w::Column::new()
            .push(top.align_items(Alignment::Center).spacing(GAP))
            .push(self.movie.view(cx, movie).map(Message::MovieActions));

        if !movie.overview.is_empty() {
            header = header.push(w::text(&movie.overview).shaping(w::text::Shaping::Advanced));
        }

        let header = centered(header.spacing(GAP), None).padding(GAP);

        Ok(w::Column::new()
            .push(header)
            .width(Length::Fill)
            .spacing(GAP2)
            .into())
    }
}
