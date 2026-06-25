//! SPS Execution Layer (Phase 8).
//!
//! Advanced execution: code analysis, project generation, tool chains.
//! Built on top of Phase 1's Effect System.

#![allow(clippy::module_name_repetitions)]

pub mod analysis;
pub mod generation;
pub mod reducer;

pub use analysis::{CodeAnalyzer, CodeAnalysis, FileAnalysis};
pub use generation::{ProjectGenerator, ProjectSpec, GeneratedFile};
pub use reducer::{ExecutionReducer, ExecutionState, ExecutionRecord, ExecutionOutcome};
