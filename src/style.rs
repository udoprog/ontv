use iced::{widget as w, Element, Pixels};

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

impl<'element, M, T> From<TextBuilder<'_, T>> for Element<'element, M>
where
    T: ToString,
{
    fn from(builder: TextBuilder<'_, T>) -> Element<'element, M> {
        let string = builder.text.to_string();
        let all_ascii = string.is_ascii();

        let mut text = w::text(string);

        if !all_ascii {
            text = text.shaping(w::text::Shaping::Advanced);
        }

        if let Some(size) = builder.size {
            text = text.size(size);
        }

        text.into()
    }
}
