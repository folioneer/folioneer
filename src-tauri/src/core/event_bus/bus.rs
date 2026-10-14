//! Global event bus for cross-context communication.

use tokio::sync::broadcast::{self, error::RecvError, error::TryRecvError};

use crate::core::logger::BACKEND;

use super::event::Event;

/// How many events a subscriber may fall behind before the oldest are lost. The window's
/// forwarder only re-emits each event, so it never comes near it; a lag is logged.
const CAPACITY: usize = 1024;

/// Global event bus for side effects across features (B45): every event published reaches
/// every subscriber, in the order published — two back-to-back publishes deliver both.
#[derive(Debug, Clone)]
pub struct SideEffectEventBus {
    tx: broadcast::Sender<Event>,
}

impl SideEffectEventBus {
    /// Create a new event bus instance
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CAPACITY);
        Self { tx }
    }

    /// Publish an event to all current subscribers
    pub fn publish(&self, event: Event) {
        // Ignore if no subscribers (send will fail gracefully)
        let _ = self.tx.send(event);
    }

    /// Subscribe to the events published from now on. Dropping the receiver unsubscribes.
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver(self.tx.subscribe())
    }
}

impl Default for SideEffectEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// One subscriber's view of the bus: the events published since it subscribed, in order.
#[derive(Debug)]
pub struct EventReceiver(broadcast::Receiver<Event>);

impl EventReceiver {
    /// The next event, waiting for it; `None` once every handle on the bus is gone.
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            match self.0.recv().await {
                Ok(event) => return Some(event),
                Err(RecvError::Lagged(missed)) => fell_behind(missed),
                Err(RecvError::Closed) => return None,
            }
        }
    }

    /// The events published since the last read, without waiting.
    pub fn drain(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            match self.0.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Lagged(missed)) => fell_behind(missed),
                Err(TryRecvError::Empty | TryRecvError::Closed) => return events,
            }
        }
    }
}

/// The oldest events were lost to a subscriber that fell behind; the newer ones follow.
fn fell_behind(missed: u64) {
    tracing::warn!(target: BACKEND, missed, "event subscriber fell behind");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = SideEffectEventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(Event::AssetUpdated);

        assert_eq!(rx.next().await, Some(Event::AssetUpdated));
    }

    // B45 / TODO-083 — two publishes with nothing between both reach a subscriber, in order.
    #[tokio::test]
    async fn back_to_back_publishes_all_reach_a_subscriber_in_order() {
        let bus = SideEffectEventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(Event::AssetUpdated);
        bus.publish(Event::TransactionUpdated);
        bus.publish(Event::AgentConnectionChanged);

        assert_eq!(rx.next().await, Some(Event::AssetUpdated));
        assert_eq!(
            rx.drain(),
            vec![Event::TransactionUpdated, Event::AgentConnectionChanged]
        );
    }

    // A subscriber that falls behind loses the oldest events and keeps the newest.
    #[tokio::test]
    async fn a_subscriber_that_falls_behind_keeps_the_newest_events() {
        let bus = SideEffectEventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(Event::AssetUpdated);
        bus.publish(Event::AssetUpdated);
        for _ in 0..CAPACITY {
            bus.publish(Event::TransactionUpdated);
        }

        let kept = rx.drain();
        assert_eq!(kept.len(), CAPACITY);
        assert!(kept.iter().all(|event| *event == Event::TransactionUpdated));
        bus.publish(Event::AccountUpdated);
        assert_eq!(rx.next().await, Some(Event::AccountUpdated));
    }
}
