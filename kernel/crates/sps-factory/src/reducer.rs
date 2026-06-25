//! Factory reducer + state slice.
//!
//! Phase 11A: Full factory lifecycle — 8 stages all materialized, with
//! stage_started/stage_failed/run_failed/run_retried/rollback_completed
//! events. FactoryRun now tracks failure_reason, retry_count, and
//! generated_file_paths for rollback support.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::workflow::FactoryStage;

/// Extension key.
pub const EXTENSION_KEY: &str = "factory";

/// Status of a factory run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryRunStatus {
    /// Running — at least one stage started, not yet completed/failed.
    Running,
    /// Completed successfully (all 8 stages done).
    Completed,
    /// Failed — a stage failed and the run was not retried (or retries exhausted).
    Failed,
    /// Phase 11A.8: Rolled back — files removed, run terminated.
    RolledBack,
    /// Phase 11A.8: Retry scheduled — run will be retried from failed stage.
    RetryScheduled,
}

impl Default for FactoryRunStatus {
    fn default() -> Self {
        Self::Running
    }
}

/// A factory run record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactoryRun {
    /// Unique id.
    pub id: Uuid,
    /// Project name produced.
    pub project_name: SmolStr,
    /// Stages completed (in order).
    #[serde(default)]
    pub completed_stages: Vec<FactoryStage>,
    /// Current stage (None if run is terminal).
    #[serde(default)]
    pub current_stage: Option<FactoryStage>,
    /// Run status.
    #[serde(default)]
    pub status: FactoryRunStatus,
    /// Number of files generated.
    #[serde(default)]
    pub files_generated: u32,
    /// Originating tick.
    pub origin_tick: u64,
    /// Phase 11A.7: Reason for failure (if status == Failed or RetryScheduled).
    #[serde(default)]
    pub failure_reason: Option<String>,
    /// Phase 11A.7: Stage that failed (if any).
    #[serde(default)]
    pub failed_stage: Option<FactoryStage>,
    /// Phase 11A.7: Retry count (incremented on each retry).
    #[serde(default)]
    pub retry_count: u32,
    /// Phase 11A.7: File paths generated (for rollback).
    #[serde(default)]
    pub generated_file_paths: Vec<String>,
    /// Phase 11A.7: Output directory for the run.
    #[serde(default)]
    pub output_dir: Option<String>,
}

impl Default for FactoryRun {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            project_name: SmolStr::new(""),
            completed_stages: Vec::new(),
            current_stage: None,
            status: FactoryRunStatus::Running,
            files_generated: 0,
            origin_tick: 0,
            failure_reason: None,
            failed_stage: None,
            retry_count: 0,
            generated_file_paths: Vec::new(),
            output_dir: None,
        }
    }
}

/// Factory state slice.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FactoryState {
    /// All runs keyed by id.
    #[serde(default)]
    pub runs: std::collections::BTreeMap<Uuid, FactoryRun>,
}

impl FactoryState {
    /// Read from canonical state. P3D: typed first, JSON fallback.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<FactoryState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// P3D: Read from typed extension.
    pub fn from_typed_state(state: &CanonicalState) -> Option<Arc<FactoryState>> {
        state.get_typed_extension::<FactoryState>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }

    /// Phase 11A: Query a run by id.
    pub fn get_run(&self, id: Uuid) -> Option<&FactoryRun> {
        self.runs.get(&id)
    }

    /// Phase 11A: Query all runs in a given status.
    pub fn runs_with_status(&self, status: FactoryRunStatus) -> Vec<&FactoryRun> {
        self.runs.values().filter(|r| r.status == status).collect()
    }
}

/// Reducer for factory events.
#[derive(Debug, Default)]
pub struct FactoryReducer;

impl FactoryReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            // Existing.
            "factory.run_started",
            "factory.stage_completed",
            "factory.run_completed",
            "factory.run_failed",
            // Phase 11A.1: stage_started.
            "factory.stage_started",
            // Phase 11A.2: stage_failed.
            "factory.stage_failed",
            // Phase 11A.8: retry + rollback.
            "factory.run_retried",
            "factory.rollback_completed",
            // Phase 11A.7: file tracking.
            "factory.file_generated",
            // Phase 11C: supervisor decisions.
            "factory.supervisor_decision",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<FactoryState>(EXTENSION_KEY);
    }
}

