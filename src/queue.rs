use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::model::{Task, TaskFinished, TaskKind};

const DELAY_SECONDS: i64 = 2500;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TaskRunning {
    pub(crate) id: Uuid,
    pub(crate) kind: TaskKind,
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
    status: HashMap<TaskKind, TaskStatus>,
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
    pub(crate) fn status(&self, kind: &TaskKind) -> Option<TaskStatus> {
        self.status.get(kind).copied()
    }

    /// Mark the given task kind as completed, returns `true` if the task was
    /// present in the queue.
    #[inline]
    pub(crate) fn complete(&mut self, task: &Task) -> Option<TaskStatus> {
        self.running.retain(|t| t.id != task.id);
        self.status.remove(&task.kind)
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
                let _ = self.status.remove(&data.kind);
                removed += 1;
            }
        }

        self.data.retain(move |task| !predicate(task));
        self.modified |= removed > 0;
        removed
    }

    /// Sort an item.
    pub(crate) fn sort(&mut self) {
        self.data.rotate_right(self.data.as_slices().1.len());
        debug_assert!(self.data.as_slices().1.is_empty());
        self.data
            .as_mut_slices()
            .0
            .sort_by(|a, b| a.scheduled.cmp(&b.scheduled));
        self.modified = true;
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

        if !matches!(timed_out, Some(id) if id == task.id) && task.scheduled > *now {
            return None;
        }

        let task = self.data.pop_front()?;
        self.status.insert(task.kind, TaskStatus::Running);

        self.running.push(TaskRunning {
            id: task.id,
            kind: task.kind,
        });

        Some(task)
    }

    /// Next sleep.
    pub(crate) fn next_sleep(&self, now: &DateTime<Utc>) -> Option<(u64, Uuid)> {
        let task = self.data.front()?;
        let seconds = u64::try_from(
            task.scheduled
                .signed_duration_since(*now)
                .num_seconds()
                .max(0),
        )
        .ok()?;
        let id = task.id;
        Some((seconds, id))
    }

    /// Push a task onto the queue.
    pub(crate) fn push(&mut self, kind: TaskKind, finished: Option<TaskFinished>) -> bool {
        if self.status.contains_key(&kind) {
            return false;
        }

        let scheduled = self
            .data
            .back()
            .map(|t| t.scheduled)
            .unwrap_or_else(Utc::now)
            + Duration::milliseconds(DELAY_SECONDS);

        self.status.insert(kind, TaskStatus::Pending);

        self.data.push_back(Task {
            id: Uuid::new_v4(),
            scheduled,
            kind,
            finished,
        });

        self.modified = true;
        true
    }

    /// Push a raw task without performing scheduling.
    pub(crate) fn push_raw(&mut self, task: Task) {
        if self.status.contains_key(&task.kind) {
            return;
        }

        self.status.insert(task.kind, TaskStatus::Pending);
        self.data.push_back(task);
        self.modified = true;
    }
}
