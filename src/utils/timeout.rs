use std::future::Future;

use std::time::Duration;
use tokio::sync::oneshot;

/// The state of a timeout.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TimedOut {
    /// Timeout timed out.
    TimedOut,
    /// Timeout was cancelled.
    Cancelled,
}

/// Timeout running the given future.
#[derive(Default)]
#[repr(transparent)]
pub(crate) struct Timeout {
    tx: Option<oneshot::Sender<()>>,
}

impl Timeout {
    /// Clear timeout.
    pub(crate) fn clear(&mut self) {
        self.tx = None;
    }

    /// Test if timeout is set.
    pub(crate) fn is_set(&self) -> bool {
        self.tx.is_some()
    }

    /// Set a new timeout.
    pub(crate) fn set(&mut self, duration: Duration) -> impl Future<Output = TimedOut> + use<> {
        let (tx, rx) = oneshot::channel();
        self.tx = Some(tx);

        async move {
            let sleep = tokio::time::sleep(duration);

            tokio::select! {
                _ = rx => TimedOut::Cancelled,
                _ = sleep => TimedOut::TimedOut,
            }
        }
    }
}
