use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{
    button, container, horizontal_rule, image, text, vertical_space, Column, Row, Space, Text,
};
use iced::{theme, Element};
use iced::{Alignment, Length};

use crate::model::{Episode, EpisodeId, ImageV2, SeasonNumber, SeriesId};
use crate::params::{centered, GAP, GAP2, POSTER_HINT, SMALL, SPACE, SUBTITLE_SIZE};
use crate::service::PendingRef;
use crate::state::{Page, State};
use crate::utils::Hoverable;
use crate::{comps, style};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    #[allow(unused)]
    Calendar(comps::calendar::Message),
    /// Hover a scheduled series.
    HoverScheduled(SeriesId),
    /// Skip an episode.
    Skip(SeriesId, EpisodeId),
    /// Watch an episode.
    Watch(SeriesId, EpisodeId),
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
            schedule_focus,
        }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        if let Some(id) = self.schedule_focus.as_ref().and_then(|d| d.1.as_ref()) {
            s.assets.mark_with_hint([id], POSTER_HINT);
        }

        let limit = s.service.config().dashboard_limit();

        s.assets.mark_with_hint(
            s.service
                .pending(*s.today())
                .rev()
                .take(limit)
                .flat_map(|p| p.season.and_then(|s| s.poster()).or(p.series.poster())),
            POSTER_HINT,
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
            Message::Watch(series_id, episode_id) => {
                let now = Utc::now();
                s.service.watch(&now, &series_id, &episode_id);
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
        let up_next_title = text("Watch next")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let mut modify = Row::new().push(Space::new(Length::Fill, Length::Shrink));

        if s.service.config().dashboard_page > 1 {
            modify = modify.push(
                button(
                    text("-")
                        .width(Length::Units(SMALL))
                        .size(SMALL)
                        .horizontal_alignment(Horizontal::Center),
                )
                .style(theme::Button::Secondary)
                .on_press(Message::DecrementPage),
            );
        }

        modify = modify.push(
            button(
                text("+")
                    .width(Length::Units(SMALL))
                    .size(SMALL)
                    .horizontal_alignment(Horizontal::Center),
            )
            .style(theme::Button::Secondary)
            .on_press(Message::IncrementPage),
        );

        if s.service.config().dashboard_limit > 1 {
            modify = modify.push(
                button(text("reset").size(SMALL))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ResetPending),
            );

            modify = modify.push(
                button(text("show less...").size(SMALL))
                    .style(theme::Button::Secondary)
                    .on_press(Message::ShowLessPending),
            );
        }

        modify = modify.push(
            button(text("show more...").size(SMALL))
                .style(theme::Button::Secondary)
                .on_press(Message::ShowMorePending),
        );

        let pending = Column::new()
            .push(modify.spacing(SPACE).width(Length::Fill))
            .push(self.render_pending(s));

        let scheduled_title = text("Upcoming")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let scheduled = self.render_scheduled(s);

        Column::new()
            // .push(self.calendar.view().map(Message::Calendar))
            .push(vertical_space(Length::Shrink))
            .push(centered(up_next_title, None))
            .push(centered(
                pending.padding(GAP).spacing(GAP),
                Some(style::weak),
            ))
            .push(centered(scheduled_title, None))
            .push(centered(scheduled.padding(GAP).spacing(GAP), None))
            .push(vertical_space(Length::Shrink))
            .spacing(GAP2)
            .into()
    }

    fn render_pending(&self, s: &State) -> Column<'static, Message> {
        let mut cols = Column::new();

        let mut pending = Row::new();
        let mut count = 0;

        let limit = s.service.config().dashboard_limit();
        let page = s.service.config().dashboard_page();

        for (
            index,
            PendingRef {
                series,
                season,
                episode,
                ..
            },
        ) in s.service.pending(*s.today()).rev().take(limit).enumerate()
        {
            if index % page == 0 && index > 0 {
                cols = cols.push(pending.spacing(GAP));
                pending = Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let poster = match season
                .and_then(|s| s.poster())
                .or(series.poster())
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.missing_poster(),
            };

            let mut panel = Column::new();

            panel = panel.push(
                button(image(poster).width(Length::Fill))
                    .padding(0)
                    .style(theme::Button::Text)
                    .on_press(Message::Navigate(Page::Series(series.id))),
            );

            let mut actions = Row::new();

            actions = actions.push(
                button(
                    text("Mark")
                        .horizontal_alignment(Horizontal::Center)
                        .size(SMALL),
                )
                .style(theme::Button::Positive)
                .on_press(Message::Watch(series.id, episode.id))
                .width(Length::FillPortion(2)),
            );

            actions = actions.push(
                button(
                    text("Skip")
                        .horizontal_alignment(Horizontal::Center)
                        .size(SMALL),
                )
                .style(theme::Button::Secondary)
                .on_press(Message::Skip(series.id, episode.id))
                .width(Length::FillPortion(2)),
            );

            panel = panel.push(actions.spacing(SPACE));

            let episode_title = episode_title(episode);

            if let Some(air_date) = &episode.aired {
                panel = panel.push(text(format!("Aired: {air_date}")).size(SMALL));
            }

            panel = panel.push(
                button(
                    episode_title
                        .size(SMALL)
                        .horizontal_alignment(Horizontal::Center),
                )
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Season(series.id, episode.season))),
            );

            pending = pending.push(
                container(
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

    fn render_scheduled(&self, s: &State) -> Column<'static, Message> {
        let mut scheduled_rows = Column::new();
        let mut cols = Row::new();
        let mut count = 0;
        let mut first = true;

        let page = s.service.config().schedule_page();

        for (n, day) in s.service.schedule().iter().enumerate() {
            if n % page == 0 && n > 0 {
                scheduled_rows = scheduled_rows.push(cols.spacing(GAP));
                cols = Row::new();
                count = 0;
            } else {
                count += 1;
            }

            let mut column = Column::new();

            column = column.push(
                match day.date.signed_duration_since(s.service.now()).num_days() {
                    0 => text("Today"),
                    1 => text("Tomorrow"),
                    _ => text(day.date),
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
                    button(image(poster))
                        .padding(0)
                        .style(theme::Button::Text)
                        .on_press(Message::Navigate(Page::Series(*series_id)))
                        .width(Length::FillPortion(1)),
                );

                count += 1;
                first = false;
            }

            while let Some((series, schedule)) = it.next() {
                let mut series_column = Column::new();
                let mut episodes = Column::new();

                for episode_id in &schedule.episodes {
                    let Some(episode) = s.service.episodes(&schedule.series_id).iter().find(|e| e.id == *episode_id) else {
                        continue;
                    };

                    let name = match &episode.name {
                        Some(name) => {
                            format!("{}x{} {name}", episode.season.short(), episode.number)
                        }
                        None => format!("{}x{}", episode.season.short(), episode.number),
                    };

                    let episode = button(text(name).size(SMALL))
                        .style(theme::Button::Text)
                        .padding(0)
                        .on_press(Message::Navigate(Page::Season(series.id, episode.season)));

                    episodes = episodes
                        .push(Hoverable::new(episode).on_hover(Message::HoverScheduled(series.id)));
                }

                let title = button(text(&series.title))
                    .padding(0)
                    .style(theme::Button::Text)
                    .on_press(Message::Navigate(Page::Series(series.id)));

                series_column = series_column
                    .push(Hoverable::new(title).on_hover(Message::HoverScheduled(series.id)));

                series_column = series_column.push(episodes.spacing(SPACE));

                column = column.push(series_column.spacing(SPACE));

                if it.peek().is_some() {
                    column = column.push(horizontal_rule(1));
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

fn episode_title(episode: &Episode) -> Text<'static> {
    let mut episode_number = match episode.season {
        SeasonNumber::Number(number) => format!("{}x{}", number, episode.number),
        SeasonNumber::Specials => format!("Special {}", episode.number),
    };

    if let Some(number) = episode.absolute_number {
        use std::fmt::Write;
        write!(episode_number, " ({number})").unwrap();
    }

    if let Some(name) = &episode.name {
        text(format!("{episode_number}: {name}"))
    } else {
        text(episode_number)
    }
}
