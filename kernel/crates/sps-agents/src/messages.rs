//! Inter-agent messaging.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::agent::AgentId;

/// Kind of message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// Task assignment.
    TaskAssignment,
    /// Task result.
    TaskResult,
    /// Question.
    Question,
    /// Answer.
    Answer,
    /// Delegation.
    Delegation,
    /// Status update.
    StatusUpdate,
    /// Error report.
    ErrorReport,
}

/// A message between agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message id.
    pub id: Uuid,
    /// Sender agent id.
    pub from: AgentId,
    /// Recipient agent id (None = broadcast).
    pub to: Option<AgentId>,
    /// Message kind.
    pub kind: MessageKind,
    /// Subject (short summary).
    pub subject: SmolStr,
    /// Body (free-form text or JSON).
    pub body: serde_json::Value,
    /// Tick when the message was sent.
    pub tick: u64,
}

impl AgentMessage {
    /// Create a new message.
    pub fn new(
        from: AgentId,
        to: Option<AgentId>,
        kind: MessageKind,
        subject: impl Into<SmolStr>,
        body: serde_json::Value,
        tick: u64,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            from,
            to,
            kind,
            subject: subject.into(),
            body,
            tick,
        }
    }
}
