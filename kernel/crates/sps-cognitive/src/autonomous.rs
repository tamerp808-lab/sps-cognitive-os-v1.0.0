//! SPS Phase 15D — Long-Term Autonomous Behavior.
//!
//! Enables SPS to manage long-running missions that span hours, days,
//! or weeks. Includes:
//! - Mission Manager: tracks multi-goal missions
//! - Background Planning: plans during idle time
//! - Daily/Weekly Review: scheduled self-assessment
//! - Resource Optimization: budget management
//! - Autonomous Scheduling: decides what to work on next

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A mission — a collection of related goals executed over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub goal_ids: Vec<Uuid>,
    pub state: MissionState,
    pub created_at_ms: u64,
    pub deadline_ms: Option<u64>,
    pub priority: u32,
    pub progress: f64, // 0.0 to 1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionState {
    /// Mission is planned but not started.
    Planned,
    /// Mission is active — goals are being executed.
    Active,
    /// Mission is paused (e.g., waiting for external input).
    Paused,
    /// Mission completed successfully.
    Completed,
    /// Mission failed — some goals could not be completed.
    Failed,
    /// Mission cancelled by the owner.
    Cancelled,
}

/// A scheduled review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledReview {
    pub id: Uuid,
    pub kind: ReviewKind,
    pub mission_id: Option<Uuid>,
    pub scheduled_for_ms: u64,
    pub completed: bool,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewKind {
    Daily,
    Weekly,
    Monthly,
    Milestone,
    PostMortem,
}

/// Resource budget for autonomous operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// Maximum LLM tokens per day.
    pub max_llm_tokens_per_day: u64,
    /// Maximum factory runs per day.
    pub max_factory_runs_per_day: u32,
    /// Maximum wall-time per day (ms).
    pub max_active_time_per_day_ms: u64,
    /// Current usage (reset daily).
    pub current_llm_tokens: u64,
    pub current_factory_runs: u32,
    pub current_active_time_ms: u64,
}

impl ResourceBudget {
    /// Check if we can afford an LLM call.
    pub fn can_call_llm(&self, estimated_tokens: u64) -> bool {
        self.current_llm_tokens + estimated_tokens <= self.max_llm_tokens_per_day
    }

    /// Check if we can run a factory.
    pub fn can_run_factory(&self) -> bool {
        self.current_factory_runs < self.max_factory_runs_per_day
    }

    /// Record LLM usage.
    pub fn record_llm(&mut self, tokens: u64) {
        self.current_llm_tokens += tokens;
    }

    /// Record factory run.
    pub fn record_factory(&mut self) {
        self.current_factory_runs += 1;
    }

    /// Reset daily counters.
    pub fn reset_daily(&mut self) {
        self.current_llm_tokens = 0;
        self.current_factory_runs = 0;
        self.current_active_time_ms = 0;
    }

    /// Usage as percentages.
    pub fn llm_usage_pct(&self) -> f64 {
        if self.max_llm_tokens_per_day == 0 { return 0.0; }
        self.current_llm_tokens as f64 / self.max_llm_tokens_per_day as f64
    }

    pub fn factory_usage_pct(&self) -> f64 {
        if self.max_factory_runs_per_day == 0 { return 0.0; }
        self.current_factory_runs as f64 / self.max_factory_runs_per_day as f64
    }
}

/// A scheduling decision — what to work on next.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingDecision {
    pub mission_id: Option<Uuid>,
    pub goal_id: Option<Uuid>,
    pub action: SchedulingAction,
    pub reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingAction {
    /// Work on this goal now.
    ExecuteGoal,
    /// Plan the next steps for this mission.
    PlanMission,
    /// Run a review.
    RunReview,
    /// Wait — nothing to do right now.
    Idle,
    /// Resource budget exhausted — wait for reset.
    WaitForBudget,
}

/// The Autonomous Scheduler.
pub struct AutonomousScheduler {
    /// Minimum time between reviews (ms).
    pub min_review_interval_ms: u64,
    /// Prefer higher-priority missions.
    pub prefer_higher_priority: bool,
}

impl Default for AutonomousScheduler {
    fn default() -> Self {
        Self {
            min_review_interval_ms: 3_600_000, // 1 hour
            prefer_higher_priority: true,
        }
    }
}

impl AutonomousScheduler {
    /// Decide what to work on next.
    pub fn decide(
        &self,
        missions: &[Mission],
        budget: &ResourceBudget,
        pending_reviews: &[ScheduledReview],
    ) -> SchedulingDecision {
        // 1. Check if budget is exhausted.
        if budget.llm_usage_pct() > 0.95 || budget.factory_usage_pct() > 0.95 {
            return SchedulingDecision {
                mission_id: None,
                goal_id: None,
                action: SchedulingAction::WaitForBudget,
                reasoning: "Resource budget nearly exhausted. Waiting for daily reset.".into(),
            };
        }

        // 2. Check for pending reviews.
        if let Some(review) = pending_reviews.iter().find(|r| !r.completed) {
            return SchedulingDecision {
                mission_id: review.mission_id,
                goal_id: None,
                action: SchedulingAction::RunReview,
                reasoning: format!("Pending {:?} review", review.kind),
            };
        }

        // 3. Find active missions.
        let active: Vec<_> = missions
            .iter()
            .filter(|m| m.state == MissionState::Active)
            .collect();

        if active.is_empty() {
            return SchedulingDecision {
                mission_id: None,
                goal_id: None,
                action: SchedulingAction::Idle,
                reasoning: "No active missions. Waiting for new goals.".into(),
            };
        }

        // 4. Pick the highest-priority mission with uncompleted goals.
        let mut sorted = active.clone();
        if self.prefer_higher_priority {
            sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        let mission = &sorted[0];

        // 5. If mission has goals, execute the next one.
        if !mission.goal_ids.is_empty() {
            SchedulingDecision {
                mission_id: Some(mission.id),
                goal_id: Some(mission.goal_ids[0]),
                action: SchedulingAction::ExecuteGoal,
                reasoning: format!(
                    "Executing mission '{}' (priority={}, progress={:.0}%)",
                    mission.name, mission.priority, mission.progress * 100.0
                ),
            }
        } else {
            SchedulingDecision {
                mission_id: Some(mission.id),
                goal_id: None,
                action: SchedulingAction::PlanMission,
                reasoning: format!("Mission '{}' needs planning — no goals assigned", mission.name),
            }
        }
    }
}

/// The Mission Manager — tracks multi-goal missions.
pub struct MissionManager {
    pub missions: Vec<Mission>,
}

impl Default for MissionManager {
    fn default() -> Self {
        Self { missions: Vec::new() }
    }
}

impl MissionManager {
    /// Create a new mission.
    pub fn create(&mut self, name: String, description: String, priority: u32) -> Uuid {
        let id = Uuid::now_v7();
        self.missions.push(Mission {
            id,
            name,
            description,
            goal_ids: Vec::new(),
            state: MissionState::Planned,
            created_at_ms: 0,
            deadline_ms: None,
            priority,
            progress: 0.0,
        });
        id
    }

