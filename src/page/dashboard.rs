use crate::prelude::*;
use crate::service::PendingRef;
use crate::utils::Hoverable;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    #[allow(unused)]
    Calendar(comps::calendar::Message),
    /// Hover a scheduled series.
    HoverScheduled(SeriesId),
    /// Skip an episode.
    Skip(SeriesId, EpisodeId),
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
    pub(crate) fn new(s: &State) -> Self {
        let mut schedule_focus = None;

        if let Some(scheduled) = s
            .service
            .schedule()
            .first()
            .and_then(|d| d.schedule.first())
        {
            if let Some(series) = s.service.series(&scheduled.series_id) {
                schedule_focus = Some((series.id, series.graphics.poster.clone()));
            }
        }

        Self {
            calendar: comps::Calendar::new(*s.today(), chrono::Weekday::Sun),
            watch: Vec::new(),
            schedule_focus,
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        if let Some(id) = self.schedule_focus.as_ref().and_then(|d| d.1.as_ref()) {
            s.assets.mark_with_hint([id], POSTER_HINT);
        }

        let limit = s.service.config().dashboard_limit();
        let today = *s.today();

        s.assets.mark_with_hint(
            s.service
                .pending(&today)
                .rev()
                .take(limit)
                .flat_map(|p| p.poster()),
            POSTER_HINT,
        );

        self.watch.init_from_iter(
            s.service
                .pending(&today)
                .rev()
                .map(|p| comps::watch::Props::new(p.episode.id)),
        );
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::Calendar(message) => {
                self.calendar.update(message);
            }
            Message::HoverScheduled(series_id) => {
                if let Some(series) = s.service.series(&series_id) {
                    self.schedule_focus = Some((series_id, series.poster().cloned()));
                }
            }
            Message::Skip(series_id, episode_id) => {
                let now = Utc::now();
                s.service.skip(&now, &series_id, &episode_id);
            }
            Message::Watch(index, message) => {
                if let Some(w) = self.watch.get_mut(index) {
                    w.update(s, message);
                }
            }
            Message::Navigate(page) => {
                s.push_history(page);
            }
            Message::ResetPending => {
                s.service.config_mut().dashboard_limit = 1;
                s.service.config_mut().dashboard_page = 6;
            }
            Message::ShowLessPending => {
                let limit = s.service.config().dashboard_limit.saturating_sub(1).max(1);
                s.service.config_mut().dashboard_limit = limit;
            }
            Message::ShowMorePending => {
                let limit = s.service.config().dashboard_limit + 1;
                s.service.config_mut().dashboard_limit = limit;
            }
            Message::DecrementPage => {
                let page = s.service.config().dashboard_page.saturating_sub(1).max(1);
                s.service.config_mut().dashboard_page = page;
            }
            Message::IncrementPage => {
                let page = s.service.config().dashboard_page + 1;
                s.service.config_mut().dashboard_page = page;
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let up_next_title = w::button(w::text("Watch next").size(SUBTITLE_SIZE))
            .on_press(Message::Navigate(Page::WatchNext))
            .padding(0)
            .style(theme::Button::Text)
            .width(Length::Fill);

        let mut modify = w::Row::new().push(w::Space::new(Length::Fill, Length::Shrink));

        if s.service.config().dashboard_page > 1 {
            modify = modify.push(
                w::button(
                    w::text("-")
                        .width(SMALL)
                        .size(SMALL)
                        .horizontal_alignment(Horizontal::Center),
                )
                .style(theme::Button::Secondary)
                .on_press(Message::DecrementPage),
            );
        }

        modify = modify.push(
            w::button(
                w::text("+")
                    .width(SMALL)
                    .size(SMALL)
                    .horizontal_alignment(Horizontal::Center),
            )
            .style(theme::Button::Secondary)
            .on_press(Message::IncrementPage),
        );

        if s.service.config().dashboard_limit > 1 {
            modify = modify.push(
                w::button(w::text("reset").size(SMALL))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ResetPending),
            );

            modify = modify.push(
                w::button(w::text("show less...").size(SMALL))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ShowLessPending),
            );
        }

        modify = modify.push(
            w::button(w::text("show more...").size(SMALL))
                .style(theme::Button::Secondary)
                .on_press(Message::ShowMorePending),
        );

        let pending = w::Column::new()
            .push(modify.spacing(SPACE).width(Length::Fill))
            .push(self.render_pending(s));

        let scheduled_title = w::text("Upcoming")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let scheduled = self.render_scheduled(s);

        w::Column::new()
            // .push(self.calendar.view().map(Message::Calendar))
            .push(w::vertical_space(Length::Shrink))
            .push(centered(up_next_title, None))
            .push(centered(
                pending.padding(GAP).spacing(GAP),
                Some(style::weak),
            ))
            .push(centered(scheduled_title, None))
            .push(centered(scheduled.padding(GAP).spacing(GAP), None))
            .push(w::vertical_space(Length::Shrink))
            .spacing(GAP2)
            .into()
    }

    fn render_pending(&self, s: &State) -> w::Column<'static, Message> {
        let mut cols = w::Column::new();

        let mut pending = w::Row::new();
        let mut count = 0;

        let limit = s.service.config().dashboard_limit();
        let page = s.service.config().dashboard_page();

        for (index, (watch, pending_ref)) in self
            .watch
            .iter()
            .zip(s.service.pending(s.today()).rev().take(limit))
            .enumerate()
        {
            let p @ PendingRef {
                series, episode, ..
            } = pending_ref;

            if index % page == 0 && index > 0 {
                cols = cols.push(pending.spacing(GAP));
                pending = w::Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let poster = match p
                .poster()
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.missing_poster(),
            };

            let mut panel = w::Column::new();

            panel = panel.push(
                w::button(w::image(poster).width(Length::Fill))
                    .padding(0)
                    .style(theme::Button::Text)
                    .on_press(Message::Navigate(Page::Series(series.id))),
            );

            let mut actions = w::Row::new();

            actions = actions.push(
                watch
                    .view(
                        "Mark",
                        theme::Button::Positive,
                        theme::Button::Positive,
                        Length::FillPortion(5),
                        Horizontal::Center,
                        false,
                    )
                    .map(move |m| Message::Watch(index, m)),
            );

            if !watch.is_confirm() {
                actions = actions.push(
                    w::button(
                        w::text("Skip")
                            .horizontal_alignment(Horizontal::Center)
                            .size(SMALL),
                    )
                    .style(theme::Button::Secondary)
                    .on_press(Message::Skip(series.id, episode.id))
                    .width(Length::FillPortion(5)),
                );

                let len = s.service.watched(&episode.id).len();

                let style = match len {
                    0 => theme::Button::Text,
                    _ => theme::Button::Positive,
                };

                actions = actions.push(
                    w::button(
                        w::text(format_args!("{len}"))
                            .horizontal_alignment(Horizontal::Center)
                            .size(SMALL),
                    )
                    .style(style)
                    .width(Length::FillPortion(2)),
                );
            }

            panel = panel.push(actions.spacing(SPACE));

            let episode_title = episode_title(&episode);

            if let Some(air_date) = &episode.aired {
                panel = panel.push(w::text(format!("Aired: {air_date}")).size(SMALL));
            }

            panel = panel.push(
                w::button(
                    episode_title
                        .size(SMALL)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Season(series.id, episode.season))),
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

    fn render_scheduled(&self, s: &State) -> w::Column<'static, Message> {
        let mut scheduled_rows = w::Column::new();
        let mut cols = w::Row::new();
        let mut count = 0;
        let mut first = true;

        let page = s.service.config().schedule_page();

        for (n, day) in s.service.schedule().iter().enumerate() {
            if n % page == 0 && n > 0 {
                scheduled_rows = scheduled_rows.push(cols.spacing(GAP));
                cols = w::Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let mut column = w::Column::new();

            column = column.push(
                match day.date.signed_duration_since(*s.service.now()).num_days() {
                    0 => w::text("Today"),
                    1 => w::text("Tomorrow"),
                    _ => w::text(day.date),
                },
            );

            let mut it = day
                .schedule
                .iter()
                .flat_map(|sched| {
                    s.service
                        .series(&sched.series_id)
                        .into_iter()
                        .map(move |series| (series, sched))
                })
                .peekable();

            if let Some((series_id, id)) = self.schedule_focus.as_ref().filter(|_| first) {
                let poster = match id
                    .as_ref()
                    .and_then(|id| s.assets.image_with_hint(&id, POSTER_HINT))
                {
                    Some(image) => image,
                    None => s.missing_poster(),
                };

                cols = cols.push(
                    w::button(w::image(poster))
                        .padding(0)
                        .style(theme::Button::Text)
                        .on_press(Message::Navigate(Page::Series(*series_id)))
                        .width(Length::FillPortion(1)),
                );

                count += 1;
                first = false;
            }

            while let Some((series, schedule)) = it.next() {
                let mut series_column = w::Column::new();
                let mut episodes = w::Column::new();

                for episode_id in &schedule.episodes {
                    let Some(episode) = s.service.episode(&episode_id) else {
                        continue;
                    };

                    let name = match &episode.name {
                        Some(name) => {
                            format!("{}x{} {name}", episode.season.short(), episode.number)
                        }
                        None => format!("{}x{}", episode.season.short(), episode.number),
                    };

                    let episode = w::button(w::text(name).size(SMALL))
                        .style(theme::Button::Text)
                        .padding(0)
                        .on_press(Message::Navigate(Page::Season(series.id, episode.season)));

                    episodes = episodes
                        .push(Hoverable::new(episode).on_hover(Message::HoverScheduled(series.id)));
                }

                let title = w::button(w::text(&series.title))
                    .padding(0)
                    .style(theme::Button::Text)
                    .on_press(Message::Navigate(Page::Series(series.id)));

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
        w::text(format!("{episode_number}: {name}"))
    } else {
        w::text(episode_number)
    }
}
