//! Phase 11A: Factory Hardening Tests.
//!
//! Verifies that all 8 stages are materialized, plus retry + rollback
//! semantics work correctly.
//!
//! Tests:
//! 1. All 8 stages materialized (stage_started + stage_completed for each)
//! 2. stage_failed + run_failed on validation failure
//! 3. retry_run dispatches run_retried + new run succeeds
//! 4. rollback_run dispatches rollback_completed + status = RolledBack
//! 5. FactoryRun tracks failure_reason, failed_stage, retry_count, generated_file_paths

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::sink::EventSink;
use sps_core::storage::port::StoragePort;
use sps_factory::workflow::{FactoryStage, FactoryWorkflow, ProjectRequest};
use sps_storage_memory::InMemoryStorage;
use smol_str::SmolStr;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let mut typed_reg = sps_core::state::TypedExtensionRegistry::new();
    sps_factory::reducer::FactoryReducer::register_typed_extensions(&mut typed_reg);
    sps_world::reducer::WorldReducer::register_typed_extensions(&mut typed_reg);
    sps_execution::reducer::ExecutionReducer::register_typed_extensions(&mut typed_reg);
    let config = KernelConfig::default().with_typed_registry(typed_reg);
    SpsKernel::boot_with(storage, config, |reg| {
        sps_factory::reducer::FactoryReducer::register(reg);
        sps_world::reducer::WorldReducer::register(reg);
        sps_execution::reducer::ExecutionReducer::register(reg);
    })
    .unwrap()
    .into()
}

fn make_request(desc: &str) -> ProjectRequest {
    ProjectRequest {
        description: desc.into(),
        preferred_name: Some(SmolStr::new("phase11a-test")),
        output_dir: Some("/tmp/phase11a-test".into()),
    }
}

#[test]
fn phase11a_test_1_all_8_stages_materialized() {
    println!("\n=== Phase 11A Test 1: All 8 stages materialized ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    let result = FactoryWorkflow::run_with_sink(
        make_request("rust cli tool"),
        "/tmp/phase11a-test",
        sink,
        None,
    )
    .expect("factory run failed");

    let (stages, status, file_count) = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| {
                fs.runs.get(&result.run_id).map(|r| {
                    (
                        r.completed_stages.len(),
                        r.status,
                        r.generated_file_paths.len(),
                    )
                })
            })
            .unwrap_or((0, sps_factory::reducer::FactoryRunStatus::Running, 0))
    });

    assert_eq!(
        stages,
        8,
        "FAIL: expected 8 completed stages, got {}",
        stages
    );
    assert_eq!(
        status,
        sps_factory::reducer::FactoryRunStatus::Completed,
        "FAIL: expected Completed status, got {:?}",
        status
    );
    assert!(
        file_count > 0,
        "FAIL: expected >0 generated_file_paths, got {}",
        file_count
    );
    println!("  PASS — 8 stages materialized, status = Completed");
    println!("  PASS — {} file paths tracked", file_count);
}

#[test]
fn phase11a_test_2_stage_failed_on_validation() {
    println!("\n=== Phase 11A Test 2: stage_failed + run_failed on validation ===");
    // We can't easily inject a failure into run_with_sink (it always succeeds
    // with the current implementation). Instead, we dispatch stage_failed +
    // run_failed manually and verify the reducer materializes them correctly.
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let run_id = uuid::Uuid::now_v7();
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.run_started",
            json!({"id": run_id.to_string(), "project_name": "test"}),
            Actor::system("test"),
            0,
        ))
        .unwrap();
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.stage_started",
            json!({"id": run_id.to_string(), "stage": "validation"}),
            Actor::system("test"),
            0,
        ))
        .unwrap();
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.stage_failed",
            json!({
                "id": run_id.to_string(),
                "stage": "validation",
                "reason": "compile error: missing semicolon"
            }),
            Actor::system("test"),
            0,
        ))
        .unwrap();
    kernel
        .dispatch_trusted(RawEvent::new(
            "factory.run_failed",
            json!({"id": run_id.to_string()}),
            Actor::system("test"),
            0,
        ))
        .unwrap();

    let (status, failure_reason, failed_stage) = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| {
                fs.runs.get(&run_id).map(|r| {
                    (
                        r.status,
                        r.failure_reason.clone(),
                        r.failed_stage,
                    )
                })
            })
            .unwrap_or((sps_factory::reducer::FactoryRunStatus::Running, None, None))
    });

    assert_eq!(
        status,
        sps_factory::reducer::FactoryRunStatus::Failed,
        "FAIL: expected Failed status, got {:?}",
        status
    );
    assert_eq!(
        failure_reason,
        Some("compile error: missing semicolon".to_string()),
        "FAIL: failure_reason mismatch"
    );
    assert_eq!(
        failed_stage,
        Some(FactoryStage::Validation),
        "FAIL: failed_stage mismatch"
    );
    println!("  PASS — status = Failed, reason = compile error, stage = Validation");
}

