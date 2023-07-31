use std::fmt;

use serde::{Deserialize, Serialize};

use crate::prelude::*;
use crate::queue::{TaskKind, TaskRef, TaskStatus};

/// Number of results per page.
const PER_PAGE: usize = 5;

/// Message generated by dashboard page.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    Error(ErrorInfo),
    Navigate(Page),
    Search,
    Change(String),
    SeriesPage(usize),
    MoviesPage(usize),
    Result(Vec<SearchSeries>, Vec<SearchMovie>),
    SearchKindChanged(SearchKind),
    AddSeriesByRemote(RemoteSeriesId),
    SwitchSeries(SeriesId, RemoteSeriesId),
    RemoveSeries(SeriesId),
    AddMovieByRemote(RemoteMovieId),
    SwitchMovie(MovieId, RemoteMovieId),
    RemoveMovie(MovieId),
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SearchKind {
    Tvdb,
    #[default]
    Tmdb,
}

impl fmt::Display for SearchKind {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchKind::Tvdb => write!(f, "thetvdb.com"),
            SearchKind::Tmdb => write!(f, "themoviedb.com"),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct State {
    text: String,
    series_page: usize,
    movies_page: usize,
    // Unique identifier of last search so that we can look up any recorded errors.
    search_id: Uuid,
    // Current search kind.
    kind: SearchKind,
}

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Search {
    series: Vec<SearchSeries>,
    movies: Vec<SearchMovie>,
    initialized: bool,
}

impl Search {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(
        &mut self,
        cx: &mut Ctxt<'_>,
        state: &mut State,
        commands: impl Commands<Message>,
    ) {
        cx.assets.mark_with_hint(
            self.series
                .iter()
                .skip(state.series_page * PER_PAGE)
                .take(PER_PAGE)
                .flat_map(|s| s.poster()),
            POSTER_HINT,
        );

        cx.assets.mark_with_hint(
            self.movies
                .iter()
                .skip(state.movies_page * PER_PAGE)
                .take(PER_PAGE)
                .flat_map(|s| s.poster()),
            POSTER_HINT,
        );

        if !self.initialized {
            self.initialized = true;
            self.search(cx, state, commands);
        }
    }

    /// Handle theme change.
    pub(crate) fn update(
        &mut self,
        cx: &mut Ctxt<'_>,
        state: &mut State,
        message: Message,
        commands: impl Commands<Message>,
    ) {
        match message {
            Message::Error(error) => {
                cx.state.handle_error(error);
            }
            Message::Navigate(page) => {
                cx.push_history(page);
            }
            Message::Search => {
                self.search(cx, state, commands);
            }
            Message::Change(text) => {
                state.text = text;
            }
            Message::SeriesPage(page) => {
                state.series_page = page;
                cx.assets.clear();
            }
            Message::MoviesPage(page) => {
                state.movies_page = page;
                cx.assets.clear();
            }
            Message::Result(series, movies) => {
                self.series = series;
                self.movies = movies;
                cx.assets.clear();
            }
            Message::SearchKindChanged(kind) => {
                state.kind = kind;
                self.search(cx, state, commands);
            }
            Message::AddSeriesByRemote(remote_id) => {
                cx.service
                    .push_task_without_delay(TaskKind::DownloadSeriesByRemoteId { remote_id });
            }
            Message::SwitchSeries(series_id, remote_id) => {
                cx.remove_series(&series_id);
                cx.service
                    .push_task_without_delay(TaskKind::DownloadSeriesByRemoteId { remote_id });
            }
            Message::RemoveSeries(series_id) => {
                cx.remove_series(&series_id);
            }
            Message::AddMovieByRemote(_) => {}
            Message::SwitchMovie(_, _) => {}
            Message::RemoveMovie(_) => {}
        }
    }

