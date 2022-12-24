use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};

use crate::model::{Task, TaskKind};

/// Queue of scheduled actions.
#[derive(Default)]
pub(crate) struct Queue {
    // Pending tasks.
    pending: HashSet<TaskKind>,
    // Items in the download queue.
    data: Vec<Task>,
    /// Test if queue has been locally modified.
    modified: bool,
}

impl Queue {
    /// Test if queue contains a task of the given kind.
    pub(crate) fn contains(&self, kind: &TaskKind) -> bool {
        self.pending.contains(kind)
    }

    /// Get queue data.
    pub(crate) fn data(&self) -> &[Task] {
        &self.data
    }

    /// Remove all matching tasks.
    pub(crate) fn remove_tasks_by<P>(&mut self, mut predicate: P) -> usize
    where
        P: FnMut(&Task) -> bool,
    {
        let mut removed = 0;

        for data in &self.data {
            if predicate(data) {
                let _ = self.pending.remove(&data.kind);
                removed += 1;
            }
        }

        self.data.retain(move |task| !predicate(task));
        self.modified |= removed > 0;
        removed
    }

    pub(crate) fn sort(&mut self) {
        self.data.sort_by(|a, b| a.scheduled.cmp(&b.scheduled));
        self.modified = true;
    }

    /// Take if the queue has been modified.
    #[inline]
    pub(crate) fn take_modified(&mut self) -> bool {
        std::mem::take(&mut self.modified)
    }

    /// Get the next item from the queue.
    pub(crate) fn next_item(&mut self, now: &DateTime<Utc>) -> Option<Task> {
        let scheduled = self.data.last().map(|task| task.scheduled)?;

        if scheduled > *now {
            return None;
        }

        self.data.pop()
    }

    /// Next sleep.
    pub(crate) fn next_sleep(&self, now: &DateTime<Utc>) -> Option<u64> {
        let scheduled = self.data.last()?.scheduled;

        if scheduled + Duration::seconds(1) <= *now {
            return Some(0);
        }

        u64::try_from(scheduled.signed_duration_since(*now).num_seconds().max(0)).ok()
    }

    /// Push a task onto the queue.
    pub(crate) fn push(&mut self, task: Task) -> bool {
        if self.pending.insert(task.kind) {
            self.data.push(task);
            self.modified = true;
            true
        } else {
            false
        }
    }
}
