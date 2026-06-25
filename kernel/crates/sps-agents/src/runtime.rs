//! Agent runtime — orchestrates agent dispatch, messaging, delegation.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use smol_str::SmolStr;
use uuid::Uuid;

use crate::agent::{Agent, AgentArchetype, AgentContext, AgentId};
use crate::messages::{AgentMessage, MessageKind};
use crate::reducer::AgentStatus;

/// Result of dispatching a task to an agent.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// The agent that was dispatched.
    pub agent_id: AgentId,
    /// The task id.
    pub task_id: Uuid,
    /// Messages produced by the agent (if any).
    pub messages: Vec<AgentMessage>,
}

/// Agent runtime configuration.
#[derive(Debug, Clone)]
pub struct AgentRuntimeConfig {
    /// Maximum number of concurrent active agents.
    pub max_concurrent: usize,
    /// Whether to allow broadcast messages.
    pub allow_broadcast: bool,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 6,
            allow_broadcast: true,
        }
    }
}

/// The agent runtime.
pub struct AgentRuntime {
    config: AgentRuntimeConfig,
    /// All registered agents.
    agents: RwLock<HashMap<AgentId, Arc<Agent>>>,
    /// Index by archetype.
    by_archetype: RwLock<HashMap<AgentArchetype, Vec<AgentId>>>,
    /// Message inbox per agent.
    inboxes: RwLock<HashMap<AgentId, Vec<AgentMessage>>>,
    /// Fix #8: Optional EventSink for dispatching agent events.
    sink: Option<Arc<dyn sps_core::sink::EventSink>>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new(AgentRuntimeConfig::default())
    }
}

impl AgentRuntime {
    /// Create a new runtime.
    pub fn new(config: AgentRuntimeConfig) -> Self {
        Self {
            config,
            agents: RwLock::new(HashMap::new()),
            by_archetype: RwLock::new(HashMap::new()),
            inboxes: RwLock::new(HashMap::new()),
            sink: None,
        }
    }

    /// Fix #8: Create a runtime wired to an EventSink. The runtime holds
    /// an Arc<dyn EventSink> so it can dispatch events without a direct
    /// dependency on SpsKernel. All agent actions (register, dispatch,
    /// delegate, send) dispatch events through the sink.
    pub fn with_sink(
        config: AgentRuntimeConfig,
        sink: Arc<dyn sps_core::sink::EventSink>,
    ) -> Self {
        Self {
            config,
            agents: RwLock::new(HashMap::new()),
            by_archetype: RwLock::new(HashMap::new()),
            inboxes: RwLock::new(HashMap::new()),
            sink: Some(sink),
        }
    }

    /// Register an agent.
    pub fn register(&self, agent: Arc<Agent>) -> AgentId {
        let id = agent.id;
        let archetype = agent.archetype;
        self.agents.write().insert(id, agent.clone());
        self.by_archetype.write().entry(archetype).or_default().push(id);
        self.inboxes.write().insert(id, Vec::new());

        // Fix #8: dispatch agent.activated event if sink is wired.
        if let Some(sink) = &self.sink {
            use sps_core::actor::Actor;
            use sps_core::event::RawEvent;
            let agent_value = serde_json::to_value(agent.as_ref()).unwrap_or_default();
            let payload = serde_json::json!({"agent": agent_value});
            let _ = sink.dispatch_trusted(RawEvent::new(
                "agent.activated",
                payload,
                Actor::system("agent_runtime"),
                0,
            ));
        }

        id
    }

    /// Register all six built-in archetypes.
    pub fn register_builtins(&self) -> Vec<AgentId> {
        crate::archetypes::builtin_archetypes()
            .into_iter()
            .map(|a| self.register(a))
            .collect()
    }

    /// Get an agent by id.
    pub fn get(&self, id: &AgentId) -> Option<Arc<Agent>> {
        self.agents.read().get(id).cloned()
    }

