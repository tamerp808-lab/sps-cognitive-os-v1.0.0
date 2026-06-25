//! World Model graph — stores entities and typed relationships.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{
    AgentDescriptor, EntityId, ExternalSystem, FileNode, Project, ToolDescriptor,
};

/// Kind of relationship between two entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldLinkKind {
    /// Project contains file.
    Contains,
    /// Project uses agent.
    Uses,
    /// Project uses tool.
    UsesTool,
    /// Project depends on external system.
    DependsOn,
    /// File imports file.
    Imports,
    /// Generic related.
    Related,
}

/// A typed relationship between two entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldRelationship {
    /// Unique id (Fix #10: deterministic from payload, not random).
    #[serde(default)]
    pub id: Uuid,
    /// Source entity id.
    pub from: EntityId,
    /// Target entity id.
    pub to: EntityId,
    /// Kind of relationship.
    pub kind: WorldLinkKind,
}

/// Link id.
pub type RelationshipId = Uuid;

/// The World Model graph.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldGraph {
    /// All projects.
    pub projects: BTreeMap<Uuid, Project>,
    /// All files.
    pub files: BTreeMap<Uuid, FileNode>,
    /// All agents.
    pub agents: BTreeMap<Uuid, AgentDescriptor>,
    /// All tools.
    pub tools: BTreeMap<Uuid, ToolDescriptor>,
    /// All external systems.
    pub external_systems: BTreeMap<Uuid, ExternalSystem>,
    /// All relationships.
    pub relationships: BTreeMap<RelationshipId, WorldRelationship>,
}

impl WorldGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a project.
    pub fn add_project(&mut self, p: Project) {
        self.projects.insert(p.id.0, p);
    }

    /// Add a file.
    pub fn add_file(&mut self, f: FileNode) {
        self.files.insert(f.id.0, f);
    }

    /// Add an agent.
    pub fn add_agent(&mut self, a: AgentDescriptor) {
        self.agents.insert(a.id.0, a);
    }

    /// Add a tool.
    pub fn add_tool(&mut self, t: ToolDescriptor) {
        self.tools.insert(t.id.0, t);
    }

    /// Add an external system.
    pub fn add_external_system(&mut self, s: ExternalSystem) {
        self.external_systems.insert(s.id.0, s);
    }

    /// Add a relationship.
    pub fn add_relationship(&mut self, r: WorldRelationship) -> RelationshipId {
        let id = Uuid::now_v7();
        self.relationships.insert(id, r);
        id
    }

    /// List files in a project.
    pub fn files_in_project(&self, project_id: &EntityId) -> Vec<&FileNode> {
        self.files
            .values()
            .filter(|f| f.project_id == *project_id)
            .collect()
    }

    /// Remove an entity by id (and any relationships pointing to it).
    pub fn remove_entity(&mut self, id: &EntityId) -> bool {
        let mut removed = false;
        if self.projects.remove(&id.0).is_some() {
            removed = true;
        }
        if self.files.remove(&id.0).is_some() {
            removed = true;
        }
        if self.agents.remove(&id.0).is_some() {
            removed = true;
        }
        if self.tools.remove(&id.0).is_some() {
            removed = true;
        }
        if self.external_systems.remove(&id.0).is_some() {
            removed = true;
        }
        if removed {
            self.relationships
                .retain(|_, r| r.from != *id && r.to != *id);
        }
        removed
    }

    /// Total entity count.
    pub fn entity_count(&self) -> usize {
        self.projects.len()
            + self.files.len()
            + self.agents.len()
            + self.tools.len()
            + self.external_systems.len()
    }
}

// Custom Serialize/Deserialize to skip transient indexes (none in Phase 4
// but the pattern is established here for future phases).
impl Serialize for WorldGraph {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("WorldGraph", 6)?;
        st.serialize_field("projects", &self.projects)?;
        st.serialize_field("files", &self.files)?;
        st.serialize_field("agents", &self.agents)?;
        st.serialize_field("tools", &self.tools)?;
        st.serialize_field("external_systems", &self.external_systems)?;
        st.serialize_field("relationships", &self.relationships)?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for WorldGraph {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            projects: BTreeMap<Uuid, Project>,
            files: BTreeMap<Uuid, FileNode>,
            agents: BTreeMap<Uuid, AgentDescriptor>,
            tools: BTreeMap<Uuid, ToolDescriptor>,
            external_systems: BTreeMap<Uuid, ExternalSystem>,
            relationships: BTreeMap<RelationshipId, WorldRelationship>,
        }
        let raw = Raw::deserialize(d)?;
        Ok(WorldGraph {
            projects: raw.projects,
            files: raw.files,
            agents: raw.agents,
            tools: raw.tools,
            external_systems: raw.external_systems,
            relationships: raw.relationships,
        })
    }
}
