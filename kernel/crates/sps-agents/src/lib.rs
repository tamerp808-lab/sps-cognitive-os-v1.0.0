//! SPS Agent Runtime.
//!
//! Six built-in agent archetypes:
//! - Architect — designs solutions, decomposes goals
//! - Developer — implements code
//! - Reviewer — reviews implementations
//! - Tester — writes and runs tests
//! - DevOps — handles deployment, infrastructure
//! - Researcher — gathers and analyzes information
//!
//! Each agent is a stateless specialization with a system prompt and
//! a set of capabilities. The runtime orchestrates them: dispatching
//! tasks, handling inter-agent messages, and recording all activity
//! as events.

#![allow(clippy::module_name_repetitions)]

pub mod agent;
pub mod archetypes;
pub mod runtime;
pub mod messages;
pub mod reducer;

pub use agent::{Agent, AgentId, AgentArchetype, AgentCapabilities, AgentContext};
pub use archetypes::{Architect, Developer, Reviewer, Tester, DevOps, Researcher, builtin_archetypes};
pub use runtime::{AgentRuntime, AgentRuntimeConfig, DispatchResult};
pub use messages::{AgentMessage, MessageKind};
pub use reducer::{AgentReducer, AgentState, AgentRecord, AgentStatus};
