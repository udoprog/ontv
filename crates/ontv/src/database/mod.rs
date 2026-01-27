use core::str;

use std::collections::HashSet;

use anyhow::{anyhow, Result, Context};
use jiff::Timestamp;
use relative_path::{RelativePath, RelativePathBuf};
use sqll::OpenOptions;
use tokio::sync::Mutex;
use rust_embed::RustEmbed;

use crate::Paths;

#[derive(RustEmbed)]
#[folder = "migrations"]
struct Migrations;

struct Inner {}

/// A database connection.
pub struct Database {
    connection: Mutex<Inner>,
}

impl Database {
    /// Open a database at the given paths prepared for migrations.
    pub fn open(paths: &Paths, memory: bool) -> Result<Self> {
        let mut open = OpenOptions::new();
        open.create().read_write().no_mutex();

        let c = if memory {
            open.open_in_memory()?
        } else {
            open.open(paths.db.as_ref())?
        };

        let count = c.prepare("SELECT COUNT(*) FROM `sqlite_master` WHERE `type` = 'table' AND `name` = 'migrations'")?.next::<i64>()?.unwrap_or(0);

        if count == 0 {
            c.execute(
                "CREATE TABLE `migrations` (
                    `id` TEXT PRIMARY KEY,
                    `applied_at` INTEGER NOT NULL
                )",
            )?;
        }

        let mut applied = HashSet::new();

        for row in c.prepare("SELECT id FROM migrations")?.iter::<String>() {
            applied.insert(RelativePathBuf::from(row?));
        }

        let mut to_run = Vec::new();

        for id in Migrations::iter() {
            let path = RelativePath::new(id.as_ref());

            if !matches!(path.extension(), Some("sql")) {
                continue;
            }

            if !applied.contains(path) {
                to_run.push(path.to_owned());
            }
        }

        to_run.sort();

        for path in to_run {
            tracing::info!(?path, "Applying migration");

            let sql = Migrations::get(path.as_str()).expect("embedded migration").data;
            let sql = str::from_utf8(&sql)?;

            c.execute("BEGIN TRANSACTION")?;
            c.execute(sql).with_context(|| anyhow!("migration {path}"))?;
            c.execute("COMMIT TRANSACTION")?;

            let mut insert = c.prepare(
                "INSERT INTO migrations (id, applied_at) VALUES (?, ?)",
            )?;

            let now = Timestamp::now();
            insert.execute((path.as_str(), now.as_millisecond()))?;
        }

        Ok(Self {
            connection: Mutex::new(Inner {}),
        })
    }
}
