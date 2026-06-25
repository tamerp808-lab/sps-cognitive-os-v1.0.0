//! Phase 11C: FactorySupervisor integration tests.
//!
//! Verifies that the supervisor correctly:
//! 1. Retries on stage_failed (within max_retries)
//! 2. Rollbacks after retries exhausted
//! 3. Aborts on critical stage failure
//! 4. Dispatches supervisor_decision events
//! 5. observe_and_act end-to-end dispatches correct events

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::sink::EventSink;
use sps_core::storage::port::StoragePort;
use sps_factory::reducer::{FactoryRun, FactoryRunStatus};
use sps_factory::supervisor::{
    FactorySupervisor, SupervisorAction, SupervisorPolicy,
};
use sps_factory::workflow::FactoryStage;
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let mut typed_reg = sps_core::state::TypedExtensionRegistry::new();
    sps_factory::reducer::FactoryReducer::register_typed_extensions(&mut typed_reg);
    let config = KernelConfig::default().with_typed_registry(typed_reg);
    SpsKernel::boot_with(storage, config, |reg| {
        sps_factory::reducer::FactoryReducer::register(reg);
    })
    .unwrap()
    .into()
}

fn dispatch_stage_failed(kernel: &SpsKernel, run_id: uuid::Uuid, stage: &str, reason: &str) {
    let payload = json!({
        "id": run_id.to_string(),
        "stage": stage,
        "reason": reason,
    });
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.stage_failed",
            payload,
            Actor::system("test"),
            0,
        ))
        .unwrap();
}

fn dispatch_run_started(kernel: &SpsKernel, run_id: uuid::Uuid) {
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.run_started",
            json!({"id": run_id.to_string(), "project_name": "test"}),
            Actor::system("test"),
            0,
        ))
        .unwrap();
}

fn get_run(kernel: &SpsKernel, run_id: uuid::Uuid) -> FactoryRun {
    kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&run_id).cloned())
            .expect("run should exist")
    })
}

