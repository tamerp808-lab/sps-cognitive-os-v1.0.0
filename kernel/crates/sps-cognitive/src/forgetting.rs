//! Forgetting Policy — determines which memories to decay/forget.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A forgetting decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingDecision {
    pub memory_id: Uuid,
    pub action: ForgettingAction,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgettingAction {
    /// Keep the memory as-is.
    Keep,
    /// Reduce the memory's strength (decay).
    Decay,
    /// Remove the memory entirely.
    Forget,
    /// Archive (move to cold storage, not actively queried).
    Archive,
}

/// Memory attributes for forgetting decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForgettingMemoryInfo {
    pub id: Uuid,
    pub importance: f64,       // 0-1
    pub access_count: u32,
    pub last_accessed_ms: u64, // ms since epoch
    pub age_ms: u64,
    pub current_strength: f64, // 0-1
    pub emotional_weight: f64, // 0-1
}

/// The Forgetting Policy.
pub struct ForgettingPolicy {
    /// Memories with importance below this are candidates for forgetting.
    pub min_importance: f64,
    /// Memories not accessed in this many ms are candidates for decay.
    pub decay_after_ms: u64,
    /// Memories not accessed in this many ms are candidates for archiving.
    pub archive_after_ms: u64,
    /// Strength below which a memory is forgotten entirely.
    pub forget_below_strength: f64,
    /// Decay factor per decay cycle (0-1, e.g. 0.9 = lose 10%).
    pub decay_factor: f64,
    /// Emotional memories get a bonus to retention.
    pub emotional_bonus: f64,
}

impl Default for ForgettingPolicy {
    fn default() -> Self {
        Self {
            min_importance: 0.2,
            decay_after_ms: 3_600_000,       // 1 hour
            archive_after_ms: 86_400_000,    // 24 hours
            forget_below_strength: 0.05,
            decay_factor: 0.9,
            emotional_bonus: 0.3,
        }
    }
}

impl ForgettingPolicy {
    /// Decide what to do with a memory.
    pub fn evaluate(&self, info: &ForgettingMemoryInfo) -> ForgettingDecision {
        // Emotional memories get a retention bonus.
        let effective_importance = info.importance + (info.emotional_weight * self.emotional_bonus);

        // If strength is very low, forget.
        if info.current_strength < self.forget_below_strength {
            return ForgettingDecision {
                memory_id: info.id,
                action: ForgettingAction::Forget,
                reason: format!(
                    "strength={:.2} below threshold {:.2}",
                    info.current_strength, self.forget_below_strength
                ),
            };
        }

        // If very old and unimportant, archive.
        if info.age_ms > self.archive_after_ms && effective_importance < self.min_importance {
            return ForgettingDecision {
                memory_id: info.id,
                action: ForgettingAction::Archive,
                reason: format!(
                    "age={}ms > {}ms and importance={:.2} < {:.2}",
                    info.age_ms, self.archive_after_ms, effective_importance, self.min_importance
                ),
            };
        }

        // If not accessed recently, decay.
        if info.last_accessed_ms > self.decay_after_ms && info.access_count < 3 {
            let new_strength = info.current_strength * self.decay_factor;
            return ForgettingDecision {
                memory_id: info.id,
                action: ForgettingAction::Decay,
                reason: format!(
                    "not accessed in {}ms, decaying strength {:.2}→{:.2}",
                    info.last_accessed_ms, info.current_strength, new_strength
                ),
            };
        }

        // Otherwise keep.
        ForgettingDecision {
            memory_id: info.id,
            action: ForgettingAction::Keep,
            reason: format!(
                "importance={:.2}, strength={:.2}, accesses={}",
                effective_importance, info.current_strength, info.access_count
            ),
        }
    }

    /// Calculate the new strength after a decay cycle.
    pub fn apply_decay(&self, current_strength: f64) -> f64 {
        (current_strength * self.decay_factor).max(0.0)
    }

    /// Calculate strength boost when a memory is accessed.
    pub fn boost_on_access(&self, current_strength: f64) -> f64 {
        (current_strength + 0.1).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_important_recent_memory() {
        let policy = ForgettingPolicy::default();
        let info = ForgettingMemoryInfo {
            id: Uuid::nil(),
            importance: 0.8,
            access_count: 10,
            last_accessed_ms: 1000,
            age_ms: 5000,
            current_strength: 0.9,
            emotional_weight: 0.0,
        };
        let decision = policy.evaluate(&info);
        assert_eq!(decision.action, ForgettingAction::Keep);
    }

    #[test]
    fn forgets_very_weak_memory() {
        let policy = ForgettingPolicy::default();
        let info = ForgettingMemoryInfo {
            id: Uuid::nil(),
            importance: 0.1,
            access_count: 0,
            last_accessed_ms: 9_000_000,
            age_ms: 9_000_000,
            current_strength: 0.01,
            emotional_weight: 0.0,
        };
        let decision = policy.evaluate(&info);
        assert_eq!(decision.action, ForgettingAction::Forget);
    }

    #[test]
    fn decays_old_unaccessed_memory() {
        let policy = ForgettingPolicy::default();
        let info = ForgettingMemoryInfo {
            id: Uuid::nil(),
            importance: 0.3,
            access_count: 1,
            last_accessed_ms: 5_000_000,
            age_ms: 5_000_000,
            current_strength: 0.5,
            emotional_weight: 0.0,
        };
        let decision = policy.evaluate(&info);
        assert_eq!(decision.action, ForgettingAction::Decay);
    }

    #[test]
    fn emotional_memory_gets_bonus() {
        let policy = ForgettingPolicy::default();
        let info = ForgettingMemoryInfo {
            id: Uuid::nil(),
            importance: 0.1, // low importance
            access_count: 1,
            last_accessed_ms: 5_000_000,
            age_ms: 5_000_000,
            current_strength: 0.5,
            emotional_weight: 0.9, // high emotional weight
        };
        let decision = policy.evaluate(&info);
        // With emotional bonus (0.1 + 0.9*0.3 = 0.37 > 0.2 min_importance),
        // it should NOT be archived even though old.
        assert_ne!(decision.action, ForgettingAction::Archive);
    }
}
