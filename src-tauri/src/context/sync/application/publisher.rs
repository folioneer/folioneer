//! Settling-interval batcher (SYN-067): after a recorded change, publishing waits 5 seconds of
//! quiet — restarted by each further change, capped at 30 seconds from the first — then
//! publishes everything recorded since the last segment as one segment.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::context::sync::infrastructure::RecordedChangeHook;

/// How long publishing waits after the last change before firing (SYN-067).
pub const SETTLING_INTERVAL: Duration = Duration::from_secs(5);
/// The longest a burst of changes can delay a publish (SYN-067).
pub const MAX_BATCH_INTERVAL: Duration = Duration::from_secs(30);

/// A burst of changes waiting for its settling window to elapse.
struct Burst {
    /// When the first change of the burst was recorded — the cap counts from here.
    first_at: Instant,
    /// When the burst publishes unless another change restarts the window first.
    deadline: Instant,
    /// What to call when the window elapses — the latest registered callback.
    publish: Box<dyn Fn() + Send + Sync>,
}

/// Coalesces a burst of changes into one publish call (SYN-067).
pub struct Publisher {
    burst: Arc<Mutex<Option<Burst>>>,
    publish_count: Arc<AtomicUsize>,
}

impl Default for Publisher {
    fn default() -> Self {
        Self::new()
    }
}

impl Publisher {
    /// Creates a batcher with no burst in progress.
    pub fn new() -> Self {
        Self {
            burst: Arc::new(Mutex::new(None)),
            publish_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Registers that a change was just recorded, (re)starting the settling window — capped at
    /// `MAX_BATCH_INTERVAL` from the first change in the burst (SYN-067). Calls `publish`
    /// exactly once per burst, once the window elapses.
    pub async fn notify_change<F>(&self, publish: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let now = Instant::now();
        let mut burst = self.burst.lock().await;
        match burst.as_mut() {
            Some(burst) => {
                burst.deadline = (now + SETTLING_INTERVAL).min(burst.first_at + MAX_BATCH_INTERVAL);
                burst.publish = Box::new(publish);
            }
            None => {
                *burst = Some(Burst {
                    first_at: now,
                    deadline: now + SETTLING_INTERVAL,
                    publish: Box::new(publish),
                });
                tokio::spawn(Self::settle(
                    Arc::clone(&self.burst),
                    Arc::clone(&self.publish_count),
                ));
            }
        }
    }

    /// Sleeps until the burst's deadline — re-reading it after every wake, since a further
    /// change may have pushed it back — then fires the publish once.
    async fn settle(burst: Arc<Mutex<Option<Burst>>>, publish_count: Arc<AtomicUsize>) {
        loop {
            let deadline = burst.lock().await.as_ref().map(|burst| burst.deadline);
            let Some(deadline) = deadline else {
                return;
            };
            if Instant::now() < deadline {
                tokio::time::sleep_until(deadline).await;
            }
            let mut guard = burst.lock().await;
            let elapsed = guard
                .as_ref()
                .is_some_and(|burst| Instant::now() >= burst.deadline);
            if elapsed {
                if let Some(burst) = guard.take() {
                    drop(guard);
                    (burst.publish)();
                    publish_count.fetch_add(1, Ordering::SeqCst);
                }
                return;
            }
        }
    }

    /// How many times `publish` has fired so far — the tests' observation point.
    #[cfg(test)]
    pub fn publish_count(&self) -> usize {
        self.publish_count.load(Ordering::SeqCst)
    }

    /// The hook the change recorder calls after every recorded change (SYN-067): each call
    /// (re)starts the settling window; once it elapses, `publish` runs once on the async
    /// runtime.
    pub fn recorded_change_hook<F, Fut>(self: Arc<Self>, publish: F) -> RecordedChangeHook
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let publish = Arc::new(publish);
        Arc::new(move || {
            let publisher = Arc::clone(&self);
            let publish = Arc::clone(&publish);
            Box::pin(async move {
                publisher
                    .notify_change(move || {
                        tokio::spawn(publish());
                    })
                    .await;
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize as CounterCell;

    // SYN-067 — a single change publishes exactly once, after 5 seconds of quiet.
    #[tokio::test(start_paused = true)]
    async fn single_change_publishes_once_after_five_seconds_of_quiet() {
        let publisher = Publisher::new();
        let fired = Arc::new(CounterCell::new(0));
        let fired_for_callback = Arc::clone(&fired);
        publisher
            .notify_change(move || {
                fired_for_callback.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        tokio::time::advance(SETTLING_INTERVAL + Duration::from_millis(1)).await;
        assert_eq!(fired.load(Ordering::SeqCst), 1);
        assert_eq!(publisher.publish_count(), 1);
    }

    // SYN-067 — further changes inside the settling window restart it: the whole burst
    // publishes exactly once.
    #[tokio::test(start_paused = true)]
    async fn changes_within_the_settling_window_publish_only_once() {
        let publisher = Publisher::new();
        let fired = Arc::new(CounterCell::new(0));

        for _ in 0..3 {
            let fired_for_callback = Arc::clone(&fired);
            publisher
                .notify_change(move || {
                    fired_for_callback.fetch_add(1, Ordering::SeqCst);
                })
                .await;
            tokio::time::advance(Duration::from_secs(2)).await;
        }
        tokio::time::advance(SETTLING_INTERVAL + Duration::from_millis(1)).await;

        assert_eq!(
            fired.load(Ordering::SeqCst),
            1,
            "a burst inside the settling window must publish exactly once"
        );
    }

    // SYN-067 — continuous changes (each arriving before the previous settling window
    // elapses) still publish once the 30-second cap from the first change is reached.
    #[tokio::test(start_paused = true)]
    async fn continuous_changes_publish_at_the_thirty_second_cap() {
        let publisher = Publisher::new();
        let fired = Arc::new(CounterCell::new(0));

        // A change every 4 seconds for 40 seconds never lets the 5s settling window elapse on
        // its own — only the 30s cap can fire the publish.
        for _ in 0..10 {
            let fired_for_callback = Arc::clone(&fired);
            publisher
                .notify_change(move || {
                    fired_for_callback.fetch_add(1, Ordering::SeqCst);
                })
                .await;
            tokio::time::advance(Duration::from_secs(4)).await;
        }

        assert_eq!(
            fired.load(Ordering::SeqCst),
            1,
            "the 30-second cap must fire even under continuous changes"
        );
    }
}
