#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

use anyhow::Result;
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
}

pub fn main() -> Result<()> {
    pretty_env_logger::init();
    let mut service = ontv::Service::new()?;

    let opts = Opts::try_parse()?;

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
