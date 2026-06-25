//! Event Bus — outbound pub/sub for surfaces and subscribers.

use std::sync::Arc;

use parking_lot::RwLock;
use sps_core::event::Event;
use sps_core::storage::port::StoragePort;
use sps_core::CoreResult;
use uuid::Uuid;

/// A subscription id.
pub type SubscriptionId = Uuid;

/// A subscriber callback.
pub type EventCallback = Arc<dyn Fn(&Event) + Send + Sync>;

/// An event subscription.
#[derive(Clone)]
pub struct EventSubscription {
    /// Subscription id.
    pub id: SubscriptionId,
    /// Optional event type filter (None = all events).
    pub filter: Option<String>,
    /// The callback.
    pub callback: EventCallback,
}

/// The Event Bus. Subscribers register callbacks; the bus polls the
/// event store for new events and dispatches them.
pub struct EventBus {
    subscriptions: RwLock<Vec<EventSubscription>>,
    last_dispatched_tick: RwLock<u64>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    /// Create a new Event Bus.
    pub fn new() -> Self {
        Self {
            subscriptions: RwLock::new(Vec::new()),
            last_dispatched_tick: RwLock::new(0),
        }
    }

    /// Subscribe to events. Returns a subscription id that can be used
    /// to unsubscribe. If `filter` is provided, only events of that
    /// type are delivered.
    pub fn subscribe(
        &self,
        filter: Option<String>,
        callback: EventCallback,
    ) -> SubscriptionId {
        let id = Uuid::now_v7();
        self.subscriptions.write().push(EventSubscription {
            id,
            filter,
            callback,
        });
        id
    }

    /// Unsubscribe.
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut subs = self.subscriptions.write();
        let before = subs.len();
        subs.retain(|s| s.id != id);
        subs.len() < before
    }

    /// Poll the storage for new events since the last dispatched tick
    /// and dispatch them to subscribers. Returns the number of events
    /// dispatched.
    pub fn poll(&self, storage: &dyn StoragePort) -> CoreResult<usize> {
        let chunk_size = 256usize;
        let mut last = *self.last_dispatched_tick.read();
        let mut total = 0usize;
        loop {
            let from = last.saturating_add(1);
            let chunk = storage.read_events_from(from, chunk_size)?;
            if chunk.is_empty() {
                break;
            }
            let subs = self.subscriptions.read().clone();
            for event in &chunk {
                for sub in &subs {
                    if let Some(ref filter) = sub.filter {
                        if event.event_type.as_str() != filter {
                            continue;
                        }
                    }
                    (sub.callback)(event);
                }
                last = event.tick;
                total += 1;
            }
            if chunk.len() < chunk_size {
                break;
            }
        }
        *self.last_dispatched_tick.write() = last;
        Ok(total)
    }

    /// Dispatch a single event directly (used in tests).
    pub fn dispatch_event(&self, event: &Event) {
        let subs = self.subscriptions.read().clone();
        for sub in &subs {
            if let Some(ref filter) = sub.filter {
                if event.event_type.as_str() != filter {
                    continue;
                }
            }
            (sub.callback)(event);
        }
    }

    /// Number of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.read().len()
    }
}
