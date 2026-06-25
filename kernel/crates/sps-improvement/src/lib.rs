//! SPS Self-Improvement Layer (Phase 10 + Phase 12C).
//!
//! Phase 10: Governance-gated self-modification. The system PROPOSES
//! improvements but does NOT apply them without owner approval.
//!
//! Phase 12C: Self-Improvement Loop — connects Reflection → Improvement →
//! FactorySupervisor so the system learns from factory run outcomes and
//! proposes policy adjustments automatically.

#![allow(clippy::module_name_repetitions)]

pub mod analyzers;
pub mod reducer;
pub mod loop_engine;

pub use analyzers::{
    BottleneckDetector, Bottleneck, PerformanceAnalyzer, PerformanceReport,
    WorkflowOptimizer, WorkflowProposal, PromptOptimizer, PromptProposal,
    OptimizationKind,
};
pub use reducer::{ImprovementReducer, ImprovementState, ImprovementProposal, ImprovementStatus};
pub use loop_engine::{SelfImprovementLoop, ImprovementPattern, apply_improvement_to_policy};
