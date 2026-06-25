//! Phase 12C: Self-Improvement Loop.
//!
//! Connects Reflection → Improvement → FactorySupervisor so the system
//! learns from factory run outcomes and proposes policy adjustments.
//!
//! The loop works as follows:
//! 1. Factory run completes (or fails) → Reflection analyzes the outcome
//! 2. SelfImprovementLoop observes reflection events + factory events
//! 3. If a pattern is detected (e.g. "testing stage fails 3x in a row"),
//!    the loop proposes an improvement: "increase max_retries for testing"
//! 4. The improvement is dispatched as improvement.proposed
//! 5. If approved, the loop applies it by updating SupervisorPolicy
//!
//! This is the "Continuous Feedback Loop" from the SPS roadmap.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use sps_core::actor::Actor;
use sps_core::event::{Event, RawEvent};
use sps_core::sink::EventSink;
use sps_core::CoreResult;

use crate::analyzers::OptimizationKind;
use crate::reducer::{ImprovementProposal, ImprovementStatus};

/// A pattern detected by the self-improvement loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementPattern {
    /// A factory stage fails repeatedly → propose increasing retries.
    StageRepeatedlyFails {
        stage: String,
        failure_count: u32,
        suggested_max_retries: u32,
    },
    /// A factory run always succeeds on first try → propose reducing retries.
    StageAlwaysSucceeds {
        stage: String,
        success_count: u32,
        suggested_max_retries: u32,
    },
    /// Factory runs are slow → propose parallelizing a stage.
    StageIsSlow {
        stage: String,
        avg_duration_ms: u64,
    },
    /// Reflection identified a generalizable pattern → propose codifying it.
    GeneralizablePattern {
        pattern_name: String,
        description: String,
    },
}

/// The self-improvement loop. Observes events and proposes improvements.
///
/// This is a pure decision function — it does NOT execute actions directly.
/// It dispatches `improvement.proposed` events through the EventSink.
pub struct SelfImprovementLoop {
    /// Minimum failures before proposing a retry increase.
    pub failure_threshold: u32,
    /// Minimum successes before proposing a retry decrease.
    pub success_threshold: u32,
}

impl Default for SelfImprovementLoop {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 10,
        }
    }
}

