//! Goal hierarchy types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Goal id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GoalId(pub Uuid);

impl GoalId {
    /// Generate a new id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for GoalId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GoalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Objective id.
pub type ObjectiveId = Uuid;
/// Milestone id.
pub type MilestoneId = Uuid;
/// Task id.
pub type TaskId = Uuid;

/// Goal status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// Just created.
    Pending,
    /// Being planned.
    Planning,
    /// Active — has tasks in progress.
    Active,
    /// Blocked.
    Blocked,
    /// Completed.
    Completed,
    /// Abandoned.
    Abandoned,
}

/// Task status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Pending start.
    Pending,
    /// In progress.
    InProgress,
    /// Completed.
    Completed,
    /// Failed.
    Failed,
    /// Blocked.
    Blocked,
}

/// A goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    /// Unique id.
    pub id: GoalId,
    /// Display title.
    pub title: SmolStr,
    /// Description.
    pub description: String,
    /// Priority (higher = more important).
    #[serde(default)]
    pub priority: i32,
    /// Status.
    pub status: GoalStatus,
    /// Objectives.
    #[serde(default)]
    pub objectives: Vec<Objective>,
    /// Dependencies (other goals that must complete first).
    #[serde(default)]
    pub dependencies: Vec<GoalId>,
    /// Wall time created (display only).
    pub created_at: u64,
    /// Originating tick.
    pub origin_tick: u64,
}

/// An objective within a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    /// Unique id.
    pub id: ObjectiveId,
    /// Title.
    pub title: SmolStr,
    /// Milestones.
    #[serde(default)]
    pub milestones: Vec<Milestone>,
}

/// A milestone within an objective.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Milestone {
    /// Unique id.
    pub id: MilestoneId,
    /// Title.
    pub title: SmolStr,
    /// Tasks.
    #[serde(default)]
    pub tasks: Vec<Task>,
}

/// A task — the atomic unit of work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Unique id.
    pub id: TaskId,
    /// Title.
    pub title: SmolStr,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Status.
    pub status: TaskStatus,
    /// Assigned agent archetype (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<SmolStr>,
    /// Originating tick.
    pub origin_tick: u64,
}

/// Verification result for a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Goal id.
    pub goal_id: GoalId,
    /// Whether the goal is verified as complete.
    pub verified: bool,
    /// Reason.
    pub reason: String,
    /// Tasks total.
    pub tasks_total: u32,
    /// Tasks completed.
    pub tasks_completed: u32,
}

/// The goal tree — a projection of all goals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GoalTree {
    /// All goals keyed by id.
    pub goals: BTreeMap<Uuid, Goal>,
}

impl GoalTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a goal.
    pub fn add_goal(&mut self, goal: Goal) {
        self.goals.insert(goal.id.0, goal);
    }

    /// Get a goal by id.
    pub fn get(&self, id: &GoalId) -> Option<&Goal> {
        self.goals.get(&id.0)
    }

    /// Get a mutable goal by id.
    pub fn get_mut(&mut self, id: &GoalId) -> Option<&mut Goal> {
        self.goals.get_mut(&id.0)
    }

    /// Remove a goal.
    pub fn remove(&mut self, id: &GoalId) -> Option<Goal> {
        self.goals.remove(&id.0)
    }

    /// Count all tasks across all goals.
    pub fn total_tasks(&self) -> u32 {
        self.goals
            .values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len() as u32)
            .sum()
    }

    /// Count completed tasks.
    pub fn completed_tasks(&self) -> u32 {
        self.goals
            .values()
            .flat_map(|g| &g.objectives)
            .flat_map(|o| &o.milestones)
            .flat_map(|m| &m.tasks)
            .filter(|t| t.status == TaskStatus::Completed)
            .count() as u32
    }

    /// Verify if a goal is complete (all tasks completed).
    pub fn verify(&self, goal_id: &GoalId) -> VerificationResult {
        let goal = match self.get(goal_id) {
            Some(g) => g,
            None => {
                return VerificationResult {
                    goal_id: *goal_id,
                    verified: false,
                    reason: "goal not found".into(),
                    tasks_total: 0,
                    tasks_completed: 0,
                }
            }
        };
        let total: u32 = goal
            .objectives
            .iter()
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len() as u32)
            .sum();
        let completed: u32 = goal
            .objectives
            .iter()
            .flat_map(|o| &o.milestones)
            .flat_map(|m| &m.tasks)
            .filter(|t| t.status == TaskStatus::Completed)
            .count() as u32;
        let verified = total > 0 && total == completed;
        VerificationResult {
            goal_id: *goal_id,
            verified,
            reason: if verified {
                "all tasks completed".into()
            } else {
                format!("{}/{} tasks completed", completed, total)
            },
            tasks_total: total,
            tasks_completed: completed,
        }
    }

    /// Sort goals by priority (descending).
    pub fn by_priority(&self) -> Vec<&Goal> {
        let mut goals: Vec<&Goal> = self.goals.values().collect();
        goals.sort_by(|a, b| b.priority.cmp(&a.priority));
        goals
    }

    /// Active goals.
    pub fn active(&self) -> Vec<&Goal> {
        self.goals
            .values()
            .filter(|g| g.status == GoalStatus::Active)
            .collect()
    }
}
