use std::collections::{HashMap, VecDeque};

use arrayvec::ArrayVec;
use chrono::{DateTime, Duration, Utc};

use crate::prelude::{RemoteMovieId, RemoteSeriesId, SeriesId, TaskId};

/// Number of milliseconds of delay to add by default to scheduled tasks.
const DELAY_MILLIS: i64 = 5000;
// Soft capacity, that some processes which might add a lot of stuff can check.
const CAPACITY: usize = 50;

/// The current task status.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TaskStatus {
    /// Task is pending.
    Pending,
    /// Task is currently running.
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TaskRef {
    /// Task to download series data.
    Series { series_id: SeriesId },
    /// Task to add a series by a remote identifier.
    RemoteSeries { remote_id: RemoteSeriesId },
    /// Task to add download a movie by a remote identifier.
    RemoteMovie { remote_id: RemoteMovieId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TaskKind {
    /// Check for updates.
    CheckForUpdates {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
    },
    /// Task to download series data.
    DownloadSeries {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
        last_modified: Option<DateTime<Utc>>,
        force: bool,
    },
    /// Task to add a series by a remote identifier.
    DownloadSeriesByRemoteId { remote_id: RemoteSeriesId },
    /// Task to add download a movie by a remote identifier.
    #[allow(unused)]
    DownloadMovieByRemoteId { remote_id: RemoteMovieId },
}

impl TaskKind {
    pub(crate) fn task_ids(&self) -> ArrayVec<TaskRef, 2> {
        let mut ids = ArrayVec::new();

        match *self {
            TaskKind::CheckForUpdates {
                series_id,
                remote_id,
                ..
            } => {
                ids.push(TaskRef::Series { series_id });
                ids.push(TaskRef::RemoteSeries { remote_id });
            }
            TaskKind::DownloadSeries {
                series_id,
                remote_id,
                ..
            } => {
                ids.push(TaskRef::Series { series_id });
                ids.push(TaskRef::RemoteSeries { remote_id });
            }
            TaskKind::DownloadSeriesByRemoteId { remote_id, .. } => {
                ids.push(TaskRef::RemoteSeries { remote_id });
            }
            TaskKind::DownloadMovieByRemoteId { remote_id } => {
                ids.push(TaskRef::RemoteMovie { remote_id });
            }
        }

        ids
    }
}

/// A task in a queue.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct Task {
    /// The identifier of the task.
    pub(crate) id: TaskId,
    /// The kind of the task.
    pub(crate) kind: TaskKind,
    /// When the task is scheduled for.
    pub(crate) scheduled: Option<DateTime<Utc>>,
}

impl Task {
    /// Test if task involves the given series.
    pub(crate) fn is_series(&self, id: &SeriesId) -> bool {
        match &self.kind {
            TaskKind::DownloadSeries { series_id, .. } => *series_id == *id,
            TaskKind::CheckForUpdates { series_id, .. } => *series_id == *id,
            TaskKind::DownloadSeriesByRemoteId { .. } => false,
            TaskKind::DownloadMovieByRemoteId { .. } => false,
        }
    }
}

/// Queue of scheduled actions.
#[derive(Default)]
pub(crate) struct Queue {
    /// Pending tasks.
    status: HashMap<TaskId, TaskStatus>,
    /// Blocked tasks.
    task_ids: HashMap<TaskRef, TaskId>,
    /// Items in the download queue.
    data: VecDeque<Task>,
    /// Collection of running tasks.
    running: Vec<Task>,
    /// Test if queue has been locally modified.
    modified: bool,
}

impl Queue {
    /// Test if queue contains a task of the given kind.
    #[inline]
    pub(crate) fn status(&self, id: TaskRef) -> Option<TaskStatus> {
        let id = self.task_ids.get(&id)?;
        self.status.get(id).copied()
    }

    /// Mark the given task kind as completed, returns `true` if the task was
    /// present in the queue.
    #[inline]
    pub(crate) fn complete(&mut self, task: &Task) -> Option<TaskStatus> {
        self.running.retain(|t| t.id != task.id);
        let status = self.status.remove(&task.id)?;

        for id in task.kind.task_ids() {
            let _ = self.task_ids.remove(&id);
        }

        Some(status)
    }

    /// Running tasks.
    #[inline]
    pub(crate) fn running(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.running.iter()
    }

    /// Get queue data.
    #[inline]
    pub(crate) fn pending(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.data.iter()
    }

    /// Remove all matching tasks.
    pub(crate) fn remove_tasks_by<P>(&mut self, mut predicate: P) -> usize
    where
        P: FnMut(&Task) -> bool,
    {
        let mut removed = 0;

        for data in &self.data {
            if predicate(data) {
                let _ = self.status.remove(&data.id);

                for task_id in data.kind.task_ids() {
                    let _ = self.task_ids.remove(&task_id);
                }

                removed += 1;
            }
        }

        self.data.retain(move |task| !predicate(task));
        self.modified |= removed > 0;
        removed
    }

    /// Take if the queue has been modified.
    #[inline]
    pub(crate) fn take_modified(&mut self) -> bool {
        std::mem::take(&mut self.modified)
    }

    /// Get the next item from the queue.
    pub(crate) fn next_task(
        &mut self,
        now: &DateTime<Utc>,
        timed_out: Option<TaskId>,
    ) -> Option<Task> {
        let task = self.data.front()?;

        if !matches!(timed_out, Some(id) if id == task.id)
            && task.scheduled.map(|s| s > *now).unwrap_or_default()
        {
            return None;
        }

        let task = self.data.pop_front()?;
        self.status.insert(task.id, TaskStatus::Running);
        self.running.push(task.clone());
        Some(task)
    }

    /// Next sleep.
    pub(crate) fn next_sleep(&self, now: &DateTime<Utc>) -> Option<(u64, TaskId)> {
        let task = self.data.front()?;
        let id = task.id;

        let Some(scheduled) = &task.scheduled else {
            return Some((0, id));
        };

        let seconds =
            u64::try_from(scheduled.signed_duration_since(*now).num_seconds().max(0)).ok()?;

        Some((seconds, id))
    }

    /// Push without delay.
    pub(crate) fn push_without_delay(&mut self, kind: TaskKind) -> bool {
        let task_ids = kind.task_ids();

        for task_id in &task_ids {
            if self.task_ids.contains_key(task_id) {
                return false;
            }
        }

        let id = TaskId::random();
        self.status.insert(id, TaskStatus::Pending);

        for task_id in task_ids {
            self.task_ids.insert(task_id, id);
        }

        self.data.push_front(Task {
            id,
            kind,
            scheduled: None,
        });

        self.modified = true;
        true
    }

    /// Push a task onto the queue.
    pub(crate) fn push(&mut self, kind: TaskKind) -> bool {
        let task_ids = kind.task_ids();

        for task_id in &task_ids {
            if self.task_ids.contains_key(task_id) {
                return false;
            }
        }

        let id = TaskId::random();

        for task_id in task_ids {
            self.task_ids.insert(task_id, id);
        }

        self.status.insert(id, TaskStatus::Pending);

        let scheduled = self
            .data
            .iter()
            .flat_map(|t| t.scheduled)
            .next_back()
            .unwrap_or_else(Utc::now)
            + Duration::milliseconds(DELAY_MILLIS);

        self.data.push_back(Task {
            id,
            kind,
            scheduled: Some(scheduled),
        });

        self.modified = true;
        true
    }

    /// Check if queue is at its soft capacity.
    pub(crate) fn at_soft_capacity(&self) -> bool {
        self.data.len() + self.running.len() >= CAPACITY
    }
}
