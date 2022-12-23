use iced::widget::{button, image, text, text_input, vertical_space, Column, Row};
use iced::{theme, Command};
use iced::{Element, Length};

use crate::cache::ImageHint;
use crate::component::*;
use crate::comps;
use crate::message::Page;
use crate::params::{centered, GAP, GAP2, IMAGE_HEIGHT, SPACE, SUBTITLE_SIZE};
use crate::state::State;
use crate::style;

/// Posters are defined by their maximum height.
const POSTER_HINT: ImageHint = ImageHint::Height(IMAGE_HEIGHT as u32);

/// Messages generated and handled by [SeriesList].
#[derive(Debug, Clone)]
pub(crate) enum Message {
    ChangeFilter(String),
    SeriesActions(usize, comps::series_actions::Message),
    Navigate(Page),
}

#[derive(Default, Clone)]
pub(crate) struct SeriesList {
    filter: String,
    filtered: Option<Box<[usize]>>,
    actions: Vec<comps::SeriesActions>,
}

impl SeriesList {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, s: &mut State) {
        self.actions
            .init_from_iter(s.service.all_series().iter().map(|s| s.id));

        if let Some(filtered) = &self.filtered {
            let series = s.service.all_series();
            let images = filtered.iter().flat_map(|&i| series.get(i)?.poster);
            s.assets.mark_with_hint(images, POSTER_HINT);
        } else {
            s.assets.mark_with_hint(
                s.service.all_series().iter().flat_map(|s| s.poster),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) -> Command<Message> {
        match message {
            Message::ChangeFilter(filter) => {
                self.filter = filter;
                let filter = crate::search::Tokens::new(&self.filter);

                self.filtered = if !filter.is_empty() {
                    let mut filtered = Vec::new();

                    for (index, s) in s.service.all_series().iter().enumerate() {
                        if filter.matches(&s.title) {
                            filtered.push(index);
                        }
                    }

                    Some(filtered.into())
                } else {
                    None
                };

                Command::none()
            }
            Message::SeriesActions(index, message) => {
                if let Some(actions) = self.actions.get_mut(index) {
                    actions
                        .update(s, message)
                        .map(move |m| Message::SeriesActions(index, m))
                } else {
                    Command::none()
                }
            }
            Message::Navigate(page) => {
                s.push_history(page);
                Command::none()
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut rows = Column::new();

        let mut it;
        let mut it2;

        let iter: &mut dyn Iterator<Item = _> = if let Some(filtered) = &self.filtered {
            it = filtered.iter().flat_map(|i| s.service.all_series().get(*i));
            &mut it
        } else {
            it2 = s.service.all_series().iter();
            &mut it2
        };

        for (index, (series, actions)) in iter.zip(&self.actions).enumerate() {
            let poster = match series
                .poster
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.assets.missing_poster(),
            };

            let graphic = button(image(poster).height(Length::Units(IMAGE_HEIGHT)))
                .on_press(Message::Navigate(Page::Series(series.id)))
                .style(theme::Button::Text)
                .padding(0);

            let episodes = s.service.episodes(&series.id);

            let title = button(text(&series.title).size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Series(series.id)));

            let actions = actions
                .view(s, series)
                .map(move |m| Message::SeriesActions(index, m));

            let mut content = Column::new().width(Length::Fill);

            content = content.push(
                Column::new()
                    .push(title)
                    .push(text(format!("{} episode(s)", episodes.len())))
                    .push(actions)
                    .spacing(SPACE),
            );

            if let Some(overview) = &series.overview {
                content = content.push(text(overview));
            }

            rows = rows.push(
                centered(
                    Row::new()
                        .push(graphic)
                        .push(content.spacing(GAP))
                        .spacing(GAP)
                        .width(Length::Fill),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let filter = text_input("Filter...", &self.filter, |value| {
            Message::ChangeFilter(value)
        })
        .width(Length::Fill);

        Column::new()
            .push(vertical_space(Length::Shrink))
            .push(centered(Row::new().push(filter).width(Length::Fill), None))
            .push(rows.spacing(GAP2))
            .width(Length::Fill)
            .spacing(GAP)
            .into()
    }
}
