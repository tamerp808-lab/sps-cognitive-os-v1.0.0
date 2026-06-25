//! Reflection analyzers.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Success analysis — what worked, why, can it be generalized?
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuccessAnalysis {
    /// Goal/task id.
    pub id: Uuid,
    /// What worked.
    pub what_worked: Vec<String>,
    /// Why it worked.
    pub why: String,
    /// Whether the approach can be generalized.
    pub generalizable: bool,
    /// Suggested pattern name (if generalizable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_name: Option<SmolStr>,
}

/// Failure analysis — root cause classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureAnalysis {
    /// Goal/task id.
    pub id: Uuid,
    /// Root cause category.
    pub root_cause: RootCause,
    /// Description.
    pub description: String,
    /// Suggested fix.
    pub suggested_fix: String,
}

/// Root cause categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootCause {
    /// Effect execution failed (provider down, fs error, etc.).
    EffectFailure,
    /// Plan was wrong.
    PlanningError,
    /// Goal was ambiguous.
    Ambiguity,
    /// Provider returned bad output.
    ProviderIssue,
    /// Resource conflict.
    ResourceConflict,
    /// Timeout.
    Timeout,
    /// Unknown.
    Unknown,
}

/// A detected pattern across runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Pattern name.
    pub name: SmolStr,
    /// Description.
    pub description: String,
    /// Occurrence count.
    pub count: u32,
    /// Confidence (0.0–1.0).
    pub confidence: f32,
}

/// Analyze a successful execution.
pub struct SuccessAnalyzer;

impl SuccessAnalyzer {
    /// Analyze success.
    pub fn analyze(id: Uuid, what_worked: Vec<String>, why: String, generalizable: bool) -> SuccessAnalysis {
        SuccessAnalysis {
            id,
            what_worked,
            why,
            generalizable,
            pattern_name: if generalizable {
                Some(SmolStr::new("successful_pattern"))
            } else {
                None
            },
        }
    }
}

/// Analyze a failed execution.
pub struct FailureAnalyzer;

impl FailureAnalyzer {
    /// Analyze failure by inspecting the error message.
    pub fn analyze(id: Uuid, error_message: &str) -> FailureAnalysis {
        let lower = error_message.to_lowercase();
        let (root_cause, suggested_fix) = if lower.contains("no provider") {
            (RootCause::ProviderIssue, "Configure a provider and retry.".to_string())
        } else if lower.contains("timeout") {
            (RootCause::Timeout, "Increase timeout or retry with smaller input.".to_string())
        } else if lower.contains("conflict") {
            (RootCause::ResourceConflict, "Serialize access to the resource.".to_string())
        } else if lower.contains("ambiguous") || lower.contains("unclear") {
            (RootCause::Ambiguity, "Clarify the goal with the user.".to_string())
        } else if lower.contains("plan") {
            (RootCause::PlanningError, "Re-plan with adjusted decomposition.".to_string())
        } else if lower.contains("effect") || lower.contains("executor") {
            (RootCause::EffectFailure, "Check effect executor logs and retry.".to_string())
        } else {
            (RootCause::Unknown, "Investigate the error manually.".to_string())
        };
        FailureAnalysis {
            id,
            root_cause,
            description: error_message.to_string(),
            suggested_fix,
        }
    }
}

/// Extract patterns from a list of analyses.
pub struct PatternExtractor;

impl PatternExtractor {
    /// Extract patterns from a list of (root_cause, count) pairs.
    pub fn extract(items: &[(RootCause, u32)]) -> Vec<Pattern> {
        items
            .iter()
            .map(|(rc, count)| Pattern {
                name: SmolStr::new(format!("{:?}", rc).to_lowercase()),
                description: format!("Recurring root cause: {:?}", rc),
                count: *count,
                confidence: ((*count as f32) / 10.0).min(1.0),
            })
            .collect()
    }
}

/// Consolidate knowledge (promote patterns to semantic memory).
pub struct KnowledgeConsolidator;

impl KnowledgeConsolidator {
    /// Decide which patterns are strong enough to consolidate.
    pub fn consolidate(patterns: &[Pattern], min_confidence: f32) -> Vec<&Pattern> {
        patterns.iter().filter(|p| p.confidence >= min_confidence).collect()
    }
}
