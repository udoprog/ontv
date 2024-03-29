use crate::service::PendingRef;
use crate::utils::Hoverable;
use crate::{prelude::*, Service};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    #[allow(unused)]
    Calendar(comps::calendar::Message),
    /// Hover a scheduled series.
    HoverScheduled(SeriesId),
    /// Skip an episode.
    SkipSeries(SeriesId, EpisodeId),
    /// Skip a movie.
    SkipMovie(MovieId),
    /// Watch an episode.
    Watch(usize, comps::watch::Message),
    /// Navigate.
    Navigate(Page),
    /// Reset show list.
    ResetPending,
    ShowLessPending,
    ShowMorePending,
    DecrementPage,
    IncrementPage,
}

/// The state for the settings page.
pub(crate) struct Dashboard {
    calendar: comps::Calendar,
    watch: Vec<comps::Watch>,
    schedule_focus: Option<(SeriesId, Option<ImageV2>)>,
}

impl Dashboard {
    pub(crate) fn new(state: &State, service: &Service) -> Self {
        let mut schedule_focus = None;

        if let Some(scheduled) = service.schedule().first().and_then(|d| d.schedule.first()) {
            if let Some(series) = service.series(&scheduled.series_id) {
                schedule_focus = Some((series.id, series.graphics.poster.clone()));
            }
        }

        Self {
            calendar: comps::Calendar::new(*state.today(), chrono::Weekday::Sun),
            watch: Vec::new(),
            schedule_focus,
        }
    }

    pub(crate) fn prepare(&mut self, cx: &mut Ctxt<'_>) {
        if let Some(id) = self.schedule_focus.as_ref().and_then(|d| d.1.as_ref()) {
            cx.assets.mark_with_hint([id], POSTER_HINT);
        }

        let limit = cx.service.config().dashboard_limit();
        let today = *cx.state.today();

        let iter = cx
            .service
            .pending()
            .filter(|p| p.has_aired(&today))
            .rev()
            .take(limit);

        cx.assets
            .mark_with_hint(iter.clone().flat_map(|p| p.poster()), POSTER_HINT);

        self.watch.init_from_iter(iter.map(|p| {
            comps::watch::Props::new(match p {
                PendingRef::Episode { episode, .. } => comps::watch::Kind::Episode(episode.id),
                PendingRef::Movie { movie } => comps::watch::Kind::Movie(movie.id),
            })
        }));
    }

    pub(crate) fn update(&mut self, cx: &mut Ctxt<'_>, message: Message) {
        match message {
            Message::Calendar(message) => {
                self.calendar.update(message);
            }
            Message::HoverScheduled(series_id) => {
                if let Some(series) = cx.service.series(&series_id) {
                    self.schedule_focus = Some((series_id, series.poster().cloned()));
                }
            }
            Message::SkipSeries(series_id, episode_id) => {
                let now = Utc::now();
                cx.service.skip(&now, &series_id, &episode_id);
            }
            Message::SkipMovie(movie_id) => {
                let now = Utc::now();
                cx.service.skip_movie(&now, &movie_id);
            }
            Message::Watch(index, message) => {
                if let Some(w) = self.watch.get_mut(index) {
                    w.update(cx, message);
                }
            }
            Message::Navigate(page) => {
                cx.push_history(page);
            }
            Message::ResetPending => {
                cx.service.config_mut().dashboard_limit = 1;
                cx.service.config_mut().dashboard_page = 6;
            }
            Message::ShowLessPending => {
                let limit = cx.service.config().dashboard_limit.saturating_sub(1).max(1);
                cx.service.config_mut().dashboard_limit = limit;
            }
            Message::ShowMorePending => {
                let limit = cx.service.config().dashboard_limit + 1;
                cx.service.config_mut().dashboard_limit = limit;
            }
            Message::DecrementPage => {
                let page = cx.service.config().dashboard_page.saturating_sub(1).max(1);
                cx.service.config_mut().dashboard_page = page;
            }
            Message::IncrementPage => {
                let page = cx.service.config().dashboard_page + 1;
                cx.service.config_mut().dashboard_page = page;
            }
        }
    }

