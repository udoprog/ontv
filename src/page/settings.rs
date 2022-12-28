use iced::widget::{button, horizontal_rule, radio, text, text_input, Column};
use iced::Element;

use crate::model::ThemeType;
use crate::params::{default_container, GAP, SPACE};

use crate::state::State;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    ThemeChanged(ThemeType),
    TvdbLegacyApiKeyChange(String),
    TmdbApiKeyChange(String),
    ClearLastSync,
}

#[derive(Default)]
pub(crate) struct Settings;

impl Settings {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, _: &mut State) {}

    /// Handle theme change.
    pub(crate) fn update(&mut self, s: &mut State, message: Message) {
        match message {
            Message::ThemeChanged(theme) => {
                s.service.set_theme(theme);
            }
            Message::TvdbLegacyApiKeyChange(string) => {
                s.service.set_tvdb_legacy_api_key(string);
            }
            Message::TmdbApiKeyChange(string) => {
                s.service.set_tmdb_api_key(string);
            }
            Message::ClearLastSync => {
                for s in s.service.all_series_mut() {
                    s.last_sync.clear();
                }
            }
        }
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let config = s.service.config();

        let mut page = Column::new();

        page = page.push([ThemeType::Light, ThemeType::Dark].iter().fold(
            Column::new().push(text("Theme:")).spacing(SPACE),
            |column, theme| {
                column.push(radio(
                    format!("{theme:?}"),
                    *theme,
                    Some(config.theme),
                    Message::ThemeChanged,
                ))
            },
        ));

        page = page.push(
            Column::new()
                .push(text("TheTVDB Legacy API Key:"))
                .push(text_input("Key...", &config.tvdb_legacy_apikey, |value| {
                    Message::TvdbLegacyApiKeyChange(value)
                }))
                .spacing(SPACE),
        );

        page = page.push(
            Column::new()
                .push(text("TheMovieDB API Key:"))
                .push(text_input("Key...", &config.tmdb_api_key, |value| {
                    Message::TmdbApiKeyChange(value)
                }))
                .spacing(SPACE),
        );

        page = page.push(horizontal_rule(1));

        page = page.push("Clear last sync times in database:");
        page = page.push(button("Clear last sync").on_press(Message::ClearLastSync));

        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}
