//! Memory Consolidation — promotes episodic memories to semantic/procedural.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Memory kind hierarchy (episodic → semantic → procedural).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Raw event memories (what happened).
    Episodic,
    /// Abstracted knowledge (what is true).
    Semantic,
    /// How-to knowledge (what to do).
    Procedural,
    /// Automatic pattern (unconscious skill).
    Automatic,
}

/// A consolidation candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationCandidate {
    pub memory_id: Uuid,
    pub current_tier: MemoryTier,
    pub proposed_tier: MemoryTier,
    pub reason: String,
    pub confidence: f64,
}

/// Criteria for promotion to the next tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionCriteria {
    /// Minimum access count for promotion.
    pub min_access_count: u32,
    /// Minimum success correlation (0-1).
    pub min_success_correlation: f64,
    /// Minimum age in ms before promotion.
    pub min_age_ms: u64,
}

impl Default for PromotionCriteria {
    fn default() -> Self {
        Self {
            min_access_count: 3,
            min_success_correlation: 0.6,
            min_age_ms: 60_000, // 1 minute (fast for testing)
        }
    }
}

/// The Memory Consolidator.
pub struct MemoryConsolidator {
    pub criteria: PromotionCriteria,
}

impl Default for MemoryConsolidator {
    fn default() -> Self {
        Self {
            criteria: PromotionCriteria::default(),
        }
    }
}

/// Memory stats for consolidation decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    pub access_count: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub age_ms: u64,
    pub current_tier: MemoryTier,
}

impl Default for MemoryTier {
    fn default() -> Self {
        Self::Episodic
    }
}

impl MemoryConsolidator {
    /// Evaluate whether a memory should be promoted.
    pub fn evaluate(&self, memory_id: Uuid, stats: &MemoryStats) -> Option<ConsolidationCandidate> {
        let success_rate = if stats.success_count + stats.failure_count > 0 {
            stats.success_count as f64 / (stats.success_count + stats.failure_count) as f64
        } else {
            0.0
        };

        let next_tier = match stats.current_tier {
            MemoryTier::Episodic => Some(MemoryTier::Semantic),
            MemoryTier::Semantic => Some(MemoryTier::Procedural),
            MemoryTier::Procedural => Some(MemoryTier::Automatic),
            MemoryTier::Automatic => None,
        };

        let next_tier = next_tier?;

        let meets_access = stats.access_count >= self.criteria.min_access_count;
        let meets_success = success_rate >= self.criteria.min_success_correlation;
        let meets_age = stats.age_ms >= self.criteria.min_age_ms;

        if meets_access && meets_success && meets_age {
            Some(ConsolidationCandidate {
                memory_id,
                current_tier: stats.current_tier,
                proposed_tier: next_tier,
                reason: format!(
                    "access={}, success_rate={:.0}%, age={}ms — promoted {:?}→{:?}",
                    stats.access_count,
                    success_rate * 100.0,
                    stats.age_ms,
                    stats.current_tier,
                    next_tier,
                ),
                confidence: success_rate.min(1.0),
            })
        } else {
            None
        }
    }

    /// Batch evaluate multiple memories.
    pub fn evaluate_batch(&self, memories: &[(Uuid, MemoryStats)]) -> Vec<ConsolidationCandidate> {
        memories
            .iter()
            .filter_map(|(id, stats)| self.evaluate(*id, stats))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotes_episodic_to_semantic() {
        let c = MemoryConsolidator::default();
        let stats = MemoryStats {
            access_count: 5,
            success_count: 4,
            failure_count: 1,
            age_ms: 120_000,
            current_tier: MemoryTier::Episodic,
        };
        let result = c.evaluate(Uuid::nil(), &stats);
        assert!(result.is_some());
        let candidate = result.unwrap();
        assert_eq!(candidate.proposed_tier, MemoryTier::Semantic);
    }

    #[test]
    fn does_not_promote_with_low_access() {
        let c = MemoryConsolidator::default();
        let stats = MemoryStats {
            access_count: 1,
            success_count: 5,
            failure_count: 0,
            age_ms: 120_000,
            current_tier: MemoryTier::Episodic,
        };
        let result = c.evaluate(Uuid::nil(), &stats);
        assert!(result.is_none());
    }

    #[test]
    fn does_not_promote_automatic() {
        let c = MemoryConsolidator::default();
        let stats = MemoryStats {
            access_count: 100,
            success_count: 100,
            failure_count: 0,
            age_ms: 1_000_000,
            current_tier: MemoryTier::Automatic,
        };
        let result = c.evaluate(Uuid::nil(), &stats);
        assert!(result.is_none());
    }
}
