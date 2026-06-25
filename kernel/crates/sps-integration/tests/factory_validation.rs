//! Factory Validation Suite — 8/8 PASS required.

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

fn make_request(name: &str) -> ProjectRequest {
    ProjectRequest {
        description: "A CLI tool".to_string(),
        preferred_name: Some(SmolStr::new(name)),
        output_dir: Some("/tmp/test".to_string()),
    }
}

fn run_factory(kernel: &Arc<SpsKernel>, request: ProjectRequest) -> sps_factory::workflow::RunResult {
    FactoryWorkflow::run_with_sink(
        request,
        "/tmp/test",
        kernel.as_ref() as &dyn sps_core::sink::EventSink,
        None,
    ).unwrap()
}

fn factory_runs(kernel: &SpsKernel) -> usize {
    kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .map(|fs| fs.runs.len())
            .unwrap_or(0)
    })
}

// ─── Checkpoint 1 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_1_run_started_materializes() {
    println!("\n=== FACTORY CHECKPOINT 1: factory.run_started materializes FactoryState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("test-proj"));
    println!("  Factory run completed (run_id={}...)", &result.run_id.to_string()[..8]);

    if factory_runs(&kernel) == 1 {
        println!("  PASS — 1 factory run in FactoryState");
    } else {
        println!("  FAIL — expected 1, got {}", factory_runs(&kernel));
        panic!("FACTORY CHECKPOINT 1 FAILED");
    }
}

// ─── Checkpoint 2 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_2_stage_completed_advances() {
    println!("\n=== FACTORY CHECKPOINT 2: factory.stage_completed advances stages ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("stage-test"));
    let (stages, status) = kernel.query(|s| {
        sps_factory::reducer::FactoryState::from_state(s)
            .and_then(|fs| fs.runs.get(&result.run_id).map(|r| (r.completed_stages.len(), r.status)))
            .unwrap_or((0, sps_factory::reducer::FactoryRunStatus::Running))
    });

    if stages == 8 && status == sps_factory::reducer::FactoryRunStatus::Completed {
        println!("  PASS — {} stages completed, status={:?}", stages, status);
    } else {
        println!("  FAIL — stages={}, status={:?}", stages, status);
        panic!("FACTORY CHECKPOINT 2 FAILED");
    }
}

// ─── Checkpoint 3 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_3_world_artifacts_tracked() {
    println!("\n=== FACTORY CHECKPOINT 3: factory dispatches world.project_added + world.file_added ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("world-test"));
    println!("  Factory run produced {} files", result.files.len());

    let (projects, files) = kernel.query(|s| {
        let ws = sps_world::reducer::WorldState::from_state(s).unwrap_or_default();
        (ws.graph.projects.len(), ws.graph.files.len())
    });

    if projects == 1 && files > 0 {
        println!("  PASS — WorldState has {} project(s) + {} file(s)", projects, files);
    } else {
        println!("  FAIL — projects={}, files={}", projects, files);
        panic!("FACTORY CHECKPOINT 3 FAILED");
    }
}

// ─── Checkpoint 4 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_4_execution_link() {
    println!("\n=== FACTORY CHECKPOINT 4: FactoryRun → Execution link (factory_run_id) ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("exec-link-test"));
    let execs = kernel.query(|s| {
        let es = sps_execution::reducer::ExecutionState::from_state(s).unwrap();
        es.for_factory_run(result.run_id).len()
    });

    if execs == 1 {
        println!("  PASS — for_factory_run(run_id) returned 1 execution");
    } else {
        println!("  FAIL — expected 1, got {}", execs);
        panic!("FACTORY CHECKPOINT 4 FAILED");
    }
}

// ─── Checkpoint 5 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_5_replay() {
    println!("\n=== FACTORY CHECKPOINT 5: replay produces identical FactoryState + WorldState ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("replay-test"));

    let live = kernel.query(|s| s.clone());
    let live_hash = live.last_hash().clone();

    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_factory::reducer::FactoryReducer::register(&mut reg);
        sps_world::reducer::WorldReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    assert_eq!(replayed.last_hash(), live_hash, "FAIL: hash mismatch");

    let live_fs = sps_factory::reducer::FactoryState::from_state(&live).unwrap();
    let replayed_fs = sps_factory::reducer::FactoryState::from_state(&replayed).unwrap();
    if live_fs.runs.len() == replayed_fs.runs.len() {
        println!("  PASS — FactoryState runs match ({} == {})", live_fs.runs.len(), replayed_fs.runs.len());
    } else {
        println!("  FAIL — run count mismatch");
        panic!("FACTORY CHECKPOINT 5 FAILED");
    }

    let live_ws = sps_world::reducer::WorldState::from_state(&live).unwrap();
    let replayed_ws = sps_world::reducer::WorldState::from_state(&replayed).unwrap();
    if live_ws.graph.projects.len() == replayed_ws.graph.projects.len()
        && live_ws.graph.files.len() == replayed_ws.graph.files.len() {
        println!("  PASS — WorldState matches ({} projects, {} files)",
            replayed_ws.graph.projects.len(), replayed_ws.graph.files.len());
    } else {
        println!("  FAIL — WorldState mismatch");
        panic!("FACTORY CHECKPOINT 5 FAILED");
    }
}