impl Reducer for FactoryReducer {
    fn name(&self) -> &'static str {
        "factory"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // P3D: Use typed extension as source of truth.
        state.with_typed_extension(EXTENSION_KEY, |fs: &mut FactoryState| {
            match event.event_type.as_str() {
                "factory.run_started" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let project_name = event.payload["project_name"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string();
                    let output_dir = event.payload.get("output_dir").and_then(|v| v.as_str()).map(String::from);
                    // Phase 11A: merge with existing run if present (run_started
                    // may fire twice — once with empty name, once with real name
                    // after requirement analysis). Preserve completed_stages.
                    let existing = fs.runs.get(&id).cloned();
                    let run = if let Some(mut existing) = existing {
                        existing.project_name = project_name.into();
                        if existing.output_dir.is_none() {
                            existing.output_dir = output_dir;
                        }
                        existing
                    } else {
                        FactoryRun {
                            id,
                            project_name: project_name.into(),
                            completed_stages: Vec::new(),
                            current_stage: Some(FactoryStage::RequirementAnalysis),
                            status: FactoryRunStatus::Running,
                            files_generated: 0,
                            origin_tick: event.tick,
                            failure_reason: None,
                            failed_stage: None,
                            retry_count: 0,
                            generated_file_paths: Vec::new(),
                            output_dir,
                        }
                    };
                    fs.runs.insert(id, run);
                }
                // Phase 11A.1: stage_started.
                "factory.stage_started" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let stage_str = event.payload["stage"].as_str().unwrap_or("");
                    if let Some(stage) = stage_from_str(stage_str) {
                        if let Some(run) = fs.runs.get_mut(&id) {
                            run.current_stage = Some(stage);
                            // If retrying, clear failure state.
                            if run.status == FactoryRunStatus::RetryScheduled {
                                run.status = FactoryRunStatus::Running;
                                run.failure_reason = None;
                            }
                        }
                    }
                }
                "factory.stage_completed" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let stage_str = event.payload["stage"].as_str().unwrap_or("");
                    let stage = stage_from_str(stage_str);
                    if let Some(run) = fs.runs.get_mut(&id) {
                        if let Some(s) = stage {
                            // Avoid duplicate if stage_completed fires twice for the same stage.
                            if run.completed_stages.last() != Some(&s) {
                                run.completed_stages.push(s);
                            }
                        }
                        run.current_stage = stage.and_then(next_stage);
                        if let Some(files) = event.payload.get("files_generated").and_then(|v| v.as_u64()) {
                            run.files_generated = files as u32;
                        }
                        // Clear failure state on successful stage completion.
                        run.failure_reason = None;
                        run.failed_stage = None;
                    }
                }
                // Phase 11A.2: stage_failed.
                "factory.stage_failed" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let stage_str = event.payload["stage"].as_str().unwrap_or("");
                    let reason = event.payload.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let stage = stage_from_str(stage_str);
                    if let Some(run) = fs.runs.get_mut(&id) {
                        run.failure_reason = Some(reason);
                        run.failed_stage = stage;
                        // Don't change status here — run_failed or run_retried will set it.
                    }
                }
                "factory.run_completed" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    if let Some(run) = fs.runs.get_mut(&id) {
                        run.status = FactoryRunStatus::Completed;
                        run.current_stage = None;
                        run.failure_reason = None;
                        run.failed_stage = None;
                    }
                }
                "factory.run_failed" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    if let Some(run) = fs.runs.get_mut(&id) {
                        run.status = FactoryRunStatus::Failed;
                        run.current_stage = None;
                    }
                }
                // Phase 11A.8: run_retried — marks the run for retry.
                "factory.run_retried" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    if let Some(run) = fs.runs.get_mut(&id) {
                        run.retry_count = run.retry_count.saturating_add(1);
                        run.status = FactoryRunStatus::RetryScheduled;
                    }
                }
                // Phase 11A.8: rollback_completed — files removed.
                "factory.rollback_completed" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let files_removed = event.payload.get("files_removed")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    if let Some(run) = fs.runs.get_mut(&id) {
                        run.status = FactoryRunStatus::RolledBack;
                        run.current_stage = None;
                        run.generated_file_paths.clear();
                        run.files_generated = run.files_generated.saturating_sub(files_removed);
                    }
                }
                // Phase 11A.7: file_generated — track individual file paths.
                "factory.file_generated" => {
                    let id: Uuid = serde_json::from_value(event.payload["id"].clone())
                        .unwrap_or_default();
                    let path = event.payload.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !path.is_empty() {
                        if let Some(run) = fs.runs.get_mut(&id) {
                            run.generated_file_paths.push(path);
                        }
                    }
                }
                // Phase 11C: supervisor_decision — record the decision for audit.
                // The actual action (retry/rollback/abort) is carried out by
                // the supervisor dispatching the corresponding factory.* events,
                // which the reducer handles above. This arm just records that
                // a supervisor decision was made.
                "factory.supervisor_decision" => {
                    let run_id: Uuid = serde_json::from_value(event.payload["run_id"].clone())
                        .unwrap_or_default();
                    let action = event.payload.get("action").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    // The decision is recorded as a no-op on the run itself;
                    // the side effects (retry_count increment, status change)
                    // come from the factory.run_retried / rollback_completed /
                    // run_failed events that the supervisor also dispatches.
                    // We do set failure_reason if it's an abort.
                    if action == "abort" {
                        if let Some(run) = fs.runs.get_mut(&run_id) {
                            run.failure_reason = Some(
                                event.payload.get("reason").and_then(|v| v.as_str())
                                    .unwrap_or("supervisor abort").to_string()
                            );
                        }
                    }
                    let _ = run_id; // suppress unused
                    let _ = action;
                }
                _ => {}
            }
        });
        // P3D: No per-dispatch JSON sync.
        Ok(())
    }
}

fn stage_from_str(s: &str) -> Option<FactoryStage> {
    match s {
        "requirement_analysis" => Some(FactoryStage::RequirementAnalysis),
        "architecture_design" => Some(FactoryStage::ArchitectureDesign),
        "planning" => Some(FactoryStage::Planning),
        "code_generation" => Some(FactoryStage::CodeGeneration),
        "testing" => Some(FactoryStage::Testing),
        "validation" => Some(FactoryStage::Validation),
        "packaging" => Some(FactoryStage::Packaging),
        "deployment_prep" => Some(FactoryStage::DeploymentPrep),
        _ => None,
    }
}

fn next_stage(s: FactoryStage) -> Option<FactoryStage> {
    match s {
        FactoryStage::RequirementAnalysis => Some(FactoryStage::ArchitectureDesign),
        FactoryStage::ArchitectureDesign => Some(FactoryStage::Planning),
        FactoryStage::Planning => Some(FactoryStage::CodeGeneration),
        FactoryStage::CodeGeneration => Some(FactoryStage::Testing),
        FactoryStage::Testing => Some(FactoryStage::Validation),
        FactoryStage::Validation => Some(FactoryStage::Packaging),
        FactoryStage::Packaging => Some(FactoryStage::DeploymentPrep),
        FactoryStage::DeploymentPrep => None,
    }
}
