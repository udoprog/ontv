use std::collections::{btree_map, BTreeMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{Etag, RemoteSeriesId, SeriesId};

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
    id: Uuid,
    remote: RemoteSeriesId,
    #[serde(flatten)]
    entry: Entry,
}

/// Database of synchronization state.
#[derive(Default)]
pub struct Database {
    data: BTreeMap<(Uuid, RemoteSeriesId), Entry>,
}

impl Database {
    /// Export the contents of the database.
    pub(crate) fn export(&self) -> impl Iterator<Item = Export> {
        self.data
            .clone()
            .into_iter()
            .map(|((id, remote), entry)| Export { id, remote, entry })
            .filter(|export| !export.entry.is_empty())
    }

    /// Import sync state.
    pub(crate) fn import_push(&mut self, sync: Export) {
        if !sync.entry.is_empty() {
            self.data.insert((sync.id, sync.remote), sync.entry);
        }
    }

    /// Update series last sync.
    #[must_use]
    pub(crate) fn import_last_sync(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
        now: &DateTime<Utc>,
    ) -> bool {
        let e = self.data.entry((*series_id.id(), *remote_id)).or_default();
        e.last_sync.replace(*now) != Some(*now)
    }

    /// Update series last modified.
    #[must_use]
    pub(crate) fn update_last_modified(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
        last_modified: Option<&DateTime<Utc>>,
    ) -> bool {
        let e = self.data.entry((*series_id.id(), *remote_id)).or_default();

        if let Some(last_modified) = last_modified {
            e.last_modified.replace(*last_modified) != Some(*last_modified)
        } else {
            e.last_modified.take().is_some()
        }
    }

    /// Update series.
    #[must_use]
    pub(crate) fn series_update_sync(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
        now: &DateTime<Utc>,
        last_modified: Option<&DateTime<Utc>>,
    ) -> bool {
        let e = self.data.entry((*series_id.id(), *remote_id)).or_default();
        let mut updated = e.last_sync.replace(*now) != Some(*now);

        if let Some(last_modified) = last_modified {
            updated |= e.last_modified.replace(*last_modified) != Some(*last_modified);
        } else {
            updated |= e.last_modified.take().is_some();
        }

        updated
    }

    /// Insert last etag.
    #[must_use]
    pub(crate) fn update_last_etag(
        &mut self,
        id: &SeriesId,
        remote_id: &RemoteSeriesId,
        etag: Etag,
    ) -> bool {
        match self.data.entry((*id.id(), *remote_id)) {
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
        self.data.get(&(*id.id(), *remote_id))?.last_sync.as_ref()
    }

    /// Get last modified for the given time series.
    pub(crate) fn last_modified(
        &self,
        id: &SeriesId,
        remote_id: &RemoteSeriesId,
    ) -> Option<&DateTime<Utc>> {
        self.data
            .get(&(*id.id(), *remote_id))?
            .last_modified
            .as_ref()
    }

    /// Last etag for the given series id.
    pub(crate) fn last_etag(&self, id: &SeriesId, remote_id: &RemoteSeriesId) -> Option<&Etag> {
        let entry = self.data.get(&(*id.id(), *remote_id))?;
        entry.etag.as_ref()
    }

    /// Clear last sync for the given time series.
    pub(crate) fn clear_last_sync(&mut self, id: &SeriesId, remote_id: &RemoteSeriesId) -> bool {
        match self.data.entry((*id.id(), *remote_id)) {
            btree_map::Entry::Vacant(..) => false,
            btree_map::Entry::Occupied(mut e) => {
                let non_empty = !e.get().last_sync.is_none();
                e.get_mut().last_sync = None;

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
