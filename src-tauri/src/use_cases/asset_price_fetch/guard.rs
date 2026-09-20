use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// RAII lease that clears the in-flight flag when dropped (MKT-113).
/// Moving the lease into the background task ensures the flag is cleared even
/// on task panic.
pub struct FetchGuardLease {
    flag: Arc<AtomicBool>,
}

impl Drop for FetchGuardLease {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

/// Global in-flight guard ensuring at most one fetch task runs at a time (MKT-113).
pub struct FetchGuard {
    running: Arc<AtomicBool>,
}

impl FetchGuard {
    /// Creates a new guard in the released state.
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Attempts to acquire the guard. Returns `Some(FetchGuardLease)` on the
    /// first call; returns `None` if the guard is already held (MKT-113).
    pub fn try_acquire(self: &Arc<Self>) -> Option<FetchGuardLease> {
        match self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Some(FetchGuardLease {
                flag: self.running.clone(),
            }),
            Err(_) => None,
        }
    }
}

impl Default for FetchGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // MKT-113 — try_acquire returns Some on first call (guard is free)
    #[test]
    fn try_acquire_returns_some_when_free() {
        let guard = Arc::new(FetchGuard::new());
        let lease = guard.try_acquire();
        assert!(lease.is_some(), "expected Some lease when guard is free");
    }

    // MKT-113 — try_acquire returns None on second concurrent call (guard already held)
    #[test]
    fn try_acquire_returns_none_when_already_held() {
        let guard = Arc::new(FetchGuard::new());
        let _lease = guard.try_acquire().expect("first acquire must succeed");
        let second = guard.try_acquire();
        assert!(second.is_none(), "expected None while lease is still held");
    }

    // MKT-113 — guard is released when the lease is dropped; next try_acquire succeeds
    #[test]
    fn lease_drop_releases_guard() {
        let guard = Arc::new(FetchGuard::new());
        {
            let _lease = guard.try_acquire().expect("first acquire must succeed");
        }
        let after_drop = guard.try_acquire();
        assert!(
            after_drop.is_some(),
            "expected Some after lease was dropped, got None"
        );
    }
}
