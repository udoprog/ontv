use core::fmt;

use anyhow::Result;
use iced::widget::{button, image, radio, text, text_input, Column, Row};
use iced::{theme, Alignment, Element};
use iced::{Command, Length};

use crate::cache::ImageHint;
use crate::message::ErrorMessage;
use crate::model::{RemoteSeriesId, SearchSeries, SeriesId};
use crate::params::{
    default_container, ACTION_SIZE, GAP, GAP2, IMAGE_HEIGHT, SMALL_SIZE, SPACE, TITLE_SIZE,
};
use crate::service::NewSeries;
use crate::state::State;

/// Number of results per page.
const PER_PAGE: usize = 5;
/// Posters are defined by their maximum height.
const POSTER_HINT: ImageHint = ImageHint::Height(IMAGE_HEIGHT as u32);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SearchKind {
    #[default]
    Tvdb,
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

/// Message generated by dashboard page.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    Error(ErrorMessage),
    Search,
    Change(String),
    Page(usize),
    Result(Vec<SearchSeries>),
    SearchKindChanged(SearchKind),
    AddSeriesByRemote(RemoteSeriesId),
    SwitchSeries(SeriesId, RemoteSeriesId),
    RemoveSeries(SeriesId),
    SeriesDownloadToTrack(RemoteSeriesId, NewSeries),
    SeriesDownloadFailed(RemoteSeriesId, ErrorMessage),
}

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Search {
    kind: SearchKind,
    text: String,
    series: Vec<SearchSeries>,
    page: usize,
}