#[test]
fn phase11c_test_1_supervisor_retries_on_stage_failed() {
    println!("\n=== Phase 11C Test 1: Supervisor retries on stage_failed ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    dispatch_run_started(&kernel, run_id);

    let supervisor = FactorySupervisor::default_policy();
    let run = get_run(&kernel, run_id);

    // Simulate a stage_failed event.
    let raw = RawEvent::new(
        "factory.stage_failed",
        json!({"id": run_id.to_string(), "stage": "testing", "reason": "flaky test"}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);

    let action = supervisor.decide(&event, Some(&run));
    match action {
        SupervisorAction::Retry { attempt, stage, .. } => {
            assert_eq!(attempt, 1, "first retry should be attempt 1");
            assert_eq!(stage, FactoryStage::Testing);
            println!("  PASS — Supervisor decided Retry (attempt 1, stage = Testing)");
        }
        other => panic!("expected Retry, got {:?}", other),
    }

    // Execute the decision — should dispatch supervisor_decision + run_retried.
    let sink: &dyn EventSink = kernel.as_ref();
    supervisor.execute_decision(action, sink).unwrap();

    // Verify run_retried was dispatched and retry_count incremented.
    let run_after = get_run(&kernel, run_id);
    assert_eq!(run_after.retry_count, 1, "retry_count should be 1 after first retry");
    println!("  PASS — retry_count incremented to 1");

    // Verify supervisor_decision event was dispatched.
    let events = kernel.store().read_from(0, 100).unwrap();
    let has_decision = events.iter().any(|e| {
        e.event_type.as_str() == "factory.supervisor_decision"
            && e.payload.get("action").and_then(|v| v.as_str()) == Some("retry")
    });
    assert!(has_decision, "FAIL: no supervisor_decision (retry) event found");
    println!("  PASS — supervisor_decision (retry) event dispatched");
}

#[test]
fn phase11c_test_2_supervisor_rollbacks_after_max_retries() {
    println!("\n=== Phase 11C Test 2: Supervisor rollbacks after max_retries ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    dispatch_run_started(&kernel, run_id);

    let supervisor = FactorySupervisor::default_policy();
    let sink: &dyn EventSink = kernel.as_ref();

    // Simulate 2 retries (max_retries = 2).
    for _ in 0..2 {
        dispatch_stage_failed(&kernel, run_id, "testing", "flaky");
        let run = get_run(&kernel, run_id);
        let raw = RawEvent::new(
            "factory.stage_failed",
            json!({"id": run_id.to_string(), "stage": "testing", "reason": "flaky"}),
            Actor::system("test"),
            0,
        );
        let event = raw.finalize(1, EventHash::GENESIS);
        let action = supervisor.decide(&event, Some(&run));
        supervisor.execute_decision(action, sink).unwrap();
    }

    // Now retry_count = 2 (max). Next stage_failed should trigger Rollback.
    dispatch_stage_failed(&kernel, run_id, "testing", "flaky again");
    let run = get_run(&kernel, run_id);
    let raw = RawEvent::new(
        "factory.stage_failed",
        json!({"id": run_id.to_string(), "stage": "testing", "reason": "flaky again"}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);
    let action = supervisor.decide(&event, Some(&run));
    match &action {
        SupervisorAction::Rollback { reason, .. } => {
            assert!(reason.contains("retries exhausted"));
            println!("  PASS — Supervisor decided Rollback after retries exhausted");
        }
        other => panic!("expected Rollback, got {:?}", other),
    }

    // Execute rollback — should dispatch supervisor_decision + rollback_completed.
    supervisor.execute_decision(action, sink).unwrap();

    let run_after = get_run(&kernel, run_id);
    assert_eq!(
        run_after.status,
        FactoryRunStatus::RolledBack,
        "FAIL: expected RolledBack, got {:?}",
        run_after.status
    );
    println!("  PASS — Run status = RolledBack after supervisor rollback");
}

#[test]
fn phase11c_test_3_supervisor_aborts_on_critical_stage() {
    println!("\n=== Phase 11C Test 3: Supervisor aborts on critical stage ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    dispatch_run_started(&kernel, run_id);

    // Supervisor with CodeGeneration as critical stage.
    let policy = SupervisorPolicy {
        critical_stages: vec![FactoryStage::CodeGeneration],
        ..Default::default()
    };
    let supervisor = FactorySupervisor::new(policy);

    let run = get_run(&kernel, run_id);
    let raw = RawEvent::new(
        "factory.stage_failed",
        json!({"id": run_id.to_string(), "stage": "code_generation", "reason": "fatal"}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);

    let action = supervisor.decide(&event, Some(&run));
    match &action {
        SupervisorAction::Abort { reason, .. } => {
            assert!(reason.contains("critical"));
            println!("  PASS — Supervisor decided Abort on critical stage failure");
        }
        other => panic!("expected Abort, got {:?}", other),
    }

    // Execute abort.
    let sink: &dyn EventSink = kernel.as_ref();
    supervisor.execute_decision(action, sink).unwrap();

    let run_after = get_run(&kernel, run_id);
    assert_eq!(
        run_after.status,
        FactoryRunStatus::Failed,
        "FAIL: expected Failed after abort, got {:?}",
        run_after.status
    );
    assert!(run_after.failure_reason.is_some(), "FAIL: failure_reason should be set");
    println!("  PASS — Run status = Failed, failure_reason set after abort");
}

#[test]
fn phase11c_test_4_supervisor_no_action_on_informational_events() {
    println!("\n=== Phase 11C Test 4: No action on informational events ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    dispatch_run_started(&kernel, run_id);

    let supervisor = FactorySupervisor::default_policy();
    let run = get_run(&kernel, run_id);

    // stage_started — informational, no action.
    let raw = RawEvent::new(
        "factory.stage_started",
        json!({"id": run_id.to_string(), "stage": "testing"}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);
    let action = supervisor.decide(&event, Some(&run));
    assert_eq!(action, SupervisorAction::NoAction);
    println!("  PASS — NoAction on stage_started");

    // stage_completed — informational, no action.
    let raw = RawEvent::new(
        "factory.stage_completed",
        json!({"id": run_id.to_string(), "stage": "testing", "files_generated": 0}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);
    let action = supervisor.decide(&event, Some(&run));
    assert_eq!(action, SupervisorAction::NoAction);
    println!("  PASS — NoAction on stage_completed");

    // run_completed — informational, no action.
    let raw = RawEvent::new(
        "factory.run_completed",
        json!({"id": run_id.to_string()}),
        Actor::system("test"),
        0,
    );
    let event = raw.finalize(1, EventHash::GENESIS);
    let action = supervisor.decide(&event, Some(&run));
    assert_eq!(action, SupervisorAction::NoAction);
    println!("  PASS — NoAction on run_completed");
}

#[test]
fn phase11c_test_5_observe_and_act_end_to_end() {
    println!("\n=== Phase 11C Test 5: observe_and_act end-to-end ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    dispatch_run_started(&kernel, run_id);

    let supervisor = FactorySupervisor::default_policy();
    let sink: &dyn EventSink = kernel.as_ref();

    // Simulate stage_failed + observe_and_act in one call.
    let raw = RawEvent::new(
        "factory.stage_failed",
        json!({"id": run_id.to_string(), "stage": "testing", "reason": "fail"}),
        Actor::system("test"),
        0,
    );
    // First dispatch the event so it's in the store.
    kernel.dispatch_trusted(raw.clone()).unwrap();

    // Now observe the dispatched event and act.
    let events = kernel.store().read_from(0, 100).unwrap();
    let stage_failed_event = events
        .iter()
        .find(|e| e.event_type.as_str() == "factory.stage_failed")
        .expect("stage_failed event should exist");

    let run = get_run(&kernel, run_id);
    let action = supervisor
        .observe_and_act(stage_failed_event, Some(&run), sink)
        .unwrap();

    match action {
        SupervisorAction::Retry { attempt, .. } => {
            assert_eq!(attempt, 1);
            println!("  PASS — observe_and_act returned Retry (attempt 1)");
        }
        other => panic!("expected Retry, got {:?}", other),
    }

    // Verify both supervisor_decision + run_retried events were dispatched.
    let events_after = kernel.store().read_from(0, 100).unwrap();
    let has_decision = events_after.iter().any(|e| {
        e.event_type.as_str() == "factory.supervisor_decision"
    });
    let has_retried = events_after.iter().any(|e| {
        e.event_type.as_str() == "factory.run_retried"
    });
    assert!(has_decision, "FAIL: supervisor_decision not dispatched");
    assert!(has_retried, "FAIL: run_retried not dispatched");
    println!("  PASS — Both supervisor_decision + run_retried dispatched");

    // Verify hash chain intact.
    let report = kernel.verify().unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — Hash chain intact ({} events)", report.events_verified);
}