#[test]
fn phase11a_test_3_retry_run_dispatches_run_retried() {
    println!("\n=== Phase 11A Test 3: retry_run dispatches run_retried ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    // First run (succeeds).
    let result1 = FactoryWorkflow::run_with_sink(
        make_request("rust cli"),
        "/tmp/phase11a-retry",
        sink,
        None,
    )
    .unwrap();

    // Retry the run (simulating retry after a failure).
    let result2 = FactoryWorkflow::retry_run(
        make_request("rust cli"),
        "/tmp/phase11a-retry",
        result1.run_id,
        sink,
        None,
    )
    .unwrap();

    // The original run should have retry_count = 1.
    let retry_count = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result1.run_id).map(|r| r.retry_count))
            .unwrap_or(0)
    });
    assert_eq!(
        retry_count, 1,
        "FAIL: expected retry_count = 1, got {}",
        retry_count
    );

    // The new run (result2) should be a fresh run with 8 stages completed.
    let (stages, status) = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| {
                fs.runs
                    .get(&result2.run_id)
                    .map(|r| (r.completed_stages.len(), r.status))
            })
            .unwrap_or((0, sps_factory::reducer::FactoryRunStatus::Running))
    });
    assert_eq!(stages, 8, "FAIL: retry run should have 8 stages, got {}", stages);
    assert_eq!(
        status,
        sps_factory::reducer::FactoryRunStatus::Completed,
        "FAIL: retry run should be Completed"
    );
    println!("  PASS — original run retry_count = 1, retry run completed with 8 stages");
}

#[test]
fn phase11a_test_4_rollback_run_dispatches_rollback_completed() {
    println!("\n=== Phase 11A Test 4: rollback_run dispatches rollback_completed ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    // Run a factory workflow.
    let result = FactoryWorkflow::run_with_sink(
        make_request("rust cli"),
        "/tmp/phase11a-rollback",
        sink,
        None,
    )
    .unwrap();

    // Verify the run has generated_file_paths.
    let file_count_before = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).map(|r| r.generated_file_paths.len()))
            .unwrap_or(0)
    });
    assert!(file_count_before > 0, "FAIL: expected files before rollback");

    // Rollback the run.
    FactoryWorkflow::rollback_run(result.run_id, sink).unwrap();

    let (status, file_count_after) = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| {
                fs.runs
                    .get(&result.run_id)
                    .map(|r| (r.status, r.generated_file_paths.len()))
            })
            .unwrap_or((sps_factory::reducer::FactoryRunStatus::Running, 0))
    });
    assert_eq!(
        status,
        sps_factory::reducer::FactoryRunStatus::RolledBack,
        "FAIL: expected RolledBack, got {:?}",
        status
    );
    assert_eq!(
        file_count_after, 0,
        "FAIL: expected 0 file_paths after rollback, got {}",
        file_count_after
    );
    println!("  PASS — status = RolledBack, file_paths cleared");
}

#[test]
fn phase11a_test_5_factory_run_tracks_all_fields() {
    println!("\n=== Phase 11A Test 5: FactoryRun tracks all fields ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    let result = FactoryWorkflow::run_with_sink(
        make_request("rust cli tool for testing"),
        "/tmp/phase11a-fields",
        sink,
        None,
    )
    .unwrap();

    let run = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).cloned())
            .expect("run should exist")
    });

    // Verify all Phase 11A.7 fields are populated.
    assert_eq!(run.id, result.run_id);
    assert!(!run.project_name.is_empty(), "project_name should be non-empty");
    assert_eq!(run.completed_stages.len(), 8);
    assert_eq!(run.current_stage, None); // terminal
    assert_eq!(run.status, sps_factory::reducer::FactoryRunStatus::Completed);
    assert!(run.files_generated > 0);
    assert_eq!(run.failure_reason, None);
    assert_eq!(run.failed_stage, None);
    assert_eq!(run.retry_count, 0);
    assert!(!run.generated_file_paths.is_empty());
    assert!(run.output_dir.is_some());
    println!("  PASS — all FactoryRun fields correctly populated");
    println!("    project_name: {:?}", run.project_name);
    println!("    files_generated: {}", run.files_generated);
    println!("    generated_file_paths: {} entries", run.generated_file_paths.len());
    println!("    output_dir: {:?}", run.output_dir);
}
