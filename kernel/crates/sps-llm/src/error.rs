//! LLM layer errors.

use thiserror::Error;

/// LLM layer error.
#[derive(Debug, Error)]
pub enum LlmError {
    /// Provider error.
    #[error("provider error: {0}")]
    Provider(#[from] anyhow::Error),

    /// No provider configured.
    #[error("no provider configured")]
    NoProvider,

    /// Conversation not found.
    #[error("conversation {0} not found")]
    ConversationNotFound(String),

    /// Context window exceeded.
    #[error("context window exceeded: {used} tokens > {limit} limit")]
    ContextWindowExceeded { used: usize, limit: usize },

    /// Tool not found.
    #[error("tool '{0}' not found")]
    ToolNotFound(String),

    /// Tool execution failed.
    #[error("tool '{tool}' failed: {message}")]
    ToolFailed { tool: String, message: String },

    /// Structured output parse failure.
    #[error("structured output parse failure: {0}")]
    ParseFailure(String),

    /// Schema validation failure.
    #[error("schema validation failure: {0}")]
    SchemaValidation(String),

    /// Streaming error.
    #[error("streaming error: {0}")]
    Streaming(String),
}

/// Convenience alias.
pub type LlmResult<T> = Result<T, LlmError>;
