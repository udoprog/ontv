#![cfg_attr(all(not(feature = "cli"), windows), windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

const DEFUALT_FILTER: &str = "ontv=info";

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
    let builder = EnvFilter::builder().with_default_directive(LevelFilter::INFO.into());
    let env_filter;

    if let Ok(log) = env::var("ONTV_LOG") {
        env_filter = builder.parse(log).context("parsing ONTV_LOG")?;
    } else {
        env_filter = builder
            .parse(DEFUALT_FILTER)
            .context("parsing default log filter")?;
    }

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let Some(_lock) = ontv::lock::try_global_lock("se.tedro.OnTV")? else {
        tracing::error!("Failed to lock process, it's possible multiple processes are running",);
        return Ok(());
    };

    let opts = Opts::try_parse()?;

    let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
        .context("missing project dirs")?;

    let config = match &opts.config {
        Some(config) => config,
        None => dirs.config_dir(),
    };

    let cache = dirs.cache_dir();

    if opts.paths {
        tracing::info!("Config: {}", config.display());
        tracing::info!("Cache: {}", cache.display());
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
