use iced::widget::container;
use iced::{widget as w, Element, Pixels};
use iced::{Background, Theme};

use crate::params::*;

pub(crate) struct Style;

impl Style {
    #[inline]
    pub(crate) fn text<T>(&self, text: T) -> TextBuilder<'_, T> {
        TextBuilder {
            style: self,
            text,
            size: None,
        }
    }
}

pub(crate) struct TextBuilder<'a, T> {
    #[allow(unused)]
    style: &'a Style,
    text: T,
    size: Option<Pixels>,
}

impl<T> TextBuilder<'_, T> {
    #[inline]
    pub(crate) fn sm(mut self) -> Self {
        self.size = Some(SMALL_SIZE);
        self
    }

    #[inline]
    pub(crate) fn sub(mut self) -> Self {
        self.size = Some(SUBTITLE_SIZE);
        self
    }

    #[inline]
    pub(crate) fn title(mut self) -> Self {
        self.size = Some(TITLE_SIZE);
        self
    }
}

impl<M, T> Into<Element<'static, M>> for TextBuilder<'_, T>
where
    T: ToString,
{
    fn into(self) -> Element<'static, M> {
        let string = self.text.to_string();
        let all_ascii = string.chars().all(|c| c.is_ascii());

        let mut text = w::text(string);

        if !all_ascii {
            text = text.shaping(w::text::Shaping::Advanced);
        }

        if let Some(size) = self.size {
            text = text.size(size);
        }

        text.into()
    }
}

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
