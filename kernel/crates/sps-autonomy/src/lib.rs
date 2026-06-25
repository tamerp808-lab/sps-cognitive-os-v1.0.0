//! SPS Autonomy Layer (Phase 12).
//!
//! DISABLED BY DEFAULT. The owner must explicitly enable autonomy via
//! the `autonomy.enable` command (which emits an `autonomy.enabled`
//! event). Even when enabled, governance gates still apply to sensitive
//! effects.
//!
//! Provides:
//! - AutonomyGovernor — top-level controller.
//! - LongRunningGoalRunner — manages goals that span hours/days.
//! - AutonomySandbox — restricts autonomous effects to a workspace.

#![allow(clippy::module_name_repetitions)]

pub mod governor;
pub mod sandbox;
pub mod reducer;

pub use governor::{AutonomyGovernor, AutonomyConfig, AutonomyStatus, LongRunningGoalRunner};
pub use sandbox::{AutonomySandbox, SandboxBoundary, SandboxViolation};
pub use reducer::{AutonomyReducer, AutonomyState};
