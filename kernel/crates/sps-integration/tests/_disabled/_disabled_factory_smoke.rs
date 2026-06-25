//! Factory Smoke Test — verify Fix #12a + #12b before full validation.

use std::sync::Arc;

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::reducer::ReducerRegistry;
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::storage::port::StoragePort;
use sps_factory::workflow::{FactoryWorkflow, ProjectRequest};
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    let kernel = SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
        sps_bus::state_ext::OwnerReducer::register(reg);
        sps_goals::reducer::GoalReducer::register(reg);
        sps_memory::reducer::MemoryReducer::register(reg);
        sps_reflection::reducer::ReflectionReducer::register(reg);
        sps_planner::reducer::PlannerReducer::register(reg);
        sps_world::reducer::WorldReducer::register(reg);
        sps_agents::reducer::AgentReducer::register(reg);
        sps_reasoning::reducer::ReasoningReducer::register(reg);
        sps_improvement::reducer::ImprovementReducer::register(reg);
        sps_execution::reducer::ExecutionReducer::register(reg);
        sps_factory::reducer::FactoryReducer::register(reg);
        sps_autonomy::reducer::AutonomyReducer::register(reg);
        sps_vectors::reducer::VectorReducer::register(reg);
    })
    .expect("kernel boot failed");
    Arc::new(kernel)
}