    fn search(
        &mut self,
        cx: &mut Ctxt<'_>,
        state: &mut State,
        mut commands: impl Commands<Message>,
    ) {
        if state.text.is_empty() {
            return;
        }

        state.series_page = 0;
        state.movies_page = 0;

        let search_id = Uuid::new_v4();
        let query = state.text.clone();
        state.search_id = search_id;
        let kind = state.kind;

        match kind {
            SearchKind::Tvdb => {
                let op = cx.service.search_tvdb(&state.text);

                let translate = move |out: Result<_>| match out
                    .with_context(|| anyhow!("Searching {kind} for `{query}`"))
                {
                    Ok(series) => Message::Result(series, Vec::new()),
                    Err(error) => Message::Error(ErrorInfo::new(ErrorId::Search(search_id), error)),
                };

                commands.perform(op, translate);
            }
            SearchKind::Tmdb => {
                let series = cx.service.search_series_tmdb(&state.text);
                let movies = cx.service.search_movies_tmdb(&state.text);

                let op = async move {
                    match tokio::try_join!(series, movies) {
                        Ok((series, movies)) => Message::Result(series, movies),
                        Err(error) => {
                            Message::Error(ErrorInfo::new(ErrorId::Search(search_id), error))
                        }
                    }
                };

                commands.perform(op, |out| out);
            }
        }
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, cx: &CtxtRef<'_>, state: &State) -> Element<'static, Message> {
        let mut series = w::Column::new();

        for s in self
            .series
            .iter()
            .skip(state.series_page * PER_PAGE)
            .take(PER_PAGE)
        {
            let local_series = cx.service.get_series_by_remote(&s.id);

            let handle = match s
                .poster()
                .and_then(|p| cx.assets.image_with_hint(p, POSTER_HINT))
            {
                Some(handle) => handle,
                None => cx.missing_poster(),
            };

            let mut actions = w::Row::new();

            let status = cx
                .service
                .task_status(TaskRef::RemoteSeries { remote_id: s.id });

            match status {
                Some(TaskStatus::Pending) => {
                    actions = actions.push(
                        w::button(w::text("Queued...").size(SMALL_SIZE))
                            .style(theme::Button::Primary),
                    );
                }
                Some(TaskStatus::Running) => {
                    actions = actions.push(
                        w::button(w::text("Downloading...").size(SMALL_SIZE))
                            .style(theme::Button::Primary),
                    );
                }
                None => {
                    if let Some(local) = local_series {
                        if local.remote_id != Some(s.id) {
                            actions = actions.push(
                                w::button(w::text("Switch").size(SMALL_SIZE))
                                    .style(theme::Button::Primary)
                                    .on_press(Message::SwitchSeries(local.id, s.id)),
                            );
                        }

                        actions = actions.push(
                            w::button(w::text("Remove").size(SMALL_SIZE))
                                .style(theme::Button::Destructive)
                                .on_press(Message::RemoveSeries(local.id)),
                        );
                    } else {
                        actions = actions.push(
                            w::button(w::text("Add").size(SMALL_SIZE))
                                .style(theme::Button::Positive)
                                .on_press(Message::AddSeriesByRemote(s.id)),
                        );
                    }
                }
            }

            let mut first_aired = w::Column::new();

            if let Some(date) = s.first_aired {
                first_aired =
                    first_aired.push(w::text(format!("First aired: {date}")).size(SMALL_SIZE));
            }

            let mut result = w::Column::new();

            let series_name = w::text(&s.name)
                .shaping(w::text::Shaping::Advanced)
                .size(SUBTITLE_SIZE);

            if let Some(local_series) = local_series {
                result = result.push(
                    link(series_name)
                        .on_press(Message::Navigate(page::series::page(local_series.id))),
                );
            } else {
                result = result.push(series_name);
            }

            result = result.push(first_aired);
            result = result.push(actions.spacing(SPACE));

            series = series.push(
                w::Row::new()
                    .push(w::image(handle).height(IMAGE_HEIGHT))
                    .push(
                        w::Column::new()
                            .push(result.spacing(SPACE))
                            .push(w::text(&s.overview).shaping(w::text::Shaping::Advanced))
                            .spacing(GAP),
                    )
                    .spacing(GAP),
            );
        }

        series = series.push(paginate(
            state.series_page,
            self.series.len(),
            Message::SeriesPage,
        ));

        let mut movies = w::Column::new();

        for m in self
            .movies
            .iter()
            .skip(state.movies_page * PER_PAGE)
            .take(PER_PAGE)
        {
            let local_movie = cx.service.get_movie_by_remote(&m.id);

            let handle = match m
                .poster()
                .and_then(|p| cx.assets.image_with_hint(p, POSTER_HINT))
            {
                Some(handle) => handle,
                None => cx.missing_poster(),
            };

            let mut actions = w::Row::new();

            let status = cx
                .service
                .task_status(TaskRef::RemoteMovie { remote_id: m.id });

            match status {
                Some(TaskStatus::Pending) => {
                    actions = actions.push(
                        w::button(w::text("Queued...").size(SMALL_SIZE))
                            .style(theme::Button::Primary),
                    );
                }
                Some(TaskStatus::Running) => {
                    actions = actions.push(
                        w::button(w::text("Downloading...").size(SMALL_SIZE))
                            .style(theme::Button::Primary),
                    );
                }
                None => {
                    if let Some(local) = local_movie {
                        if local.remote_id != Some(m.id) {
                            actions = actions.push(
                                w::button(w::text("Switch").size(SMALL_SIZE))
                                    .style(theme::Button::Primary)
                                    .on_press(Message::SwitchMovie(local.id, m.id)),
                            );
                        }

                        actions = actions.push(
                            w::button(w::text("Remove").size(SMALL_SIZE))
                                .style(theme::Button::Destructive)
                                .on_press(Message::RemoveMovie(local.id)),
                        );
                    } else {
                        actions = actions.push(
                            w::button(w::text("Add").size(SMALL_SIZE))
                                .style(theme::Button::Positive)
                                .on_press(Message::AddMovieByRemote(m.id)),
                        );
                    }
                }
            }

            let mut release_date = w::Column::new();

            if let Some(date) = m.release_date {
                release_date =
                    release_date.push(w::text(format!("First aired: {date}")).size(SMALL_SIZE));
            }

            let mut result = w::Column::new();

            let movie_title = w::text(&m.title)
                .shaping(w::text::Shaping::Advanced)
                .size(SUBTITLE_SIZE);

            if let Some(local_movie) = local_movie {
                result = result.push(
                    link(movie_title)
                        .on_press(Message::Navigate(page::movie::page(local_movie.id))),
                );
            } else {
                result = result.push(movie_title);
            }

            result = result.push(release_date);
            result = result.push(actions.spacing(SPACE));

            movies = movies.push(
                w::Row::new()
                    .push(w::image(handle).height(IMAGE_HEIGHT))
                    .push(
                        w::Column::new()
                            .push(result.spacing(SPACE))
                            .push(w::text(&m.overview).shaping(w::text::Shaping::Advanced))
                            .spacing(GAP),
                    )
                    .spacing(GAP),
            );
        }

        movies = movies.push(paginate(
            state.movies_page,
            self.movies.len(),
            Message::MoviesPage,
        ));

        let query = w::text_input("Query...", &state.text)
            .on_input(Message::Change)
            .on_submit(Message::Search);

        let submit = w::button("Search");

        let submit = if !state.text.is_empty() {
            submit.on_press(Message::Search)
        } else {
            submit
        };

        let mut search_kind = w::Column::new().push(w::text("Source:").size(SMALL_SIZE));

        search_kind =
            [SearchKind::Tvdb, SearchKind::Tmdb]
                .iter()
                .fold(search_kind, |column, kind| {
                    column.push(
                        w::radio(
                            kind.to_string(),
                            *kind,
                            Some(state.kind),
                            Message::SearchKindChanged,
                        )
                        .size(SMALL_SIZE),
                    )
                });

        let mut page = w::Column::new();

        page = page.push(w::text("Search").size(TITLE_SIZE));
        page = page.push(w::Row::new().push(query).push(submit));

        if let Some(e) = cx.state.get_error(ErrorId::Search(state.search_id)) {
            page = page.push(
                w::button(w::text(format!("Error: {}", e.message)))
                    .width(Length::Fill)
                    .style(theme::Button::Destructive)
                    .on_press(Message::Navigate(Page::Errors)),
            );
        }

        page = page.push(search_kind.spacing(SPACE));

        let mut row = w::Row::new();
        row = row.push(series.spacing(GAP2).width(Length::FillPortion(1)));
        row = row.push(movies.spacing(GAP2).width(Length::FillPortion(1)));
        page = page.push(row.spacing(GAP2));
        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}

fn paginate<M>(page: usize, len: usize, m: M) -> w::Row<'static, Message>
where
    M: FnOnce(usize) -> Message + Copy,
{
    let mut row = w::Row::new();

    if len > PER_PAGE {
        let mut prev = w::button("previous page").style(theme::Button::Positive);
        let mut next = w::button("next page").style(theme::Button::Positive);

        if let Some(page) = page.checked_sub(1) {
            prev = prev.on_press(m(page));
        }

        if (page + 1) * PER_PAGE < len {
            next = next.on_press(m(page + 1));
        }

        let text = w::text(format!(
            "{}-{} ({})",
            page * PER_PAGE,
            ((page + 1) * PER_PAGE).min(len),
            len,
        ));

        row = w::Row::new()
            .push(prev)
            .push(next)
            .push(text)
            .align_items(Alignment::Center)
            .spacing(GAP);
    }

    row
}
