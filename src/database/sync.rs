use std::collections::{btree_map, BTreeMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{Etag, RemoteSeriesId, SeriesId};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct Entry {
    #[serde(skip_serializing_if = "Option::is_none")]
    etag: Option<Etag>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "crate::model::btree_as_vec"
    )]
    last_sync: BTreeMap<RemoteSeriesId, DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_modified: Option<(RemoteSeriesId, DateTime<Utc>)>,
}

impl Entry {
    /// Test if entry is empty.
    fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_sync.is_empty() && self.last_modified.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Export {
    id: Uuid,
    #[serde(flatten)]
    entry: Entry,
}

/// Database of synchronization state.
#[derive(Default)]
pub struct Database {
    data: BTreeMap<Uuid, Entry>,
}

impl Database {
    /// Export the contents of the database.
    pub(crate) fn export(&self) -> impl ExactSizeIterator<Item = Export> {
        self.data
            .clone()
            .into_iter()
            .map(|(id, entry)| Export { id, entry })
    }

    /// Import sync state.
    pub(crate) fn import_push(&mut self, sync: Export) {
        self.data.insert(sync.id, sync.entry);
    }

    /// Update series last sync.
    #[must_use]
    pub(crate) fn series_last_sync(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
        now: DateTime<Utc>,
    ) -> bool {
        let e = self.data.entry(*series_id.id()).or_default();
        e.last_sync.insert(*remote_id, now) != Some(now)
    }

    /// Update series last modified.
    #[must_use]
    pub(crate) fn series_last_modified(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
        last_modified: DateTime<Utc>,
    ) -> bool {
        let e = self.data.entry(*series_id.id()).or_default();
        e.last_modified.replace((*remote_id, last_modified)) != Some((*remote_id, last_modified))
    }

    /// Insert last etag.
    #[must_use]
    pub(crate) fn series_last_etag(&mut self, series_id: &SeriesId, etag: Etag) -> bool {
        match self.data.entry(*series_id.id()) {
            btree_map::Entry::Vacant(e) => {
                let e = e.insert(Entry::default());
                e.etag = Some(etag);
                true
            }
            btree_map::Entry::Occupied(mut e) => {
                if e.get().etag.as_ref() == Some(&etag) {
                    return false;
                }

                e.get_mut().etag = Some(etag);
                true
            }
        }
    }

    /// Get last sync for the given time series.
    pub(crate) fn last_sync(
        &self,
        id: &SeriesId,
        remote_id: &RemoteSeriesId,
    ) -> Option<&DateTime<Utc>> {
        let entry = self.data.get(id.id())?;
        entry.last_sync.get(remote_id)
    }

    /// Last modified timestamp.
    pub(crate) fn last_modified(
        &self,
        id: &SeriesId,
        remote_id: &RemoteSeriesId,
    ) -> Option<&DateTime<Utc>> {
        let entry = self.data.get(id.id())?;
        let (expected, last_modified) = entry.last_modified.as_ref()?;

        if expected != remote_id {
            return None;
        }

        Some(last_modified)
    }

    /// Last etag for the given series id.
    pub(crate) fn last_etag(&self, series_id: &SeriesId) -> Option<&Etag> {
        let entry = self.data.get(series_id.id())?;
        entry.etag.as_ref()
    }

    /// Clear last sync for the given time series.
    pub(crate) fn clear_last_sync(&mut self, series_id: &SeriesId) -> bool {
        match self.data.entry(*series_id.id()) {
            btree_map::Entry::Vacant(..) => false,
            btree_map::Entry::Occupied(mut e) => {
                let non_empty = !e.get().last_sync.is_empty();
                e.get_mut().last_sync.clear();

                if e.get().is_empty() {
                    e.remove();
                    true
                } else {
                    non_empty
                }
            }
        }
    }
}
