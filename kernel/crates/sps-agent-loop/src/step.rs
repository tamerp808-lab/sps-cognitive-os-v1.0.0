//! A single step in the agent loop.

use serde::{Deserialize, Serialize};

/// Kind of step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepKind {
    /// The agent's thought (reasoning).
    Thought {
        /// The reasoning text.
        text: String,
    },
    /// The agent's action (tool call).
    Action {
        /// Tool name.
        tool: String,
        /// Tool arguments (JSON string).
        arguments: String,
    },
    /// The observation (tool result).
    Observation {
        /// Tool that was called.
        tool: String,
        /// Result content.
        content: String,
        /// Whether the tool succeeded.
        success: bool,
    },
    /// The agent's final answer.
    Answer {
        /// The answer text.
        text: String,
    },
    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
}

/// Status of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step is being executed.
    Running,
    /// Step completed successfully.
    Done,
    /// Step failed.
    Failed,
}

/// A single step in the agent loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopStep {
    /// Step number (0-indexed).
    pub index: usize,
    /// Kind + data.
    #[serde(flatten)]
    pub kind: StepKind,
    /// Status.
    pub status: StepStatus,
}

impl LoopStep {
    /// Create a thought step.
    pub fn thought(index: usize, text: impl Into<String>) -> Self {
        Self {
            index,
            kind: StepKind::Thought { text: text.into() },
            status: StepStatus::Done,
        }
    }

    /// Create an action step.
    pub fn action(index: usize, tool: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            index,
            kind: StepKind::Action {
                tool: tool.into(),
                arguments: arguments.into(),
            },
            status: StepStatus::Running,
        }
    }

    /// Create an observation step.
    pub fn observation(
        index: usize,
        tool: impl Into<String>,
        content: impl Into<String>,
        success: bool,
    ) -> Self {
        Self {
            index,
            kind: StepKind::Observation {
                tool: tool.into(),
                content: content.into(),
                success,
            },
            status: StepStatus::Done,
        }
    }

    /// Create an answer step.
    pub fn answer(index: usize, text: impl Into<String>) -> Self {
        Self {
            index,
            kind: StepKind::Answer { text: text.into() },
            status: StepStatus::Done,
        }
    }

    /// Create an error step.
    pub fn error(index: usize, message: impl Into<String>) -> Self {
        Self {
            index,
            kind: StepKind::Error { message: message.into() },
            status: StepStatus::Failed,
        }
    }
}
