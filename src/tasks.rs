//! Buffer of asynchronous actions.

mod buf;
pub(crate) use self::buf::TasksBuf;

use std::future::Future;

use iced::Task;
use iced_futures::MaybeSend;

/// Send tasks to an iced application.
#[doc(hidden)]
pub trait Tasks<T> {
    /// Helper to generically reborrow the task buffer mutably.
    ///
    /// This is useful if you have a function that takes `mut tasks: impl
    /// Tasks<T>` and you want to use a method such as [Tasks::map] which would
    /// otherwise consume the task buffer.
    ///
    /// This can still be done through an expression like `(&mut tasks).map(/*
    /// */)`, but having a method like this reduces the number of references
    /// involves in case the `impl Tasks<T>` is already a reference.
    ///
    /// Note that naming is inspired by [`Iterator::by_ref`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use ontv::tasks::Tasks;
    /// enum Message {
    ///     Component1(component1::Message),
    ///     Component2(component2::Message),
    /// }
    ///
    /// fn update(mut tasks: impl Tasks<Message>) {
    ///     component1::update(tasks.by_ref().map(Message::Component1));
    ///     component2::update(tasks.by_ref().map(Message::Component2));
    /// }
    ///
    /// mod component1 {
    ///     # use ontv::tasks::Tasks;
    ///     pub(crate) enum Message {
    ///         Tick,
    ///     }
    ///
    ///     pub(crate) fn update(mut tasks: impl Tasks<Message>) {
    ///         // emit tasks
    ///     }
    /// }
    ///
    /// mod component2 {
    ///     #    use ontv::tasks::Tasks;
    ///     pub(crate) enum Message {
    ///         Tick,
    ///     }
    ///
    ///     pub(crate) fn update(mut tasks: impl Tasks<Message>) {
    ///         // emit tasks
    ///     }
    /// }
    /// ```
    ///
    /// Without this method, you'd have to do the following while also
    /// potentially constructing another reference that you don't really need:
    ///
    /// ```
    /// # use ontv::tasks::Tasks;
    /// # enum Message { Component1(component1::Message), Component2(component2::Message) }
    /// fn update(mut tasks: impl Tasks<Message>) {
    ///     component1::update((&mut tasks).map(Message::Component1));
    ///     component2::update((&mut tasks).map(Message::Component2));
    /// }
    /// # mod component1 {
    /// # use ontv::tasks::Tasks;
    /// # pub(crate) enum Message { Tick }
    /// # pub(crate) fn update(mut tasks: impl Tasks<Message>) { }
    /// # }
    /// # mod component2 {
    /// # use ontv::tasks::Tasks;
    /// # pub(crate) enum Message { Tick }
    /// # pub(crate) fn update(mut tasks: impl Tasks<Message>) { }
    /// # }
    /// ```
    fn by_ref(&mut self) -> impl Tasks<T> + '_;

    /// Perform a single asynchronous action and map its output into the
    /// expected message type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ontv::tasks::Tasks;
    /// enum Message {
    ///     Greeting(String),
    /// }
    ///
    /// async fn asynchronous_update() -> String {
    ///     "Hello World".to_string()
    /// }
    ///
    /// fn update(mut tasks: impl Tasks<Message>) {
    ///     tasks.perform(asynchronous_update(), Message::Greeting);
    /// }
    /// ```
    fn perform<F>(&mut self, future: F, map: impl 'static + MaybeSend + Fn(F::Output) -> T)
    where
        F: 'static + MaybeSend + Future<Output: 'static + MaybeSend>;

    /// Insert a task directly into the task buffer.
    ///
    /// This is primarily used for built-in tasks such as window messages.
    ///
    /// # Examples
    ///
    /// ```
    /// # // NB: we don't have access to iced here so faking it.
    /// # mod iced { pub(crate) mod window { pub(crate) fn close<Message>() -> iced::Task<Message> { todo!() } } }
    /// # use ontv::tasks::Tasks;
    /// enum Message {
    ///     /* snip */
    /// }
    ///
    /// fn update(mut tasks: impl Tasks<Message>) {
    ///     tasks.task(iced::window::close());
    /// }
    /// ```
    fn task(&mut self, task: Task<T>);

    /// Extend the current task buffer with an iterator.
    #[inline]
    fn extend(&mut self, iter: impl IntoIterator<Item = Task<T>>) {
        for task in iter {
            self.task(task);
        }
    }

    /// Map the current task buffer so that it can be used with a different
    /// message type `U`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use ontv::tasks::Tasks;
    /// enum Message {
    ///     Component1(component1::Message),
    ///     Component2(component2::Message),
    /// }
    ///
    /// fn update(mut tasks: impl Tasks<Message>) {
    ///     component1::update(tasks.by_ref().map(Message::Component1));
    ///     component2::update(tasks.by_ref().map(Message::Component2));
    /// }
    ///
    /// mod component1 {
    ///     # use ontv::tasks::Tasks;
    ///     pub(crate) enum Message {
    ///         Tick,
    ///     }
    ///
    ///     pub(crate) fn update(mut tasks: impl Tasks<Message>) {
    ///         // emit tasks
    ///     }
    /// }
    ///
    /// mod component2 {
    ///     # use ontv::tasks::Tasks;
    ///     pub(crate) enum Message {
    ///         Tick,
    ///     }
    ///
    ///     pub(crate) fn update(mut tasks: impl Tasks<Message>) {
    ///         // emit tasks
    ///     }
    /// }
    /// ```
    #[inline]
    fn map<F, U>(self, map: F) -> Map<Self, F>
    where
        Self: Sized,
        F: MaybeSend + Sync + Clone + Fn(U) -> T,
    {
        Map { tasks: self, map }
    }
}

/// Wrapper produced by [`Tasks::map`].
#[derive(Debug)]
pub struct Map<C, F> {
    tasks: C,
    map: F,
}

impl<T, C, F, U> Tasks<U> for Map<C, F>
where
    T: 'static + MaybeSend,
    C: Tasks<T>,
    F: 'static + MaybeSend + Sync + Clone + Fn(U) -> T,
    U: 'static + MaybeSend,
{
    #[inline]
    fn by_ref(&mut self) -> impl Tasks<U> + '_ {
        self
    }

    #[inline]
    fn perform<Fut>(&mut self, future: Fut, outer: impl 'static + MaybeSend + Fn(Fut::Output) -> U)
    where
        Fut: 'static + MaybeSend + Future<Output: 'static + MaybeSend>,
    {
        let map = self.map.clone();

        self.tasks
            .perform(future, move |message| map(outer(message)));
    }

    #[inline]
    fn task(&mut self, task: Task<U>) {
        let map = self.map.clone();
        self.tasks.task(task.map(map));
    }
}

impl<C, M> Tasks<M> for &mut C
where
    C: Tasks<M>,
{
    #[inline]
    fn by_ref(&mut self) -> impl Tasks<M> + '_ {
        (*self).by_ref()
    }

    #[inline]
    fn perform<Fut>(&mut self, future: Fut, map: impl 'static + MaybeSend + Fn(Fut::Output) -> M)
    where
        Fut: 'static + MaybeSend + Future<Output: 'static + MaybeSend>,
    {
        (**self).perform(future, map);
    }

    #[inline]
    fn task(&mut self, task: Task<M>) {
        (**self).task(task);
    }
}
