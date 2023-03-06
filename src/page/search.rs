use anyhow::{anyhow, Context, Result};
use iced::widget::{button, image, radio, text, text_input, Column, Row};
use iced::{theme, Alignment, Element, Length};
use uuid::Uuid;

use crate::commands::Commands;
use crate::error::{ErrorId, ErrorInfo};
use crate::model::{
    MovieId, RemoteMovieId, RemoteSeriesId, SearchKind, SearchMovie, SearchSeries, SeriesId,
    TaskId, TaskKind,
};
use crate::params::{
    default_container, GAP, GAP2, IMAGE_HEIGHT, POSTER_HINT, SMALL, SPACE, SUBTITLE_SIZE,
    TITLE_SIZE,
};
use crate::queue::TaskStatus;
use crate::state::{Page, State};

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

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Search {
    text: String,
    series: Vec<SearchSeries>,
    movies: Vec<SearchMovie>,
    series_page: usize,
    movies_page: usize,
    // Unique identifier of last search so that we can look up any recorded errors.
    search_id: Uuid,
}

impl Search {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, s: &mut State) {
        s.assets.mark_with_hint(
            self.series
                .iter()
                .skip(self.series_page * PER_PAGE)
                .take(PER_PAGE)
                .flat_map(|s| s.poster()),
            POSTER_HINT,
        );

