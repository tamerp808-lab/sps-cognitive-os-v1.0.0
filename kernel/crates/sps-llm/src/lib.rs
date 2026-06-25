//! SPS LLM Layer — streaming, conversations, tool use, structured output.
//!
//! This crate sits on top of `sps-providers-http` and adds:
//!
//! - **Streaming**: token-by-token delivery via async streams
//! - **Conversations**: multi-turn context with token counting + truncation
//! - **Tool use**: function calling protocol (OpenAI-compatible)
//! - **Structured output**: JSON mode with schema validation
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │            ConversationEngine               │
//! │  ┌─────────────┐  ┌──────────────────────┐  │
//! │  │  Context    │  │  Tool Registry       │  │
//! │  │  Manager    │  │  (function calling)  │  │
//! │  └─────────────┘  └──────────────────────┘  │
//! │  ┌─────────────┐  ┌──────────────────────┐  │
//! │  │  Streaming  │  │  Structured Output   │  │
//! │  │  Collector  │  │  (JSON schema)       │  │
//! │  └─────────────┘  └──────────────────────┘  │
//! └─────────────────────────────────────────────┘
//!                      │
//!                      ▼
//!            ┌──────────────────┐
//!            │  LlmProvider     │
//!            │  (sps-effects)   │
//!            └──────────────────┘
//! ```

#![allow(clippy::module_name_repetitions)]

pub mod streaming;
pub mod conversation;
pub mod tools;
pub mod structured;
pub mod error;

pub use streaming::{StreamEvent, StreamCollector, StreamingCompletion};
pub use conversation::{Conversation, ConversationEngine, ConversationId, Message, MessageRole, ContextWindow};
pub use tools::{ToolRegistry, ToolDefinition, ToolCall, ToolResult, Tool, ToolSchema};
pub use structured::{StructuredOutput, JsonSchema};
pub use error::{LlmError, LlmResult};

/// Re-export the LLM provider trait for convenience.
pub use sps_effects::providers::llm::{LlmProvider, LlmRequest, LlmCompletion, ProviderConfig};
