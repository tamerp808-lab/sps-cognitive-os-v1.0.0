//! Actor that produced an event.
//!
//! The owner is the singleton SPS user. Agents and the kernel itself are
//! the other actor kinds. Non-owner actors are introduced in later phases;
//! for Phase 0 only `Owner` and `System` are used.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// Kind of actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorKind {
    /// The singleton SPS owner (you).
    Owner,
    /// A built-in or custom agent (Phase 12+).
    Agent,
    /// The kernel itself (boot, snapshot, replay, etc.).
    System,
}

/// An actor that emits events. The `id` is a stable identifier within the
/// kind's namespace. For `Owner` it is always `"owner"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Actor {
    /// Actor kind.
    pub kind: ActorKind,
    /// Actor identifier (stable within kind).
    pub id: SmolStr,
}

impl Actor {
    /// The owner singleton.
    pub const fn owner() -> Self {
        Self {
            kind: ActorKind::Owner,
            id: smol_str::SmolStr::new_inline("owner"),
        }
    }

    /// A system actor with a given sub-id (e.g. `"boot"`, `"snapshot"`).
    pub fn system(id: impl Into<SmolStr>) -> Self {
        Self {
            kind: ActorKind::System,
            id: id.into(),
        }
    }

    /// An agent actor (Phase 12+).
    pub fn agent(id: impl Into<SmolStr>) -> Self {
        Self {
            kind: ActorKind::Agent,
            id: id.into(),
        }
    }
}

#[cfg(test)]
impl Default for Actor {
    fn default() -> Self {
        Self::owner()
    }
}
