//! Agent Runtime tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_agents::agent::{Agent, AgentArchetype, AgentCapabilities};
use sps_agents::archetypes::{Architect, Developer, DevOps, Researcher, Reviewer, Tester};
use sps_agents::messages::{AgentMessage, MessageKind};
use sps_agents::reducer::{AgentReducer, AgentState, AgentStatus};
use sps_agents::runtime::AgentRuntime;

fn fresh_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    AgentReducer::register(&mut reg);
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

#[test]
fn all_six_archetypes_have_distinct_prompts() {
    let agents = sps_agents::archetypes::builtin_archetypes();
    assert_eq!(agents.len(), 6);
    let prompts: Vec<&str> = agents.iter().map(|a| a.system_prompt.as_str()).collect();
    // All distinct.
    let unique: std::collections::HashSet<&str> = prompts.iter().copied().collect();
    assert_eq!(unique.len(), 6);
}

#[test]
fn architect_agent_has_delegation_capability() {
    let a = Architect::new();
    assert_eq!(a.archetype, AgentArchetype::Architect);
    assert!(a.capabilities.can_delegate);
    assert!(a.capabilities.can_create_goals);
}

#[test]
fn developer_agent_can_write_files_and_exec_shell() {
    let a = Developer::new();
    assert_eq!(a.archetype, AgentArchetype::Developer);
    assert!(a.capabilities.can_write_files);
    assert!(a.capabilities.can_exec_shell);
    assert!(!a.capabilities.can_delegate);
}

#[test]
fn reviewer_agent_is_read_only() {
    let a = Reviewer::new();
    assert!(!a.capabilities.can_write_files);
    assert!(!a.capabilities.can_exec_shell);
}

#[test]
fn tester_agent_can_write_and_exec() {
    let a = Tester::new();
    assert!(a.capabilities.can_write_files);
    assert!(a.capabilities.can_exec_shell);
}

#[test]
fn devops_agent_can_deploy() {
    let a = DevOps::new();
    assert!(a.capabilities.can_write_files);
    assert!(a.capabilities.can_exec_shell);
}

#[test]
fn researcher_agent_can_delegate() {
    let a = Researcher::new();
    assert!(a.capabilities.can_delegate);
}

#[test]
fn agent_runtime_registers_builtins() {
    let rt = AgentRuntime::default();
    let ids = rt.register_builtins();
    assert_eq!(ids.len(), 6);
    assert_eq!(rt.count(), 6);
}

#[test]
fn agent_runtime_find_by_archetype() {
    let rt = AgentRuntime::default();
    rt.register_builtins();
    let dev = rt.find_by_archetype(AgentArchetype::Developer);
    assert!(dev.is_some());
    assert_eq!(dev.unwrap().archetype, AgentArchetype::Developer);
}

#[test]
fn agent_runtime_send_and_drain_inbox() {
    let rt = AgentRuntime::default();
    let ids = rt.register_builtins();
    let from = ids[0];
    let to = ids[1];
    let msg = AgentMessage::new(
        from,
        Some(to),
        MessageKind::TaskAssignment,
        "test task",
        json!({"task_id": "abc"}),
        1,
    );
    rt.send(msg);
    let inbox = rt.drain_inbox(&to);
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].subject, "test task");
}

#[test]
fn agent_runtime_broadcast_reaches_all_except_sender() {
    let rt = AgentRuntime::default();
    let ids = rt.register_builtins();
    let from = ids[0];
    let msg = AgentMessage::new(
        from,
        None,
        MessageKind::StatusUpdate,
        "broadcast",
        json!({}),
        1,
    );
    rt.send(msg);
    for (i, id) in ids.iter().enumerate() {
        let inbox = rt.drain_inbox(id);
        if i == 0 {
            assert_eq!(inbox.len(), 0, "sender should not receive own broadcast");
        } else {
            assert_eq!(inbox.len(), 1, "recipient {} should have 1 message", i);
        }
    }
}

#[test]
fn agent_runtime_dispatch_creates_task_assignment() {
    let rt = AgentRuntime::default();
    rt.register_builtins();
    let result = rt
        .dispatch(
            AgentArchetype::Developer,
            "implement feature X",
            "build the user authentication module",
            1,
            1000,
        )
        .expect("dispatch should succeed");
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].kind, MessageKind::TaskAssignment);
    assert_eq!(result.messages[0].subject, "implement feature X");
}

