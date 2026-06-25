//! Phase 11B: Effect-Based Factory tests.
//!
//! Verifies that the factory now dispatches effect.intent + effect.executed
//! events through the EventSink instead of performing direct actions.
//! All 4 new effect types are exercised:
//! - WriteFile (during CodeGeneration)
//! - RunTests (during Testing)
//! - BuildProject (during Validation)
//! - PackageProject (during Packaging)
//!
//! Tests:
//! 1. WriteFile effect dispatched for each generated file
//! 2. RunTests effect dispatched during Testing stage
//! 3. BuildProject effect dispatched during Validation stage
//! 4. PackageProject effect dispatched during Packaging stage
//! 5. Effect types registered correctly in FactoryExecutor
//! 6. End-to-end: factory run produces effect.intent + effect.executed events

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::sink::EventSink;
use sps_core::storage::port::StoragePort;
use sps_effects::effect::EffectType;
use sps_effects::executors::{FactoryExecutor, FactoryExecutorConfig};
use sps_effects::registry::EffectExecutor;
use sps_factory::workflow::{FactoryWorkflow, ProjectRequest};
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
        preferred_name: Some(SmolStr::new("phase11b-test")),
        output_dir: Some("/tmp/phase11b-test".into()),
    }
}

#[test]
fn phase11b_test_1_write_file_effect_dispatched() {
    println!("\n=== Phase 11B Test 1: WriteFile effect dispatched per generated file ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    let result = FactoryWorkflow::run_with_sink(
        make_request("rust cli tool"),
        "/tmp/phase11b-write",
        sink,
        None,
    )
    .unwrap();

    // Count effect.intent events for factory.write_file.
    let events = kernel.store().read_from(0, 1000).unwrap();
    let write_file_intents: Vec<_> = events
        .iter()
        .filter(|e| {
            e.event_type.as_str() == "effect.intent"
                && e.payload.get("effect_type").and_then(|v| v.as_str()) == Some("factory.write_file")
        })
        .collect();
    let write_file_executed: Vec<_> = events
        .iter()
        .filter(|e| {
            e.event_type.as_str() == "effect.executed"
                && e.payload.get("effect_type").and_then(|v| v.as_str()) == Some("factory.write_file")
        })
        .collect();

    assert!(
        !write_file_intents.is_empty(),
        "FAIL: expected >0 factory.write_file intents, got 0"
    );
    assert_eq!(
        write_file_intents.len(),
        write_file_executed.len(),
        "FAIL: intent count ({}) != executed count ({})",
        write_file_intents.len(),
        write_file_executed.len()
    );
    println!(
        "  PASS — {} WriteFile effects dispatched (intent + executed pairs)",
        write_file_intents.len()
    );

    // Verify the generated_file_paths count matches.
    let file_paths = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).map(|r| r.generated_file_paths.len()))
            .unwrap_or(0)
    });
    assert!(
        file_paths >= write_file_intents.len(),
        "FAIL: generated_file_paths ({}) < WriteFile intents ({})",
        file_paths,
        write_file_intents.len()
    );
    println!("  PASS — generated_file_paths tracked: {}", file_paths);
}

#[test]
fn phase11b_test_2_run_tests_effect_dispatched() {
    println!("\n=== Phase 11B Test 2: RunTests effect dispatched during Testing ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    FactoryWorkflow::run_with_sink(
        make_request("rust cli"),
        "/tmp/phase11b-tests",
        sink,
        None,
    )
    .unwrap();

    let events = kernel.store().read_from(0, 1000).unwrap();
    let run_tests: Vec<_> = events
        .iter()
        .filter(|e| {
            e.event_type.as_str() == "effect.intent"
                && e.payload.get("effect_type").and_then(|v| v.as_str()) == Some("factory.run_tests")
        })
        .collect();

    assert_eq!(
        run_tests.len(),
        1,
        "FAIL: expected exactly 1 RunTests intent, got {}",
        run_tests.len()
    );

    // Verify the intent has the right input.
    let intent = &run_tests[0];
    let input = &intent.payload["input"];
    let test_framework = input.get("test_framework").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        test_framework == "cargo" || test_framework == "npm",
        "FAIL: test_framework should be cargo/npm, got '{}'",
        test_framework
    );
    println!("  PASS — RunTests dispatched with framework = {}", test_framework);
}

#[test]
fn phase11b_test_3_build_project_effect_dispatched() {
    println!("\n=== Phase 11B Test 3: BuildProject effect dispatched during Validation ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    FactoryWorkflow::run_with_sink(
        make_request("rust cli"),
        "/tmp/phase11b-build",
        sink,
        None,
    )
    .unwrap();

    let events = kernel.store().read_from(0, 1000).unwrap();
    let build: Vec<_> = events
        .iter()
        .filter(|e| {
            e.event_type.as_str() == "effect.intent"
                && e.payload.get("effect_type").and_then(|v| v.as_str()) == Some("factory.build_project")
        })
        .collect();

    assert_eq!(
        build.len(),
        1,
        "FAIL: expected exactly 1 BuildProject intent, got {}",
        build.len()
    );
    println!("  PASS — BuildProject dispatched during Validation stage");
}

