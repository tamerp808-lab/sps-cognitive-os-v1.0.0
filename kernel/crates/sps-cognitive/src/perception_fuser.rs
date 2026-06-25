//! SPS Gap 1: Multi-modal Perception Fuser.
//!
//! Merges ALL input sources (voice, screen, notifications, location,
//! time, device state, files) into a single unified PerceptionContext
//! that feeds the CognitiveLoop.
//!
//! Instead of the CognitiveLoop receiving one input at a time, the
//! PerceptionFuser continuously collects from all sources and produces
//! a fused context that represents the COMPLETE state of the user's
//! world at the moment of cognition.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single perception from one modality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Perception {
    /// Which modality produced this.
    pub source: PerceptionSource,
    /// The perceived content.
    pub content: serde_json::Value,
    /// Confidence (0-1).
    pub confidence: f64,
    /// Timestamp (ms since epoch).
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerceptionSource {
    /// Voice transcription (STT).
    Voice,
    /// Screen content (Accessibility Service).
    Screen,
    /// Active notifications (NotificationListener).
    Notifications,
    /// Camera frame analysis (VLM).
    Camera,
    /// File system changes.
    FileSystem,
    /// GPS location.
    Location,
    /// System clock.
    Time,
    /// Device state (battery, network, thermal).
    DeviceState,
    /// User text input.
    TextInput,
    /// SPS internal scheduled trigger.
    Scheduled,
}

/// The fused context — what SPS knows about the world RIGHT NOW.
///
/// This is what CognitiveLoop receives instead of a single CognitiveInput.
/// It represents the complete multi-modal perception of the user's world.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerceptionContext {
    /// All active perceptions, keyed by source.
    pub perceptions: BTreeMap<String, Perception>,
    /// Fused summary — natural language description of what's happening.
    pub summary: String,
    /// Detected user intent (fused from all sources).
    pub fused_intent: String,
    /// Urgency level (0 = low, 1 = critical).
    pub urgency: f64,
    /// Whether the user is actively interacting.
    pub user_is_active: bool,
    /// Current location label (if known).
    pub location: Option<String>,
    /// Time of day category.
    pub time_of_day: TimeOfDay,
    /// Device battery level (0-100, -1 = unknown).
    pub battery_level: i32,
    /// Active app package (if known from screen).
    pub active_app: Option<String>,
    /// Unread notification count.
    pub notification_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeOfDay {
    EarlyMorning,  // 5-8
    Morning,       // 8-12
    Afternoon,     // 12-17
    Evening,       // 17-20
    Night,         // 20-23
    LateNight,     // 23-5
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self::Morning
    }
}

/// The Perception Fuser.
///
/// Collects perceptions from all modalities and fuses them into a
/// single PerceptionContext. The CognitiveLoop uses this context
/// instead of a single input.
pub struct PerceptionFuser {
    /// Buffer of recent perceptions (per source, keeps latest).
    buffer: BTreeMap<String, Perception>,
    /// How long to keep perceptions before they expire (ms).
    pub perception_ttl_ms: u64,
}

impl Default for PerceptionFuser {
    fn default() -> Self {
        Self {
            buffer: BTreeMap::new(),
            perception_ttl_ms: 300_000, // 5 minutes
        }
    }
}

