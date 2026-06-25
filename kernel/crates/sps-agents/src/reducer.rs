//! Agent reducer + state slice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::agent::{Agent, AgentArchetype};
use crate::messages::AgentMessage;

/// Extension key.
pub const EXTENSION_KEY: &str = "agents";

/// Agent status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Idle, waiting for tasks.
    Idle,
    /// Active — working on a task.
    Active,
    /// Blocked.
    Blocked,
    /// Deactivated.
    Deactivated,
}

/// Fix #9: A record of a delegation from one agent to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationRecord {
    /// The agent that delegated.
    pub from: Uuid,
    /// The agent that received the delegation.
    pub to: Uuid,
    /// Task title.
    pub title: SmolStr,
    /// Task description.
    #[serde(default)]
    pub description: String,
    /// Originating tick.
    pub origin_tick: u64,
}

/// A record of an agent's activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRecord {
    /// The agent descriptor.
    pub agent: Agent,
    /// Current status.
    pub status: AgentStatus,
    /// Messages sent by this agent.
    #[serde(default)]
    pub messages_sent: Vec<AgentMessage>,
    /// Messages received by this agent.
    #[serde(default)]
    pub messages_received: Vec<AgentMessage>,
    /// Tasks dispatched to this agent.
    #[serde(default)]
    pub tasks_dispatched: Vec<Uuid>,
    /// Fix #9: Delegations sent by this agent (to others).
    #[serde(default)]
    pub delegations_sent: Vec<DelegationRecord>,
    /// Fix #9: Delegations received by this agent (from others).
    #[serde(default)]
    pub delegations_received: Vec<DelegationRecord>,
}

/// Agent state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentState {
    /// All agent records keyed by id.
    #[serde(default)]
    pub agents: std::collections::BTreeMap<Uuid, AgentRecord>,
}

impl AgentState {
    /// Read from canonical state. P3D: typed first, JSON fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<AgentState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// P3D: Read from typed extension.
    pub fn from_typed_state(state: &CanonicalState) -> Option<Arc<AgentState>> {
        state.get_typed_extension::<AgentState>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }
}

/// Reducer for agent events.
#[derive(Debug, Default)]
pub struct AgentReducer;

impl AgentReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "agent.activated",
            "agent.idle",
            "agent.blocked",
            "agent.deactivated",
            "agent.message_sent",
            "agent.message_received",
            "agent.dispatched",
            "agent.delegated",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<AgentState>(EXTENSION_KEY);
    }
}

impl Reducer for AgentReducer {
    fn name(&self) -> &'static str {
        "agents"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // P3D: Use typed extension as source of truth.
        state.with_typed_extension(EXTENSION_KEY, |as_: &mut AgentState| {
            match event.event_type.as_str() {
                "agent.activated" => {
                    let agent: Agent = serde_json::from_value(event.payload["agent"].clone())
                        .unwrap_or_default();
                    let record = AgentRecord {
                        agent,
                        status: AgentStatus::Active,
                        messages_sent: Vec::new(),
                        messages_received: Vec::new(),
                        tasks_dispatched: Vec::new(),
                        delegations_sent: Vec::new(),
                        delegations_received: Vec::new(),
                    };
                    as_.agents.insert(record.agent.id.0, record);
                }
                "agent.idle" | "agent.blocked" | "agent.deactivated" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let new_status = match event.event_type.as_str() {
                        "agent.idle" => AgentStatus::Idle,
                        "agent.blocked" => AgentStatus::Blocked,
                        "agent.deactivated" => AgentStatus::Deactivated,
                        _ => AgentStatus::Idle,
                    };
                    if let Some(r) = as_.agents.get_mut(&id) {
                        r.status = new_status;
                    }
                }
                "agent.message_sent" => {
                    if let Ok(msg) = serde_json::from_value::<AgentMessage>(event.payload.clone()) {
                        if let Some(r) = as_.agents.get_mut(&msg.from.0) {
                            r.messages_sent.push(msg);
                        }
                    }
                }
                "agent.message_received" => {
                    if let Ok(msg) = serde_json::from_value::<AgentMessage>(event.payload.clone()) {
                        if let Some(recipient) = msg.to {
                            if let Some(r) = as_.agents.get_mut(&recipient.0) {
                                r.messages_received.push(msg);
                            }
                        }
                    }
                }
                "agent.dispatched" => {
                    let agent_id: Uuid = serde_json::from_value(event.payload["agent_id"].clone())
                        .unwrap_or_default();
                    let task_id: Uuid = serde_json::from_value(event.payload["task_id"].clone())
                        .unwrap_or_default();
                    if let Some(r) = as_.agents.get_mut(&agent_id) {
                        r.tasks_dispatched.push(task_id);
                        r.status = AgentStatus::Active;
                    }
                }
                // Fix #9: materialize agent.delegated into DelegationRecord
                // on both sender and receiver.
                "agent.delegated" => {
                    let from: Uuid = serde_json::from_value(event.payload["from"].clone())
                        .unwrap_or_default();
                    let to: Uuid = serde_json::from_value(event.payload["to"].clone())
                        .unwrap_or_default();
                    let title = event.payload.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let description = event.payload.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let del = DelegationRecord {
                        from,
                        to,
                        title: SmolStr::new(title),
                        description,
                        origin_tick: event.tick,
                    };
                    if let Some(r) = as_.agents.get_mut(&from) {
                        r.delegations_sent.push(del.clone());
                    }
                    if let Some(r) = as_.agents.get_mut(&to) {
                        r.delegations_received.push(del);
                        r.status = AgentStatus::Active;
                    }
                }
                _ => {}
            }
        });
        // P3D: No per-dispatch JSON sync.
        Ok(())
    }
}

// Re-export SmolStr for convenience.
#[allow(dead_code)]
fn _use_smolstr(s: SmolStr) -> SmolStr {
    s
}

// Re-export AgentArchetype for convenience.
#[allow(dead_code)]
fn _use_archetype(a: AgentArchetype) -> AgentArchetype {
    a
}
