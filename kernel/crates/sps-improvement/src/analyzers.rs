//! Self-improvement analyzers.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Kind of optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationKind {
    /// Workflow optimization.
    Workflow,
    /// Agent prompt optimization.
    AgentPrompt,
    /// Knowledge optimization.
    Knowledge,
    /// Performance optimization.
    Performance,
    /// Bottleneck removal.
    Bottleneck,
}

/// A performance report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Average latency per subsystem, in ms.
    pub avg_latencies_ms: std::collections::BTreeMap<String, f64>,
    /// Failure rate per subsystem.
    pub failure_rates: std::collections::BTreeMap<String, f64>,
    /// Total events processed.
    pub total_events: u64,
}

/// A detected bottleneck.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Subsystem name.
    pub subsystem: SmolStr,
    /// Latency in ms.
    pub latency_ms: f64,
    /// Severity (0.0–1.0).
    pub severity: f32,
    /// Suggested fix.
    pub suggested_fix: String,
}

/// A workflow optimization proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowProposal {
    /// Proposal id.
    pub id: Uuid,
    /// Affected workflow/template name.
    pub workflow: SmolStr,
    /// Description of the change.
    pub description: String,
    /// Estimated improvement (0.0–1.0).
    pub estimated_improvement: f32,
}

/// A prompt optimization proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptProposal {
    /// Proposal id.
    pub id: Uuid,
    /// Agent archetype.
    pub agent_archetype: SmolStr,
    /// Current prompt (summary).
    pub current_prompt_summary: String,
    /// Proposed prompt (summary).
    pub proposed_prompt_summary: String,
    /// Rationale.
    pub rationale: String,
}

/// Analyze performance metrics.
pub struct PerformanceAnalyzer;

impl PerformanceAnalyzer {
    /// Analyze a performance report and produce a list of subsystems
    /// that exceed acceptable thresholds.
    pub fn analyze(report: &PerformanceReport) -> Vec<String> {
        report
            .avg_latencies_ms
            .iter()
            .filter(|(_, &lat)| lat > 1000.0)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

/// Detect bottlenecks from a performance report.
pub struct BottleneckDetector;

impl BottleneckDetector {
    /// Detect bottlenecks (latency > threshold or failure rate > 5%).
    pub fn detect(report: &PerformanceReport) -> Vec<Bottleneck> {
        let mut out = Vec::new();
        for (sub, &lat) in &report.avg_latencies_ms {
            if lat > 500.0 {
                let severity = ((lat / 1000.0) as f32).min(1.0);
                out.push(Bottleneck {
                    subsystem: sub.as_str().into(),
                    latency_ms: lat,
                    severity,
                    suggested_fix: format!("Investigate {} — latency {}ms exceeds 500ms", sub, lat),
                });
            }
        }
        for (sub, &rate) in &report.failure_rates {
            if rate > 0.05 {
                out.push(Bottleneck {
                    subsystem: sub.as_str().into(),
                    latency_ms: 0.0,
                    severity: (rate as f32).min(1.0),
                    suggested_fix: format!("Investigate {} — failure rate {:.1}% exceeds 5%", sub, rate * 100.0),
                });
            }
        }
        out
    }
}

/// Optimize workflows (propose changes — does not apply).
pub struct WorkflowOptimizer;

impl WorkflowOptimizer {
    /// Propose a workflow optimization.
    pub fn propose(workflow: &str, description: &str, improvement: f32) -> WorkflowProposal {
        WorkflowProposal {
            id: Uuid::now_v7(),
            workflow: workflow.into(),
            description: description.to_string(),
            estimated_improvement: improvement,
        }
    }
}

/// Optimize agent prompts (propose changes — does not apply).
pub struct PromptOptimizer;

impl PromptOptimizer {
    /// Propose a prompt optimization.
    pub fn propose(
        archetype: &str,
        current: &str,
        proposed: &str,
        rationale: &str,
    ) -> PromptProposal {
        PromptProposal {
            id: Uuid::now_v7(),
            agent_archetype: archetype.into(),
            current_prompt_summary: current.to_string(),
            proposed_prompt_summary: proposed.to_string(),
            rationale: rationale.to_string(),
        }
    }
}
