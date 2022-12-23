#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

mod api;
mod application;
mod assets;
mod cache;
mod component;
mod comps;
pub mod import;
mod message;
mod model;
mod page;
mod params;
mod search;
mod service;
mod state;
pub mod style;
mod utils;

pub use self::service::Service;

/// Run the GUI application.
pub fn run(service: service::Service) -> anyhow::Result<()> {
    use iced::Application;
    let mut settings = iced::Settings::with_flags(application::Flags { service });
    settings.exit_on_close_request = false;
    application::Application::run(settings)?;
    Ok(())
}
