//! Autonomy reducer + state slice.
//!
//! Fix #2 / E1: Unified goal lifecycle under `autonomous.*` namespace.
//! Removed legacy `autonomy.long_goal_started` / `autonomy.long_goal_stopped`
//! (they were registered but never produced by any caller).
//!
//! Fix #2 / E2: `autonomous.goal_activated` and `autonomous.goal_deactivated`
//! are the sole event types for goal lifecycle. LongRunningGoalRunner
//! dispatches them via EventSink. AutonomyState.active_goals is the
//! single source of truth (replaces the runner's in-memory Vec<GoalId>).
//!
//! Fix #13 (preserved): `autonomous.weekly_review` materializes weekly
//! review records for later querying.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sps_core::event::Event;
use sps_core::reducer::{Reducer, ReducerRegistry};
use sps_core::state::CanonicalState;
use sps_core::CoreResult;
use uuid::Uuid;

use crate::governor::{AutonomyConfig, AutonomyStatus};

/// Extension key.
pub const EXTENSION_KEY: &str = "autonomy";

/// A record of a goal being activated for autonomous pursuit (Fix #13).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GoalActivation {
    pub goal_id: Uuid,
    #[serde(default)]
    pub milestones: serde_json::Value,
    pub activated_at: u64,
    pub origin_tick: u64,
}

/// A weekly review record (Fix #13).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WeeklyReview {
    pub goal_id: Uuid,
    pub review: String,
    pub reviewed_at: u64,
    pub origin_tick: u64,
}

/// Autonomy state slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyState {
    pub config: AutonomyConfig,
    /// Goals currently being autonomously pursued (Fix #13, Fix #2 E2).
    #[serde(default)]
    pub active_goals: std::collections::BTreeMap<Uuid, GoalActivation>,
    /// Weekly reviews (Fix #13).
    #[serde(default)]
    pub reviews: Vec<WeeklyReview>,
}

impl Default for AutonomyState {
    fn default() -> Self {
        Self {
            config: AutonomyConfig::default(),
            active_goals: std::collections::BTreeMap::new(),
            reviews: Vec::new(),
        }
    }
}

impl AutonomyState {
    /// Read from canonical state. P3D: typed-first lookup with JSON
    /// fallback. The typed path is populated by snapshot load (via
    /// `rebuild_typed_from_json`); the JSON path is populated by the
    /// reducer's `save_to` on each dispatch.
    pub fn from_state(state: &CanonicalState) -> Option<Self> {
        if let Some(arc) = state.get_typed_extension::<AutonomyState>(EXTENSION_KEY) {
            return Some((*arc).clone());
        }
        state.get_extension(EXTENSION_KEY)
    }

    /// Read the typed extension directly (no clone of the inner value
    /// beyond the `Arc`). Returns `None` if the typed slot is absent
    /// (e.g. before any snapshot load on a fresh dispatch path).
    pub fn from_typed_state(state: &CanonicalState) -> Option<std::sync::Arc<Self>> {
        state.get_typed_extension::<Self>(EXTENSION_KEY)
    }

    /// Save to canonical state.
    pub fn save_to(&self, state: &mut CanonicalState) -> serde_json::Result<()> {
        state.set_extension(EXTENSION_KEY, self)
    }

    /// Query all reviews for a given goal (Fix #13).
    pub fn reviews_for_goal(&self, goal_id: Uuid) -> Vec<&WeeklyReview> {
        self.reviews.iter().filter(|r| r.goal_id == goal_id).collect()
    }
}

/// Reducer for autonomy events.
#[derive(Debug, Default)]
pub struct AutonomyReducer;

