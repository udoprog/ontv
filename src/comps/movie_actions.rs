use crate::prelude::*;
use crate::queue::{TaskKind, TaskRef, TaskStatus};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    RefreshMovie(RemoteId),
    RemoveMovie,
}

#[derive(Debug, Clone)]
pub(crate) struct MovieActions {
    movie_id: MovieId,
    confirm: bool,
}

impl Component<MovieId> for MovieActions {
    #[inline]
    fn new(movie_id: MovieId) -> Self {
        Self {
            movie_id,
            confirm: false,
        }
    }

    #[inline]
    fn changed(&mut self, movie_id: MovieId) {
        if self.movie_id != movie_id {
            self.movie_id = movie_id;
            self.confirm = false;
        }
    }
}

impl MovieActions {
    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::RefreshMovie(remote_id) => {
                cx.service.push_task_without_delay(TaskKind::DownloadMovie {
                    movie_id: self.movie_id,
                    remote_id,
                    last_modified: None,
                    force: true,
                });
            }
            Message::RemoveMovie => {
                cx.remove_movie(&self.movie_id);
            }
        }
    }

    pub(crate) fn view(&self, cx: &CtxtRef<'_>, movie: &Movie) -> Element<'static, Message> {
        let mut row = w::Row::new();

        let status = cx
            .service
            .task_status(TaskRef::Movie { movie_id: movie.id });

        match status {
            Some(TaskStatus::Pending) => {
                row = row.push(
                    w::button(w::text("Refresh").size(SMALL_SIZE)).style(theme::Button::Positive),
                );
            }
            Some(TaskStatus::Running) => {
                row = row.push(
                    w::button(w::text("Downloading...").size(SMALL_SIZE))
                        .style(theme::Button::Primary),
                );
            }
            None => {
                if let Some(remote_id) = movie.remote_id {
                    row = row.push(
                        w::button(w::text("Refresh").size(SMALL_SIZE))
                            .style(theme::Button::Positive)
                            .on_press(Message::RefreshMovie(remote_id)),
                    );
                }
            }
        }

        row = row.push(
            w::button(w::text("Remove").size(SMALL_SIZE))
                .style(theme::Button::Destructive)
                .on_press(Message::RemoveMovie),
        );

        row.spacing(SPACE).into()
    }
}
