//! Decision Scorer — multi-factor decision ranking.

use serde::{Deserialize, Serialize};

/// A decision option to be scored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionOption {
    pub id: String,
    pub label: String,
    pub factors: DecisionFactors,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionFactors {
    /// Expected benefit (0-1, higher is better).
    pub benefit: f64,
    /// Cost in resources (0-1, lower is better).
    pub cost: f64,
    /// Risk level (0-1, lower is better).
    pub risk: f64,
    /// Time to execute (0-1, lower is better).
    pub time: f64,
    /// Alignment with current goals (0-1, higher is better).
    pub alignment: f64,
    /// Reversibility (0-1, higher is better — easy to undo).
    pub reversibility: f64,
}

/// A scored decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDecision {
    pub id: String,
    pub label: String,
    pub score: f64,
    pub reasoning: String,
}

/// The Decision Scorer.
pub struct DecisionScorer {
    pub weight_benefit: f64,
    pub weight_cost: f64,
    pub weight_risk: f64,
    pub weight_time: f64,
    pub weight_alignment: f64,
    pub weight_reversibility: f64,
}

impl Default for DecisionScorer {
    fn default() -> Self {
        Self {
            weight_benefit: 0.30,
            weight_cost: 0.15,
            weight_risk: 0.20,
            weight_time: 0.10,
            weight_alignment: 0.15,
            weight_reversibility: 0.10,
        }
    }
}

impl DecisionScorer {
    /// Score a single option.
    pub fn score(&self, option: &DecisionOption) -> ScoredDecision {
        let f = &option.factors;
        let score = self.weight_benefit * f.benefit
            - self.weight_cost * f.cost
            - self.weight_risk * f.risk
            - self.weight_time * f.time
            + self.weight_alignment * f.alignment
            + self.weight_reversibility * f.reversibility;

        let reasoning = format!(
            "benefit={:.0}%, cost={:.0}%, risk={:.0}%, time={:.0}%, align={:.0}%, reversible={:.0}%",
            f.benefit * 100.0,
            f.cost * 100.0,
            f.risk * 100.0,
            f.time * 100.0,
            f.alignment * 100.0,
            f.reversibility * 100.0,
        );

        ScoredDecision {
            id: option.id.clone(),
            label: option.label.clone(),
            score,
            reasoning,
        }
    }

    /// Rank multiple options (highest score first).
    pub fn rank(&self, options: &[DecisionOption]) -> Vec<ScoredDecision> {
        let mut scored: Vec<_> = options.iter().map(|o| self.score(o)).collect();
        scored.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }

    /// Pick the best option.
    pub fn decide(&self, options: &[DecisionOption]) -> Option<ScoredDecision> {
        self.rank(options).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_benefit_low_risk_wins() {
        let scorer = DecisionScorer::default();
        let options = vec![
            DecisionOption {
                id: "safe".into(),
                label: "Safe option".into(),
                factors: DecisionFactors {
                    benefit: 0.8,
                    cost: 0.2,
                    risk: 0.1,
                    time: 0.3,
                    alignment: 0.7,
                    reversibility: 0.9,
                },
            },
            DecisionOption {
                id: "risky".into(),
                label: "Risky option".into(),
                factors: DecisionFactors {
                    benefit: 0.9,
                    cost: 0.5,
                    risk: 0.8,
                    time: 0.6,
                    alignment: 0.5,
                    reversibility: 0.1,
                },
            },
        ];
        let ranked = scorer.rank(&options);
        assert_eq!(ranked[0].id, "safe");
        assert!(ranked[0].score > ranked[1].score);
    }
}
