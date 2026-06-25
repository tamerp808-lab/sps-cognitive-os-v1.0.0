//! SPS Agent Execution Loop — ReAct (Reasoning + Acting) pattern.
//!
//! The ReAct loop:
//! 1. **Observe**: the agent receives a task + context
//! 2. **Reason**: the LLM thinks about what to do next (Thought)
//! 3. **Act**: the agent executes a tool or produces final answer (Action)
//! 4. **Observe**: the agent sees the result of the action (Observation)
//! 5. Repeat until done or max iterations reached
//!
//! This crate implements the loop generically — it works with any
//! `LlmProvider` and any set of tools registered in `sps-llm::ToolRegistry`.

#![allow(clippy::module_name_repetitions)]

pub mod loop_engine;
pub mod step;
pub mod prompt;

pub use loop_engine::{AgentLoop, AgentLoopConfig, LoopResult};
pub use step::{LoopStep, StepKind, StepStatus};
pub use prompt::build_react_prompt;
