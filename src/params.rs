use chrono::Duration;
use iced::widget::{Column, Container, Text};
use iced::Element;

use crate::style;

pub(crate) const SPACE: u16 = 5;
pub(crate) const HALF_GAP: u16 = 10;
pub(crate) const GAP: u16 = 20;
pub(crate) const GAP2: u16 = GAP * 2;

pub(crate) const SMALL_SIZE: u16 = 16;
pub(crate) const TITLE_SIZE: u16 = 32;
pub(crate) const SUBTITLE_SIZE: u16 = 24;
pub(crate) const ACTION_SIZE: u16 = 16;
pub(crate) const SUB_MENU_SIZE: u16 = 16;

pub(crate) const CONTAINER_WIDTH: u32 = 1200;

/// Standard poster height used in lists.
pub(crate) const IMAGE_HEIGHT: u16 = 200;

/// Standard screencap height.
pub(crate) const SCREENCAP_HEIGHT: u16 = 270;

/// Build a default container.
pub(crate) fn default_container<'a, E, M: 'a>(content: E) -> Column<'a, M>
where
    Element<'a, M>: From<E>,
{
    use iced::widget::container;
    use iced::{Alignment, Length};

    Column::new()
        .push(container(content).max_width(CONTAINER_WIDTH))
        .align_items(Alignment::Center)
        .width(Length::Fill)
}

/// Alternate container with background color.
pub(crate) fn centered<'a, E, M: 'a>(
    content: E,
    style: Option<style::StyleSheet>,
) -> Container<'a, M>
where
    Element<'a, M>: From<E>,
{
    use iced::alignment::Horizontal;
    use iced::widget::container;
    use iced::Length;

    let content = container(content).max_width(CONTAINER_WIDTH);

    let mut container = container(content)
        .align_x(Horizontal::Center)
        .width(Length::Fill);

    if let Some(style) = style {
        container = container.style(style);
    }

    container
}

/// Convert a chrono duration into something that is pretty to display.
pub(crate) fn duration_display(d: Duration) -> Text<'static> {
    let seconds = d.num_seconds();

    if seconds > 0 {
        let seconds = seconds.unsigned_abs();

        match seconds {
            n if n >= 60 => match seconds / 60 {
                1 => Text::new("one minute ago"),
                n => Text::new(format!("{n} minutes ago")),
            },
            _ => Text::new(format!("< one minute ago")),
        }
    } else {
        let seconds = seconds.unsigned_abs();

        match seconds {
            0 => Text::new("right now"),
            n if n >= 60 => match seconds / 60 {
                1 => Text::new("in one minute"),
                n => Text::new(format!("in {n} minutes")),
            },
            _ => Text::new(format!("< one minute")),
        }
    }
}