    /// Find an agent by archetype (returns the first registered).
    pub fn find_by_archetype(&self, archetype: AgentArchetype) -> Option<Arc<Agent>> {
        let ids = self.by_archetype.read();
        let id = ids.get(&archetype)?.first()?.clone();
        self.agents.read().get(&id).cloned()
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<Arc<Agent>> {
        self.agents.read().values().cloned().collect()
    }

    /// Number of registered agents.
    pub fn count(&self) -> usize {
        self.agents.read().len()
    }

    /// Send a message to an agent (or broadcast).
    pub fn send(&self, msg: AgentMessage) {
        // Fix #8: dispatch message_sent + message_received events.
        if let Some(sink) = &self.sink {
            use sps_core::actor::Actor;
            use sps_core::event::RawEvent;
            let _ = sink.dispatch_trusted(RawEvent::new(
                "agent.message_sent",
                serde_json::to_value(&msg).unwrap_or_default(),
                Actor::system("agent_runtime"),
                0,
            ));
            if let Some(recipient) = msg.to {
                let _ = sink.dispatch_trusted(RawEvent::new(
                    "agent.message_received",
                    serde_json::to_value(&msg).unwrap_or_default(),
                    Actor::system("agent_runtime"),
                    0,
                ));
                let _ = recipient; // suppress unused
            }
        }
        match msg.to {
            Some(recipient) => {
                if let Some(inbox) = self.inboxes.write().get_mut(&recipient) {
                    inbox.push(msg);
                }
            }
            None => {
                if self.config.allow_broadcast {
                    let msg_clone = msg.clone();
                    for (id, inbox) in self.inboxes.write().iter_mut() {
                        if *id != msg_clone.from {
                            inbox.push(msg_clone.clone());
                        }
                    }
                }
            }
        }
    }

    /// Drain an agent's inbox.
    pub fn drain_inbox(&self, id: &AgentId) -> Vec<AgentMessage> {
        self.inboxes
            .write()
            .get_mut(id)
            .map(|inbox| std::mem::take(inbox))
            .unwrap_or_default()
    }

    /// Dispatch a task to an agent of the given archetype.
    ///
    /// Returns the dispatch result with any messages the agent produced.
    /// In Phase 13 this is a synchronous dispatch; the agent's actual
    /// work (LLM calls, file I/O) goes through the Effect Manager.
    pub fn dispatch(
        &self,
        archetype: AgentArchetype,
        task_title: &str,
        task_description: &str,
        origin_tick: u64,
        wall_time: u64,
    ) -> Option<DispatchResult> {
        let agent = self.find_by_archetype(archetype)?;
        let task_id = Uuid::now_v7();
        let ctx = AgentContext {
            agent_id: agent.id,
            wall_time,
            origin_tick,
            task_id: Some(task_id),
        };
        let msg = AgentMessage::new(
            agent.id,
            Some(agent.id),
            MessageKind::TaskAssignment,
            task_title,
            serde_json::json!({
                "task_id": task_id,
                "description": task_description,
                "context_tick": ctx.origin_tick,
            }),
            origin_tick,
        );

        // Fix #8: dispatch agent.dispatched event.
        if let Some(sink) = &self.sink {
            use sps_core::actor::Actor;
            use sps_core::event::RawEvent;
            let _ = sink.dispatch_trusted(RawEvent::new(
                "agent.dispatched",
                serde_json::json!({
                    "agent_id": agent.id.0.to_string(),
                    "task_id": task_id.to_string(),
                }),
                Actor::system("agent_runtime"),
                wall_time,
            ));
        }

        Some(DispatchResult {
            agent_id: agent.id,
            task_id,
            messages: vec![msg],
        })
    }

    /// Delegate a subtask from one agent to another.
    pub fn delegate(
        &self,
        from: AgentId,
        to_archetype: AgentArchetype,
        task_title: &str,
        task_description: &str,
        origin_tick: u64,
    ) -> Option<DispatchResult> {
        let to_agent = self.find_by_archetype(to_archetype)?;
        let task_id = Uuid::now_v7();
        let msg = AgentMessage::new(
            from,
            Some(to_agent.id),
            MessageKind::Delegation,
            task_title,
            serde_json::json!({
                "task_id": task_id,
                "description": task_description,
            }),
            origin_tick,
        );

        // Fix #8/#9: dispatch agent.delegated event.
        if let Some(sink) = &self.sink {
            use sps_core::actor::Actor;
            use sps_core::event::RawEvent;
            let _ = sink.dispatch_trusted(RawEvent::new(
                "agent.delegated",
                serde_json::json!({
                    "from": from.0.to_string(),
                    "to": to_agent.id.0.to_string(),
                    "title": task_title,
                    "description": task_description,
                }),
                Actor::system("agent_runtime"),
                0,
            ));
        }

        Some(DispatchResult {
            agent_id: to_agent.id,
            task_id,
            messages: vec![msg],
        })
    }

    /// Runtime config.
    pub fn config(&self) -> &AgentRuntimeConfig {
        &self.config
    }
}
