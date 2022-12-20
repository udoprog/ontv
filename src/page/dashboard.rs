use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, image, row, text, vertical_space, Column};
use iced::{Alignment, Length};

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::model::SeasonNumber;
use crate::params::{centered, style, ACTION_SIZE, GAP, SMALL_SIZE, SPACE, SUBTITLE_SIZE};
use crate::service::{PendingRef, Service};

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Dashboard {}

impl Dashboard {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, service: &Service, assets: &mut Assets) {
        assets.mark(service.pending().rev().take(5).map(|p| p.series.poster));
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, service: &Service, assets: &Assets) -> Column<'static, Message> {
        let mut pending = row![];

        for PendingRef {
            series, episode, ..
        } in service.pending().rev().take(5)
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

            let handle = match assets.image(&series.poster) {
                Some(handle) => handle,
                None => assets.missing_poster(),
            };

            let mut episode_number = match episode.season {
                SeasonNumber::Number(number) => format!("{}x{}", number, episode.number),
                SeasonNumber::Unknown => format!("{}", episode.number),
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

            let image = button(image(handle).width(Length::Fill))
                .width(Length::Fill)
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Series(series.id)));

            pending = pending.push(
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
    }
}