impl PerceptionFuser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a perception from any modality.
    pub fn feed(&mut self, perception: Perception) {
        let key = format!("{:?}", perception.source);
        self.buffer.insert(key, perception);
    }

    /// Feed voice transcription.
    pub fn feed_voice(&mut self, text: &str, confidence: f64) {
        self.feed(Perception {
            source: PerceptionSource::Voice,
            content: serde_json::json!({"text": text}),
            confidence,
            timestamp_ms: 0,
        });
    }

    /// Feed screen content.
    pub fn feed_screen(&mut self, app: &str, text: &str) {
        self.feed(Perception {
            source: PerceptionSource::Screen,
            content: serde_json::json!({"app": app, "text": text}),
            confidence: 1.0,
            timestamp_ms: 0,
        });
    }

    /// Feed notifications.
    pub fn feed_notifications(&mut self, count: u32, summaries: Vec<String>) {
        self.feed(Perception {
            source: PerceptionSource::Notifications,
            content: serde_json::json!({"count": count, "summaries": summaries}),
            confidence: 1.0,
            timestamp_ms: 0,
        });
    }

    /// Feed device state.
    pub fn feed_device_state(&mut self, battery: i32, network: &str) {
        self.feed(Perception {
            source: PerceptionSource::DeviceState,
            content: serde_json::json!({"battery": battery, "network": network}),
            confidence: 1.0,
            timestamp_ms: 0,
        });
    }

    /// Feed location.
    pub fn feed_location(&mut self, lat: f64, lon: f64, label: Option<&str>) {
        self.feed(Perception {
            source: PerceptionSource::Location,
            content: serde_json::json!({"lat": lat, "lon": lon, "label": label}),
            confidence: 0.9,
            timestamp_ms: 0,
        });
    }

    /// Feed time.
    pub fn feed_time(&mut self, hour: u32) {
        let tod = match hour {
            5..=7 => TimeOfDay::EarlyMorning,
            8..=11 => TimeOfDay::Morning,
            12..=16 => TimeOfDay::Afternoon,
            17..=19 => TimeOfDay::Evening,
            20..=22 => TimeOfDay::Night,
            _ => TimeOfDay::LateNight,
        };
        self.feed(Perception {
            source: PerceptionSource::Time,
            content: serde_json::json!({"hour": hour, "time_of_day": format!("{:?}", tod)}),
            confidence: 1.0,
            timestamp_ms: 0,
        });
    }

    /// Feed text input.
    pub fn feed_text(&mut self, text: &str) {
        self.feed(Perception {
            source: PerceptionSource::TextInput,
            content: serde_json::json!({"text": text}),
            confidence: 1.0,
            timestamp_ms: 0,
        });
    }

    /// Fuse all buffered perceptions into a single context.
    pub fn fuse(&self) -> PerceptionContext {
        let mut ctx = PerceptionContext::default();

        // Extract voice.
        if let Some(p) = self.buffer.get("Voice") {
            if let Some(text) = p.content.get("text").and_then(|v| v.as_str()) {
                ctx.summary = format!("User said: {}", text);
                ctx.user_is_active = true;
                ctx.fused_intent = detect_intent(text);
            }
        }

        // Extract screen.
        if let Some(p) = self.buffer.get("Screen") {
            if let Some(app) = p.content.get("app").and_then(|v| v.as_str()) {
                ctx.active_app = Some(app.to_string());
                if ctx.summary.is_empty() {
                    if let Some(text) = p.content.get("text").and_then(|v| v.as_str()) {
                        ctx.summary = format!("Looking at {} — {}", app, &text[..text.len().min(100)]);
                    } else {
                        ctx.summary = format!("Looking at {}", app);
                    }
                }
                ctx.user_is_active = true;
            }
        }

        // Extract notifications.
        if let Some(p) = self.buffer.get("Notifications") {
            if let Some(count) = p.content.get("count").and_then(|v| v.as_u64()) {
                ctx.notification_count = count as u32;
                if count > 5 {
                    ctx.urgency = (ctx.urgency).max(0.7);
                }
                if ctx.summary.is_empty() {
                    ctx.summary = format!("{} unread notifications", count);
                }
            }
        }

        // Extract device state.
        if let Some(p) = self.buffer.get("DeviceState") {
            if let Some(battery) = p.content.get("battery").and_then(|v| v.as_i64()) {
                ctx.battery_level = battery as i32;
                if battery < 20 {
                    ctx.urgency = ctx.urgency.max(0.5);
                }
            }
        }

        // Extract location.
        if let Some(p) = self.buffer.get("Location") {
            if let Some(label) = p.content.get("label").and_then(|v| v.as_str()) {
                if !label.is_empty() {
                    ctx.location = Some(label.to_string());
                }
            }
        }

        // Extract time.
        if let Some(p) = self.buffer.get("Time") {
            if let Some(hour) = p.content.get("hour").and_then(|v| v.as_u64()) {
                ctx.time_of_day = match hour {
                    5..=7 => TimeOfDay::EarlyMorning,
                    8..=11 => TimeOfDay::Morning,
                    12..=16 => TimeOfDay::Afternoon,
                    17..=19 => TimeOfDay::Evening,
                    20..=22 => TimeOfDay::Night,
                    _ => TimeOfDay::LateNight,
                };
            }
        }

        // If summary is still empty, use a default.
        if ctx.summary.is_empty() {
            ctx.summary = "No active input — idle state".into();
            ctx.fused_intent = "idle".into();
        }

        // Adjust urgency based on time of day.
        if ctx.time_of_day == TimeOfDay::LateNight && ctx.user_is_active {
            ctx.urgency = ctx.urgency.max(0.3); // user active late at night → mild concern
        }

        ctx.perceptions = self.buffer.clone();
        ctx
    }
}

