use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Cooperative cancellation for a running [`Runtime`](crate::Runtime).
///
/// Cancelling kills the execution's process group and request cgroup; the
/// blocking `execute` call then returns [`FaberError::Cancelled`](crate::FaberError::Cancelled).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
