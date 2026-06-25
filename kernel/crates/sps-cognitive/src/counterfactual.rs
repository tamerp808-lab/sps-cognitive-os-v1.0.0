//! Counterfactual Engine — "what if we had done X instead?"
//!
//! Analyzes past executions to answer counterfactual questions.
//! Given a failed execution, it simulates what would have happened
//! if alternative decisions had been made.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A counterfactual analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualResult {
    pub original_outcome: String,
    pub alternatives: Vec<CounterfactualAlternative>,
    pub best_alternative: Option<usize>,
    pub lesson: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualAlternative {
    pub description: String,
    pub predicted_outcome: String,
    pub probability_of_success: f64,
    pub estimated_savings_ms: u64,
}

/// A past execution to analyze.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PastExecution {
    pub goal_id: Uuid,
    pub outcome: ExecutionOutcome,
    pub steps_taken: Vec<String>,
    pub failure_point: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Success,
    Failure,
    Partial,
}

/// The Counterfactual Engine.
pub struct CounterfactualEngine;

impl CounterfactualEngine {
    /// Analyze a past execution and generate counterfactual alternatives.
    pub fn analyze(execution: &PastExecution) -> CounterfactualResult {
        let mut alternatives = Vec::new();

        match execution.outcome {
            ExecutionOutcome::Success => {
                // For successful executions, ask "could we have done it faster?"
                if execution.steps_taken.len() > 3 {
                    alternatives.push(CounterfactualAlternative {
                        description: "Parallelize independent steps".into(),
                        predicted_outcome: "Success (faster)".into(),
                        probability_of_success: 0.85,
                        estimated_savings_ms: execution.duration_ms / 3,
                    });
                }
                if execution.duration_ms > 10_000 {
                    alternatives.push(CounterfactualAlternative {
                        description: "Use cached results from similar past goals".into(),
                        predicted_outcome: "Success (much faster)".into(),
                        probability_of_success: 0.7,
                        estimated_savings_ms: execution.duration_ms / 2,
                    });
                }
                CounterfactualResult {
                    original_outcome: "success".into(),
                    best_alternative: if alternatives.is_empty() { None } else { Some(0) },
                    alternatives,
                    lesson: "Execution succeeded. Consider parallelization for next time.".into(),
                }
            }
            ExecutionOutcome::Failure => {
                // For failures, ask "what if we had done X differently?"
                if let Some(fp) = &execution.failure_point {
                    alternatives.push(CounterfactualAlternative {
                        description: format!("Skip '{}' step and retry from next step", fp),
                        predicted_outcome: "Possible success".into(),
                        probability_of_success: 0.4,
                        estimated_savings_ms: 0,
                    });
                    alternatives.push(CounterfactualAlternative {
                        description: format!("Decompose '{}' into smaller sub-steps", fp),
                        predicted_outcome: "Likely success".into(),
                        probability_of_success: 0.65,
                        estimated_savings_ms: 0,
                    });
                    alternatives.push(CounterfactualAlternative {
                        description: "Use alternative approach (different LLM provider)".into(),
                        predicted_outcome: "Possible success".into(),
                        probability_of_success: 0.55,
                        estimated_savings_ms: 0,
                    });
                }
                let best = alternatives
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        a.probability_of_success
                            .partial_cmp(&b.probability_of_success)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i);

                let lesson = if let Some(fp) = &execution.failure_point {
                    format!("Failure at '{}'. Best alternative: {}",
                        fp,
                        best.and_then(|i| alternatives.get(i))
                            .map(|a| a.description.as_str())
                            .unwrap_or("none")
                    )
                } else {
                    "Failure without identified failure point. Add more instrumentation.".into()
                };

                CounterfactualResult {
                    original_outcome: "failure".into(),
                    best_alternative: best,
                    alternatives,
                    lesson,
                }
            }
            ExecutionOutcome::Partial => {
                alternatives.push(CounterfactualAlternative {
                    description: "Retry failed steps with different parameters".into(),
                    predicted_outcome: "Possible full success".into(),
                    probability_of_success: 0.5,
                    estimated_savings_ms: 0,
                });
                CounterfactualResult {
                    original_outcome: "partial".into(),
                    best_alternative: Some(0),
                    alternatives,
                    lesson: "Partial success — retry failed steps for full completion.".into(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_failure() {
        let exec = PastExecution {
            goal_id: Uuid::nil(),
            outcome: ExecutionOutcome::Failure,
            steps_taken: vec!["setup".into(), "build".into(), "test".into()],
            failure_point: Some("test".into()),
            duration_ms: 15_000,
        };
        let result = CounterfactualEngine::analyze(&exec);
        assert_eq!(result.original_outcome, "failure");
        assert!(!result.alternatives.is_empty());
        assert!(result.best_alternative.is_some());
        assert!(result.lesson.contains("test"));
    }

    #[test]
    fn analyzes_success_for_optimization() {
        let exec = PastExecution {
            goal_id: Uuid::nil(),
            outcome: ExecutionOutcome::Success,
            steps_taken: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            failure_point: None,
            duration_ms: 20_000,
        };
        let result = CounterfactualEngine::analyze(&exec);
        assert_eq!(result.original_outcome, "success");
        assert!(!result.alternatives.is_empty());
        assert!(result.alternatives[0].estimated_savings_ms > 0);
    }
}
