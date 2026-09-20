//! Global event bus for cross-context communication.

use tokio::sync::watch;

use super::event::Event;

/// Global event bus for side effects across features.
/// Each event published here can be subscribed to by any feature service.
/// Subscriptions are auto-managed: dropping a receiver automatically unsubscribes.
#[derive(Debug, Clone)]
pub struct SideEffectEventBus {
    tx: watch::Sender<Event>,
}

impl SideEffectEventBus {
    /// Create a new event bus instance
    pub fn new() -> Self {
        let (tx, _) = watch::channel(Event::Health);
        Self { tx }
    }

    /// Publish an event to all current subscribers
    pub fn publish(&self, event: Event) {
        // Ignore if no subscribers (send will fail gracefully)
        let _ = self.tx.send(event);
    }

    /// Subscribe to events. Returns a receiver that auto-unsubscribes when dropped.
    /// The receiver will receive all future events from the moment of subscription.
    pub fn subscribe(&self) -> watch::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Get the current/last published event
    pub fn current(&self) -> Event {
        self.tx.borrow().clone()
    }
}

impl Default for SideEffectEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = SideEffectEventBus::new();
        let mut rx = bus.subscribe();

        // Publish an event
        let event = Event::AssetUpdated;
        bus.publish(event.clone());

        // Subscribe and receive
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), event);
    }
}