    /// Start a mission.
    pub fn start(&mut self, mission_id: Uuid) -> Result<(), String> {
        let m = self.missions.iter_mut()
            .find(|m| m.id == mission_id)
            .ok_or("mission not found")?;
        m.state = MissionState::Active;
        Ok(())
    }

    /// Add a goal to a mission.
    pub fn add_goal(&mut self, mission_id: Uuid, goal_id: Uuid) -> Result<(), String> {
        let m = self.missions.iter_mut()
            .find(|m| m.id == mission_id)
            .ok_or("mission not found")?;
        m.goal_ids.push(goal_id);
        Ok(())
    }

    /// Update mission progress.
    pub fn update_progress(&mut self, mission_id: Uuid, progress: f64) -> Result<(), String> {
        let m = self.missions.iter_mut()
            .find(|m| m.id == mission_id)
            .ok_or("mission not found")?;
        m.progress = progress.clamp(0.0, 1.0);
        if m.progress >= 1.0 {
            m.state = MissionState::Completed;
        }
        Ok(())
    }

    /// Get all active missions.
    pub fn active_missions(&self) -> Vec<&Mission> {
        self.missions.iter().filter(|m| m.state == MissionState::Active).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_lifecycle() {
        let mut mgr = MissionManager::default();
        let id = mgr.create("Build API".into(), "REST API project".into(), 5);
        assert_eq!(mgr.missions[0].state, MissionState::Planned);

        mgr.start(id).unwrap();
        assert_eq!(mgr.missions[0].state, MissionState::Active);

        let goal = Uuid::nil();
        mgr.add_goal(id, goal).unwrap();
        assert_eq!(mgr.missions[0].goal_ids.len(), 1);

        mgr.update_progress(id, 1.0).unwrap();
        assert_eq!(mgr.missions[0].state, MissionState::Completed);
    }

    #[test]
    fn scheduler_picks_active_mission() {
        let scheduler = AutonomousScheduler::default();
        let budget = ResourceBudget {
            max_llm_tokens_per_day: 1_000_000,
            max_factory_runs_per_day: 10,
            ..Default::default()
        };
        let missions = vec![Mission {
            id: Uuid::nil(),
            name: "test".into(),
            description: "".into(),
            goal_ids: vec![Uuid::nil()],
            state: MissionState::Active,
            created_at_ms: 0,
            deadline_ms: None,
            priority: 5,
            progress: 0.3,
        }];
        let decision = scheduler.decide(&missions, &budget, &[]);
        assert_eq!(decision.action, SchedulingAction::ExecuteGoal);
    }

    #[test]
    fn scheduler_waits_when_budget_exhausted() {
        let scheduler = AutonomousScheduler::default();
        let budget = ResourceBudget {
            max_llm_tokens_per_day: 100,
            current_llm_tokens: 99,
            max_factory_runs_per_day: 10,
            ..Default::default()
        };
        let decision = scheduler.decide(&[], &budget, &[]);
        assert_eq!(decision.action, SchedulingAction::WaitForBudget);
    }

    #[test]
    fn scheduler_runs_pending_review() {
        let scheduler = AutonomousScheduler::default();
        let budget = ResourceBudget::default();
        let reviews = vec![ScheduledReview {
            id: Uuid::nil(),
            kind: ReviewKind::Daily,
            mission_id: None,
            scheduled_for_ms: 0,
            completed: false,
            findings: vec![],
        }];
        let decision = scheduler.decide(&[], &budget, &reviews);
        assert_eq!(decision.action, SchedulingAction::RunReview);
    }

    #[test]
    fn scheduler_idles_with_no_missions() {
        let scheduler = AutonomousScheduler::default();
        let budget = ResourceBudget::default();
        let decision = scheduler.decide(&[], &budget, &[]);
        assert_eq!(decision.action, SchedulingAction::Idle);
    }

    #[test]
    fn budget_tracking() {
        let mut budget = ResourceBudget {
            max_llm_tokens_per_day: 1000,
            max_factory_runs_per_day: 5,
            ..Default::default()
        };
        assert!(budget.can_call_llm(500));
        budget.record_llm(500);
        assert_eq!(budget.llm_usage_pct(), 0.5);
        assert!(budget.can_call_llm(500));
        assert!(!budget.can_call_llm(600));
        budget.reset_daily();
        assert_eq!(budget.current_llm_tokens, 0);
    }
}
