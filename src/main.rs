#![cfg_attr(all(not(feature = "cli"), windows), windows_subsystem = "windows")]

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
struct Opts {
    /// Import watch history from trakt.
    #[arg(long, name = "path")]
    import_trakt_watched: Option<PathBuf>,
    /// Only import a show matching the given filter.
    #[arg(long, name = "string")]
    import_filter: Option<String>,
    /// Override any existing watch history.
    #[arg(long)]
    import_remove: bool,
    /// Import any missing shows encountered.
    #[arg(long)]
    import_missing: bool,
    /// Ensure that import history is saved.
    #[arg(long)]
    import_test: bool,
    /// Don't save anything.
    #[arg(long)]
    test: bool,
    /// Configuration directory.
    #[arg(long, name = "config")]
    config: Option<PathBuf>,
    /// Print project paths.
    #[arg(long)]
    paths: bool,
}

pub fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let opts = Opts::try_parse()?;

    let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
        .context("missing project dirs")?;

    let config = match &opts.config {
        Some(config) => config,
        None => dirs.config_dir(),
    };

    let cache = dirs.cache_dir();

    if opts.paths {
        tracing::info!("config: {}", config.display());
        tracing::info!("cache: {}", cache.display());
    }

    let mut service = ontv::Service::new(config, cache)?;

    if opts.test {
        service.do_not_save();
    }

    if let Some(path) = opts.import_trakt_watched {
        ontv::import::import_trakt_watched(
            &mut service,
            &path,
            opts.import_filter.as_deref(),
            opts.import_remove,
            opts.import_missing,
        )?;
    }

    ontv::run(service)?;
    Ok(())
}
