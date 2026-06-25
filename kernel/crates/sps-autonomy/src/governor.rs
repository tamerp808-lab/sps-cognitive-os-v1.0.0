//! Autonomy governor + long-running goal runner.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_core::event::RawEvent;
use sps_core::sink::EventSink;
use sps_core::actor::Actor;
use sps_goals::GoalId;

/// Autonomy status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyStatus {
    /// Disabled (default).
    Disabled,
    /// Enabled and running.
    Enabled,
    /// Paused by owner.
    Paused,
}

impl Default for AutonomyStatus {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Autonomy configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyConfig {
    /// Current status.
    pub status: AutonomyStatus,
    /// Maximum number of concurrent autonomous goals.
    pub max_concurrent_goals: u32,
    /// Sandbox boundary (paths the autonomous runner may touch).
    pub sandbox_paths: Vec<String>,
    /// Maximum wall-time budget per autonomous run, in ms.
    pub max_run_time_ms: u64,
}

impl Default for AutonomyConfig {
    fn default() -> Self {
        Self {
            status: AutonomyStatus::Disabled,
            max_concurrent_goals: 1,
            sandbox_paths: vec![],
            max_run_time_ms: 3_600_000, // 1 hour
        }
    }
}

/// Autonomy governor — top-level controller.
pub struct AutonomyGovernor {
    config: parking_lot::RwLock<AutonomyConfig>,
}

impl Default for AutonomyGovernor {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomyGovernor {
    /// Create a new governor with default (disabled) config.
    pub fn new() -> Self {
        Self {
            config: parking_lot::RwLock::new(AutonomyConfig::default()),
        }
    }

    /// Get current config.
    pub fn config(&self) -> AutonomyConfig {
        self.config.read().clone()
    }

    /// Enable autonomy. Returns the previous status.
    pub fn enable(&self) -> AutonomyStatus {
        let mut cfg = self.config.write();
        let prev = cfg.status;
        cfg.status = AutonomyStatus::Enabled;
        prev
    }

    /// Pause autonomy.
    pub fn pause(&self) {
        self.config.write().status = AutonomyStatus::Paused;
    }

    /// Disable autonomy entirely.
    pub fn disable(&self) {
        self.config.write().status = AutonomyStatus::Disabled;
    }

    /// Is autonomy currently enabled?
    pub fn is_enabled(&self) -> bool {
        self.config.read().status == AutonomyStatus::Enabled
    }

    /// Update sandbox paths.
    pub fn set_sandbox_paths(&self, paths: Vec<String>) {
        self.config.write().sandbox_paths = paths;
    }

    /// Set the maximum number of concurrent autonomous goals.
    pub fn set_max_concurrent_goals(&self, max: u32) {
        self.config.write().max_concurrent_goals = max;
    }
}

/// Long-running goal runner — manages goals that span hours/days.
///
/// Fix #2 / E2: the runner is now a thin dispatcher. It performs
/// pre-dispatch validation (is_enabled, soft capacity) and then
/// dispatches `autonomous.goal_activated` / `autonomous.goal_deactivated`
/// events through the kernel's [`EventSink`]. The authoritative state
/// lives in [`crate::AutonomyState::active_goals`] (materialized by the
/// reducer); the runner's in-memory cache is best-effort only.
pub struct LongRunningGoalRunner {
    governor: std::sync::Arc<AutonomyGovernor>,
    active_goals: parking_lot::RwLock<Vec<GoalId>>,
}

impl LongRunningGoalRunner {
    /// Create a new runner.
    pub fn new(governor: std::sync::Arc<AutonomyGovernor>) -> Self {
        Self {
            governor,
            active_goals: parking_lot::RwLock::new(Vec::new()),
        }
    }

    /// Set the maximum number of concurrent goals. Delegates to the
    /// governor's config.
    pub fn set_max_concurrent(&self, max: u32) {
        self.governor.set_max_concurrent_goals(max);
    }

