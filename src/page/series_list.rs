use iced::widget::{button, image, text, text_input, Column, Row};
use iced::{theme, Element, Length};

use crate::component::*;
use crate::comps;
use crate::model::SeriesId;
use crate::params::{centered, GAP, GAP2, IMAGE_HEIGHT, POSTER_HINT, SPACE, SUBTITLE_SIZE};
use crate::state::{Page, State};
use crate::style;

/// Messages generated and handled by [SeriesList].
#[derive(Debug, Clone)]
pub(crate) enum Message {
    ChangeFilter(String),
    SeriesActions(usize, comps::series_actions::Message),
    Navigate(Page),
}

#[derive(Default)]
pub(crate) struct SeriesList {
    filter: String,
    filtered: Option<Box<[SeriesId]>>,
    actions: Vec<comps::SeriesActions>,
}

impl SeriesList {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, s: &mut State) {
        if let Some(filtered) = &self.filtered {
            let series = filtered.iter().flat_map(|id| s.service.series(id));
            self.actions.init_from_iter(series.clone().map(|s| s.id));
            s.assets
                .mark_with_hint(series.flat_map(|s| s.poster), POSTER_HINT);
        } else {
            self.actions
                .init_from_iter(s.service.series_by_name().map(|s| s.id));

            s.assets.mark_with_hint(
                s.service.series_by_name().flat_map(|s| s.poster),
                POSTER_HINT,
            );
        }
    }

    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::ChangeFilter(filter) => {
                self.filter = filter;
                let filter = crate::search::Tokens::new(&self.filter);

                self.filtered = if !filter.is_empty() {
                    let mut filtered = Vec::new();

                    for s in s.service.series_by_name() {
                        if filter.matches(&s.title) {
                            filtered.push(s.id);
                        }
                    }

                    Some(filtered.into())
                } else {
                    None
                };
            }
            Message::SeriesActions(index, message) => {
                if let Some(actions) = self.actions.get_mut(index) {
                    actions.update(s, message);
                }
            }
            Message::Navigate(page) => {
                s.push_history(page);
            }
        }
    }

    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let mut rows = Column::new();

        let mut it;
        let mut it2;

        let iter: &mut dyn Iterator<Item = _> = if let Some(filtered) = &self.filtered {
            it = filtered.iter().flat_map(|id| s.service.series(id));
            &mut it
        } else {
            it2 = s.service.series_by_name();
            &mut it2
        };

        for (index, (series, actions)) in iter.zip(&self.actions).enumerate() {
            let poster = match series
                .poster
                .and_then(|i| s.assets.image_with_hint(&i, POSTER_HINT))
            {
                Some(handle) => handle,
                None => s.missing_poster(),
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

            if !series.overview.is_empty() {
                content = content.push(text(&series.overview));
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
            .push(centered(
                Row::new().push(filter).padding(GAP).width(Length::Fill),
                None,
            ))
            .push(rows.spacing(GAP2))
            .width(Length::Fill)
            .into()
    }
}
