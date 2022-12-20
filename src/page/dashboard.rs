use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{button, column, container, image, row, text, vertical_space, Column};
use iced::{Alignment, Length};

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::model::SeasonNumber;
use crate::params::{centered, style, ACTION_SIZE, GAP, SCREENCAP_HEIGHT, SMALL_SIZE, SPACE};
use crate::service::{PendingRef, Service};

/// The state for the settings page.
#[derive(Default)]
pub(crate) struct Dashboard {}

impl Dashboard {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, service: &Service, assets: &mut Assets) {
        assets.mark(
            service
                .pending()
                .rev()
                .take(5)
                .flat_map(|p| p.episode.filename),
        );
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
                    text("W")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Positive)
                .on_press(Message::Watch(series.id, episode.id))
                .width(Length::Units(36)),
            );

            actions = actions.push(
                button(
                    text("S")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Positive)
                .on_press(Message::Skip(series.id, episode.id))
                .width(Length::Units(36)),
            );

            actions = actions.push(
                button(
                    text("Series")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Primary)
                .on_press(Message::Navigate(Page::Series(series.id)))
                .width(Length::FillPortion(2)),
            );

            actions = actions.push(
                button(
                    text("Season")
                        .horizontal_alignment(Horizontal::Center)
                        .size(ACTION_SIZE),
                )
                .style(theme::Button::Primary)
                .on_press(Message::Navigate(Page::Season(series.id, episode.season)))
                .width(Length::FillPortion(2)),
            );

            let handle = match episode.filename.and_then(|handle| assets.image(&handle)) {
                Some(handle) => handle,
                None => assets.missing_screencap(),
            };

            let episode_number = match episode.season {
                SeasonNumber::Number(number) => text(format!("{}x{}", number, episode.number)),
                SeasonNumber::Unknown => text(format!("{} (No Season)", episode.number)),
                SeasonNumber::Specials => text(format!("Special {}", episode.number)),
            };

            let mut episode_info = row![];

            if let Some(name) = &episode.name {
                episode_info = episode_info.push(text(name));
            }

            pending = pending.push(
                column![
                    text(&series.title).size(SMALL_SIZE),
                    column![
                        container(image(handle)).max_height(SCREENCAP_HEIGHT),
                        actions,
                    ]
                    .spacing(SPACE),
                    episode_number,
                    episode_info,
                ]
                .align_items(Alignment::Center)
                .spacing(SPACE)
                .width(Length::FillPortion(1)),
            );
        }

        column![
            vertical_space(Length::Units(GAP)),
            centered(pending.spacing(GAP).padding(GAP), Some(style::weak)),
        ]
    }
}
