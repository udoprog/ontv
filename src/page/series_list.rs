use std::collections::HashSet;

use iced::widget::{button, column, image, row, text, text_input, vertical_space, Column};
use iced::Length;
use iced::{theme, Command};
use serde::{Deserialize, Serialize};

use crate::assets::Assets;
use crate::message::{Message, Page};
use crate::params::{centered, style, GAP, GAP2, POSTER_HEIGHT, SPACE, SUBTITLE_SIZE};
use crate::service::Service;

/// Messages generated and handled by [SeriesList].
#[derive(Debug, Clone)]
pub(crate) enum M {
    ChangeFilter(String),
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub(crate) struct State {
    filter: String,
    filtered: Option<Box<[usize]>>,
}

impl State {
    /// Prepare the view.
    pub(crate) fn prepare(&mut self, service: &Service, assets: &mut Assets) {
        if let Some(filtered) = &self.filtered {
            let series = service.all_series();
            let images = filtered.iter().flat_map(|&i| Some(series.get(i)?.poster));
            assets.mark(images);
        } else {
            assets.mark(service.all_series().iter().map(|s| s.poster));
        }
    }

    pub(crate) fn update(&mut self, service: &Service, message: M) -> Command<Message> {
        match message {
            M::ChangeFilter(filter) => {
                self.filter = filter;
                let filter = tokenize(&self.filter, false);

                self.filtered = if !filter.is_empty() {
                    let mut filtered = Vec::new();

                    for (index, s) in service.all_series().iter().enumerate() {
                        let title = tokenize(&s.title, true);

                        if filter.iter().all(|t| title.contains(t.as_str())) {
                            filtered.push(index);
                        }
                    }

                    Some(filtered.into())
                } else {
                    None
                };
            }
        }

        Command::none()
    }

    pub(crate) fn view(&self, service: &Service, assets: &Assets) -> Column<'static, Message> {
        let mut series = column![];

        let mut it;
        let mut it2;

        let iter: &mut dyn Iterator<Item = _> = if let Some(filtered) = &self.filtered {
            it = filtered.iter().flat_map(|i| service.all_series().get(*i));
            &mut it
        } else {
            it2 = service.all_series().iter();
            &mut it2
        };

        for s in iter {
            let handle = match assets.image(&s.poster) {
                Some(handle) => handle,
                None => assets.missing_poster(),
            };

            let graphic = button(image(handle).height(Length::Units(POSTER_HEIGHT)))
                .on_press(Message::Navigate(Page::Series(s.id)))
                .style(theme::Button::Text)
                .padding(0);

            let episodes = service.episodes(s.id);

            let actions = crate::page::series::actions(s).spacing(SPACE);

            let title = button(text(&s.title).size(SUBTITLE_SIZE))
                .padding(0)
                .style(theme::Button::Text)
                .on_press(Message::Navigate(Page::Series(s.id)));

            let mut content = column![].width(Length::Fill);

            content = content.push(
                column![
                    title,
                    text(format!("{} episode(s)", episodes.count())),
                    actions,
                ]
                .spacing(SPACE),
            );

            if let Some(overview) = &s.overview {
                content = content.push(text(overview));
            }

            series = series.push(
                centered(
                    row![graphic, content.spacing(GAP)]
                        .spacing(GAP)
                        .width(Length::Fill),
                    Some(style::weak),
                )
                .padding(GAP),
            );
        }

        let filter = text_input("Filter...", &self.filter, |value| {
            Message::SeriesList(M::ChangeFilter(value))
        })
        .width(Length::Fill);

        column![
            vertical_space(Length::Shrink),
            centered(row![filter].width(Length::Fill), None),
            series.spacing(GAP2)
        ]
        .spacing(GAP)
    }
}

/// Tokenize a string for filtering.
fn tokenize(input: &str, prefix: bool) -> HashSet<String> {
    let mut output = HashSet::new();

    let mut string = String::new();

    for part in input.split_whitespace() {
        if prefix {
            string.clear();

            for c in part.chars() {
                if !c.is_alphanumeric() {
                    continue;
                }

                string.extend(c.to_lowercase());
                output.insert(string.clone());
            }
        } else {
            string.clear();

            for c in part.chars() {
                if !c.is_alphanumeric() {
                    continue;
                }

                string.extend(c.to_lowercase());
            }

            output.insert(string.clone());
        }
    }

    output
}
