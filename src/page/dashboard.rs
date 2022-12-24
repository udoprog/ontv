use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, container, horizontal_rule, image, text, vertical_space, Column, Row};
use iced::{theme, Command, Element};
use iced::{Alignment, Length};

use crate::cache::ImageHint;
use crate::message::Page;
use crate::model::{EpisodeId, Image, SeasonNumber, SeriesId};
use crate::params::{centered, ACTION_SIZE, GAP, GAP2, SMALL_SIZE, SPACE, SUBTITLE_SIZE};
use crate::service::PendingRef;
use crate::state::State;
use crate::style;
use crate::utils::Hoverable;

/// Dashboard gets a bit more leeway, since the image is dynamically scaled.
const POSTER_HINT: ImageHint = ImageHint::Width(512);

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Hover a scheduled series.
    HoverScheduled(SeriesId),
    /// Skip an episode.
    Skip(SeriesId, EpisodeId),
    /// Watch an episode.
    Watch(SeriesId, EpisodeId),
    /// Navigate.
    Navigate(Page),
}

/// The state for the settings page.
pub(crate) struct Dashboard {
    schedule_focus: Option<(SeriesId, Option<Image>)>,
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
                schedule_focus = Some((series.id, series.poster));
            }
        }

        Self { schedule_focus }
    }

    pub(crate) fn prepare(&mut self, s: &mut State) {
        if let Some(id) = self.schedule_focus.and_then(|d| d.1) {
            s.assets.mark_with_hint([id], POSTER_HINT);
        }

        s.assets.mark_with_hint(
            s.service
                .pending()
                .rev()
                .take(5)
                .flat_map(|p| p.season.and_then(|s| s.poster).or(p.series.poster)),
            POSTER_HINT,
        );
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::HoverScheduled(series_id) => {
                if let Some(series) = s.service.series(&series_id) {
                    self.schedule_focus = Some((series_id, series.poster));
                }

                Command::none()
            }
            Message::Skip(series_id, episode_id) => {
                let now = Utc::now();
                s.service.skip(&series_id, &episode_id, now);
                Command::none()
            }
            Message::Watch(series_id, episode_id) => {
                let now = Utc::now();
                s.service.watch(&series_id, &episode_id, now);
                Command::none()
            }
            Message::Navigate(page) => {
                s.push_history(page);
                Command::none()
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let up_next_title = text("Watch next")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let pending = self.render_pending(s);

        let scheduled_title = text("Upcoming")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        let scheduled = self.render_scheduled(s);

        Column::new()
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

    fn render_pending(&self, s: &State) -> Row<'static, Message> {
        let mut pending = Row::new();

        for PendingRef {
            series,
            season,
            episode,
            ..
        } in s.service.pending().rev().take(5)
        {
            let mut actions = Row::new();

            actions = actions.push(
                button(
                    text("Mark")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Positive)
                .on_press(Message::Watch(series.id, episode.id))
                .width(Length::FillPortion(2)),
            );

            actions = actions.push(
                button(
                    text("Skip")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Secondary)
                .on_press(Message::Skip(series.id, episode.id))
                .width(Length::FillPortion(2)),
            );

            let poster = match season
                .and_then(|s| s.poster)
                .or(series.poster)
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.assets.missing_poster(),
            };

            let mut episode_number = match episode.season {
                SeasonNumber::Number(number) => format!("{}x{}", number, episode.number),
                SeasonNumber::Specials => format!("Special {}", episode.number),
            };

            if let Some(number) = episode.absolute_number {
                use std::fmt::Write;
                write!(episode_number, " ({number})").unwrap();
            }

            let mut episode_aired = Row::new();

            let episode_info = if let Some(name) = &episode.name {
                text(format!("{episode_number}: {name}"))
            } else {
                text(episode_number)
            };

            if let Some(air_date) = &episode.aired {
                episode_aired = episode_aired.push(
                    text(format!("Aired: {air_date}"))
                        .horizontal_alignment(Horizontal::Center)
                        .size(SMALL_SIZE),
                );
            }

            let series_name = button(
                text(&series.title)
                    .horizontal_alignment(Horizontal::Center)
                    .size(ACTION_SIZE),
            )
            .style(theme::Button::Text)
            .on_press(Message::Navigate(Page::Series(series.id)));

            let season_name = button(
                text(episode.season.short())
                    .horizontal_alignment(Horizontal::Center)
                    .size(ACTION_SIZE),
            )
            .style(theme::Button::Text)
            .on_press(Message::Navigate(Page::Season(series.id, episode.season)));

            let image = button(image(poster).width(Length::Fill))
                .width(Length::Fill)
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Series(series.id)));

            pending = pending.push(
                container(
                    Column::new()
                        .push(
                            Column::new()
                                .push(
                                    Row::new()
                                        .push(series_name)
                                        .push(season_name)
                                        .spacing(SPACE),
                                )
                                .push(image)
                                .push(actions.spacing(SPACE))
                                .width(Length::Fill)
                                .align_items(Alignment::Center)
                                .spacing(SPACE),
                        )
                        .push(
                            Column::new()
                                .push(episode_info.horizontal_alignment(Horizontal::Center))
                                .push(episode_aired)
                                .align_items(Alignment::Center)
                                .spacing(SPACE),
                        )
                        .spacing(GAP)
                        .align_items(Alignment::Center)
                        .width(Length::Fill),
                )
                .width(Length::FillPortion(1)),
            );
        }

        pending
    }

    fn render_scheduled(&self, s: &State) -> Column<'static, Message> {
        let mut scheduled_rows = Column::new();
        let mut scheduled_cols = Row::new();
        let mut scheduled_count = 0;
        let mut first = true;

        for (n, day) in s.service.schedule().iter().enumerate() {
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

            if let Some((series_id, id)) = self.schedule_focus.filter(|_| first) {
                let poster = match id.and_then(|id| s.assets.image_with_hint(&id, POSTER_HINT)) {
                    Some(image) => image,
                    None => s.assets.missing_poster(),
                };

                scheduled_cols = scheduled_cols.push(
                    button(image(poster))
                        .padding(0)
                        .style(theme::Button::Text)
                        .on_press(Message::Navigate(Page::Series(series_id)))
                        .width(Length::FillPortion(1)),
                );

                scheduled_count += 1;
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

                    let episode = button(text(name).size(SMALL_SIZE))
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

            scheduled_cols = scheduled_cols.push(column.width(Length::FillPortion(1)).spacing(GAP));
            scheduled_count += 1;

            if n > 0 && n % 5 == 0 {
                scheduled_rows = scheduled_rows
                    .push(std::mem::replace(&mut scheduled_cols, Row::new()).spacing(GAP));
                scheduled_count = 0;
            }
        }

        if scheduled_count > 0 {
            scheduled_rows = scheduled_rows.push(scheduled_cols.spacing(GAP));
        }

        scheduled_rows
    }
}
