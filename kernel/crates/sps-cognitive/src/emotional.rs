//! Emotional Memory — memories tagged with emotional context.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An emotional tag for a memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    Joy,
    Trust,
    Fear,
    Surprise,
    Sadness,
    Disgust,
    Anger,
    Anticipation,
    Neutral,
}

/// An emotional memory record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalMemory {
    pub memory_id: Uuid,
    pub emotion: Emotion,
    pub intensity: f64,    // 0.0 to 1.0
    pub valence: f64,      // -1.0 (negative) to +1.0 (positive)
    pub arousal: f64,      // 0.0 (calm) to 1.0 (excited)
    pub timestamp_ms: u64,
    pub context: String,
}

impl EmotionalMemory {
    /// Create a new emotional memory tag.
    pub fn new(memory_id: Uuid, emotion: Emotion, intensity: f64) -> Self {
        let (valence, arousal) = emotion_to_valence_arousal(&emotion);
        Self {
            memory_id,
            emotion,
            intensity: intensity.clamp(0.0, 1.0),
            valence,
            arousal,
            timestamp_ms: 0,
            context: String::new(),
        }
    }

    /// Compute a combined emotional weight for importance scoring.
    pub fn emotional_weight(&self) -> f64 {
        self.intensity * (0.5 + self.arousal * 0.5)
    }

    /// Is this a positive memory?
    pub fn is_positive(&self) -> bool {
        self.valence > 0.0
    }

    /// Is this a negative memory?
    pub fn is_negative(&self) -> bool {
        self.valence < 0.0
    }
}

/// Map an emotion to valence (-1 to +1) and arousal (0 to 1).
fn emotion_to_valence_arousal(emotion: &Emotion) -> (f64, f64) {
    match emotion {
        Emotion::Joy => (0.9, 0.7),
        Emotion::Trust => (0.7, 0.3),
        Emotion::Fear => (-0.8, 0.9),
        Emotion::Surprise => (0.0, 0.9),
        Emotion::Sadness => (-0.7, 0.2),
        Emotion::Disgust => (-0.6, 0.4),
        Emotion::Anger => (-0.8, 0.8),
        Emotion::Anticipation => (0.5, 0.6),
        Emotion::Neutral => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joy_is_positive_and_aroused() {
        let em = EmotionalMemory::new(Uuid::nil(), Emotion::Joy, 0.8);
        assert!(em.is_positive());
        assert!(em.arousal > 0.5);
        assert!(em.emotional_weight() > 0.4);
    }

    #[test]
    fn sadness_is_negative_and_calm() {
        let em = EmotionalMemory::new(Uuid::nil(), Emotion::Sadness, 0.6);
        assert!(em.is_negative());
        assert!(em.arousal < 0.5);
    }

    #[test]
    fn neutral_has_zero_valence() {
        let em = EmotionalMemory::new(Uuid::nil(), Emotion::Neutral, 0.0);
        assert!(!em.is_positive());
        assert!(!em.is_negative());
        assert_eq!(em.emotional_weight(), 0.0);
    }

    #[test]
    fn intensity_is_clamped() {
        let em = EmotionalMemory::new(Uuid::nil(), Emotion::Joy, 1.5);
        assert_eq!(em.intensity, 1.0);
    }
}
