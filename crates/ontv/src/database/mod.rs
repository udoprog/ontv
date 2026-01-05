use anyhow::Result;
use tokio::sync::Mutex;

use crate::Paths;

mod migrations;

struct Inner {}

/// A database connection.
pub struct Database {
    connection: Mutex<Inner>,
}

impl Database {
    /// Open a database at the given paths prepared for migrations.
    pub fn migrations(paths: &Paths) -> Result<Self> {
        Ok(Self {
            connection: Mutex::new(Inner {}),
        })
    }
}
