//! [<img alt="github" src="https://img.shields.io/badge/github-udoprog/ontv-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/ontv)
//! [<img alt="crates.io" src="https://img.shields.io/crates/v/ontv.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/ontv)
//! [<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-ontv-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/ontv)
//!
//! Reimagining of my old Python-based CLI application for tracking show
//! progress and what to watch next.
//!
//! Still in the experimental stage. Users beware!
//!
//! <br>
//!
//! ## Features
//!
//! <br>
//!
//! <div align="center">
//! <table>
//! <tr>
//! <td align="center">
//!   <a href="https://raw.githubusercontent.com/udoprog/ontv/main/images/watchnext.png">
//!     <img src="https://raw.githubusercontent.com/udoprog/ontv/main/images/watchnext.png"/>
//!   </a>
//!   <br>
//!   A friendly dashboard of what's next
//! </td>
//!
//! <td align="center">
//!   <a href="https://raw.githubusercontent.com/udoprog/ontv/main/images/scheduled.png">
//!     <img src="https://raw.githubusercontent.com/udoprog/ontv/main/images/scheduled.png"/>
//!   </a>
//!   <br>
//!   Schedule of upcoming shows
//! </td>
//! </tr>
//!
//! <tr>
//! <td align="center">
//!   <a href="https://raw.githubusercontent.com/udoprog/ontv/main/images/history.png">
//!     <img src="https://raw.githubusercontent.com/udoprog/ontv/main/images/history.png"/>
//!   </a>
//!   <br>
//!   Detailed watch history
//! </td>
//!
//! <td align="center">
//!   <a href="https://raw.githubusercontent.com/udoprog/ontv/main/images/git.png">
//!     <img src="https://raw.githubusercontent.com/udoprog/ontv/main/images/git.png"/>
//!   </a>
//!   <br>
//!   Git friendly storage
//! </td>
//! </tr>
//! </table>
//! </div>
//!
//! <br>
//!
//! ## Running ontv in read-only mode
//!
//! If you for some reason want to run ontv in read-only mode you can do that
//! with the `--test` switch. I personally use this during development to make
//! sure I don't accidentally save bad data to my local database.
//!
//! ```text
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
//! ```text
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
//! > `config.yaml` will contain your API keys.
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
//! `sync.yaml` and `queue.yaml`, unless you want to be plagued by frequent
//! changes:
//!
//! ```text
//! /sync.yaml
//! /queue.yaml
//! ```

#![allow(clippy::field_reassign_with_default, clippy::type_complexity)]
#![allow(unused)]

mod api;
mod assets;
mod cache;
mod database;
pub mod import;
pub mod lock;
mod model;
mod queue;
mod search;
mod service;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::runtime::Builder;
use tokio::sync::RwLock;

pub use self::service::Service;

/// Run the GUI application.
pub fn run(service: service::Service) -> Result<()> {
    let addr: SocketAddr = self::web::BIND.parse()?;

    let service = Arc::new(RwLock::new(service));

    Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            tracing::info!("Starting OnTV web service on http://{addr}");

            let listener = TcpListener::bind(addr).await?;
            let future = web::setup(listener, service.clone())?;

            loop {
                tokio::select! {
                    result = future => {
                        result?;
                        tracing::info!("Service shut down");
                        break;
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        })?;

    Ok(())
}
