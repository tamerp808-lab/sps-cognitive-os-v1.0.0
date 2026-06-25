//! SPS Goal System (Phase 6).
//!
//! Hierarchy: Goal → Objective → Milestone → Task.
//! Goals are tracked, prioritized, dependency-linked, and verified.

#![allow(clippy::module_name_repetitions)]

pub mod hierarchy;
pub mod reducer;

pub use hierarchy::{
    Goal, GoalId, GoalStatus, Objective, ObjectiveId, Milestone, MilestoneId, Task, TaskId,
    TaskStatus, VerificationResult, GoalTree,
};
pub use reducer::{GoalReducer, GoalState};