        s.assets.mark_with_hint(
            self.movies
                .iter()
                .skip(self.movies_page * PER_PAGE)
                .take(PER_PAGE)
                .flat_map(|s| s.poster()),
            POSTER_HINT,
        );
    }

    /// Handle theme change.
    pub(crate) fn update(
        &mut self,
        s: &mut State,
        message: Message,
        commands: impl Commands<Message>,
    ) {
        match message {
            Message::Error(error) => {
                s.handle_error(error);
            }
            Message::Navigate(page) => {
                s.push_history(page);
            }
            Message::Search => {
                self.search(s, commands);
            }
            Message::Change(text) => {
                self.text = text;
            }
            Message::SeriesPage(page) => {
                self.series_page = page;
                s.assets.clear();
            }
            Message::MoviesPage(page) => {
                self.movies_page = page;
                s.assets.clear();
            }
            Message::Result(series, movies) => {
                self.series = series;
                self.movies = movies;
                s.assets.clear();
            }
            Message::SearchKindChanged(kind) => {
                s.service.config_mut().search_kind = kind;
                self.search(s, commands);
            }
            Message::AddSeriesByRemote(remote_id) => {
                s.service
                    .push_task_without_delay(TaskKind::DownloadSeriesByRemoteId { remote_id });
            }
            Message::SwitchSeries(series_id, remote_id) => {
                s.remove_series(&series_id);
                s.service
                    .push_task_without_delay(TaskKind::DownloadSeriesByRemoteId { remote_id });
            }
            Message::RemoveSeries(series_id) => {
                s.remove_series(&series_id);
            }
            Message::AddMovieByRemote(_) => {}
            Message::SwitchMovie(_, _) => {}
            Message::RemoveMovie(_) => {}
        }
    }

    fn search(&mut self, s: &mut State, mut commands: impl Commands<Message>) {
        if self.text.is_empty() {
            return;
        }

        self.series_page = 0;
        self.movies_page = 0;

        let search_id = Uuid::new_v4();
        let query = self.text.clone();
        let search_kind = s.service.config().search_kind;
        self.search_id = search_id;

        match search_kind {
            SearchKind::Tvdb => {
                let op = s.service.search_tvdb(&self.text);

                let translate = move |out: Result<_>| match out
                    .with_context(|| anyhow!("Searching {search_kind} for `{query}`"))
                {
                    Ok(series) => Message::Result(series, Vec::new()),
                    Err(error) => Message::Error(ErrorInfo::new(ErrorId::Search(search_id), error)),
                };

                commands.perform(op, translate);
            }
            SearchKind::Tmdb => {
                let series = s.service.search_series_tmdb(&self.text);
                let movies = s.service.search_movies_tmdb(&self.text);

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
    pub(crate) fn view(&self, st: &State) -> Element<'static, Message> {
        let mut series = Column::new();

        for s in self
            .series
            .iter()
            .skip(self.series_page * PER_PAGE)
            .take(PER_PAGE)
        {
            let local_series = st.service.get_series_by_remote(&s.id);

            let handle = match s
                .poster()
                .and_then(|p| st.assets.image_with_hint(&p, POSTER_HINT))
            {
                Some(handle) => handle,
                None => st.missing_poster(),
            };

            let mut actions = Row::new();

            let status = st
                .service
                .task_status(&TaskId::DownloadSeriesByRemoteId { remote_id: s.id });

            match status {
                Some(TaskStatus::Pending) => {
                    actions = actions
                        .push(button(text("Queued...").size(SMALL)).style(theme::Button::Primary));
                }
                Some(TaskStatus::Running) => {
                    actions = actions.push(
                        button(text("Downloading...").size(SMALL)).style(theme::Button::Primary),
                    );
                }
                None => {
                    if let Some(local) = local_series {
                        if local.remote_id != Some(s.id) {
                            actions = actions.push(
                                button(text("Switch").size(SMALL))
                                    .style(theme::Button::Primary)
                                    .on_press(Message::SwitchSeries(local.id, s.id)),
                            );
                        }

                        actions = actions.push(
                            button(text("Remove").size(SMALL))
                                .style(theme::Button::Destructive)
                                .on_press(Message::RemoveSeries(local.id)),
                        );
                    } else {
                        actions = actions.push(
                            button(text("Add").size(SMALL))
                                .style(theme::Button::Positive)
                                .on_press(Message::AddSeriesByRemote(s.id)),
                        );
                    }
                }
            }

            let mut first_aired = Column::new();

            if let Some(date) = s.first_aired {
                first_aired = first_aired.push(text(format!("First aired: {date}")).size(SMALL));
            }

            let mut result = Column::new();

            let series_name = text(&s.name).size(SUBTITLE_SIZE);

            if let Some(local_series) = local_series {
                result = result.push(
                    button(series_name)
                        .style(theme::Button::Text)
                        .padding(0)
                        .on_press(Message::Navigate(Page::Series(local_series.id))),
                );
            } else {
                result = result.push(series_name);
            }

            result = result.push(first_aired);
            result = result.push(actions.spacing(SPACE));

            series = series.push(
                Row::new()
                    .push(image(handle).height(IMAGE_HEIGHT))
                    .push(
                        Column::new()
                            .push(result.spacing(SPACE))
                            .push(text(&s.overview))
                            .spacing(GAP),
                    )
                    .spacing(GAP),
            );
        }

        series = series.push(paginate(
            self.series_page,
            self.series.len(),
            Message::SeriesPage,
        ));

        let mut movies = Column::new();

        for m in self
            .movies
            .iter()
            .skip(self.movies_page * PER_PAGE)
            .take(PER_PAGE)
        {
            let local_movie = st.service.get_movie_by_remote(&m.id);

            let handle = match m
                .poster()
                .and_then(|p| st.assets.image_with_hint(&p, POSTER_HINT))
            {
                Some(handle) => handle,
                None => st.missing_poster(),
            };

            let mut actions = Row::new();

            let status = st
                .service
                .task_status(&TaskId::DownloadMovieByRemoteId { remote_id: m.id });

            match status {
                Some(TaskStatus::Pending) => {
                    actions = actions
                        .push(button(text("Queued...").size(SMALL)).style(theme::Button::Primary));
                }
                Some(TaskStatus::Running) => {
                    actions = actions.push(
                        button(text("Downloading...").size(SMALL)).style(theme::Button::Primary),
                    );
                }
                None => {
                    if let Some(local) = local_movie {
                        if local.remote_id != Some(m.id) {
                            actions = actions.push(
                                button(text("Switch").size(SMALL))
                                    .style(theme::Button::Primary)
                                    .on_press(Message::SwitchMovie(local.id, m.id)),
                            );
                        }

                        actions = actions.push(
                            button(text("Remove").size(SMALL))
                                .style(theme::Button::Destructive)
                                .on_press(Message::RemoveMovie(local.id)),
                        );
                    } else {
                        actions = actions.push(
                            button(text("Add").size(SMALL))
                                .style(theme::Button::Positive)
                                .on_press(Message::AddMovieByRemote(m.id)),
                        );
                    }
                }
            }

            let overview = m.overview.as_str();

            let mut release_date = Column::new();

            if let Some(date) = m.release_date {
                release_date = release_date.push(text(format!("First aired: {date}")).size(SMALL));
            }

            let mut result = Column::new();

            let movie_title = text(&m.title).size(SUBTITLE_SIZE);

            if let Some(local_movie) = local_movie {
                result = result.push(
                    button(movie_title)
                        .style(theme::Button::Text)
                        .padding(0)
                        .on_press(Message::Navigate(Page::Movie(local_movie.id))),
                );
            } else {
                result = result.push(movie_title);
            }

            result = result.push(release_date);
            result = result.push(actions.spacing(SPACE));

            movies = movies.push(
                Row::new()
                    .push(image(handle).height(IMAGE_HEIGHT))
                    .push(
                        Column::new()
                            .push(result.spacing(SPACE))
                            .push(text(overview))
                            .spacing(GAP),
                    )
                    .spacing(GAP),
            );
        }

        movies = movies.push(paginate(
            self.movies_page,
            self.movies.len(),
            Message::MoviesPage,
        ));

        let query = text_input("Query...", &self.text, Message::Change).on_submit(Message::Search);

        let submit = button("Search");

        let submit = if !self.text.is_empty() {
            submit.on_press(Message::Search)
        } else {
            submit
        };

        let mut search_kind = Column::new().push(text("Source:").size(SMALL));

        search_kind =
            [SearchKind::Tvdb, SearchKind::Tmdb]
                .iter()
                .fold(search_kind, |column, kind| {
                    column.push(
                        radio(
                            kind.to_string(),
                            *kind,
                            Some(st.service.config().search_kind),
                            Message::SearchKindChanged,
                        )
                        .size(SMALL),
                    )
                });

        let mut page = Column::new();

        page = page.push(text("Search").size(TITLE_SIZE));
        page = page.push(Row::new().push(query).push(submit));

        if let Some(e) = st.get_error(ErrorId::Search(self.search_id)) {
            page = page.push(
                button(text(format!("Error: {}", e.message)))
                    .width(Length::Fill)
                    .style(theme::Button::Destructive)
                    .on_press(Message::Navigate(Page::Errors)),
            );
        }

        page = page.push(search_kind.spacing(SPACE));

        let mut row = Row::new();
        row = row.push(series.spacing(GAP2).width(Length::FillPortion(1)));
        row = row.push(movies.spacing(GAP2).width(Length::FillPortion(1)));
        page = page.push(row.spacing(GAP2));
        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}

fn paginate<M>(page: usize, len: usize, m: M) -> Row<'static, Message>
where
    M: FnOnce(usize) -> Message + Copy,
{
    let mut row = Row::new();

    if len > PER_PAGE {
        let mut prev = button("previous page").style(theme::Button::Positive);
        let mut next = button("next page").style(theme::Button::Positive);

        if let Some(page) = page.checked_sub(1) {
            prev = prev.on_press(m(page));
        }

        if (page + 1) * PER_PAGE < len {
            next = next.on_press(m(page + 1));
        }

        let text = text(format!(
            "{}-{} ({})",
            page * PER_PAGE,
            ((page + 1) * PER_PAGE).min(len),
            len,
        ));

        row = Row::new()
            .push(prev)
            .push(next)
            .push(text)
            .align_items(Alignment::Center)
            .spacing(GAP);
    }

    row
}
