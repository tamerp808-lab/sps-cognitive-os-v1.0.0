//! Streaming infrastructure.
//!
//! Provides:
//! - `StreamEvent`: token-by-token events
//! - `StreamCollector`: accumulates tokens into a final completion
//! - `StreamingCompletion`: high-level streaming API
//!
//! # How it works
//!
//! The `StreamingCompletion` wraps a non-streaming provider and simulates
//! streaming by chunking the response. This is a fallback — real
//! streaming would require each provider adapter to implement SSE
//! parsing. The simulated streaming is still useful for UI: it lets
//! the user see progress instead of staring at a spinner.
//!
//! Future work: add `stream_complete` to the `LlmProvider` trait and
//! implement true SSE parsing in each adapter.

use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::{LlmError, LlmResult};
use crate::LlmProvider;

/// A single event in a streaming completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// A text token (or chunk) was received.
    Token {
        /// The token text.
        text: String,
        /// Cumulative token count (approximate).
        token_count: usize,
    },
    /// A tool call was detected (partial or complete).
    ToolCall {
        /// Tool name.
        name: String,
        /// Arguments (may be partial JSON).
        arguments: String,
    },
    /// Streaming is complete.
    Done {
        /// Total tokens generated.
        total_tokens: usize,
        /// Finish reason: "stop", "length", "tool_calls", "content_filter".
        finish_reason: String,
    },
    /// An error occurred during streaming.
    Error {
        /// Error message.
        message: String,
    },
}

/// A collector that accumulates stream events into a final result.
#[derive(Debug, Default)]
pub struct StreamCollector {
    /// Accumulated text.
    pub text: String,
    /// Tool calls collected.
    pub tool_calls: Vec<crate::tools::ToolCall>,
    /// Total tokens.
    pub total_tokens: usize,
    /// Finish reason (if done).
    pub finish_reason: Option<String>,
    /// Error (if any).
    pub error: Option<String>,
}

impl StreamCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a stream event.
    pub fn apply(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::Token { text, token_count } => {
                self.text.push_str(text);
                self.total_tokens = *token_count;
            }
            StreamEvent::ToolCall { name, arguments } => {
                self.tool_calls.push(crate::tools::ToolCall {
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
            }
            StreamEvent::Done { total_tokens, finish_reason } => {
                self.total_tokens = *total_tokens;
                self.finish_reason = Some(finish_reason.clone());
            }
            StreamEvent::Error { message } => {
                self.error = Some(message.clone());
            }
        }
    }

    /// Is streaming complete?
    pub fn is_done(&self) -> bool {
        self.finish_reason.is_some() || self.error.is_some()
    }

    /// Convert to a final completion.
    pub fn into_completion(self) -> crate::LlmCompletion {
        crate::LlmCompletion {
            text: self.text,
            model: smol_str::SmolStr::new("streamed"),
            usage: sps_effects::providers::llm::TokenUsage {
                prompt_tokens: 0,
                completion_tokens: self.total_tokens as u64,
                total_tokens: self.total_tokens as u64,
            },
            elapsed_ms: 0,
        }
    }
}

/// High-level streaming completion API.
///
/// Wraps a non-streaming provider and simulates streaming by chunking
/// the final response into word-level tokens.
pub struct StreamingCompletion {
    provider: Arc<dyn LlmProvider>,
    /// Chunk size in characters for simulated streaming.
    chunk_size: usize,
    /// Delay between chunks in milliseconds (for UX).
    chunk_delay_ms: u64,
}