    pub(crate) fn view(&self, cx: &CtxtRef<'_>) -> Element<'static, Message> {
        let up_next_title = link(w::text("Watch next").size(SUBTITLE_SIZE))
            .on_press(Message::Navigate(Page::WatchNext(
                crate::page::watch_next::State::default(),
            )))
            .width(Length::Fill);

        let mut modify = w::Row::new().push(w::Space::new(Length::Fill, Length::Shrink));

        if cx.service.config().dashboard_page > 1 {
            modify = modify.push(
                w::button(
                    w::text("-")
                        .width(SMALL_SIZE)
                        .size(SMALL_SIZE)
                        .horizontal_alignment(Horizontal::Center),
                )
                .style(theme::Button::Secondary)
                .on_press(Message::DecrementPage),
            );
        }

        modify = modify.push(
            w::button(
                w::text("+")
                    .width(SMALL_SIZE)
                    .size(SMALL_SIZE)
                    .horizontal_alignment(Horizontal::Center),
            )
            .style(theme::Button::Secondary)
            .on_press(Message::IncrementPage),
        );

        if cx.service.config().dashboard_limit > 1 {
            modify = modify.push(
                w::button(w::text("reset").size(SMALL_SIZE))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ResetPending),
            );

            modify = modify.push(
                w::button(w::text("show less...").size(SMALL_SIZE))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ShowLessPending),
            );
        }

        modify = modify.push(
            w::button(w::text("show more...").size(SMALL_SIZE))
                .style(theme::Button::Secondary)
                .on_press(Message::ShowMorePending),
        );

        let pending = w::Column::new()
            .push(modify.spacing(SPACE).width(Length::Fill))
            .push(self.render_pending(cx));

        let scheduled_title = w::text("Upcoming")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let scheduled = self.render_scheduled(cx);

