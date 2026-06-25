//! SPS Reasoning Engine (Phase 5).
//!
//! Composable analyzers that operate on the world model and goal tree:
//! - GoalAnalyzer
//! - TaskDecomposer
//! - DependencySolver
//! - ConflictDetector
//! - RiskAnalyzer
//! - PlanOptimizer
//!
//! All reasoning is observable: every step emits a `reasoning.*` event.

#![allow(clippy::module_name_repetitions)]

pub mod analyzers;
pub mod reducer;

pub use analyzers::{
    ConflictDetector, ConflictReport, DependencySolver, GoalAnalyzer, GoalAnalysis,
    PlanOptimizer, RiskAnalyzer, RiskAssessment, TaskDecomposer, TaskDecomposition,
};
pub use reducer::{
    Alternative, Conflict, Degradation, ReasoningReducer, ReasoningState, ReasoningStep,
    ReasoningTrace, Risk,
};