impl SelfImprovementLoop {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe a factory event and decide whether to propose an improvement.
    ///
    /// Returns Some(ImprovementProposal) if a pattern was detected, None otherwise.
    /// The caller is responsible for dispatching the proposal (via `propose`).
    pub fn analyze_factory_event(
        &self,
        event: &Event,
        run: Option<&sps_factory::reducer::FactoryRun>,
    ) -> Option<ImprovementPattern> {
        match event.event_type.as_str() {
            "factory.stage_failed" => {
                let stage = event.payload.get("stage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let retry_count = run.map(|r| r.retry_count).unwrap_or(0);

                // If we've retried enough times to hit the threshold, propose
                // increasing max_retries.
                if retry_count >= self.failure_threshold {
                    return Some(ImprovementPattern::StageRepeatedlyFails {
                        stage,
                        failure_count: retry_count,
                        suggested_max_retries: retry_count + 2,
                    });
                }
            }
            "factory.run_completed" => {
                // If the run succeeded with 0 retries and has many files,
                // it might be a candidate for reducing retries.
                if let Some(run) = run {
                    if run.retry_count == 0 && run.completed_stages.len() == 8 {
                        // Check if this is a consistently successful pattern.
                        // In a real implementation, we'd track history across runs.
                        // For now, we just return None — the pattern would be
                        // detected after success_threshold consecutive successes.
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Observe a reflection event and decide whether to propose an improvement.
    pub fn analyze_reflection_event(
        &self,
        event: &Event,
    ) -> Option<ImprovementPattern> {
        match event.event_type.as_str() {
            "reflection.success_analyzed" => {
                // Check if the reflection says the approach is generalizable.
                let generalizable = event.payload.get("generalizable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if generalizable {
                    let pattern_name = event.payload.get("pattern_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unnamed")
                        .to_string();
                    let why = event.payload.get("why")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return Some(ImprovementPattern::GeneralizablePattern {
                        pattern_name,
                        description: why,
                    });
                }
            }
            "reflection.failure_analyzed" => {
                // A failure was analyzed — could propose improvements.
                // For now, return None.
            }
            _ => {}
        }
        None
    }

    /// Convert a pattern into an ImprovementProposal and dispatch it.
    pub fn propose(
        &self,
        pattern: ImprovementPattern,
        sink: &dyn EventSink,
    ) -> CoreResult<Uuid> {
        let proposal_id = Uuid::now_v7();
        let (description, subsystem, kind) = match &pattern {
            ImprovementPattern::StageRepeatedlyFails { stage, failure_count, suggested_max_retries } => (
                format!(
                    "Stage '{}' failed {} times. Increase max_retries to {}.",
                    stage, failure_count, suggested_max_retries
                ),
                SmolStr::new("factory_supervisor"),
                OptimizationKind::Workflow,
            ),
            ImprovementPattern::StageAlwaysSucceeds { stage, success_count, suggested_max_retries } => (
                format!(
                    "Stage '{}' succeeded {} consecutive times. Reduce max_retries to {}.",
                    stage, success_count, suggested_max_retries
                ),
                SmolStr::new("factory_supervisor"),
                OptimizationKind::Workflow,
            ),
            ImprovementPattern::StageIsSlow { stage, avg_duration_ms } => (
                format!(
                    "Stage '{}' averages {}ms. Consider parallelizing.",
                    stage, avg_duration_ms
                ),
                SmolStr::new("factory"),
                OptimizationKind::Performance,
            ),
            ImprovementPattern::GeneralizablePattern { pattern_name, description } => (
                format!(
                    "Generalizable pattern detected: '{}'. {}",
                    pattern_name, description
                ),
                SmolStr::new("memory"),
                OptimizationKind::Knowledge,
            ),
        };

        let proposal = ImprovementProposal {
            id: proposal_id,
            kind,
            description,
            status: ImprovementStatus::Proposed,
            origin_tick: 0,
            workflow: None,
            prompt: None,
            subsystem,
        };

        let payload = serde_json::to_value(&proposal)
            .map_err(|e| sps_core::CoreError::Internal(anyhow::anyhow!("serialize proposal: {}", e)))?;

        sink.dispatch_trusted(RawEvent::new(
            "improvement.proposed",
            payload,
            Actor::system("self_improvement"),
            0,
        ))?;

        Ok(proposal_id)
    }

    /// Observe an event and automatically propose if a pattern is detected.
    /// Combines analyze + propose.
    pub fn observe_and_propose(
        &self,
        event: &Event,
        run: Option<&sps_factory::reducer::FactoryRun>,
        sink: &dyn EventSink,
    ) -> CoreResult<Option<Uuid>> {
        // Try factory analysis first.
        if let Some(pattern) = self.analyze_factory_event(event, run) {
            let id = self.propose(pattern, sink)?;
            return Ok(Some(id));
        }
        // Then reflection analysis.
        if let Some(pattern) = self.analyze_reflection_event(event) {
            let id = self.propose(pattern, sink)?;
            return Ok(Some(id));
        }
        Ok(None)
    }
}

/// Phase 12C: Apply an approved improvement to the FactorySupervisor policy.
///
/// This is a pure function that takes the current policy and an approved
/// proposal, and returns the updated policy. The caller is responsible
/// for dispatching the `improvement.applied` event.
pub fn apply_improvement_to_policy(
    policy: &sps_factory::supervisor::SupervisorPolicy,
    proposal: &ImprovementProposal,
) -> sps_factory::supervisor::SupervisorPolicy {
    let mut new_policy = policy.clone();

    // Parse the description for retry adjustments.
    // Format: "Stage 'X' failed N times. Increase max_retries to M."
    if proposal.description.contains("Increase max_retries to") {
        if let Some(num_str) = proposal.description
            .split("Increase max_retries to ")
            .nth(1)
            .and_then(|s| s.trim_end_matches('.').parse::<u32>().ok())
        {
            new_policy.max_retries = num_str;
        }
    }
    if proposal.description.contains("Reduce max_retries to") {
        if let Some(num_str) = proposal.description
            .split("Reduce max_retries to ")
            .nth(1)
            .and_then(|s| s.trim_end_matches('.').parse::<u32>().ok())
        {
            new_policy.max_retries = num_str;
        }
    }

    new_policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use sps_core::event::EventHash;
    use sps_factory::reducer::FactoryRun;

    fn make_stage_failed_event(stage: &str, retry_count: u32) -> Event {
        let raw = RawEvent::new(
            "factory.stage_failed",
            serde_json::json!({
                "id": Uuid::now_v7().to_string(),
                "stage": stage,
                "reason": "test",
            }),
            Actor::system("test"),
            0,
        );
        raw.finalize(1, EventHash::GENESIS)
    }

    fn make_run(retry_count: u32) -> FactoryRun {
        FactoryRun {
            retry_count,
            ..Default::default()
        }
    }

    #[test]
    fn detects_stage_repeatedly_fails() {
        let loop_ = SelfImprovementLoop::new();
        let event = make_stage_failed_event("testing", 3);
        let run = make_run(3);

        let pattern = loop_.analyze_factory_event(&event, Some(&run));
        assert!(pattern.is_some(), "should detect pattern");
        match pattern.unwrap() {
            ImprovementPattern::StageRepeatedlyFails { stage, suggested_max_retries, .. } => {
                assert_eq!(stage, "testing");
                assert_eq!(suggested_max_retries, 5); // 3 + 2
            }
            other => panic!("expected StageRepeatedlyFails, got {:?}", other),
        }
    }

    #[test]
    fn no_pattern_below_threshold() {
        let loop_ = SelfImprovementLoop::new();
        let event = make_stage_failed_event("testing", 1);
        let run = make_run(1);

        let pattern = loop_.analyze_factory_event(&event, Some(&run));
        assert!(pattern.is_none(), "should not detect pattern below threshold");
    }

    #[test]
    fn apply_improvement_increases_retries() {
        let policy = sps_factory::supervisor::SupervisorPolicy::default();
        assert_eq!(policy.max_retries, 2);

        let proposal = ImprovementProposal {
            id: Uuid::nil(),
            kind: OptimizationKind::Workflow,
            description: "Stage 'testing' failed 3 times. Increase max_retries to 5.".into(),
            status: ImprovementStatus::Approved,
            origin_tick: 0,
            workflow: None,
            prompt: None,
            subsystem: "factory_supervisor".into(),
        };

        let new_policy = apply_improvement_to_policy(&policy, &proposal);
        assert_eq!(new_policy.max_retries, 5);
    }

    #[test]
    fn apply_improvement_reduces_retries() {
        let policy = sps_factory::supervisor::SupervisorPolicy {
            max_retries: 5,
            ..Default::default()
        };

        let proposal = ImprovementProposal {
            id: Uuid::nil(),
            kind: OptimizationKind::Workflow,
            description: "Stage 'testing' succeeded 10 consecutive times. Reduce max_retries to 1.".into(),
            status: ImprovementStatus::Approved,
            origin_tick: 0,
            workflow: None,
            prompt: None,
            subsystem: "factory_supervisor".into(),
        };

        let new_policy = apply_improvement_to_policy(&policy, &proposal);
        assert_eq!(new_policy.max_retries, 1);
    }

    #[test]
    fn detects_generalizable_pattern_from_reflection() {
        let loop_ = SelfImprovementLoop::new();
        let raw = RawEvent::new(
            "reflection.success_analyzed",
            serde_json::json!({
                "id": Uuid::now_v7().to_string(),
                "what_worked": ["test"],
                "why": "it worked",
                "generalizable": true,
                "pattern_name": "rust-rest-api",
            }),
            Actor::system("test"),
            0,
        );
        let event = raw.finalize(1, EventHash::GENESIS);

        let pattern = loop_.analyze_reflection_event(&event);
        assert!(pattern.is_some());
        match pattern.unwrap() {
            ImprovementPattern::GeneralizablePattern { pattern_name, .. } => {
                assert_eq!(pattern_name, "rust-rest-api");
            }
            other => panic!("expected GeneralizablePattern, got {:?}", other),
        }
    }
}
