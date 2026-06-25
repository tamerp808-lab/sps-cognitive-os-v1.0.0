//! SPS Software Factory (Phase 11).
//!
//! End-to-end project generation workflow:
//! requirement analysis → architecture design → planning →
//! code generation → testing → validation → packaging → deploy prep.

#![allow(clippy::module_name_repetitions)]

pub mod workflow;
pub mod reducer;
pub mod supervisor;
pub mod llm;
pub mod provider_llm;

pub use workflow::{
    FactoryStage, FactoryWorkflow, ProjectRequest, RequirementSpec, ArchitecturePlan, RunResult,
};
pub use reducer::{FactoryReducer, FactoryState, FactoryRun, FactoryRunStatus};
pub use supervisor::{
    FactorySupervisor, SupervisorAction, SupervisorDecisionRecord, SupervisorPolicy,
};
pub use llm::{LlmFactoryAdapter, LlmFactoryConfig, MockLlmAdapter};
pub use provider_llm::ProviderLlmAdapter;
