//! Streaming support for LLM providers.
//!
//! Provides a `StreamChunk` type and a `StreamHandler` trait that
//! adapters can implement to deliver tokens incrementally.

use serde::{Deserialize, Serialize};

/// A single chunk in a streaming completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Delta text (the new tokens since the last chunk).
    pub delta: String,
    /// Whether this is the final chunk.
    pub done: bool,
}

/// Handler for streaming chunks.
pub trait StreamHandler: Send + Sync {
    /// Called for each chunk.
    fn on_chunk(&self, chunk: &StreamChunk);
    /// Called when the stream completes (success).
    fn on_complete(&self);
    /// Called on error.
    fn on_error(&self, error: &str);
}

/// A no-op handler that ignores all events.
pub struct NoopStreamHandler;

impl StreamHandler for NoopStreamHandler {
    fn on_chunk(&self, _chunk: &StreamChunk) {}
    fn on_complete(&self) {}
    fn on_error(&self, _error: &str) {}
}
