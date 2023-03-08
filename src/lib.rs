//! Reimagining of my old Python-based CLI application for tracking show
//! progress and what to watch next.
//!
//! Still in the experimental stage. Users beware!
//!
//! [![splash](https://raw.githubusercontent.com/udoprog/ontv/main/images/splash.png)](https://github.com/udoprog/ontv)
//!
//! <br>
//!
//! ## Running ontv in read-only mode
//!
//! If you for some reason want to run ontv in read-only mode you can do that
//! with the `--test` switch. I personally use this during development to make
//! sure I don't accidentally save bad data to my local database.
//!
//! ```
//! $ RUST_LOG=ontv=debug ontv --test
//! ```
//!
//! <br>
//!
//! ## Importing history from trakt.tv
//!
//! You must run the application at least once, and go into `Settings` to
//! configure your themoviedb.com API key. Unfortunately I cannot help you with
//! this.
//!
//! Next you'll need to export your existing history it using [this very helpful
//! service by Darek Kay](https://darekkay.com/blog/trakt-tv-backup/).
//!
//! After you've unpacked the file, import the history by starting `ontv` like
//! this:
//!
//! ```
//! $ RUST_LOG=ontv=debug ontv --import-trakt-watched C:\Downloads\watched_shows.txt --import-missing
//! ```
//!
//! The process is incremental, so don't worry if you have to abort it. If any
//! episode already has a watch history it will simply skip over that episode.
//!
//! This will take a while, so go get a ☕.
//!
//! <br>
//!
//! ## Storing your database in git
//!
//! > **Make sure that whatever repository you're using is private**, since
//! > `config.json` will contain your API keys.
//!
//! OnTV is designed to store its state in a human-readable, filesystem-friendly
//! text format, and will probably continue to do so until it turns out to not
//! be a great idea any longer.
//!
//! If you want to store the configuration directory in git you'll have to find
//! them first:
//!
//! * Windows: `%APPDATA%/setbac/ontv/config`
//! * Linux: `~/.config/ontv` (I think).
//!
//! After this, you'll want to use a `.gitignore` file which excludes
//! `sync.json` and `queue.json`, unless you want to be plagued by frequent
//! changes:
//!
//! ```
//! /sync.json
//! /queue.json
//! ```

#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

mod api;
mod application;
mod assets;
mod cache;
mod commands;
mod component;
mod comps;
mod context;
mod database;
mod error;
mod history;
pub mod import;
mod model;
mod page;
mod params;
mod queue;
mod search;
mod service;
mod state;
pub mod style;
mod utils;

pub use self::service::Service;

mod prelude {
    pub(crate) use anyhow::{anyhow, Context, Result};
    pub(crate) use chrono::Utc;
    pub(crate) use iced::alignment::Horizontal;
    pub(crate) use iced::widget as w;
    pub(crate) use iced::{theme, Alignment, Element, Length};
    pub(crate) use uuid::Uuid;

    pub(crate) use crate::commands::Commands;
    pub(crate) use crate::component::*;
    pub(crate) use crate::comps;
    pub(crate) use crate::context::{Ctxt, CtxtRef};
    pub(crate) use crate::error::{ErrorId, ErrorInfo};
    pub(crate) use crate::history::Page;
    pub(crate) use crate::model::*;
    pub(crate) use crate::params::*;
    pub(crate) use crate::state::State;
    pub(crate) use crate::style;
}

/// Run the GUI application.
pub fn run(service: service::Service) -> anyhow::Result<()> {
    use iced::Application;
    let mut settings = iced::Settings::with_flags(application::Flags { service });
    settings.exit_on_close_request = false;
    application::Application::run(settings)?;
    Ok(())
}
