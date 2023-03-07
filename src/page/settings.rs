use crate::prelude::*;

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
                s.service.clear_last_sync();
            }
        }
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, s: &State) -> Element<'static, Message> {
        let config = s.service.config();

        let mut page = w::Column::new();

        page = page.push([ThemeType::Light, ThemeType::Dark].iter().fold(
            w::Column::new().push(w::text("Theme:")).spacing(SPACE),
            |column, theme| {
                column.push(w::radio(
                    format!("{theme:?}"),
                    *theme,
                    Some(config.theme),
                    Message::ThemeChanged,
                ))
            },
        ));

        page = page.push(
            w::Column::new()
                .push(w::text("TheTVDB Legacy API Key:"))
                .push(w::text_input(
                    "Key...",
                    &config.tvdb_legacy_apikey,
                    |value| Message::TvdbLegacyApiKeyChange(value),
                ))
                .spacing(SPACE),
        );

        page = page.push(
            w::Column::new()
                .push(w::text("TheMovieDB API Key:"))
                .push(w::text_input("Key...", &config.tmdb_api_key, |value| {
                    Message::TmdbApiKeyChange(value)
                }))
                .spacing(SPACE),
        );

        page = page.push(w::horizontal_rule(1));

        page = page.push("Clear last sync times in database:");
        page = page.push(w::button("Clear last sync").on_press(Message::ClearLastSync));

        default_container(page.spacing(GAP).padding(GAP)).into()
    }
}
