//! Importance Scorer — assigns importance scores to memories.

use serde::{Deserialize, Serialize};

/// Importance factors for a memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportanceFactors {
    /// How often this memory has been accessed.
    pub access_count: u32,
    /// How many goals reference this memory.
    pub goal_references: u32,
    /// How recent the memory is (0 = ancient, 1 = just now).
    pub recency: f64,
    /// Emotional weight (0 = neutral, 1 = highly emotional).
    pub emotional_weight: f64,
    /// Whether the memory led to a successful outcome.
    pub success_correlation: f64,
    /// Whether the memory is unique (low duplication).
    pub uniqueness: f64,
    /// Number of other memories linked to this one.
    pub link_count: u32,
}

/// The Importance Scorer.
pub struct ImportanceScorer {
    pub weight_access: f64,
    pub weight_goals: f64,
    pub weight_recency: f64,
    pub weight_emotional: f64,
    pub weight_success: f64,
    pub weight_uniqueness: f64,
    pub weight_links: f64,
}

impl Default for ImportanceScorer {
    fn default() -> Self {
        Self {
            weight_access: 0.15,
            weight_goals: 0.25,
            weight_recency: 0.10,
            weight_emotional: 0.15,
            weight_success: 0.20,
            weight_uniqueness: 0.10,
            weight_links: 0.05,
        }
    }
}

impl ImportanceScorer {
    /// Score a memory's importance (0.0 to 1.0).
    pub fn score(&self, factors: &ImportanceFactors) -> f64 {
        let access_score = (factors.access_count as f64 / 10.0).min(1.0);
        let goal_score = (factors.goal_references as f64 / 5.0).min(1.0);
        let link_score = (factors.link_count as f64 / 10.0).min(1.0);

        let raw = self.weight_access * access_score
            + self.weight_goals * goal_score
            + self.weight_recency * factors.recency
            + self.weight_emotional * factors.emotional_weight
            + self.weight_success * factors.success_correlation
            + self.weight_uniqueness * factors.uniqueness
            + self.weight_links * link_score;

        raw.clamp(0.0, 1.0)
    }

    /// Classify importance into tiers.
    pub fn classify(&self, score: f64) -> ImportanceTier {
        if score >= 0.8 {
            ImportanceTier::Critical
        } else if score >= 0.6 {
            ImportanceTier::High
        } else if score >= 0.3 {
            ImportanceTier::Medium
        } else if score >= 0.1 {
            ImportanceTier::Low
        } else {
            ImportanceTier::Forgettable
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportanceTier {
    Critical,
    High,
    Medium,
    Low,
    Forgettable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequently_accessed_success_memory_is_important() {
        let scorer = ImportanceScorer::default();
        let factors = ImportanceFactors {
            access_count: 20,
            goal_references: 3,
            recency: 0.9,
            emotional_weight: 0.3,
            success_correlation: 0.8,
            uniqueness: 0.7,
            link_count: 5,
        };
        let score = scorer.score(&factors);
        assert!(score > 0.6);
        assert_eq!(scorer.classify(score), ImportanceTier::High);
    }

    #[test]
    fn unused_old_memory_is_forgettable() {
        let scorer = ImportanceScorer::default();
        let factors = ImportanceFactors {
            access_count: 0,
            goal_references: 0,
            recency: 0.01,
            emotional_weight: 0.0,
            success_correlation: 0.0,
            uniqueness: 0.1,
            link_count: 0,
        };
        let score = scorer.score(&factors);
        assert!(score < 0.1);
        assert_eq!(scorer.classify(score), ImportanceTier::Forgettable);
    }
}
