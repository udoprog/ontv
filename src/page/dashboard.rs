use chrono::Utc;
use iced::alignment::Horizontal;
use iced::widget::{button, column, container, image, row, text, vertical_space};
use iced::{theme, Command, Element};
use iced::{Alignment, Length};

use crate::cache::ImageHint;
use crate::message::Page;
use crate::model::{EpisodeId, SeasonNumber, SeriesId};
use crate::params::{centered, style, ACTION_SIZE, GAP, SMALL_SIZE, SPACE, SUBTITLE_SIZE};
use crate::service::PendingRef;
use crate::state::State;

/// Dashboard gets a bit more leeway, since the image is dynamically scaled.
const POSTER_HINT: ImageHint = ImageHint::Width(512);

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Skip an episode.
    Skip(SeriesId, EpisodeId),
    /// Watch an episode.
    Watch(SeriesId, EpisodeId),
    /// Navigate.
    Navigate(Page),
}

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Dashboard;

impl Dashboard {
    pub(crate) fn prepare(&mut self, s: &mut State) {
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
        let mut pending = row![];

        for PendingRef {
            series,
            season,
            episode,
            ..
        } in s.service.pending().rev().take(5)
        {
            let mut actions = row![].spacing(SPACE);

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

            let mut episode_aired = row![];

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
                episode
                    .season
                    .short()
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
                    column![
                        column![
                            row![series_name, season_name].spacing(SPACE),
                            image,
                            actions,
                        ]
                        .width(Length::Fill)
                        .align_items(Alignment::Center)
                        .spacing(SPACE),
                        column![
                            episode_info.horizontal_alignment(Horizontal::Center),
                            episode_aired,
                        ]
                        .align_items(Alignment::Center)
                        .spacing(SPACE),
                    ]
                    .spacing(GAP)
                    .align_items(Alignment::Center)
                    .width(Length::Fill),
                )
                .width(Length::FillPortion(1)),
            );
        }

        let up_next_title = text("Up next...")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);
        let scheduled_title = text("Scheduled...")
            .horizontal_alignment(Horizontal::Left)
            .width(Length::Fill)
            .size(SUBTITLE_SIZE);

        column![
            vertical_space(Length::Shrink),
            centered(up_next_title, None),
            centered(pending.padding(GAP).spacing(GAP), Some(style::weak)),
            centered(scheduled_title, None),
            vertical_space(Length::Shrink),
        ]
        .spacing(GAP)
        .into()
    }
}