fn detect_intent(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("build") || lower.contains("create") || lower.contains("make") {
        "create".into()
    } else if lower.contains("search") || lower.contains("find") || lower.contains("where") {
        "search".into()
    } else if lower.contains("explain") || lower.contains("what") || lower.contains("how") {
        "explain".into()
    } else if lower.contains("fix") || lower.contains("repair") || lower.contains("debug") {
        "fix".into()
    } else if lower.contains("remind") || lower.contains("schedule") || lower.contains("alarm") {
        "schedule".into()
    } else if lower.contains("hello") || lower.contains("hi") || lower.contains("مرحبا") {
        "greeting".into()
    } else if lower.contains("انشئ") || lower.contains("ابن") || lower.contains("اصنع") {
        "create".into()
    } else if lower.contains("ابحث") || lower.contains("اين") {
        "search".into()
    } else if lower.contains("شرح") || lower.contains("كيف") {
        "explain".into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuses_voice_and_screen() {
        let mut fuser = PerceptionFuser::new();
        fuser.feed_voice("Build a todo app", 0.95);
        fuser.feed_screen("com.android.vscode", "fn main() { }");

        let ctx = fuser.fuse();
        assert!(ctx.summary.contains("Build a todo app"));
        assert_eq!(ctx.fused_intent, "create");
        assert!(ctx.user_is_active);
        assert_eq!(ctx.active_app, Some("com.android.vscode".into()));
    }

    #[test]
    fn fuses_notifications_and_urgency() {
        let mut fuser = PerceptionFuser::new();
        fuser.feed_notifications(10, vec!["Urgent: server down".into()]);
        fuser.feed_device_state(15, "wifi");

        let ctx = fuser.fuse();
        assert_eq!(ctx.notification_count, 10);
        assert!(ctx.urgency >= 0.7, "10 notifications should trigger high urgency");
        assert!(ctx.battery_level < 20, "Battery 15% should be low");
    }

    #[test]
    fn fuses_time_of_day() {
        let mut fuser = PerceptionFuser::new();
        fuser.feed_time(23);
        let ctx = fuser.fuse();
        assert_eq!(ctx.time_of_day, TimeOfDay::LateNight);
    }

    #[test]
    fn empty_fuser_produces_idle_context() {
        let fuser = PerceptionFuser::new();
        let ctx = fuser.fuse();
        assert_eq!(ctx.fused_intent, "idle");
        assert!(!ctx.user_is_active);
    }

    #[test]
    fn fuses_all_modalities() {
        let mut fuser = PerceptionFuser::new();
        fuser.feed_voice("What's on my screen?", 0.9);
        fuser.feed_screen("com.whatsapp", "Hello from Ahmed");
        fuser.feed_notifications(3, vec!["New message".into()]);
        fuser.feed_device_state(85, "5g");
        fuser.feed_location(30.0, 31.0, Some("Cairo"));
        fuser.feed_time(14);

        let ctx = fuser.fuse();
        assert!(ctx.user_is_active);
        assert_eq!(ctx.active_app, Some("com.whatsapp".into()));
        assert_eq!(ctx.notification_count, 3);
        assert_eq!(ctx.battery_level, 85);
        assert_eq!(ctx.location, Some("Cairo".into()));
        assert_eq!(ctx.time_of_day, TimeOfDay::Afternoon);
        assert_eq!(ctx.fused_intent, "explain");
        assert_eq!(ctx.perceptions.len(), 6);
    }

    #[test]
    fn arabic_intent_detection() {
        assert_eq!(detect_intent("ابن لي تطبيق"), "create");
        assert_eq!(detect_intent("ابحث عن مطعم"), "search");
        assert_eq!(detect_intent("شرح كيف يعمل"), "explain");
        assert_eq!(detect_intent("مرحبا بك"), "greeting");
    }
}