    /// Start a long-running goal. Returns Ok(()) if accepted, Err if
    /// autonomy is disabled or the max concurrent limit is reached.
    pub fn start(&self, goal_id: GoalId) -> Result<(), String> {
        if !self.governor.is_enabled() {
            return Err("autonomy is not enabled".into());
        }
        let cfg = self.governor.config();
        let mut active = self.active_goals.write();
        if active.len() as u32 >= cfg.max_concurrent_goals {
            return Err(format!(
                "max concurrent goals ({}) reached",
                cfg.max_concurrent_goals
            ));
        }
        if active.contains(&goal_id) {
            return Err(format!("goal {} is already active", goal_id.0));
        }
        active.push(goal_id);
        Ok(())
    }

    /// Stop a long-running goal.
    pub fn stop(&self, goal_id: GoalId) -> bool {
        let mut active = self.active_goals.write();
        let before = active.len();
        active.retain(|g| *g != goal_id);
        active.len() < before
    }

    /// List active goals.
    pub fn active(&self) -> Vec<GoalId> {
        self.active_goals.read().clone()
    }

    /// Activate a goal via the event-sourced path (Fix #2 / E2).
    ///
    /// Performs pre-dispatch validation (autonomy enabled, soft
    /// capacity), then dispatches `autonomous.goal_activated` through
    /// the kernel's [`EventSink`]. The reducer materializes the goal
    /// into `AutonomyState.active_goals` — that is the authoritative
    /// state, not this runner's in-memory cache.
    ///
    /// Idempotency: the reducer's BTreeMap::insert overwrites, so
    /// re-dispatching activation for an already-active goal is safe
    /// (latest-wins semantics).
    pub fn start_with_sink(
        &self,
        goal_id: GoalId,
        milestones: serde_json::Value,
        sink: &impl EventSink,
        wall_time: u64,
    ) -> Result<(), String> {
        if !self.governor.is_enabled() {
            return Err("autonomy is not enabled".into());
        }
        let cfg = self.governor.config();
        let mut active = self.active_goals.write();
        if !active.contains(&goal_id) {
            // Soft capacity only counts distinct goals.
            if active.len() as u32 >= cfg.max_concurrent_goals {
                return Err(format!(
                    "max concurrent goals ({}) reached",
                    cfg.max_concurrent_goals
                ));
            }
            active.push(goal_id);
        }
        drop(active);

        let payload = serde_json::json!({
            "goal_id": goal_id.0.to_string(),
            "milestones": milestones,
            "activated_at": wall_time,
        });
        let raw = RawEvent::new(
            "autonomous.goal_activated",
            payload,
            Actor::system("autonomy.runner"),
            wall_time,
        );
        sink.dispatch_trusted(raw)
            .map_err(|e| format!("dispatch failed: {}", e))?;
        Ok(())
    }

    /// Deactivate a goal via the event-sourced path (Fix #2 / E2).
    ///
    /// Dispatches `autonomous.goal_deactivated` through the kernel's
    /// [`EventSink`]. Idempotent — the reducer's `BTreeMap::remove` is
    /// a no-op if the goal wasn't active, so re-dispatching
    /// deactivation is safe (e.g. on HTTP retry from the Android
    /// companion).
    pub fn stop_with_sink(
        &self,
        goal_id: GoalId,
        sink: &impl EventSink,
        wall_time: u64,
    ) -> Result<(), String> {
        let mut active = self.active_goals.write();
        active.retain(|g| *g != goal_id);
        drop(active);

        let payload = serde_json::json!({
            "goal_id": goal_id.0.to_string(),
        });
        let raw = RawEvent::new(
            "autonomous.goal_deactivated",
            payload,
            Actor::system("autonomy.runner"),
            wall_time,
        );
        sink.dispatch_trusted(raw)
            .map_err(|e| format!("dispatch failed: {}", e))?;
        Ok(())
    }
}

/// Convenience name for a sandbox path.
pub type SandboxPath = SmolStr;
