//! Event type registry.
//!
//! An `EventType` is a dotted string like `"goal.created"` or
//! `"effect.executed"`. It is stored as a `SmolStr` for cheap cloning.

use std::fmt;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// A dotted event type identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventType(pub SmolStr);

impl EventType {
    /// Construct from a string-like value.
    pub fn new(s: impl Into<SmolStr>) -> Self {
        Self(s.into())
    }

    /// Return the inner string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the top-level category (e.g. `"goal"` for `"goal.created"`).
    pub fn category(&self) -> &str {
        match self.0.as_str().split_once('.') {
            Some((cat, _)) => cat,
            None => self.0.as_str(),
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl From<&str> for EventType {
    fn from(s: &str) -> Self {
        Self(SmolStr::new(s))
    }
}

impl From<String> for EventType {
    fn from(s: String) -> Self {
        Self(SmolStr::new(s))
    }
}

impl From<SmolStr> for EventType {
    fn from(s: SmolStr) -> Self {
        Self(s)
    }
}

impl AsRef<str> for EventType {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

/// Well-known system event types emitted during Phase 0.
pub mod system {
    use super::EventType;

    /// Emitted once at store creation. Tick 1.
    pub fn boot() -> EventType {
        EventType::new("system.booted")
    }

    /// Emitted when a snapshot is taken.
    pub fn snapshot_taken() -> EventType {
        EventType::new("system.snapshot_taken")
    }

    /// Emitted when replay verification completes.
    pub fn replay_verified() -> EventType {
        EventType::new("system.replay_verified")
    }
}
