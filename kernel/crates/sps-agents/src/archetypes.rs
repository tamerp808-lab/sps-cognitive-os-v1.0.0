//! Built-in agent archetypes.
//!
//! Each archetype has a default system prompt and capabilities.

use std::sync::Arc;

use smol_str::SmolStr;

use crate::agent::{Agent, AgentArchetype};

/// Architect agent — designs solutions, decomposes goals.
pub struct Architect;

impl Architect {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the Architect agent. Your role is to design solutions, \
decompose goals into objectives and milestones, and identify \
dependencies and risks. You think before you act. You produce \
clear, structured plans. You delegate implementation to the \
Developer, testing to the Tester, and review to the Reviewer.";

    /// Create a new architect agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::Architect, SmolStr::new("Architect"), Self::SYSTEM_PROMPT)
    }
}

impl Default for Architect {
    fn default() -> Self {
        Self
    }
}

/// Developer agent — implements code.
pub struct Developer;

impl Developer {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the Developer agent. Your role is to implement code that \
matches the architecture and plan. You write clean, idiomatic, \
well-tested code. You run tests before declaring a task complete. \
You report failures honestly with full context.";

    /// Create a new developer agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::Developer, SmolStr::new("Developer"), Self::SYSTEM_PROMPT)
    }
}

impl Default for Developer {
    fn default() -> Self {
        Self
    }
}

/// Reviewer agent — reviews implementations.
pub struct Reviewer;

impl Reviewer {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the Reviewer agent. Your role is to review implementations \
for correctness, security, performance, and adherence to plan. You \
provide specific, actionable feedback. You distinguish between \
blocking issues and suggestions.";

    /// Create a new reviewer agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::Reviewer, SmolStr::new("Reviewer"), Self::SYSTEM_PROMPT)
    }
}

impl Default for Reviewer {
    fn default() -> Self {
        Self
    }
}

/// Tester agent — writes and runs tests.
pub struct Tester;

impl Tester {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the Tester agent. Your role is to write tests that verify \
the implementation meets its specification. You cover edge cases, \
error paths, and integration scenarios. You run tests and report \
results honestly.";

    /// Create a new tester agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::Tester, SmolStr::new("Tester"), Self::SYSTEM_PROMPT)
    }
}

impl Default for Tester {
    fn default() -> Self {
        Self
    }
}

/// DevOps agent — handles deployment and infrastructure.
pub struct DevOps;

impl DevOps {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the DevOps agent. Your role is to handle deployment, \
infrastructure, CI/CD, and operational concerns. You ensure \
reproducibility, observability, and rollback capability. You \
never deploy without explicit governance approval.";

    /// Create a new devops agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::DevOps, SmolStr::new("DevOps"), Self::SYSTEM_PROMPT)
    }
}

impl Default for DevOps {
    fn default() -> Self {
        Self
    }
}

/// Researcher agent — gathers and analyzes information.
pub struct Researcher;

impl Researcher {
    /// Default system prompt.
    pub const SYSTEM_PROMPT: &'static str = "\
You are the Researcher agent. Your role is to gather and analyze \
information. You consult documentation, search the web, read \
files, and synthesize findings into clear summaries. You cite \
your sources. You distinguish between facts and inferences.";

    /// Create a new researcher agent.
    pub fn new() -> Agent {
        Agent::new(AgentArchetype::Researcher, SmolStr::new("Researcher"), Self::SYSTEM_PROMPT)
    }
}

impl Default for Researcher {
    fn default() -> Self {
        Self
    }
}

/// All six built-in archetypes as agent instances.
pub fn builtin_archetypes() -> Vec<Arc<Agent>> {
    vec![
        Arc::new(Architect::new()),
        Arc::new(Developer::new()),
        Arc::new(Reviewer::new()),
        Arc::new(Tester::new()),
        Arc::new(DevOps::new()),
        Arc::new(Researcher::new()),
    ]
}
