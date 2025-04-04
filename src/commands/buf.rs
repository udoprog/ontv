use std::fmt;
use std::future::Future;

use iced::Task;
use iced_futures::MaybeSend;

use crate::commands::Commands;

/// A command buffer used for an application.
pub struct CommandsBuf<M> {
    tasks: Vec<Task<M>>,
}

impl<M> CommandsBuf<M> {
    /// Build a single command out of the command buffer.
    pub(crate) fn build(&mut self) -> Task<M> {
        if self.tasks.is_empty() {
            return Task::none();
        }

        Task::batch(self.tasks.drain(..))
    }
}

impl<M> Commands<M> for CommandsBuf<M> {
    type ByRef<'this>
        = &'this mut Self
    where
        Self: 'this;

    #[inline]
    fn by_ref(&mut self) -> Self::ByRef<'_> {
        self
    }

    #[inline]
    fn perform<F>(&mut self, future: F, map: impl Fn(F::Output) -> M + MaybeSend + 'static)
    where
        F: Future + 'static + MaybeSend,
    {
        self.tasks.push(Task::perform(future, map));
    }

    #[inline]
    fn command(&mut self, task: Task<M>) {
        self.tasks.push(task);
    }
}

impl<M> Default for CommandsBuf<M> {
    #[inline]
    fn default() -> Self {
        Self { tasks: Vec::new() }
    }
}

impl<M> fmt::Debug for CommandsBuf<M>
where
    M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalCommands")
            .field("commands", &self.tasks)
            .finish_non_exhaustive()
    }
}
