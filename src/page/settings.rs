use iced::widget::{column, radio, text, text_input, Column};
use iced::Command;
use serde::{Deserialize, Serialize};

use crate::assets::Assets;
use crate::message::{Message, ThemeType};
use crate::params::{default_container, GAP, SPACE};
use crate::service::Service;

/// Message generated by settings page.
#[derive(Debug, Clone)]
pub(crate) enum M {
    /// Request to change theme.
    ThemeChanged(ThemeType),
    /// Legacy API key changed.
    ThetvdbLegacyApiChanged(String),
}

/// The state for the settings page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct State {
    #[serde(default)]
    pub(crate) theme: ThemeType,
    #[serde(default)]
    pub(crate) thetvdb_legacy_apikey: String,
}

impl Default for State {
    #[inline]
    fn default() -> Self {
        Self {
            theme: ThemeType::Dark,
            thetvdb_legacy_apikey: String::new(),
        }
    }
}

impl State {
    /// Prepare data that is needed for the view.
    pub(crate) fn prepare(&mut self, _: &Service, _: &mut Assets) {}

    /// Handle theme change.
    pub(crate) fn update(&mut self, service: &mut Service, message: M) -> Command<Message> {
        match message {
            M::ThemeChanged(theme) => {
                self.theme = theme;
            }
            M::ThetvdbLegacyApiChanged(string) => {
                self.thetvdb_legacy_apikey = string;
            }
        }

        service.set_config(self.clone());
        Command::none()
    }

    /// Generate the view for the settings page.
    pub(crate) fn view(&self, _: &Assets) -> Column<'static, Message> {
        let choose_theme = [ThemeType::Light, ThemeType::Dark].iter().fold(
            column![text("Theme:")].spacing(SPACE),
            |column, theme| {
                column.push(radio(
                    format!("{:?}", theme),
                    *theme,
                    Some(self.theme),
                    |theme| Message::Settings(M::ThemeChanged(theme)),
                ))
            },
        );

        let thetvdb_legacy_apikey = column![
            text("TheTVDB Legacy API Key:"),
            text_input("Key...", &self.thetvdb_legacy_apikey, |value| {
                Message::Settings(M::ThetvdbLegacyApiChanged(value))
            }),
        ]
        .spacing(SPACE);

        let page = column![choose_theme, thetvdb_legacy_apikey]
            .spacing(GAP)
            .padding(GAP);

        default_container(page)
    }
}
