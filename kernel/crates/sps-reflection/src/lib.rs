//! SPS Reflection Layer (Phase 9).
//!
//! Post-execution analysis: success/failure analysis, pattern extraction,
//! knowledge consolidation.

#![allow(clippy::module_name_repetitions)]

pub mod analyzers;
pub mod reducer;

pub use analyzers::{
    FailureAnalyzer, FailureAnalysis, SuccessAnalyzer, SuccessAnalysis,
    PatternExtractor, Pattern, KnowledgeConsolidator,
};
pub use reducer::{ReflectionReducer, ReflectionState, Reflection};