#[test]
fn factory_smoke_test_full_chain() {
    println!("\n=== FACTORY SMOKE TEST: run_with_sink → FactoryState + WorldState + Execution ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Run the factory workflow with event dispatching (Fix #12b).
    let request = ProjectRequest {
        description: "A simple CLI tool in Rust".to_string(),
        preferred_name: Some(SmolStr::new("my-cli-tool")),
        output_dir: Some("/tmp/test-output".to_string()),
    };
    let result = FactoryWorkflow::run_with_sink(
        request,
        "/tmp/test-output",
        kernel.as_ref() as &dyn sps_core::sink::EventSink,
        None,
    ).expect("factory run failed");
    println!("  Step 1: FactoryWorkflow::run_with_sink completed");
    println!("    run_id={}...", &result.run_id.to_string()[..8]);
    println!("    project_id={}...", &result.project_id.to_string()[..8]);
    println!("    files_generated={}", result.files.len());

    // Verify FactoryState has 1 run.
    let factory_runs = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .map(|fs| fs.runs.len())
            .unwrap_or(0)
    });
    if factory_runs == 1 {
        println!("  PASS — FactoryState.runs == 1");
    } else {
        println!("  FAIL — FactoryState.runs == {} (expected 1)", factory_runs);
        panic!("FACTORY SMOKE FAILED at FactoryState");
    }

    // Verify WorldState has 1 project.
    let world_projects = kernel.query(|s| {
        sps_world::reducer::WorldState::from_state(s)
            .map(|ws| ws.graph.projects.len())
            .unwrap_or(0)
    });
    if world_projects == 1 {
        println!("  PASS — WorldState.projects == 1");
    } else {
        println!("  FAIL — WorldState.projects == {} (expected 1)", world_projects);
        panic!("FACTORY SMOKE FAILED at WorldState.projects");
    }

    // Verify WorldState has files > 0.
    let world_files = kernel.query(|s| {
        sps_world::reducer::WorldState::from_state(s)
            .map(|ws| ws.graph.files.len())
            .unwrap_or(0)
    });
    if world_files > 0 {
        println!("  PASS — WorldState.files == {} (> 0)", world_files);
    } else {
        println!("  FAIL — WorldState.files == 0 (expected > 0)");
        panic!("FACTORY SMOKE FAILED at WorldState.files");
    }

    // Verify ExecutionState.for_factory_run returns the correct execution.
    let execs_for_run = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_factory_run(result.run_id).len()
    });
    if execs_for_run == 1 {
        println!("  PASS — ExecutionState.for_factory_run(run_id) == 1");
    } else {
        println!("  FAIL — for_factory_run == {} (expected 1)", execs_for_run);
        panic!("FACTORY SMOKE FAILED at Execution link");
    }

    // Verify factory run status is Completed.
    let run_status = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).map(|r| r.status))
    });
    if run_status == Some(sps_factory::reducer::FactoryRunStatus::Completed) {
        println!("  PASS — FactoryRun.status == Completed");
    } else {
        println!("  FAIL — status = {:?} (expected Completed)", run_status);
        panic!("FACTORY SMOKE FAILED at run status");
    }

    // Verify factory run has 2 completed stages.
    let stages = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).map(|r| r.completed_stages.len()))
            .unwrap_or(0)
    });
    if stages == 2 {
        println!("  PASS — FactoryRun.completed_stages == 2 (requirement_analysis + code_generation)");
    } else {
        println!("  FAIL — completed_stages == {} (expected 2)", stages);
        panic!("FACTORY SMOKE FAILED at stages");
    }

    // Step 2: Capture live state.
    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();
    let live_event_count = live.event_count();
    println!("\n  Step 2: live state has {} events, hash={}...",
        live_event_count, &live_hash.to_string()[..16]);

    // Step 3: Verify hash chain.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Step 4: Replay from genesis.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_factory::reducer::FactoryReducer::register(&mut reg);
        sps_world::reducer::WorldReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.event_count(), live_event_count, "FAIL: event_count mismatch");
    assert_eq!(replayed.last_hash(), live_hash, "FAIL: last_hash mismatch");
    println!("  PASS — replayed event_count + last_hash match live");

    // Step 5: Verify FactoryState identical.
    let live_fs = sps_factory::reducer::FactoryState::from_state(&live).unwrap();
    let replayed_fs = sps_factory::reducer::FactoryState::from_state(&replayed).unwrap();
    assert_eq!(live_fs.runs.len(), replayed_fs.runs.len(),
        "FAIL: factory run count mismatch");
    let live_run = live_fs.runs.get(&result.run_id).unwrap();
    let replayed_run = replayed_fs.runs.get(&result.run_id).unwrap();
    assert_eq!(live_run.status, replayed_run.status, "FAIL: run status mismatch");
    assert_eq!(live_run.completed_stages, replayed_run.completed_stages,
        "FAIL: stages mismatch");
    assert_eq!(live_run.files_generated, replayed_run.files_generated,
        "FAIL: files_generated mismatch");
    println!("  PASS — FactoryState identical (status, stages, files_generated)");

    // Step 6: Verify WorldState identical.
    let live_ws = sps_world::reducer::WorldState::from_state(&live).unwrap();
    let replayed_ws = sps_world::reducer::WorldState::from_state(&replayed).unwrap();
    assert_eq!(live_ws.graph.projects.len(), replayed_ws.graph.projects.len(),
        "FAIL: project count mismatch");
    assert_eq!(live_ws.graph.files.len(), replayed_ws.graph.files.len(),
        "FAIL: file count mismatch");
    println!("  PASS — WorldState identical ({} projects, {} files)",
        replayed_ws.graph.projects.len(), replayed_ws.graph.files.len());

    // Step 7: Verify Execution links identical.
    let live_es = sps_execution::reducer::ExecutionState::from_state(&live).unwrap();
    let replayed_es = sps_execution::reducer::ExecutionState::from_state(&replayed).unwrap();
    let live_exec_count = live_es.for_factory_run(result.run_id).len();
    let replayed_exec_count = replayed_es.for_factory_run(result.run_id).len();
    assert_eq!(live_exec_count, replayed_exec_count,
        "FAIL: for_factory_run count mismatch (live={}, replayed={})",
        live_exec_count, replayed_exec_count);
    println!("  PASS — Execution links identical (for_factory_run == {})", live_exec_count);

    println!("\n  === FACTORY SMOKE TEST PASSED ===");
    println!("  FactoryState populated ✅");
    println!("  WorldState populated (project + files) ✅");
    println!("  Execution link (factory_run_id) ✅");
    println!("  Replay identical (FactoryState + WorldState + Execution) ✅");
}