#[test]
fn agent_runtime_delegate_creates_delegation_message() {
    let rt = AgentRuntime::default();
    let ids = rt.register_builtins();
    let from = ids[0]; // architect
    let result = rt
        .delegate(
            from,
            AgentArchetype::Developer,
            "subtask",
            "implement the auth module",
            1,
        )
        .expect("delegate should succeed");
    assert_eq!(result.messages[0].kind, MessageKind::Delegation);
    assert_eq!(result.messages[0].from, from);
}

#[test]
fn agent_activated_event_persists_record() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let agent = Developer::new();
    let agent_id = agent.id;
    let event = RawEvent::new(
        "agent.activated",
        json!({"agent": agent}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &event).unwrap();
    let as_ = AgentState::from_state(&state).unwrap();
    assert_eq!(as_.agents.len(), 1);
    let record = as_.agents.get(&agent_id.0).unwrap();
    assert_eq!(record.status, AgentStatus::Active);
}

#[test]
fn agent_message_sent_event_records_message() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    // First activate an agent.
    let agent = Developer::new();
    let agent_id = agent.id;
    let e1 = RawEvent::new(
        "agent.activated",
        json!({"agent": agent}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    // Then send a message.
    let msg = AgentMessage::new(
        agent_id,
        None,
        MessageKind::StatusUpdate,
        "working",
        json!({}),
        2,
    );
    let e2 = RawEvent::new(
        "agent.message_sent",
        serde_json::to_value(&msg).unwrap(),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let as_ = AgentState::from_state(&state).unwrap();
    let record = as_.agents.get(&agent_id.0).unwrap();
    assert_eq!(record.messages_sent.len(), 1);
}

#[test]
fn agent_dispatched_event_records_task() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();

    let agent = Developer::new();
    let agent_id = agent.id;
    let e1 = RawEvent::new(
        "agent.activated",
        json!({"agent": agent}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();

    let task_id = uuid::Uuid::now_v7();
    let e2 = RawEvent::new(
        "agent.dispatched",
        json!({"agent_id": agent_id, "task_id": task_id}),
        Actor::owner(),
        0,
    )
    .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();

    let as_ = AgentState::from_state(&state).unwrap();
    let record = as_.agents.get(&agent_id.0).unwrap();
    assert!(record.tasks_dispatched.contains(&task_id));
    assert_eq!(record.status, AgentStatus::Active);
}

#[test]
fn agent_status_transitions() {
    let pipeline = fresh_pipeline();
    let mut state = CanonicalState::genesis();
    let agent = Developer::new();
    let agent_id = agent.id;
    let e1 = RawEvent::new(
        "agent.activated",
        json!({"agent": agent}),
        Actor::owner(),
        0,
    )
    .finalize(1, EventHash::GENESIS);
    pipeline.apply(&mut state, &e1).unwrap();
    assert_eq!(
        AgentState::from_state(&state).unwrap().agents.get(&agent_id.0).unwrap().status,
        AgentStatus::Active
    );

    let e2 = RawEvent::new("agent.idle", json!({"id": agent_id}), Actor::owner(), 0)
        .finalize(2, e1.hash);
    pipeline.apply(&mut state, &e2).unwrap();
    assert_eq!(
        AgentState::from_state(&state).unwrap().agents.get(&agent_id.0).unwrap().status,
        AgentStatus::Idle
    );

    let e3 = RawEvent::new("agent.blocked", json!({"id": agent_id}), Actor::owner(), 0)
        .finalize(3, e2.hash);
    pipeline.apply(&mut state, &e3).unwrap();
    assert_eq!(
        AgentState::from_state(&state).unwrap().agents.get(&agent_id.0).unwrap().status,
        AgentStatus::Blocked
    );

    let e4 = RawEvent::new("agent.deactivated", json!({"id": agent_id}), Actor::owner(), 0)
        .finalize(4, e3.hash);
    pipeline.apply(&mut state, &e4).unwrap();
    assert_eq!(
        AgentState::from_state(&state).unwrap().agents.get(&agent_id.0).unwrap().status,
        AgentStatus::Deactivated
    );
}
