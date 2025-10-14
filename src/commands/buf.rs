use std::fmt;
use std::future::Future;

use iced::Task as Command;
use iced_futures::MaybeSend;

use crate::commands::Commands;

/// A command buffer used for an application.
pub struct CommandsBuf<M> {
    commands: Vec<Command<M>>,
}

impl<T> CommandsBuf<T>
where
    T: 'static,
{
    /// Build a single command out of the command buffer.
    pub(crate) fn build(&mut self) -> Command<T> {
        if self.commands.is_empty() {
            return Command::none();
        }

        Command::batch(self.commands.drain(..))
    }
}

impl<T> Commands<T> for CommandsBuf<T>
where
    T: 'static + MaybeSend,
{
    type ByRef<'this>
        = &'this mut Self
    where
        Self: 'this;

    #[inline]
    fn by_ref(&mut self) -> Self::ByRef<'_> {
        self
    }

    #[inline]
    fn perform<F>(&mut self, future: F, map: impl 'static + MaybeSend + Fn(F::Output) -> T)
    where
        F: 'static + MaybeSend + Future<Output: 'static + MaybeSend>,
    {
        self.commands.push(Command::perform(future, map));
    }

    #[inline]
    fn command(&mut self, command: Command<T>) {
        self.commands.push(command);
    }
}

impl<M> Default for CommandsBuf<M> {
    #[inline]
    fn default() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

impl<M> fmt::Debug for CommandsBuf<M>
where
    M: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalCommands")
            .field("commands", &self.commands.len())
            .finish_non_exhaustive()
    }
}
