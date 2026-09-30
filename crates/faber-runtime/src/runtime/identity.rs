use std::collections::BTreeSet;
use std::sync::Mutex;

use rand::Rng;

use crate::prelude::*;

/// Host UIDs/GIDs handed to sandboxes, one per running request.
const FIRST_ID: u32 = 100_000;
const ID_COUNT: u32 = 65_536;

struct Pool {
    first: u32,
    count: u32,
    next: u32,
    in_use: BTreeSet<u32>,
}

impl Pool {
    fn new(first: u32, count: u32) -> Self {
        Self {
            first,
            count,
            // Start at a random offset so that several Faber processes on one
            // kernel rarely hand out the same identity.
            next: rand::rng().random_range(0..count),
            in_use: BTreeSet::new(),
        }
    }
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
    /// Use `count` identities starting at `first` instead of the defaults.
    /// Refused once an identity has been leased.
    pub(crate) fn configure(first: u32, count: u32) -> Result<()> {
        let invalid = |message: &str| FaberError::Generic {
            message: format!("Invalid sandbox identity range {first}+{count}: {message}"),
        };
        if count == 0 {
            return Err(invalid("at least one identity is needed"));
        }
        // 65535 is the overflow ID and u32::MAX is "no ID"; stay below both.
        match first.checked_add(count) {
            Some(end) if end < 65_535 || first > 65_535 => {}
            _ => return Err(invalid("must not include 65535 or overflow")),
        }
        let mut pool = Self::pool()?;
        if pool.as_ref().is_some_and(|pool| !pool.in_use.is_empty()) {
            return Err(FaberError::Generic {
                message: "Sandbox identities cannot be reconfigured while leased".to_string(),
            });
        }
        *pool = Some(Pool::new(first, count));
        Ok(())
    }

    pub(crate) fn acquire() -> Result<Self> {
        let mut pool = Self::pool()?;
        let pool = pool.get_or_insert_with(|| Pool::new(FIRST_ID, ID_COUNT));
        for _ in 0..pool.count {
            let id = pool.first + pool.next;
            pool.next = (pool.next + 1) % pool.count;
            if pool.in_use.insert(id) {
                return Ok(Self(id));
            }
        }
        Err(FaberError::Generic {
            message: "Every sandbox identity is in use".to_string(),
        })
    }

    fn pool() -> Result<std::sync::MutexGuard<'static, Option<Pool>>> {
        POOL.lock().map_err(|_| FaberError::Generic {
            message: "Sandbox identity pool lock was poisoned".to_string(),
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
        // Both tests share the process-wide pool.
        let _serial = super::super::core::Runtime::configure_identities(FIRST_ID, ID_COUNT);
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

    #[test]
    fn ranges_must_avoid_the_overflow_id_and_leased_identities() {
        assert!(SandboxIdentity::configure(60_000, 10_000).is_err());
        assert!(SandboxIdentity::configure(100_000, 0).is_err());
        assert!(SandboxIdentity::configure(u32::MAX - 5, 10).is_err());
        assert!(SandboxIdentity::configure(1_000, 1_000).is_ok());
        let leased = SandboxIdentity::acquire().unwrap();
        assert!((1_000..2_000).contains(&leased.id()));
        assert!(SandboxIdentity::configure(FIRST_ID, ID_COUNT).is_err());
        drop(leased);
        assert!(SandboxIdentity::configure(FIRST_ID, ID_COUNT).is_ok());
    }
}
