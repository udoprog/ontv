use iced_futures::MaybeSend;
use iced_native::command::Action;

use std::time::Duration;
use tokio::sync::oneshot;

/// Timeout running the given future.
#[derive(Default)]
#[repr(transparent)]
pub(crate) struct Timeout {
    tx: Option<oneshot::Sender<()>>,
}

impl Timeout {
    /// Set a new timeout.
    pub(crate) fn set<T, O>(&mut self, duration: Duration, output: T) -> Action<O>
    where
        T: MaybeSend + 'static + FnOnce(bool) -> O,
    {
        let (tx, rx) = oneshot::channel();
        self.tx = Some(tx);

        Action::Future(Box::pin(async move {
            let sleep = tokio::time::sleep(duration);

            tokio::select! {
                _ = rx => output(false),
                _ = sleep => output(true),
            }
        }))
    }
}
