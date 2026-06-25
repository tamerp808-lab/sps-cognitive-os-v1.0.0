//! SPS Phase 15A + 15B + 15C + 15D — Advanced Intelligence.
//!
//! Phase 15A: Cognitive Intelligence
//! - Predictive Planner 2.0: scores plans by predicted outcome
//! - Goal Forecasting: predicts goal success probability
//! - Scenario Simulation: simulates execution paths (Monte Carlo)
//! - Decision Scoring: multi-factor decision ranking
//! - Opportunity Detection: finds parallelizable goals
//! - Counterfactual Reasoning: "what if we had done X?"
//!
//! Phase 15B: Advanced Memory
//! - Memory consolidation (episodic → semantic → procedural → automatic)
//! - Forgetting policy (decay + importance-based retention)
//! - Importance scoring (multi-factor)
//! - Emotional memory (valence/arousal tagging)
//! - Knowledge graph expansion (auto-link related memories)
//!
//! Phase 15C: Self Modification under Governance
//! - Proposal → Simulation → Validation → Approval → Application → Verification
//! - Risk levels: None/Low/Medium/High/Critical
//! - Auto-approve for low risk, human approval for high risk
//! - Revert capability
//!
//! Phase 15D: Long-Term Autonomous Behavior
//! - Mission Manager: multi-goal missions with progress tracking
//! - Autonomous Scheduler: decides what to work on next
//! - Resource Budget: token/factory/time limits with daily reset
//! - Scheduled Reviews: Daily/Weekly/Monthly/Milestone/PostMortem

// Phase 15A: Cognitive Intelligence
pub mod predictive_planner;
pub mod forecaster;
pub mod simulator;
pub mod decision_scorer;
pub mod opportunity;
pub mod counterfactual;

// Phase 15B: Advanced Memory
pub mod consolidation;
pub mod forgetting;
pub mod importance;
pub mod emotional;
pub mod knowledge_graph;

// Phase 15C: Self Modification
pub mod self_modification;

// Phase 15D: Long-Term Autonomous Behavior
pub mod autonomous;

// The Single Mind — wires ALL modules into one cognitive pipeline
pub mod cognitive_loop;

// Decision-driven behavioral tests
pub mod decision_tests;

// Gap 1: Multi-modal Perception Fuser
pub mod perception_fuser;

// Gap 2: Complete Self-Modification Pipeline
pub mod self_mod_pipeline;

// Gap 3: Background Scheduler — Autonomous Long-term Operation
pub mod background_scheduler;

// Platform Adapters — all hardware interfaces
pub mod platform_adapters;

// Re-exports — 15A
pub use predictive_planner::PredictivePlanner;
pub use forecaster::GoalForecaster;
pub use simulator::{ScenarioSimulator, SimulationResult, MonteCarloResult};
pub use decision_scorer::DecisionScorer;
pub use opportunity::OpportunityDetector;
pub use counterfactual::CounterfactualEngine;

// Re-exports — 15B
pub use consolidation::MemoryConsolidator;
pub use forgetting::ForgettingPolicy;
pub use importance::ImportanceScorer;
pub use emotional::EmotionalMemory;
pub use knowledge_graph::KnowledgeGraphExpander;

// Re-exports — 15C
pub use self_modification::{SelfModificationGovernor, SelfModificationProposal, ModificationKind, RiskLevel};

// Re-exports — 15D
pub use autonomous::{MissionManager, AutonomousScheduler, ResourceBudget, Mission, MissionState};

// Re-exports — CognitiveLoop (the Single Mind)
pub use cognitive_loop::{CognitiveLoop, CognitiveCycle, CognitiveInput, CognitiveOutput, CognitiveStep};
