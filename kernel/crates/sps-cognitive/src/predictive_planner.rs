//! Predictive Planner 2.0 — scores plans by predicted outcome.
//!
//! Uses historical execution data (from Reflection + Execution state)
//! to predict the success probability of each plan before execution.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A scored plan — the output of PredictivePlanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredPlan {
    /// Plan id.
    pub plan_id: Uuid,
    /// Predicted success probability (0.0 - 1.0).
    pub success_probability: f64,
    /// Predicted duration in ms.
    pub estimated_duration_ms: u64,
    /// Risk score (0.0 = safe, 1.0 = dangerous).
    pub risk_score: f64,
    /// Overall score (weighted combination — higher is better).
    pub overall_score: f64,
    /// Human-readable reasoning.
    pub reasoning: String,
}

/// Factors that influence plan scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringFactors {
    /// Historical success rate for similar plans (0-1).
    pub historical_success_rate: f64,
    /// Complexity score (0 = trivial, 1 = extremely complex).
    pub complexity: f64,
    /// Number of dependencies (more = riskier).
    pub dependency_count: u32,
    /// Whether the plan has been attempted before.
    pub previously_attempted: bool,
    /// Number of steps in the plan.
    pub step_count: u32,
    /// Parallelizable steps ratio (0-1).
    pub parallelizable_ratio: f64,
}

impl Default for ScoringFactors {
    fn default() -> Self {
        Self {
            historical_success_rate: 0.5,
            complexity: 0.5,
            dependency_count: 0,
            previously_attempted: false,
            step_count: 1,
            parallelizable_ratio: 0.0,
        }
    }
}

/// The Predictive Planner.
pub struct PredictivePlanner {
    /// Weight for historical success (default: 0.4).
    pub weight_history: f64,
    /// Weight for complexity penalty (default: 0.2).
    pub weight_complexity: f64,
    /// Weight for dependency penalty (default: 0.15).
    pub weight_dependencies: f64,
    /// Weight for parallelization bonus (default: 0.15).
    pub weight_parallel: f64,
    /// Weight for prior attempt bonus (default: 0.1).
    pub weight_prior: f64,
}

impl Default for PredictivePlanner {
    fn default() -> Self {
        Self {
            weight_history: 0.4,
            weight_complexity: 0.2,
            weight_dependencies: 0.15,
            weight_parallel: 0.15,
            weight_prior: 0.1,
        }
    }
}

impl PredictivePlanner {
    /// Score a plan given its factors.
    pub fn score(&self, plan_id: Uuid, factors: &ScoringFactors) -> ScoredPlan {
        // Success probability: primarily from history, adjusted by complexity.
        let success_prob = factors.historical_success_rate
            * (1.0 - factors.complexity * 0.3)
            * (1.0 - (factors.dependency_count as f64 * 0.05).min(0.5));

        // Risk: inverse of success, increased by complexity + dependencies.
        let risk = (1.0 - success_prob) * 0.5
            + factors.complexity * 0.3
            + (factors.dependency_count as f64 * 0.03).min(0.2);

        // Duration estimate: more steps + complexity = longer.
        let estimated_ms = (factors.step_count as u64 * 5000)
            + (factors.complexity * 10000.0) as u64;

        // Parallelization reduces effective duration.
        let parallel_bonus = factors.parallelizable_ratio * 0.3;
        let adjusted_duration = (estimated_ms as f64 * (1.0 - parallel_bonus)) as u64;

        // Overall score: weighted combination.
        let overall = self.weight_history * success_prob
            - self.weight_complexity * factors.complexity
            - self.weight_dependencies * (factors.dependency_count as f64 / 10.0).min(1.0)
            + self.weight_parallel * factors.parallelizable_ratio
            + if factors.previously_attempted {
                self.weight_prior * 0.5
            } else {
                0.0
            };

        let reasoning = format!(
            "success={:.0}%, risk={:.0}%, complexity={:.0}%, deps={}, steps={}, parallel={:.0}%",
            success_prob * 100.0,
            risk * 100.0,
            factors.complexity * 100.0,
            factors.dependency_count,
            factors.step_count,
            factors.parallelizable_ratio * 100.0,
        );

        ScoredPlan {
            plan_id,
            success_probability: success_prob.clamp(0.0, 1.0),
            estimated_duration_ms: adjusted_duration,
            risk_score: risk.clamp(0.0, 1.0),
            overall_score: overall,
            reasoning,
        }
    }

    /// Rank multiple plans by overall score (highest first).
    pub fn rank(&self, plans: &[(Uuid, ScoringFactors)]) -> Vec<ScoredPlan> {
        let mut scored: Vec<_> = plans
            .iter()
            .map(|(id, factors)| self.score(*id, factors))
            .collect();
        scored.sort_by(|a, b| {
            b.overall_score
                .partial_cmp(&a.overall_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_plan_scores_high() {
        let planner = PredictivePlanner::default();
        let factors = ScoringFactors {
            historical_success_rate: 0.9,
            complexity: 0.2,
            dependency_count: 1,
            previously_attempted: true,
            step_count: 3,
            parallelizable_ratio: 0.5,
        };
        let scored = planner.score(Uuid::nil(), &factors);
        assert!(scored.success_probability > 0.5);
        assert!(scored.risk_score < 0.3);
        assert!(scored.overall_score > 0.0);
    }

    #[test]
    fn complex_plan_scores_low() {
        let planner = PredictivePlanner::default();
        let factors = ScoringFactors {
            historical_success_rate: 0.3,
            complexity: 0.9,
            dependency_count: 8,
            previously_attempted: false,
            step_count: 15,
            parallelizable_ratio: 0.0,
        };
        let scored = planner.score(Uuid::nil(), &factors);
        assert!(scored.success_probability < 0.5);
        assert!(scored.risk_score > 0.5);
    }

    #[test]
    fn rank_orders_by_score() {
        let planner = PredictivePlanner::default();
        let plans = vec![
            (Uuid::nil(), ScoringFactors {
                historical_success_rate: 0.3,
                complexity: 0.9,
                dependency_count: 8,
                ..Default::default()
            }),
            (Uuid::nil(), ScoringFactors {
                historical_success_rate: 0.9,
                complexity: 0.1,
                dependency_count: 0,
                ..Default::default()
            }),
        ];
        let ranked = planner.rank(&plans);
        assert!(ranked[0].overall_score > ranked[1].overall_score);
    }
}
