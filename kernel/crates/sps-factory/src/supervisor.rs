//! Phase 11C: Factory Supervisor — automated retry/rollback/abort decisions.
//!
//! The supervisor observes factory events and applies a configurable policy
//! to decide what action to take when a stage fails:
//! - Retry: re-run the failed stage (up to max_retries)
//! - Rollback: undo the run (delete generated files, mark as RolledBack)
//! - Abort: terminate immediately without rollback (critical error)
//!
//! The supervisor is a pure decision function — it does NOT execute actions
//! directly. Instead, it dispatches `factory.supervisor_decision` events
//! through the EventSink, which the runtime then acts upon.
//!
//! Policy is configurable:
//! - max_retries: how many times to retry a failed stage (default: 2)
//! - auto_rollback: whether to automatically rollback after retries exhausted (default: true)
//! - critical_stages: stages where failure triggers immediate abort (default: [])

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::sink::EventSink;
use sps_core::CoreResult;

use crate::workflow::FactoryStage;

/// Supervisor policy — configures retry/rollback/abort behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorPolicy {
    /// Maximum retry attempts per failed stage. Default: 2.
    pub max_retries: u32,
    /// Whether to automatically rollback after retries exhausted. Default: true.
    pub auto_rollback: bool,
    /// Stages where failure triggers immediate abort (no retry).
    /// Default: empty (all stages are retryable).
    pub critical_stages: Vec<FactoryStage>,
    /// Minimum wall-time between retries (ms). Display only — does not
    /// affect the hash chain. Default: 1000.
    pub retry_delay_ms: u64,
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            auto_rollback: true,
            critical_stages: Vec::new(),
            retry_delay_ms: 1_000,
        }
    }
}

/// A decision made by the supervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorAction {
    /// No action needed — the event was informational (stage_started, etc.).
    NoAction,
    /// Retry the failed stage. The runtime should re-run the stage.
    Retry {
        /// The run id to retry.
        run_id: Uuid,
        /// The stage to retry.
        stage: FactoryStage,
        /// Which retry attempt this is (1-indexed).
        attempt: u32,
    },
    /// Rollback the run — delete generated files, mark as RolledBack.
    Rollback {
        /// The run id to rollback.
        run_id: Uuid,
        /// Why rollback was triggered.
        reason: String,
    },
    /// Abort immediately — critical failure, no rollback.
    Abort {
        /// The run id to abort.
        run_id: Uuid,
        /// Why abort was triggered.
        reason: String,
    },
}

/// The Factory Supervisor. Observes events and makes decisions.
pub struct FactorySupervisor {
    policy: SupervisorPolicy,
}

impl FactorySupervisor {
    /// Create a new supervisor with the given policy.
    pub fn new(policy: SupervisorPolicy) -> Self {
        Self { policy }
    }

    /// Create a supervisor with default policy.
    pub fn default_policy() -> Self {
        Self::new(SupervisorPolicy::default())
    }

    /// Access the policy.
    pub fn policy(&self) -> &SupervisorPolicy {
        &self.policy
    }

    /// Observe an event and decide what action to take.
    ///
    /// This is a PURE function — it does not dispatch events. The caller
    /// is responsible for executing the decision (via `execute_decision`).
    pub fn decide(&self, event: &Event, run: Option<&crate::reducer::FactoryRun>) -> SupervisorAction {
        match event.event_type.as_str() {
            "factory.stage_failed" => {
                let run_id: Uuid = serde_json::from_value(event.payload["id"].clone())
                    .unwrap_or_default();
                let stage_str = event.payload["stage"].as_str().unwrap_or("");
                let stage = stage_from_str(stage_str);

                // If stage is critical, abort immediately.
                if let Some(s) = stage {
                    if self.policy.critical_stages.contains(&s) {
                        return SupervisorAction::Abort {
                            run_id,
                            reason: format!("critical stage {:?} failed", s),
                        };
                    }
                }

                // Check retry count.
                let current_retries = run.map(|r| r.retry_count).unwrap_or(0);
                if current_retries < self.policy.max_retries {
                    return SupervisorAction::Retry {
                        run_id,
                        stage: stage.unwrap_or(FactoryStage::RequirementAnalysis),
                        attempt: current_retries + 1,
                    };
                }

                // Retries exhausted — rollback or abort.
                if self.policy.auto_rollback {
                    SupervisorAction::Rollback {
                        run_id,
                        reason: format!(
                            "retries exhausted ({} attempts) for stage {:?}",
                            current_retries,
                            stage.unwrap_or(FactoryStage::RequirementAnalysis)
                        ),
                    }
                } else {
                    SupervisorAction::Abort {
                        run_id,
                        reason: "retries exhausted, auto_rollback disabled".into(),
                    }
                }
            }
            "factory.run_failed" => {
                let run_id: Uuid = serde_json::from_value(event.payload["id"].clone())
                    .unwrap_or_default();
                // If auto_rollback is on and the run failed, rollback.
                if self.policy.auto_rollback {
                    SupervisorAction::Rollback {
                        run_id,
                        reason: "run failed, auto_rollback enabled".into(),
                    }
                } else {
                    SupervisorAction::NoAction
                }
            }
            _ => SupervisorAction::NoAction,
        }
    }

