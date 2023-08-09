use std::collections::BTreeMap;
use std::mem::replace;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::{Etag, RemoteId};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<Etag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_sync: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_modified: Option<DateTime<Utc>>,
}

impl Entry {
    /// Test if entry is empty.
    fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_sync.is_none() && self.last_modified.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Export {
    id: RemoteId,
    #[serde(flatten)]
    entry: Entry,
}

/// Database of synchronization state.
#[derive(Default)]
pub struct Database {
    data: BTreeMap<RemoteId, Entry>,
}

impl Database {
    /// Export the contents of the database.
    pub(crate) fn export(&self) -> impl Iterator<Item = Export> {
        self.data
            .clone()
            .into_iter()
            .map(|(id, entry)| Export { id, entry })
            .filter(|export| !export.entry.is_empty())
    }

    /// Import sync state.
    pub(crate) fn import_push(&mut self, sync: Export) {
        if !sync.entry.is_empty() {
            self.data.insert(sync.id, sync.entry);
        }
    }

    /// Update series last sync.
    #[must_use]
    pub(crate) fn import_last_sync(&mut self, id: RemoteId, now: DateTime<Utc>) -> bool {
        let e = self.data.entry(id).or_default();
        e.last_sync.replace(now) != Some(now)
    }

    /// Update series last modified.
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub(crate) fn update_last_modified(
        &mut self,
        id: RemoteId,
        last_modified: Option<DateTime<Utc>>,
    ) -> bool {
        tracing::trace!("series update last modified");

        let e = self.data.entry(id).or_default();
        replace(&mut e.last_modified, last_modified) != last_modified
    }

    /// Update series.
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub(crate) fn series_update_sync(
        &mut self,
        id: RemoteId,
        now: DateTime<Utc>,
        last_modified: Option<DateTime<Utc>>,
    ) -> bool {
        tracing::trace!("series update sync");

        let e = self.data.entry(id).or_default();
        (e.last_sync.replace(now) != Some(now))
            | (replace(&mut e.last_modified, last_modified) != last_modified)
    }

    /// Insert last etag.
    #[must_use]
    pub(crate) fn update_last_etag(&mut self, id: RemoteId, etag: Option<Etag>) -> bool {
        let e = self.data.entry(id).or_default();

        if e.etag.as_ref() == etag.as_ref() {
            return false;
        }

        e.etag = etag;
        true
    }

    /// Get last sync for the given time series.
    pub(crate) fn last_sync(&self, id: &RemoteId) -> Option<&DateTime<Utc>> {
        self.data.get(id)?.last_sync.as_ref()
    }

    /// Get last modified for the given time series.
    pub(crate) fn last_modified(&self, id: &RemoteId) -> Option<&DateTime<Utc>> {
        self.data.get(id)?.last_modified.as_ref()
    }

    /// Last etag for the given series id.
    pub(crate) fn last_etag(&self, id: &RemoteId) -> Option<&Etag> {
        let entry = self.data.get(id)?;
        entry.etag.as_ref()
    }

    /// Clear all sync data.
    pub(crate) fn clear(&mut self) {
        self.data.clear();
    }
}
