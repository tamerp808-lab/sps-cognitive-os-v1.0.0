//! World Model entities.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// A unique entity id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub Uuid);

impl EntityId {
    /// Generate a new id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Kind of entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    /// A project.
    Project,
    /// A file.
    File,
    /// An agent.
    Agent,
    /// A goal.
    Goal,
    /// A task.
    Task,
    /// A tool.
    Tool,
    /// An external system.
    ExternalSystem,
}

/// Project id (alias of EntityId).
pub type ProjectId = EntityId;
/// File id.
pub type FileId = EntityId;
/// Agent id.
pub type AgentId = EntityId;
/// Tool id.
pub type ToolId = EntityId;
/// External system id.
pub type ExternalSystemId = EntityId;

/// A project entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// Unique id.
    pub id: ProjectId,
    /// Display name.
    pub name: SmolStr,
    /// Filesystem path (absolute or relative to workspace).
    pub path: SmolStr,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<SmolStr>,
    /// Created at (wall time, display only).
    pub created_at: u64,
    /// Originating tick.
    pub origin_tick: u64,
}

/// A file entity (virtual FS representation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    /// Unique id.
    pub id: FileId,
    /// Owning project id.
    pub project_id: ProjectId,
    /// Path relative to project root.
    pub path: SmolStr,
    /// Content hash (SHA-256) — set when file is written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// File size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Originating tick.
    pub origin_tick: u64,
}

/// An agent entity (descriptor — runtime lives in Agent Runtime phase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    /// Unique id.
    pub id: AgentId,
    /// Archetype: architect | developer | reviewer | tester | devops | researcher.
    pub archetype: SmolStr,
    /// Display name.
    pub name: SmolStr,
    /// Originating tick.
    pub origin_tick: u64,
}

/// A tool entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    /// Unique id.
    pub id: ToolId,
    /// Tool name.
    pub name: SmolStr,
    /// Tool version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<SmolStr>,
    /// Originating tick.
    pub origin_tick: u64,
}

/// An external system entity (e.g. a git remote, an API endpoint).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalSystem {
    /// Unique id.
    pub id: ExternalSystemId,
    /// System name.
    pub name: SmolStr,
    /// System kind (e.g. "git_remote", "api", "database").
    pub kind: SmolStr,
    /// Endpoint URL or path.
    pub endpoint: String,
    /// Originating tick.
    pub origin_tick: u64,
}
