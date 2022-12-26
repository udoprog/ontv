use iced::widget::container;
use iced::{Background, Theme};

pub type StyleSheet = fn(theme: &Theme) -> container::Appearance;

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

/// Generate warning text.
pub fn warning_text(theme: &Theme) -> iced::theme::Text {
    let extended = theme.extended_palette();
    let color = extended.danger.base.color;
    iced::theme::Text::Color(color)
}