// ─── Checkpoint 6 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_6_sqlite() {
    println!("\n=== FACTORY CHECKPOINT 6: SQLite backend ===");
    let storage: Arc<dyn StoragePort> = Arc::new(
        sps_storage_sqlite::SqliteStorage::open_in_memory().unwrap()
    );
    let kernel = boot_kernel(storage.clone());
    assert_eq!(kernel.backend_name(), "sqlite");

    let result = run_factory(&kernel, make_request("sqlite-test"));
    println!("  Factory run via SQLite (run_id={}...)", &result.run_id.to_string()[..8]);

    assert_eq!(factory_runs(&kernel), 1);

    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none());

    drop(kernel);
    let kernel2 = boot_kernel(storage.clone());
    if factory_runs(&kernel2) == 1 {
        println!("  PASS — after restart, 1 factory run still present");
    } else {
        println!("  FAIL — after restart, got {}", factory_runs(&kernel2));
        panic!("FACTORY CHECKPOINT 6 FAILED");
    }
}

// ─── Checkpoint 7 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_7_multi_run_isolation() {
    println!("\n=== FACTORY CHECKPOINT 7: multi-run isolation ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let r1 = run_factory(&kernel, make_request("proj-A"));
    let r2 = run_factory(&kernel, make_request("proj-B"));
    let r3 = run_factory(&kernel, make_request("proj-C"));
    println!("  Ran 3 factory runs");

    assert_eq!(factory_runs(&kernel), 3);

    // Verify each run has distinct project name.
    let names = kernel.query(|s| {
        let fs = sps_factory::reducer::FactoryState::from_state(s).unwrap();
        vec![
            fs.runs.get(&r1.run_id).map(|r| r.project_name.as_str().to_string()),
            fs.runs.get(&r2.run_id).map(|r| r.project_name.as_str().to_string()),
            fs.runs.get(&r3.run_id).map(|r| r.project_name.as_str().to_string()),
        ]
    });

    if names == vec![Some("proj-A".into()), Some("proj-B".into()), Some("proj-C".into())] {
        println!("  PASS — 3 distinct project names (no cross-contamination)");
    } else {
        println!("  FAIL — names: {:?}", names);
        panic!("FACTORY CHECKPOINT 7 FAILED");
    }

    // Verify WorldState has 3 projects.
    let world_projects = kernel.query(|s| {
        sps_world::reducer::WorldState::from_state(s)
            .map(|ws| ws.graph.projects.len())
            .unwrap_or(0)
    });
    if world_projects == 3 {
        println!("  PASS — WorldState has 3 projects (one per run)");
    } else {
        println!("  FAIL — WorldState has {} projects (expected 3)", world_projects);
        panic!("FACTORY CHECKPOINT 7 FAILED");
    }

    // Verify ExecutionState has 3 executions (one per run).
    let exec_count = kernel.query(|s| {
        sps_execution::reducer::ExecutionState::from_state(s)
            .map(|es| es.records.len())
            .unwrap_or(0)
    });
    if exec_count == 3 {
        println!("  PASS — ExecutionState has 3 executions (one per run)");
    } else {
        println!("  FAIL — ExecutionState has {} (expected 3)", exec_count);
        panic!("FACTORY CHECKPOINT 7 FAILED");
    }
}

// ─── Checkpoint 8 ─────────────────────────────────────────────────────────

#[test]
fn factory_checkpoint_8_deterministic_state() {
    println!("\n=== FACTORY CHECKPOINT 8: deterministic state across replay ===");
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    let result = run_factory(&kernel, make_request("det-test"));

    // Capture live state details.
    let (live_run_id, live_project_name, live_stages, live_files_gen) = kernel.query(|s| {
        let fs = sps_factory::reducer::FactoryState::from_state(s).unwrap();
        let run = fs.runs.get(&result.run_id).unwrap();
        (run.id, run.project_name.as_str().to_string(), run.completed_stages.len(), run.files_generated)
    });

    // Replay.
    let pipeline = std::sync::Arc::new(sps_core::reducer::ReducerPipeline::new({
        let mut reg = ReducerRegistry::new();
        sps_factory::reducer::FactoryReducer::register(&mut reg);
        sps_world::reducer::WorldReducer::register(&mut reg);
        sps_execution::reducer::ExecutionReducer::register(&mut reg);
        std::sync::Arc::new(reg)
    }));
    let engine = ReplayEngine::new(pipeline);
    let replayed = engine.replay_from_genesis(storage.as_ref()).unwrap();

    let replayed_fs = sps_factory::reducer::FactoryState::from_state(&replayed).unwrap();
    let replayed_run = replayed_fs.runs.get(&result.run_id).unwrap();

    if replayed_run.id == live_run_id
        && replayed_run.project_name.as_str() == live_project_name
        && replayed_run.completed_stages.len() == live_stages
        && replayed_run.files_generated == live_files_gen {
        println!("  PASS — run_id, project_name, stages, files_generated all match");
        println!("    run_id: {}", &live_run_id.to_string()[..8]);
        println!("    project_name: '{}'", live_project_name);
        println!("    stages: {}", live_stages);
        println!("    files_generated: {}", live_files_gen);
    } else {
        println!("  FAIL — mismatch after replay");
        panic!("FACTORY CHECKPOINT 8 FAILED");
    }
}
