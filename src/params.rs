use chrono::Duration;
use iced::widget::{Column, Container, Text};
use iced::{Color, Element};

use crate::message::Message;

pub(crate) const SPACE: u16 = 5;
pub(crate) const HALF_GAP: u16 = 10;
pub(crate) const GAP: u16 = 20;
pub(crate) const GAP2: u16 = GAP * 2;

pub(crate) const SMALL_SIZE: u16 = 14;
pub(crate) const TITLE_SIZE: u16 = 32;
pub(crate) const SUBTITLE_SIZE: u16 = 24;
pub(crate) const ACTION_SIZE: u16 = 16;
pub(crate) const SUB_MENU_SIZE: u16 = 16;

pub(crate) const CONTAINER_WIDTH: u32 = 1200;

/// Warning color.
pub(crate) const WARNING_COLOR: Color = Color::from_rgba(0.5, 0.0, 0.0, 1.0);

/// Build a default container.
pub(crate) fn default_container<E>(content: E) -> Column<'static, Message>
where
    Element<'static, Message>: From<E>,
{
    use iced::widget::container;
    use iced::{Alignment, Length};

    Column::new()
        .push(container(content).max_width(CONTAINER_WIDTH))
        .align_items(Alignment::Center)
        .width(Length::Fill)
}

/// Alternate container with background color.
pub(crate) fn centered<E>(
    content: E,
    style: Option<style::StyleSheet>,
) -> Container<'static, Message>
where
    Element<'static, Message>: From<E>,
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
            1 => Text::new("one second ago"),
            n if n >= 60 => match seconds / 60 {
                1 => Text::new("one minute ago"),
                n => Text::new(format!("{n} minutes ago")),
            },
            n => Text::new(format!("{n} seconds ago")),
        }
    } else {
        let seconds = seconds.unsigned_abs();

        match seconds {
            1 => Text::new("in one second"),
            n if n >= 60 => match seconds / 60 {
                1 => Text::new("in one minute"),
                n => Text::new(format!("in {n} minutes")),
            },
            n => Text::new(format!("in {n} seconds")),
        }
    }
}

pub(crate) mod style {
    use iced::widget::container;
    use iced::{Background, Theme};

    pub(crate) type StyleSheet = fn(theme: &Theme) -> container::Appearance;

    /// Weaker background color.
    pub(crate) fn weak(theme: &Theme) -> container::Appearance {
        let extended = theme.extended_palette();

        let pair = extended.background.weak;

        container::Appearance {
            background: Some(Background::Color(pair.color)),
            text_color: Some(pair.text),
            ..Default::default()
        }
    }
}
