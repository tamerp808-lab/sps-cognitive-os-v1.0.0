//! Goal Forecaster — predicts goal success probability + timeline.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A goal forecast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalForecast {
    pub goal_id: Uuid,
    pub success_probability: f64,
    pub estimated_completion_ms: u64,
    pub confidence: f64,
    pub blockers: Vec<String>,
    pub recommendation: ForecastRecommendation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastRecommendation {
    Proceed,
    ProceedWithCaution,
    NeedsDelegation,
    NeedsDecomposition,
    Abort,
}

/// The Goal Forecaster.
pub struct GoalForecaster {
    pub min_confidence: f64,
}

impl Default for GoalForecaster {
    fn default() -> Self {
        Self { min_confidence: 0.3 }
    }
}

impl GoalForecaster {
    /// Forecast a goal based on its characteristics.
    pub fn forecast(
        &self,
        goal_id: Uuid,
        priority: u32,
        objective_count: usize,
        dependency_count: usize,
        historical_success: f64,
    ) -> GoalForecast {
        let mut blockers = Vec::new();

        // Base probability from history.
        let mut prob = historical_success;

        // Priority bonus (higher priority = more attention = better outcome).
        prob += (priority as f64 / 10.0) * 0.05;

        // Objective complexity penalty.
        let complexity = (objective_count as f64 / 10.0).min(1.0);
        prob -= complexity * 0.15;

        // Dependency penalty.
        if dependency_count > 3 {
            prob -= 0.2;
            blockers.push(format!("{} dependencies — high coupling", dependency_count));
        } else if dependency_count > 0 {
            prob -= (dependency_count as f64 * 0.03);
        }

        // Estimate duration.
        let base_ms = 10_000;
        let per_objective_ms = 5_000;
        let per_dependency_ms = 3_000;
        let estimated = base_ms
            + (objective_count as u64 * per_objective_ms)
            + (dependency_count as u64 * per_dependency_ms);

        // Confidence: how sure are we in this forecast?
        let confidence = if historical_success > 0.0 {
            0.8 // we have data
        } else {
            0.4 // guessing
        };

        // Recommendation.
        let recommendation = if prob > 0.7 {
            ForecastRecommendation::Proceed
        } else if prob > 0.5 {
            if dependency_count > 3 {
                ForecastRecommendation::NeedsDelegation
            } else {
                ForecastRecommendation::ProceedWithCaution
            }
        } else if prob > 0.3 {
            if objective_count > 5 {
                ForecastRecommendation::NeedsDecomposition
            } else {
                ForecastRecommendation::NeedsDelegation
            }
        } else {
            ForecastRecommendation::Abort
        };

        if objective_count > 5 {
            blockers.push("Too many objectives — consider decomposition".into());
        }

        GoalForecast {
            goal_id,
            success_probability: prob.clamp(0.0, 1.0),
            estimated_completion_ms: estimated,
            confidence,
            blockers,
            recommendation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_goal_proceeds() {
        let f = GoalForecaster::default();
        let forecast = f.forecast(Uuid::nil(), 5, 2, 0, 0.8);
        assert!(forecast.success_probability > 0.7);
        assert_eq!(forecast.recommendation, ForecastRecommendation::Proceed);
    }

    #[test]
    fn complex_goal_needs_decomposition() {
        let f = GoalForecaster::default();
        let forecast = f.forecast(Uuid::nil(), 1, 8, 5, 0.2);
        assert!(forecast.success_probability < 0.5);
        assert!(matches!(
            forecast.recommendation,
            ForecastRecommendation::NeedsDecomposition | ForecastRecommendation::Abort
        ));
        assert!(!forecast.blockers.is_empty());
    }
}
