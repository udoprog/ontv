use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::model::{Task, TaskId, TaskKind};

/// Number of milliseconds of delay to add by default to scheduled tasks.
const DELAY_MILLIS: i64 = 250;
// Soft capacity, that some processes which might add a lot of stuff can check.
const CAPACITY: usize = 50;

#[derive(Debug, Clone)]
pub(crate) struct TaskRunning {
    pub(crate) id: Uuid,
    pub(crate) kind: TaskId,
}

/// The current task status.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TaskStatus {
    /// Task is pending.
    Pending,
    /// Task is currently running.
    Running,
}

/// Queue of scheduled actions.
#[derive(Default)]
pub(crate) struct Queue {
    /// Pending tasks.
    status: HashMap<TaskId, TaskStatus>,
    /// Items in the download queue.
    data: VecDeque<Task>,
    /// Collection of running tasks.
    running: Vec<TaskRunning>,
    /// Test if queue has been locally modified.
    modified: bool,
}

impl Queue {
    /// Test if queue contains a task of the given kind.
    #[inline]
    pub(crate) fn status(&self, id: &TaskId) -> Option<TaskStatus> {
        self.status.get(id).copied()
    }

    /// Mark the given task kind as completed, returns `true` if the task was
    /// present in the queue.
    #[inline]
    pub(crate) fn complete(&mut self, task: &Task) -> Option<TaskStatus> {
        self.running.retain(|t| t.id != task.id);
        self.status.remove(&task.kind.id())
    }

    /// Running tasks.
    #[inline]
    pub(crate) fn running(&self) -> impl ExactSizeIterator<Item = &TaskRunning> {
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
                let _ = self.status.remove(&data.kind.id());
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
        timed_out: Option<Uuid>,
    ) -> Option<Task> {
        let task = self.data.front()?;

        if !matches!(timed_out, Some(id) if id == task.id)
            && task.scheduled.map(|s| s > *now).unwrap_or_default()
        {
            return None;
        }

        let task = self.data.pop_front()?;
        self.status.insert(task.kind.id(), TaskStatus::Running);

        self.running.push(TaskRunning {
            id: task.id,
            kind: task.kind.id(),
        });

        Some(task)
    }

    /// Next sleep.
    pub(crate) fn next_sleep(&self, now: &DateTime<Utc>) -> Option<(u64, Uuid)> {
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
        let id = kind.id();

        if self.status.contains_key(&id) {
            return false;
        }

        self.status.insert(id, TaskStatus::Pending);

        self.data.push_back(Task {
            id: Uuid::new_v4(),
            kind,
            scheduled: None,
        });

        self.modified = true;
        true
    }

    /// Push a task onto the queue.
    pub(crate) fn push(&mut self, kind: TaskKind) -> bool {
        let id = kind.id();

        if self.status.contains_key(&id) {
            return false;
        }

        let scheduled = self
            .data
            .iter()
            .flat_map(|t| t.scheduled)
            .next_back()
            .unwrap_or_else(Utc::now)
            + Duration::milliseconds(DELAY_MILLIS);

        self.status.insert(id, TaskStatus::Pending);

        self.data.push_back(Task {
            id: Uuid::new_v4(),
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