impl StreamingCompletion {
    /// Create a new streaming wrapper.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            provider,
            chunk_size: 8, // ~1-2 words per chunk
            chunk_delay_ms: 20, // 20ms between chunks — feels live
        }
    }

    /// Set the chunk size (characters).
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size.max(1);
        self
    }

    /// Set the delay between chunks.
    pub fn with_chunk_delay(mut self, delay_ms: u64) -> Self {
        self.chunk_delay_ms = delay_ms;
        self
    }

    /// Stream a completion as a boxed async stream of `StreamEvent`s.
    ///
    /// This calls the underlying provider's `complete` (non-streaming),
    /// then chunks the result into token events. Real streaming would
    /// require provider-level SSE support.
    pub async fn stream(
        &self,
        request: sps_effects::providers::llm::LlmRequest,
    ) -> LlmResult<futures_util::stream::BoxStream<'static, StreamEvent>> {
        let provider = self.provider.clone();
        let chunk_size = self.chunk_size;
        let chunk_delay = self.chunk_delay_ms;

        let stream = async_stream::stream! {
            // 1. Call the provider (non-streaming under the hood).
            let completion = match provider.complete(&request) {
                Ok(c) => c,
                Err(e) => {
                    yield StreamEvent::Error { message: e.to_string() };
                    return;
                }
            };

            // 2. Chunk the text into tokens.
            let text = completion.text;
            let mut token_count = 0usize;
            let mut pos = 0;
            while pos < text.len() {
                // Find the next chunk boundary (don't split UTF-8 chars).
                let end = (pos + chunk_size).min(text.len());
                let end = text[..end].char_indices().last().map(|(i, _)| i).unwrap_or(end);
                let chunk = &text[pos..end];
                if !chunk.is_empty() {
                    token_count += 1;
                    yield StreamEvent::Token {
                        text: chunk.to_string(),
                        token_count,
                    };
                    if chunk_delay > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(chunk_delay)).await;
                    }
                }
                pos = end;
            }

            // 3. Emit done event.
            yield StreamEvent::Done {
                total_tokens: token_count,
                finish_reason: "stop".to_string(),
            };
        };

        Ok(stream.boxed())
    }

    /// Stream and collect into a final completion. Useful when you want
    /// the streaming UX but also need the final result.
    pub async fn stream_and_collect(
        &self,
        request: sps_effects::providers::llm::LlmRequest,
    ) -> LlmResult<StreamCollector> {
        use futures_util::StreamExt;
        let mut collector = StreamCollector::new();
        let mut stream = self.stream(request).await?;
        while let Some(event) = stream.next().await {
            if matches!(event, StreamEvent::Error { .. }) {
                return Err(LlmError::Streaming(
                    collector.error.unwrap_or_else(|| "unknown streaming error".to_string())
                ));
            }
            collector.apply(&event);
        }
        Ok(collector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sps_effects::providers::adapters::StaticAdapter;
    use sps_effects::providers::llm::{LlmRequest, ProviderConfig};

    #[tokio::test]
    async fn stream_chunks_text_into_tokens() {
        let provider = Arc::new(StaticAdapter::new("test", "hello world from rust"));
        let config = ProviderConfig {
            id: "test".into(),
            name: "Test".into(),
            api_url: "http://localhost".into(),
            api_key: None,
            model_name: "test-model".into(),
            metadata: Default::default(),
        };
        provider.configure(config);

        let streaming = StreamingCompletion::new(provider)
            .with_chunk_size(5)
            .with_chunk_delay(0); // no delay in tests

        let req = LlmRequest {
            provider_id: "test".into(),
            model: None,
            system: None,
            user: "hi".into(),
            max_tokens: None,
            temperature: None,
        };

        let collector = streaming.stream_and_collect(req).await.unwrap();
        assert!(collector.text.contains("hello"));
        assert!(collector.text.contains("world"));
        assert!(collector.finish_reason.is_some());
    }

    #[test]
    fn collector_accumulates_tokens() {
        let mut c = StreamCollector::new();
        c.apply(&StreamEvent::Token { text: "hello ".into(), token_count: 1 });
        c.apply(&StreamEvent::Token { text: "world".into(), token_count: 2 });
        c.apply(&StreamEvent::Done { total_tokens: 2, finish_reason: "stop".into() });
        assert_eq!(c.text, "hello world");
        assert_eq!(c.total_tokens, 2);
        assert!(c.is_done());
    }

    #[test]
    fn collector_handles_errors() {
        let mut c = StreamCollector::new();
        c.apply(&StreamEvent::Error { message: "boom".into() });
        assert!(c.is_done());
        assert_eq!(c.error, Some("boom".to_string()));
    }
}
