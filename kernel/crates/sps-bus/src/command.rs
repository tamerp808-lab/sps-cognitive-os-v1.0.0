//! Command Bus — inbound API for the kernel.
//!
//! Commands are typed intents that get translated into one or more
//! events appended to the Event Store. Each command type has a
//! registered handler that knows how to translate it.

use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::event_store::EventStore;
use sps_core::{CoreError, CoreResult};

/// A command — inbound request to the kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Command type, e.g. "owner.set_name", "provider.register".
    pub command_type: SmolStr,
    /// Strongly-typed payload.
    pub payload: serde_json::Value,
    /// Correlation id (auto-generated if absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<uuid::Uuid>,
}

impl Command {
    /// Create a new command.
    pub fn new(command_type: impl Into<SmolStr>, payload: serde_json::Value) -> Self {
        Self {
            command_type: command_type.into(),
            payload,
            correlation_id: Some(uuid::Uuid::now_v7()),
        }
    }
}

/// A command handler translates a command into one or more events.
pub trait CommandHandler: Send + Sync + 'static {
    /// Command type this handler accepts.
    fn command_type(&self) -> &str;

    /// Handle the command. Appends events to the store. Returns the
    /// ticks of the appended events.
    fn handle(
        &self,
        command: &Command,
        store: &EventStore,
        actor: &Actor,
        wall_time: u64,
    ) -> CoreResult<Vec<u64>>;
}

/// Registry of command handlers.
#[derive(Default)]
pub struct CommandRegistry {
    handlers: RwLock<std::collections::HashMap<String, Arc<dyn CommandHandler>>>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler.
    pub fn register(&self, handler: Arc<dyn CommandHandler>) {
        let ct = handler.command_type().to_string();
        self.handlers.write().insert(ct, handler);
    }

    /// Look up a handler by command type.
    pub fn get(&self, command_type: &str) -> Option<Arc<dyn CommandHandler>> {
        self.handlers.read().get(command_type).cloned()
    }
}

/// The Command Bus.
pub struct CommandBus {
    registry: Arc<CommandRegistry>,
    store: Arc<EventStore>,
    default_actor: Actor,
}

impl CommandBus {
    /// Create a new Command Bus.
    pub fn new(registry: Arc<CommandRegistry>, store: Arc<EventStore>) -> Self {
        Self {
            registry,
            store,
            default_actor: Actor::owner(),
        }
    }

    /// Dispatch a command. Returns the ticks of the appended events.
    pub fn dispatch(&self, command: &Command) -> CoreResult<Vec<u64>> {
        self.dispatch_with(command, &self.default_actor.clone(), current_wall_time())
    }

    /// Dispatch with explicit actor + wall time (for tests / replay).
    pub fn dispatch_with(
        &self,
        command: &Command,
        actor: &Actor,
        wall_time: u64,
    ) -> CoreResult<Vec<u64>> {
        let handler = self
            .registry
            .get(command.command_type.as_str())
            .ok_or_else(|| {
                CoreError::Internal(anyhow::anyhow!(
                    "no handler for command type '{}'",
                    command.command_type
                ))
            })?;
        handler.handle(command, &self.store, actor, wall_time)
    }

    /// Append a single raw event. Convenience for handlers that produce
    /// exactly one event.
    pub fn append_raw(&self, raw: RawEvent) -> CoreResult<u64> {
        Ok(self.store.append(raw)?.tick)
    }

    /// Reference to the underlying store.
    pub fn store(&self) -> &Arc<EventStore> {
        &self.store
    }
}

/// Get the current wall time in ms (display only).
pub fn current_wall_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
