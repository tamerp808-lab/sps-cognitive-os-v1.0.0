//! Agent core types.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Agent id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub Uuid);

impl AgentId {
    /// Generate a new id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Agent archetype — one of the six built-in specializations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentArchetype {
    /// Designs solutions, decomposes goals.
    Architect,
    /// Implements code.
    Developer,
    /// Reviews implementations.
    Reviewer,
    /// Writes and runs tests.
    Tester,
    /// Handles deployment, infrastructure.
    DevOps,
    /// Gathers and analyzes information.
    Researcher,
}

impl AgentArchetype {
    /// String identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Architect => "architect",
            Self::Developer => "developer",
            Self::Reviewer => "reviewer",
            Self::Tester => "tester",
            Self::DevOps => "devops",
            Self::Researcher => "researcher",
        }
    }

    /// All archetypes.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Architect,
            Self::Developer,
            Self::Reviewer,
            Self::Tester,
            Self::DevOps,
            Self::Researcher,
        ]
    }
}

impl std::str::FromStr for AgentArchetype {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "architect" => Ok(Self::Architect),
            "developer" => Ok(Self::Developer),
            "reviewer" => Ok(Self::Reviewer),
            "tester" => Ok(Self::Tester),
            "devops" => Ok(Self::DevOps),
            "researcher" => Ok(Self::Researcher),
            other => Err(format!("unknown archetype: {}", other)),
        }
    }
}

impl std::fmt::Display for AgentArchetype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Agent capabilities — what an agent can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Whether the agent can read files.
    pub can_read_files: bool,
    /// Whether the agent can write files.
    pub can_write_files: bool,
    /// Whether the agent can execute shell commands.
    pub can_exec_shell: bool,
    /// Whether the agent can call LLMs.
    pub can_call_llm: bool,
    /// Whether the agent can delegate to other agents.
    pub can_delegate: bool,
    /// Whether the agent can create goals/tasks.
    pub can_create_goals: bool,
}

impl Default for AgentCapabilities {
    fn default() -> Self {
        Self {
            can_read_files: true,
            can_write_files: false,
            can_exec_shell: false,
            can_call_llm: true,
            can_delegate: false,
            can_create_goals: false,
        }
    }
}

/// An agent descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    /// Unique id.
    pub id: AgentId,
    /// Archetype.
    pub archetype: AgentArchetype,
    /// Display name.
    pub name: SmolStr,
    /// System prompt — defines the agent's persona and instructions.
    pub system_prompt: String,
    /// Capabilities.
    pub capabilities: AgentCapabilities,
    /// Originating tick.
    pub origin_tick: u64,
}

impl Default for Agent {
    fn default() -> Self {
        Self {
            id: AgentId::default(),
            archetype: AgentArchetype::Architect,
            name: SmolStr::new("default"),
            system_prompt: String::new(),
            capabilities: AgentCapabilities::default(),
            origin_tick: 0,
        }
    }
}

impl Agent {
    /// Create a new agent with default capabilities for its archetype.
    pub fn new(archetype: AgentArchetype, name: impl Into<SmolStr>, system_prompt: impl Into<String>) -> Self {
        let capabilities = default_capabilities(archetype);
        Self {
            id: AgentId::new(),
            archetype,
            name: name.into(),
            system_prompt: system_prompt.into(),
            capabilities,
            origin_tick: 0,
        }
    }
}

/// Default capabilities for an archetype.
pub fn default_capabilities(archetype: AgentArchetype) -> AgentCapabilities {
    match archetype {
        AgentArchetype::Architect => AgentCapabilities {
            can_read_files: true,
            can_write_files: false,
            can_exec_shell: false,
            can_call_llm: true,
            can_delegate: true,
            can_create_goals: true,
        },
        AgentArchetype::Developer => AgentCapabilities {
            can_read_files: true,
            can_write_files: true,
            can_exec_shell: true,
            can_call_llm: true,
            can_delegate: false,
            can_create_goals: false,
        },
        AgentArchetype::Reviewer => AgentCapabilities {
            can_read_files: true,
            can_write_files: false,
            can_exec_shell: false,
            can_call_llm: true,
            can_delegate: false,
            can_create_goals: false,
        },
        AgentArchetype::Tester => AgentCapabilities {
            can_read_files: true,
            can_write_files: true,
            can_exec_shell: true,
            can_call_llm: true,
            can_delegate: false,
            can_create_goals: false,
        },
        AgentArchetype::DevOps => AgentCapabilities {
            can_read_files: true,
            can_write_files: true,
            can_exec_shell: true,
            can_call_llm: true,
            can_delegate: false,
            can_create_goals: false,
        },
        AgentArchetype::Researcher => AgentCapabilities {
            can_read_files: true,
            can_write_files: false,
            can_exec_shell: false,
            can_call_llm: true,
            can_delegate: true,
            can_create_goals: false,
        },
    }
}

/// Context passed to an agent when handling a message or task.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// The agent's own id.
    pub agent_id: AgentId,
    /// Wall time (display only).
    pub wall_time: u64,
    /// Originating tick.
    pub origin_tick: u64,
    /// Optional task id (if dispatched for a specific task).
    pub task_id: Option<uuid::Uuid>,
}
