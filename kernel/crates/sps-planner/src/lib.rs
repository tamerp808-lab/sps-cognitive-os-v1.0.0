//! SPS Planner (Phase 7).
//!
//! Generates plans from goals. A Plan is an ordered set of tasks with
//! dependencies, produced by a PlanTemplate + the goal description.

#![allow(clippy::module_name_repetitions)]

pub mod plan;
pub mod templates;
pub mod reducer;

pub use plan::{Plan, PlanId, PlanStatus, PlanStep};
pub use templates::{PlanTemplate, TemplateRegistry, builtin_templates};
pub use reducer::{PlannerReducer, PlannerState};