#[test]
fn phase11b_test_4_package_project_effect_dispatched() {
    println!("\n=== Phase 11B Test 4: PackageProject effect dispatched during Packaging ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    FactoryWorkflow::run_with_sink(
        make_request("rust cli"),
        "/tmp/phase11b-package",
        sink,
        None,
    )
    .unwrap();

    let events = kernel.store().read_from(0, 1000).unwrap();
    let package: Vec<_> = events
        .iter()
        .filter(|e| {
            e.event_type.as_str() == "effect.intent"
                && e.payload.get("effect_type").and_then(|v| v.as_str()) == Some("factory.package_project")
        })
        .collect();

    assert_eq!(
        package.len(),
        1,
        "FAIL: expected exactly 1 PackageProject intent, got {}",
        package.len()
    );
    println!("  PASS — PackageProject dispatched during Packaging stage");
}

#[test]
fn phase11b_test_5_factory_executor_handles_all_4_types() {
    println!("\n=== Phase 11B Test 5: FactoryExecutor handles all 4 effect types ===");
    let config = FactoryExecutorConfig::default();
    let executor = FactoryExecutor::new(config);

    // WriteFile
    let intent = sps_effects::effect::EffectIntent::new(
        EffectType::WriteFile,
        json!({"path": "test.txt", "content": "hello"}),
    );
    let result = executor.execute(&intent, 1).expect("WriteFile should succeed");
    assert!(result.output.get("bytes_written").is_some());
    println!("  PASS — WriteFile executor returns bytes_written");

    // RunTests
    let intent = sps_effects::effect::EffectIntent::new(
        EffectType::RunTests,
        json!({"project_path": "/tmp", "test_framework": "cargo"}),
    );
    let result = executor.execute(&intent, 2).expect("RunTests should succeed");
    assert_eq!(
        result.output.get("passed").and_then(|v| v.as_bool()),
        Some(true),
        "dry_run mode should pass tests"
    );
    println!("  PASS — RunTests executor returns passed=true (dry_run)");

    // BuildProject
    let intent = sps_effects::effect::EffectIntent::new(
        EffectType::BuildProject,
        json!({"project_path": "/tmp", "build_system": "cargo"}),
    );
    let result = executor.execute(&intent, 3).expect("BuildProject should succeed");
    assert_eq!(
        result.output.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "dry_run mode should succeed build"
    );
    println!("  PASS — BuildProject executor returns success=true (dry_run)");

    // PackageProject
    let intent = sps_effects::effect::EffectIntent::new(
        EffectType::PackageProject,
        json!({"project_path": "/tmp", "format": "tarball"}),
    );
    let result = executor.execute(&intent, 4).expect("PackageProject should succeed");
    assert!(result.output.get("artifact_path").is_some());
    println!("  PASS — PackageProject executor returns artifact_path");

    // Verify unknown effect type returns error.
    let intent = sps_effects::effect::EffectIntent::new(
        EffectType::LlmComplete,
        json!({}),
    );
    let result = executor.execute(&intent, 5);
    assert!(result.is_err(), "FAIL: unknown effect type should error");
    println!("  PASS — Unknown effect type returns NoExecutor error");
}

#[test]
fn phase11b_test_6_end_to_end_all_effects_in_event_stream() {
    println!("\n=== Phase 11B Test 6: End-to-end — all 4 effect types in event stream ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());
    let sink: &dyn EventSink = kernel.as_ref();

    FactoryWorkflow::run_with_sink(
        make_request("rust cli tool"),
        "/tmp/phase11b-e2e",
        sink,
        None,
    )
    .unwrap();

    let events = kernel.store().read_from(0, 1000).unwrap();
    let effect_types: Vec<String> = events
        .iter()
        .filter(|e| e.event_type.as_str() == "effect.intent")
        .map(|e| {
            e.payload
                .get("effect_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();

    println!("  Effect types dispatched: {:?}", effect_types);

    // Verify all 4 factory effect types are present.
    assert!(
        effect_types.iter().any(|t| t == "factory.write_file"),
        "FAIL: factory.write_file not in event stream"
    );
    assert!(
        effect_types.iter().any(|t| t == "factory.run_tests"),
        "FAIL: factory.run_tests not in event stream"
    );
    assert!(
        effect_types.iter().any(|t| t == "factory.build_project"),
        "FAIL: factory.build_project not in event stream"
    );
    assert!(
        effect_types.iter().any(|t| t == "factory.package_project"),
        "FAIL: factory.package_project not in event stream"
    );

    println!("  PASS — All 4 factory effect types dispatched");

    // Count total events: should have intent + executed pairs for each.
    let intents = events.iter().filter(|e| e.event_type.as_str() == "effect.intent").count();
    let executed = events.iter().filter(|e| e.event_type.as_str() == "effect.executed").count();
    assert_eq!(
        intents, executed,
        "FAIL: intent count ({}) != executed count ({})",
        intents, executed
    );
    println!("  PASS — {} intent+executed pairs in event stream", intents);

    // Verify hash chain integrity (all events are part of the chain).
    let report = kernel.verify().unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — Hash chain intact ({} events verified)", report.events_verified);
}