    /// Execute a supervisor decision by dispatching the appropriate events.
    ///
    /// This is the side-effecting counterpart to `decide`. The caller
    /// typically does:
    ///   let action = supervisor.decide(&event, run);
    ///   supervisor.execute_decision(action, &sink)?;
    pub fn execute_decision(
        &self,
        action: SupervisorAction,
        sink: &dyn EventSink,
    ) -> CoreResult<()> {
        match action {
            SupervisorAction::NoAction => {
                // Nothing to do.
                Ok(())
            }
            SupervisorAction::Retry { run_id, stage, attempt } => {
                let payload = serde_json::json!({
                    "action": "retry",
                    "run_id": run_id.to_string(),
                    "stage": stage.as_str(),
                    "attempt": attempt,
                });
                sink.dispatch_trusted(RawEvent::new(
                    "factory.supervisor_decision",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                // Dispatch factory.run_retried so the reducer increments retry_count.
                let payload = serde_json::json!({"id": run_id.to_string()});
                sink.dispatch_trusted(RawEvent::new(
                    "factory.run_retried",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                Ok(())
            }
            SupervisorAction::Rollback { run_id, reason } => {
                let payload = serde_json::json!({
                    "action": "rollback",
                    "run_id": run_id.to_string(),
                    "reason": reason,
                });
                sink.dispatch_trusted(RawEvent::new(
                    "factory.supervisor_decision",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                // Dispatch factory.rollback_completed so the reducer marks it.
                let payload = serde_json::json!({
                    "id": run_id.to_string(),
                    "files_removed": 0,
                });
                sink.dispatch_trusted(RawEvent::new(
                    "factory.rollback_completed",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                Ok(())
            }
            SupervisorAction::Abort { run_id, reason } => {
                let payload = serde_json::json!({
                    "action": "abort",
                    "run_id": run_id.to_string(),
                    "reason": reason,
                });
                sink.dispatch_trusted(RawEvent::new(
                    "factory.supervisor_decision",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                // Dispatch factory.run_failed to mark the run as terminal.
                let payload = serde_json::json!({"id": run_id.to_string()});
                sink.dispatch_trusted(RawEvent::new(
                    "factory.run_failed",
                    payload,
                    Actor::system("supervisor"),
                    0,
                ))?;
                Ok(())
            }
        }
    }

    /// Observe an event and automatically execute the decision.
    /// Convenience method combining `decide` + `execute_decision`.
    pub fn observe_and_act(
        &self,
        event: &Event,
        run: Option<&crate::reducer::FactoryRun>,
        sink: &dyn EventSink,
    ) -> CoreResult<SupervisorAction> {
        let action = self.decide(event, run);
        self.execute_decision(action.clone(), sink)?;
        Ok(action)
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

/// A record of a supervisor decision (for audit trail).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisorDecisionRecord {
    pub action: SmolStr,
    pub run_id: Uuid,
    pub stage: Option<FactoryStage>,
    pub attempt: Option<u32>,
    pub reason: Option<String>,
    pub origin_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::FactoryRun;

    fn make_run(retry_count: u32) -> FactoryRun {
        FactoryRun {
            retry_count,
            ..Default::default()
        }
    }

    fn make_stage_failed_event(run_id: Uuid, stage: &str) -> Event {
        use sps_core::event::{EventHash, RawEvent};
        let raw = RawEvent::new(
            "factory.stage_failed",
            serde_json::json!({
                "id": run_id.to_string(),
                "stage": stage,
                "reason": "test failure",
            }),
            Actor::system("test"),
            0,
        );
        raw.finalize(1, EventHash::GENESIS)
    }

    #[test]
    fn decide_retries_on_stage_failed() {
        let supervisor = FactorySupervisor::default_policy();
        let run_id = Uuid::now_v7();
        let event = make_stage_failed_event(run_id, "testing");
        let run = make_run(0);

        let action = supervisor.decide(&event, Some(&run));
        match action {
            SupervisorAction::Retry { attempt, .. } => assert_eq!(attempt, 1),
            other => panic!("expected Retry, got {:?}", other),
        }
    }

    #[test]
    fn decide_rollbacks_after_max_retries() {
        let supervisor = FactorySupervisor::default_policy();
        let run_id = Uuid::now_v7();
        let event = make_stage_failed_event(run_id, "testing");
        let run = make_run(2); // max_retries = 2, so this is exhausted

        let action = supervisor.decide(&event, Some(&run));
        match action {
            SupervisorAction::Rollback { .. } => {}
            other => panic!("expected Rollback, got {:?}", other),
        }
    }

    #[test]
    fn decide_aborts_on_critical_stage() {
        let policy = SupervisorPolicy {
            critical_stages: vec![FactoryStage::CodeGeneration],
            ..Default::default()
        };
        let supervisor = FactorySupervisor::new(policy);
        let run_id = Uuid::now_v7();
        let event = make_stage_failed_event(run_id, "code_generation");
        let run = make_run(0);

        let action = supervisor.decide(&event, Some(&run));
        match action {
            SupervisorAction::Abort { .. } => {}
            other => panic!("expected Abort, got {:?}", other),
        }
    }

    #[test]
    fn decide_no_action_on_informational_events() {
        let supervisor = FactorySupervisor::default_policy();
        use sps_core::event::{EventHash, RawEvent};
        let raw = RawEvent::new(
            "factory.stage_started",
            serde_json::json!({"id": Uuid::now_v7().to_string(), "stage": "testing"}),
            Actor::system("test"),
            0,
        );
        let event = raw.finalize(1, EventHash::GENESIS);

        let action = supervisor.decide(&event, None);
        assert_eq!(action, SupervisorAction::NoAction);
    }
}
