//! Phase 12C: Self-Improvement Loop integration test.
//!
//! Verifies the full loop: factory stage_failed → SelfImprovementLoop
//! detects pattern → proposes improvement → improvement.proposed dispatched.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::sink::EventSink;
use sps_core::state::TypedExtensionRegistry;
use sps_core::storage::port::StoragePort;
use sps_improvement::loop_engine::SelfImprovementLoop;
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let mut typed_reg = TypedExtensionRegistry::new();
    sps_factory::reducer::FactoryReducer::register_typed_extensions(&mut typed_reg);
    sps_improvement::reducer::ImprovementReducer::register_typed_extensions(&mut typed_reg);
    let config = KernelConfig::default().with_typed_registry(typed_reg);
    SpsKernel::boot_with(storage, config, |reg| {
        sps_factory::reducer::FactoryReducer::register(reg);
        sps_improvement::reducer::ImprovementReducer::register(reg);
    })
    .unwrap()
    .into()
}

#[test]
fn phase12c_self_improvement_loop_proposes_on_repeated_failures() {
    println!("\n=== Phase 12C: Self-Improvement Loop ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();
    let loop_ = SelfImprovementLoop::new();

    let run_id = uuid::Uuid::now_v7();
    // Start a factory run.
    kernel.dispatch_trusted(RawEvent::new(
        "factory.run_started",
        json!({"id": run_id.to_string(), "project_name": "test"}),
        Actor::system("test"),
        0,
    )).unwrap();

    // Simulate 4 retry cycles to exceed the threshold of 3.
    let mut proposed = false;
    for i in 0..4 {
        // Dispatch stage_failed.
        kernel.dispatch_trusted(RawEvent::new(
            "factory.stage_failed",
            json!({"id": run_id.to_string(), "stage": "testing", "reason": "flaky"}),
            Actor::system("test"),
            0,
        )).unwrap();

        // Dispatch run_retried to increment retry_count.
        kernel.dispatch_trusted(RawEvent::new(
            "factory.run_retried",
            json!({"id": run_id.to_string()}),
            Actor::system("test"),
            0,
        )).unwrap();

        // Now get the run's current state (retry_count should be i+1).
        let run = kernel.query(|s| {
            sps_factory::reducer::FactoryState::from_state(s)
                .and_then(|fs| fs.runs.get(&run_id).cloned())
                .unwrap_or_default()
        });

        println!("  Iteration {}: retry_count = {}", i + 1, run.retry_count);

        // Read the latest stage_failed event.
        let events = kernel.store().read_from(0, 100).unwrap();
        let stage_failed = events.iter()
            .rev()
            .find(|e| e.event_type.as_str() == "factory.stage_failed")
            .unwrap();

        // Analyze and propose if pattern detected.
        if let Some(pattern) = loop_.analyze_factory_event(stage_failed, Some(&run)) {
            let proposal_id = loop_.propose(pattern, sink).unwrap();
            println!("  Proposed improvement: {}", proposal_id);
            proposed = true;
            break;
        }
    }

    assert!(proposed, "FAIL: SelfImprovementLoop should have proposed after 4 failures");

    // Verify the improvement proposal is in ImprovementState.
    let improvement_state = kernel.query(|s| {
        sps_improvement::reducer::ImprovementState::from_state(s)
    });
    assert!(improvement_state.is_some(), "FAIL: ImprovementState missing");
    let proposals = improvement_state.unwrap().proposals;
    assert!(!proposals.is_empty(), "FAIL: no proposals in ImprovementState");
    println!("  PASS — {} improvement proposal(s) materialized", proposals.len());

    // Verify hash chain intact.
    let report = kernel.verify().unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — Hash chain intact ({} events)", report.events_verified);
}
