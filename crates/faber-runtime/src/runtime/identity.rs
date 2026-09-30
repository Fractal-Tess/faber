use std::collections::BTreeSet;
use std::sync::Mutex;

use rand::Rng;

use crate::prelude::*;

/// Host UIDs/GIDs handed to sandboxes, one per running request.
const FIRST_ID: u32 = 100_000;
const ID_COUNT: u32 = 65_536;

struct Pool {
    next: u32,
    in_use: BTreeSet<u32>,
}

static POOL: Mutex<Option<Pool>> = Mutex::new(None);

/// The host UID and GID every task of one request runs as.
///
/// The kernel keeps signal permissions and per-user limits (inotify
/// instances, pending signals, locked memory, message queues) per host UID.
/// A request with an identity of its own cannot use up another request's
/// share of them. Inside its user namespace a task still sees 65534.
pub(crate) struct SandboxIdentity(u32);

impl SandboxIdentity {
    pub(crate) fn acquire() -> Result<Self> {
        let mut pool = POOL.lock().map_err(|_| FaberError::Generic {
            message: "Sandbox identity pool lock was poisoned".to_string(),
        })?;
        // Start at a random offset so that several Faber processes on one
        // kernel rarely hand out the same identity.
        let pool = pool.get_or_insert_with(|| Pool {
            next: rand::rng().random_range(0..ID_COUNT),
            in_use: BTreeSet::new(),
        });
        for _ in 0..ID_COUNT {
            let id = FIRST_ID + pool.next;
            pool.next = (pool.next + 1) % ID_COUNT;
            if pool.in_use.insert(id) {
                return Ok(Self(id));
            }
        }
        Err(FaberError::Generic {
            message: "Every sandbox identity is in use".to_string(),
        })
    }

    pub(crate) fn id(&self) -> u32 {
        self.0
    }
}

impl Drop for SandboxIdentity {
    fn drop(&mut self) {
        if let Ok(mut pool) = POOL.lock()
            && let Some(pool) = pool.as_mut()
        {
            pool.in_use.remove(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FIRST_ID, ID_COUNT, SandboxIdentity};

    #[test]
    fn concurrent_leases_are_distinct_and_released_on_drop() {
        let first = SandboxIdentity::acquire().unwrap();
        let second = SandboxIdentity::acquire().unwrap();
        assert_ne!(first.id(), second.id());
        for identity in [&first, &second] {
            assert!((FIRST_ID..FIRST_ID + ID_COUNT).contains(&identity.id()));
        }

        let released = first.id();
        drop(first);
        let reused = (0..ID_COUNT)
            .map(|_| SandboxIdentity::acquire().unwrap().id())
            .any(|id| id == released);
        assert!(reused, "a released identity never became available again");
    }
}