impl AutonomyReducer {
    /// Register this reducer.
    pub fn register(registry: &mut ReducerRegistry) {
        let r: Arc<Self> = Arc::new(Self);
        for et in &[
            "autonomy.enabled",
            "autonomy.disabled",
            "autonomy.paused",
            "autonomy.config_updated",
            // Fix #2 / E1: unified autonomous.* namespace is the sole
            // source of truth for goal lifecycle. Removed legacy
            // `autonomy.long_goal_started` / `autonomy.long_goal_stopped`
            // — they were registered but never produced by any caller,
            // and their semantics overlapped with autonomous.goal_activated.
            "autonomous.goal_activated",
            "autonomous.goal_deactivated",
            "autonomous.weekly_review",
        ] {
            registry.register(*et, r.clone());
        }
    }

    /// P3D: Register typed-extension constructor for snapshot load.
    ///
    /// When the kernel boots from a snapshot, the typed registry walks
    /// the JSON `extensions` map and reconstructs `Arc<AutonomyState>`
    /// via this constructor. Subsequent `from_state` calls hit the
    /// typed path (no JSON round-trip).
    pub fn register_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
        reg.register::<AutonomyState>(EXTENSION_KEY);
    }
}

impl Reducer for AutonomyReducer {
    fn name(&self) -> &'static str {
        "autonomy"
    }

    fn reduce(&self, state: &mut CanonicalState, event: &Event) -> CoreResult<()> {
        // P3D: Use typed extension as the source of truth. If the typed
        // slot is empty (e.g. fresh state with no snapshot), it defaults
        // to AutonomyState::default(). The mutation goes directly into
        // the typed Arc; JSON is synced only at snapshot time.
        state.with_typed_extension(EXTENSION_KEY, |as_: &mut AutonomyState| {
            match event.event_type.as_str() {
                "autonomy.enabled" => {
                    as_.config.status = AutonomyStatus::Enabled;
                }
                "autonomy.disabled" => {
                    as_.config.status = AutonomyStatus::Disabled;
                }
                "autonomy.paused" => {
                    as_.config.status = AutonomyStatus::Paused;
                }
                "autonomy.config_updated" => {
                    if let Ok(new_config) = serde_json::from_value::<AutonomyConfig>(event.payload.clone()) {
                        as_.config = new_config;
                    }
                }
                // Fix #13: materialize autonomous.goal_activated.
                // Fix #2 / E4: idempotent — BTreeMap::insert overwrites, so
                // duplicate activations (e.g. from HTTP retry) produce 1 entry.
                "autonomous.goal_activated" => {
                    let goal_id_str = event.payload.get("goal_id").and_then(|v| v.as_str()).unwrap_or("");
                    let goal_id = Uuid::parse_str(goal_id_str).unwrap_or_default();
                    let milestones = event.payload.get("milestones").cloned().unwrap_or(serde_json::Value::Null);
                    let activated_at = event.payload.get("activated_at").and_then(|v| v.as_u64()).unwrap_or(0);
                    as_.active_goals.insert(goal_id, GoalActivation {
                        goal_id, milestones, activated_at, origin_tick: event.tick,
                    });
                }
                // Fix #2 / E1: materialize autonomous.goal_deactivated.
                // Idempotent — remove() is a no-op if goal wasn't active.
                "autonomous.goal_deactivated" => {
                    let goal_id_str = event.payload.get("goal_id").and_then(|v| v.as_str()).unwrap_or("");
                    let goal_id = Uuid::parse_str(goal_id_str).unwrap_or_default();
                    as_.active_goals.remove(&goal_id);
                }
                // Fix #13: materialize autonomous.weekly_review.
                "autonomous.weekly_review" => {
                    let goal_id_str = event.payload.get("goal_id").and_then(|v| v.as_str()).unwrap_or("");
                    let goal_id = Uuid::parse_str(goal_id_str).unwrap_or_default();
                    let review = event.payload.get("review").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let reviewed_at = event.payload.get("reviewed_at").and_then(|v| v.as_u64()).unwrap_or(0);
                    as_.reviews.push(WeeklyReview {
                        goal_id, review, reviewed_at, origin_tick: event.tick,
                    });
                }
                _ => {}
            }
        });

        // P3D: No per-dispatch JSON sync. Snapshot::take does a one-shot
        // sync_typed_to_json() at snapshot time.
        Ok(())
    }
}
