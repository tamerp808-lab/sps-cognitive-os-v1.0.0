//! SPS World Model (Phase 4).
//!
//! A continuously-updated projection of everything the kernel knows
//! about its operating environment: projects, files, agents, goals,
//! tasks, tools, external systems, and runtime state.

#![allow(clippy::module_name_repetitions)]

pub mod entities;
pub mod graph;
pub mod reducer;

pub use entities::{Project, ProjectId, FileNode, FileId, AgentDescriptor, AgentId, ToolDescriptor, ToolId, ExternalSystem, ExternalSystemId, EntityKind, EntityId};
pub use graph::{WorldGraph, WorldRelationship, WorldLinkKind};
pub use reducer::{WorldReducer, WorldState};
