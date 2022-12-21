use iced::widget::{column, radio, text, text_input, Column};
use iced::Command;

use crate::assets::Assets;
use crate::message::Message;
use crate::model::ThemeType;
use crate::params::{default_container, GAP, SPACE};
use crate::service::Service;

#[derive(Default)]
pub(crate) struct State;

/// Message generated by settings page.
#[derive(Debug, Clone)]
pub(crate) enum M {
    ThemeChanged(ThemeType),
    TvdbLegacyApiKeyChange(String),
    TmdbApiKeyChange(String),
}

impl State {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, _: &Service, _: &mut Assets) {}

    /// Handle theme change.
    pub(crate) fn update(&mut self, service: &mut Service, message: M) -> Command<Message> {
        match message {
            M::ThemeChanged(theme) => {
                service.set_theme(theme);
            }
            M::TvdbLegacyApiKeyChange(string) => {
                service.set_tvdb_legacy_api_key(string);
            }
            M::TmdbApiKeyChange(string) => {
                service.set_tmdb_api_key(string);
            }
        }

        Command::none()
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, service: &Service) -> Column<'static, Message> {
        let config = service.config();

        let mut page = Column::new();

        page = page.push([ThemeType::Light, ThemeType::Dark].iter().fold(
            column![text("Theme:")].spacing(SPACE),
            |column, theme| {
                column.push(radio(
                    format!("{:?}", theme),
                    *theme,
                    Some(config.theme),
                    |theme| Message::Settings(M::ThemeChanged(theme)),
                ))
            },
        ));

        page = page.push(
            column![
                text("TheTVDB Legacy API Key:"),
                text_input("Key...", &config.tvdb_legacy_apikey, |value| {
                    Message::Settings(M::TvdbLegacyApiKeyChange(value))
                }),
            ]
            .spacing(SPACE),
        );

        page = page.push(
            column![
                text("TheMovieDB API Key:"),
                text_input("Key...", &config.tmdb_api_key, |value| {
                    Message::Settings(M::TmdbApiKeyChange(value))
                }),
            ]
            .spacing(SPACE),
        );

        default_container(page.spacing(GAP).padding(GAP))
    }
}
