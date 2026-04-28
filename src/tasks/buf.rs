use std::fmt;
use std::future::Future;

use iced::Task;
use iced_futures::MaybeSend;

use crate::tasks::Tasks;

/// A command buffer used for an application.
pub struct TasksBuf<M> {
    tasks: Vec<Task<M>>,
}

impl<T> TasksBuf<T>
where
    T: 'static,
{
    /// Build a single command out of the command buffer.
    #[inline]
    pub(crate) fn build(&mut self) -> Task<T> {
        if self.tasks.is_empty() {
            return Task::none();
        }

        Task::batch(self.tasks.drain(..))
    }
}

impl<M> Tasks<M> for TasksBuf<M>
where
    M: 'static + MaybeSend,
{
    #[inline]
    fn by_ref(&mut self) -> impl Tasks<M> + '_ {
        self
    }

    #[inline]
    fn perform<F>(&mut self, future: F, map: impl 'static + MaybeSend + Fn(F::Output) -> M)
    where
        F: 'static + MaybeSend + Future<Output: 'static + MaybeSend>,
    {
        self.tasks.push(Task::perform(future, map));
    }

    #[inline]
    fn task(&mut self, task: Task<M>) {
        self.tasks.push(task);
    }
}

impl<M> Default for TasksBuf<M> {
    #[inline]
    fn default() -> Self {
        Self { tasks: Vec::new() }
    }
}

impl<M> fmt::Debug for TasksBuf<M>
where
    M: fmt::Debug,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tasks")
            .field("tasks", &self.tasks.len())
            .finish_non_exhaustive()
    }
}