impl Search {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, s: &mut State) {
        s.assets.mark_with_hint(
            self.series
                .iter()
                .skip(self.page * PER_PAGE)
                .take(PER_PAGE)
                .flat_map(|s| s.poster),
            POSTER_HINT,
        );
    }

    /// Handle theme change.
    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::Error(error) => {
                s.handle_error(error);
                Command::none()
            }
            Message::Search => self.search(s),
            Message::Change(text) => {
                self.text = text;
                Command::none()
            }
            Message::Page(page) => {
                self.page = page;
                s.assets.clear();
                Command::none()
            }
            Message::Result(series) => {
                self.series = series;
                s.assets.clear();
                Command::none()
            }
            Message::SearchKindChanged(kind) => {
                self.kind = kind;
                self.search(s)
            }
            Message::AddSeriesByRemote(remote_id) => self.add_series_by_remote(s, &remote_id),
            Message::SwitchSeries(series_id, remote_id) => {
                s.remove_series(&series_id);
                self.add_series_by_remote(s, &remote_id)
            }
            Message::RemoveSeries(series_id) => {
                s.remove_series(&series_id);
                Command::none()
            }
            Message::SeriesDownloadToTrack(remote_id, data) => {
                s.download_complete(remote_id);
                s.service.insert_new_series(data);
                Command::none()
            }
            Message::SeriesDownloadFailed(remote_id, error) => {
                s.download_complete(remote_id);
                s.handle_error(error);
                Command::none()
            }
        }
    }

    fn search(&mut self, s: &mut State) -> Command<Message> {
        if self.text.is_empty() {
            return Command::none();
        }

        self.page = 0;

        let query = self.text.clone();

        let translate = |out: Result<_>| match out {
            Ok(series) => Message::Result(series),
            Err(error) => Message::Error(error.into()),
        };

        match self.kind {
            SearchKind::Tvdb => {
                let op = s.service.search_tvdb(&query);
                Command::perform(op, translate)
            }
            SearchKind::Tmdb => {
                let op = s.service.search_tmdb(&query);
                Command::perform(op, translate)
            }
        }
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut results = Column::new();

        for series in self.series.iter().skip(self.page * PER_PAGE).take(PER_PAGE) {
            let handle = match series
                .poster
                .and_then(|p| s.assets.image_with_hint(&p, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.assets.missing_poster(),
            };

            let mut actions = Row::new();

            if s.is_downloading(&series.id) {
                actions = actions.push(
                    button(text("Downloading...").size(ACTION_SIZE)).style(theme::Button::Primary),
                );
            } else if let Some(s) = s.service.get_series_by_remote(series.id) {
                if s.remote_id != Some(series.id) {
                    actions = actions.push(
                        button(text("Switch").size(ACTION_SIZE))
                            .style(theme::Button::Primary)
                            .on_press(Message::SwitchSeries(s.id, series.id)),
                    );
                }

                actions = actions.push(
                    button(text("Remove").size(ACTION_SIZE))
                        .style(theme::Button::Destructive)
                        .on_press(Message::RemoveSeries(s.id)),
                );
            } else {
                actions = actions.push(
                    button(text("Add").size(ACTION_SIZE))
                        .style(theme::Button::Positive)
                        .on_press(Message::AddSeriesByRemote(series.id)),
                );
            }

            let overview = series.overview.as_deref().unwrap_or_default();

            let mut first_aired = Column::new();

            if let Some(date) = series.first_aired {
                first_aired =
                    first_aired.push(text(format!("First aired: {date}")).size(SMALL_SIZE));
            }

            results = results.push(
                Row::new()
                    .push(image(handle).height(Length::Units(IMAGE_HEIGHT)))
                    .push(
                        Column::new()
                            .push(
                                Column::new()
                                    .push(text(&series.name).size(24))
                                    .push(first_aired)
                                    .push(actions.spacing(SPACE))
                                    .spacing(SPACE),
                            )
                            .push(text(overview))
                            .spacing(GAP),
                    )
                    .spacing(GAP),
            );
        }

        let mut pages = Row::new();

        if self.series.len() > PER_PAGE {
            let mut prev = button("previous page").style(theme::Button::Positive);
            let mut next = button("next page").style(theme::Button::Positive);

            if let Some(page) = self.page.checked_sub(1) {
                prev = prev.on_press(Message::Page(page));
            }

            if (self.page + 1) * PER_PAGE < self.series.len() {
                next = next.on_press(Message::Page(self.page + 1));
            }

            let text = text(format!(
                "{}-{} ({})",
                self.page * PER_PAGE,
                ((self.page + 1) * PER_PAGE).min(self.series.len()),
                self.series.len(),
            ));

            pages = Row::new()
                .push(prev)
                .push(next)
                .push(text)
                .align_items(Alignment::Center)
                .spacing(GAP);
        }

        let query = text_input("Query...", &self.text, Message::Change).on_submit(Message::Search);

        let submit = button("Search");

        let submit = if !self.text.is_empty() {
            submit.on_press(Message::Search)
        } else {
            submit
        };

        let mut kind = Column::new().push(text("Source:"));
        kind = kind.push([SearchKind::Tvdb, SearchKind::Tmdb].iter().fold(
            Row::new().spacing(GAP),
            |column, kind| {
                column.push(radio(format!("{kind}"), *kind, Some(self.kind), |kind| {
                    Message::SearchKindChanged(kind)
                }))
            },
        ));

        let page = Column::new()
            .push(text("Search").size(TITLE_SIZE))
            .push(Row::new().push(query).push(submit))
            .push(kind.spacing(SPACE))
            .push(results.spacing(GAP2))
            .push(pages)
            .spacing(GAP)
            .padding(GAP);

        default_container(page).into()
    }
}

impl Search {
    fn add_series_by_remote(
        &mut self,
        s: &mut State,
        remote_id: &RemoteSeriesId,
    ) -> Command<Message> {
        if s.service.set_tracked_by_remote(remote_id) {
            return Command::none();
        }

        Command::perform(
            s.download_series_by_remote(remote_id),
            |(remote_id, result)| match result {
                Ok(data) => Message::SeriesDownloadToTrack(remote_id, data),
                Err(error) => Message::SeriesDownloadFailed(remote_id, error.into()),
            },
        )
    }
}