        w::Column::new()
            // .push(self.calendar.view().map(Message::Calendar))
            .push(w::vertical_space().height(Length::Shrink))
            .push(centered(up_next_title, None))
            .push(centered(
                pending.padding(GAP).spacing(GAP),
                Some(style::weak),
            ))
            .push(centered(scheduled_title, None))
            .push(centered(scheduled.padding(GAP).spacing(GAP), None))
            .push(w::vertical_space().height(Length::Shrink))
            .spacing(GAP2)
            .into()
    }

    fn render_pending(&self, cx: &CtxtRef<'_>) -> w::Column<'static, Message> {
        let mut cols = w::Column::new();

        let mut pending = w::Row::new();
        let mut count = 0;

        let limit = cx.service.config().dashboard_limit();
        let page = cx.service.config().dashboard_page();

        let iter = cx
            .service
            .pending()
            .rev()
            .filter(|p| p.has_aired(cx.state.today()))
            .take(limit);

        for (index, (watch, pending_ref)) in self.watch.iter().zip(iter).enumerate() {
            if index % page == 0 && index > 0 {
                cols = cols.push(pending.spacing(GAP));
                pending = w::Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let poster = match pending_ref
                .poster()
                .and_then(|i| cx.assets.image_with_hint(i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => cx.missing_poster(),
            };

            let mut panel = w::Column::new();

            let page = match pending_ref {
                PendingRef::Episode { series, .. } => page::series::page(series.id),
                PendingRef::Movie { movie } => page::movie::page(movie.id),
            };

            panel = panel.push(
                link(w::image(poster).width(Length::Fill))
                    .on_press(Message::Navigate(page.clone())),
            );

            let mut actions = w::Row::new();

            actions = actions.push(
                watch
                    .view(
                        "Mark",
                        theme::Button::Positive,
                        theme::Button::Positive,
                        Length::Shrink,
                        Horizontal::Center,
                        false,
                    )
                    .map(move |m| Message::Watch(index, m)),
            );

            if !watch.is_confirm() {
                let skip = match pending_ref {
                    PendingRef::Episode {
                        series, episode, ..
                    } => Message::SkipSeries(series.id, episode.id),
                    PendingRef::Movie { movie } => Message::SkipMovie(movie.id),
                };

                actions = actions.push(
                    w::button(
                        w::text("Skip")
                            .horizontal_alignment(Horizontal::Center)
                            .size(SMALL_SIZE),
                    )
                    .style(theme::Button::Secondary)
                    .on_press(skip)
                    .width(Length::FillPortion(5)),
                );

                let len = match pending_ref {
                    PendingRef::Episode { episode, .. } => {
                        cx.service.watched_by_episode(&episode.id).len()
                    }
                    PendingRef::Movie { movie } => cx.service.watched_by_movie(&movie.id).len(),
                };

                let style = match len {
                    0 => theme::Button::Text,
                    _ => theme::Button::Positive,
                };

                actions = actions.push(
                    w::button(
                        w::text(format_args!("{len}"))
                            .horizontal_alignment(Horizontal::Center)
                            .size(SMALL_SIZE),
                    )
                    .style(style)
                    .width(Length::FillPortion(2)),
                );
            }

            panel = panel.push(actions.spacing(SPACE));

            let title = match pending_ref {
                PendingRef::Episode { episode, .. } => episode_title(&episode),
                PendingRef::Movie { movie } => {
                    w::text(&movie.title).shaping(w::text::Shaping::Advanced)
                }
            };

            if let Some(date) = pending_ref.date() {
                panel = panel.push(w::text(format!("{date}")).size(SMALL_SIZE));
            }

            panel = panel.push(
                link(
                    title
                        .size(SMALL_SIZE)
                        .horizontal_alignment(Horizontal::Center),
                )
                .on_press(Message::Navigate(page)),
            );

            pending = pending.push(
                w::container(
                    panel
                        .width(Length::Fill)
                        .align_items(Alignment::Center)
                        .spacing(SPACE),
                )
                .width(Length::FillPortion(1)),
            );
        }

        if count > 0 {
            cols = cols.push(pending.spacing(GAP));
        }

        cols.spacing(GAP)
    }

    fn render_scheduled(&self, cx: &CtxtRef<'_>) -> w::Column<'static, Message> {
        let mut scheduled_rows = w::Column::new();
        let mut cols = w::Row::new();
        let mut count = 0;
        let mut first = true;

        let page = cx.service.config().schedule_page();

        for (n, day) in cx.service.schedule().iter().enumerate() {
            if n % page == 0 && n > 0 {
                scheduled_rows = scheduled_rows.push(cols.spacing(GAP));
                cols = w::Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let mut column = w::Column::new();

            column = column.push(
                match day.date.signed_duration_since(*cx.service.now()).num_days() {
                    0 => w::text("Today"),
                    1 => w::text("Tomorrow"),
                    _ => w::text(day.date),
                },
            );

            let mut it = day
                .schedule
                .iter()
                .flat_map(|sched| {
                    cx.service
                        .series(&sched.series_id)
                        .into_iter()
                        .map(move |series| (series, sched))
                })
                .peekable();

            if let Some((series_id, id)) = self.schedule_focus.as_ref().filter(|_| first) {
                let poster = match id
                    .as_ref()
                    .and_then(|id| cx.assets.image_with_hint(id, POSTER_HINT))
                {
                    Some(image) => image,
                    None => cx.missing_poster(),
                };

                cols = cols.push(
                    link(w::image(poster))
                        .on_press(Message::Navigate(page::series::page(*series_id)))
                        .width(Length::FillPortion(1)),
                );

                count += 1;
                first = false;
            }

            while let Some((series, schedule)) = it.next() {
                let mut series_column = w::Column::new();
                let mut episodes = w::Column::new();

                for episode_id in &schedule.episodes {
                    let Some(episode) = cx.service.episode(episode_id) else {
                        continue;
                    };

                    let name = match &episode.name {
                        Some(name) => {
                            format!("{}x{} {name}", episode.season.short(), episode.number)
                        }
                        None => format!("{}x{}", episode.season.short(), episode.number),
                    };

                    let episode = link(
                        w::text(name)
                            .shaping(w::text::Shaping::Advanced)
                            .size(SMALL_SIZE),
                    )
                    .on_press(Message::Navigate(page::season::page(
                        series.id,
                        episode.season,
                    )));

                    episodes = episodes
                        .push(Hoverable::new(episode).on_hover(Message::HoverScheduled(series.id)));
                }

                let title = link(w::text(&series.title).shaping(w::text::Shaping::Advanced))
                    .on_press(Message::Navigate(page::series::page(series.id)));

                series_column = series_column
                    .push(Hoverable::new(title).on_hover(Message::HoverScheduled(series.id)));

                series_column = series_column.push(episodes.spacing(SPACE));

                column = column.push(series_column.spacing(SPACE));

                if it.peek().is_some() {
                    column = column.push(w::horizontal_rule(1));
                }
            }

            cols = cols.push(column.width(Length::FillPortion(1)).spacing(GAP));
        }

        if count > 0 {
            scheduled_rows = scheduled_rows.push(cols.spacing(GAP));
        }

        scheduled_rows
    }
}

fn episode_title(episode: &Episode) -> w::Text<'static> {
    let mut episode_number = match episode.season {
        SeasonNumber::Number(number) => format!("{}x{}", number, episode.number),
        SeasonNumber::Specials => format!("Special {}", episode.number),
    };

    if let Some(number) = episode.absolute_number {
        use std::fmt::Write;
        write!(episode_number, " ({number})").unwrap();
    }

    if let Some(name) = &episode.name {
        w::text(format!("{episode_number}: {name}")).shaping(w::text::Shaping::Advanced)
    } else {
        w::text(episode_number)
    }
}
