//! Plan types.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Plan id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlanId(pub Uuid);

impl PlanId {
    /// Generate a new id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Draft.
    Draft,
    /// Approved and ready to execute.
    Approved,
    /// Being executed.
    Executing,
    /// Completed.
    Completed,
    /// Abandoned.
    Abandoned,
    /// Optimized (post-optimization).
    Optimized,
}

/// A step in a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step id.
    pub id: Uuid,
    /// Title.
    pub title: SmolStr,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Step index in the plan.
    pub index: u32,
    /// Dependencies (indices of steps that must complete first).
    #[serde(default)]
    pub depends_on: Vec<u32>,
    /// Assigned agent archetype (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<SmolStr>,
    /// Whether the step is parallelizable.
    #[serde(default)]
    pub parallelizable: bool,
}

/// A plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Unique id.
    pub id: PlanId,
    /// Goal this plan is for.
    pub goal_id: sps_goals::GoalId,
    /// Template name used to generate this plan.
    pub template: SmolStr,
    /// Steps.
    pub steps: Vec<PlanStep>,
    /// Status.
    pub status: PlanStatus,
    /// Wall time created.
    pub created_at: u64,
    /// Originating tick.
    pub origin_tick: u64,
}

impl Plan {
    /// Create a new empty plan.
    pub fn new(goal_id: sps_goals::GoalId, template: impl Into<SmolStr>) -> Self {
        Self {
            id: PlanId::new(),
            goal_id,
            template: template.into(),
            steps: Vec::new(),
            status: PlanStatus::Draft,
            created_at: 0,
            origin_tick: 0,
        }
    }

    /// Add a step.
    pub fn add_step(&mut self, step: PlanStep) {
        self.steps.push(step);
    }

    /// Mark this plan as approved.
    pub fn approve(&mut self) {
        self.status = PlanStatus::Approved;
    }

    /// Mark as executing.
    pub fn start(&mut self) {
        self.status = PlanStatus::Executing;
    }

    /// Mark as completed.
    pub fn complete(&mut self) {
        self.status = PlanStatus::Completed;
    }

    /// Count steps.
    pub fn step_count(&self) -> u32 {
        self.steps.len() as u32
    }
}
