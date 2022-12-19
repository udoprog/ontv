use std::future::Future;

use iced_futures::MaybeSend;

use tokio::sync::oneshot;

/// An operation which can only have one pending future at a time.
#[derive(Default)]
#[repr(transparent)]
pub(crate) struct Singleton {
    tx: Option<oneshot::Sender<()>>,
}

impl Singleton {
    /// If the singleton is set.
    pub(crate) fn is_set(&self) -> bool {
        self.tx.is_some()
    }

    /// Clear the singleton, causing any underlying task that if pending will be cancelled.
    pub(crate) fn clear(&mut self) {
        self.tx = None;
    }

    /// Set the current operation.
    pub(crate) fn set<F>(&mut self, future: F) -> impl Future<Output = Option<F::Output>>
    where
        F: Future + MaybeSend + 'static,
    {
        let (tx, rx) = oneshot::channel();
        self.tx = Some(tx);

        async move {
            tokio::select! {
                _ = rx => None,
                output = future => Some(output),
            }
        }
    }
}
