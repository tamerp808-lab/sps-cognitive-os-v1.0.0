//! Conversation management — multi-turn context with token counting.
//!
//! A `Conversation` is a sequence of `Message`s (system, user, assistant,
//! tool). The `ConversationEngine` manages multiple conversations, each
//! with its own context window. When the conversation exceeds the
//! context window limit, oldest messages are truncated (keeping the
//! system prompt + most recent turns).

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::error::{LlmError, LlmResult};

/// Conversation id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(pub Uuid);

impl ConversationId {
    /// Generate a new conversation id.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System / instruction prompt.
    System,
    /// User message.
    User,
    /// Assistant (LLM) message.
    Assistant,
    /// Tool call result.
    Tool,
}

/// A conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Role.
    pub role: MessageRole,
    /// Content (text).
    pub content: String,
    /// Optional tool calls (if assistant message triggered tools).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<crate::tools::ToolCall>,
    /// Optional tool call id (if this is a tool result).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Wall time (display only).
    pub wall_time: u64,
    /// Token count (approximate, set when added).
    #[serde(default)]
    pub token_count: usize,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            wall_time: 0,
            token_count: 0,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            wall_time: 0,
            token_count: 0,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            wall_time: 0,
            token_count: 0,
        }
    }

    /// Create a tool result message.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            wall_time: 0,
            token_count: 0,
        }
    }
}

/// Context window configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    /// Maximum tokens allowed in the context (including system + history).
    pub max_tokens: usize,
    /// Whether to keep the system prompt when truncating.
    pub keep_system: bool,
    /// Minimum number of recent messages to always keep.
    pub keep_recent: usize,
}

impl Default for ContextWindow {
    fn default() -> Self {
        Self {
            max_tokens: 8192, // reasonable default for most models
            keep_system: true,
            keep_recent: 4,
        }
    }
}

/// A conversation — a sequence of messages with a context window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique id.
    pub id: ConversationId,
    /// Display title (auto-generated from first user message).
    pub title: SmolStr,
    /// Provider id to use for this conversation.
    pub provider_id: SmolStr,
    /// Model override (optional).
    pub model: Option<SmolStr>,
    /// Messages in order.
    pub messages: Vec<Message>,
    /// Context window config.
    pub context_window: ContextWindow,
    /// Wall time created.
    pub created_at: u64,
    /// Wall time last updated.
    pub updated_at: u64,
}

impl Conversation {
    /// Create a new conversation with a system prompt.
    pub fn new(provider_id: impl Into<SmolStr>, system_prompt: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut conv = Self {
            id: ConversationId::new(),
            title: SmolStr::new("New conversation"),
            provider_id: provider_id.into(),
            model: None,
            messages: Vec::new(),
            context_window: ContextWindow::default(),
            created_at: now,
            updated_at: now,
        };
        let system = system_prompt.into();
        if !system.is_empty() {
            let mut msg = Message::system(system);
            msg.token_count = count_tokens(&msg.content);
            conv.messages.push(msg);
        }
        conv
    }

    /// Add a user message.
    pub fn add_user(&mut self, content: impl Into<String>) -> &mut Message {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.updated_at = now;
        let mut msg = Message::user(content);
        msg.wall_time = now;
        msg.token_count = count_tokens(&msg.content);
        // Auto-title from first user message.
        if self.title == "New conversation" {
            let content = &msg.content;
            let title: String = content.chars().take(40).collect();
            self.title = SmolStr::new(title);
        }
        self.messages.push(msg);
        self.messages.last_mut().unwrap()
    }

    /// Add an assistant message.
    pub fn add_assistant(&mut self, content: impl Into<String>) -> &mut Message {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.updated_at = now;
        let mut msg = Message::assistant(content);
        msg.wall_time = now;
        msg.token_count = count_tokens(&msg.content);
        self.messages.push(msg);
        self.messages.last_mut().unwrap()
    }

    /// Add a tool result message.
    pub fn add_tool_result(&mut self, tool_call_id: impl Into<String>, content: impl Into<String>) -> &mut Message {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.updated_at = now;
        let mut msg = Message::tool(tool_call_id, content);
        msg.wall_time = now;
        msg.token_count = count_tokens(&msg.content);
        self.messages.push(msg);
        self.messages.last_mut().unwrap()
    }

    /// Total token count of all messages.
    pub fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.token_count).sum()
    }

    /// Truncate the conversation to fit within the context window.
    /// Returns the number of messages removed.
    pub fn truncate(&mut self) -> usize {
        let total = self.total_tokens();
        if total <= self.context_window.max_tokens {
            return 0;
        }
        let original_len = self.messages.len();
        // Strategy: keep system (if configured) + keep_recent most recent.
        let system_idx = if self.context_window.keep_system {
            self.messages.iter().position(|m| m.role == MessageRole::System)
        } else {
            None
        };

        let mut kept: Vec<Message> = Vec::new();
        // Keep system.
        if let Some(idx) = system_idx {
            kept.push(self.messages[idx].clone());
        }
        // Keep the most recent N messages.
        let recent_start = self.messages.len().saturating_sub(self.context_window.keep_recent);
        kept.extend(self.messages[recent_start..].iter().cloned());

        self.messages = kept;
        original_len - self.messages.len()
    }

    /// Convert to an `LlmRequest` for the provider.
    pub fn to_request(&self) -> sps_effects::providers::llm::LlmRequest {
        // Find system prompt.
        let system = self
            .messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            .map(|m| m.content.clone());

        // Find last user message.
        let user = self
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Build conversation context as a single user message.
        // Note: This is a simplification — real multi-turn support would
        // pass the full message history to the provider. The current
        // LlmProvider trait only supports system + user, so we encode
        // the history into the user message.
        let user = if self.messages.iter().filter(|m| m.role == MessageRole::User).count() > 1 {
            let mut context = String::from("Conversation history:\n\n");
            for m in &self.messages {
                if m.role == MessageRole::User {
                    context.push_str(&format!("User: {}\n", m.content));
                } else if m.role == MessageRole::Assistant {
                    context.push_str(&format!("Assistant: {}\n", m.content));
                }
            }
            context.push_str(&format!("\nLatest message: {}", user));
            context
        } else {
            user
        };

        sps_effects::providers::llm::LlmRequest {
            provider_id: self.provider_id.clone(),
            model: self.model.clone(),
            system,
            user,
            max_tokens: None,
            temperature: None,
        }
    }
}

/// The conversation engine — manages multiple conversations.
pub struct ConversationEngine {
    conversations: RwLock<BTreeMap<Uuid, Conversation>>,
}

impl Default for ConversationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationEngine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            conversations: RwLock::new(BTreeMap::new()),
        }
    }

    /// Create a new conversation.
    pub fn create(
        &self,
        provider_id: impl Into<SmolStr>,
        system_prompt: impl Into<String>,
    ) -> ConversationId {
        let conv = Conversation::new(provider_id, system_prompt);
        let id = conv.id;
        self.conversations.write().insert(id.0, conv);
        id
    }

    /// Get a conversation by id.
    pub fn get(&self, id: &ConversationId) -> LlmResult<Conversation> {
        self.conversations
            .read()
            .get(&id.0)
            .cloned()
            .ok_or_else(|| LlmError::ConversationNotFound(id.to_string()))
    }

    /// Update a conversation.
    pub fn update(&self, conv: Conversation) {
        self.conversations.write().insert(conv.id.0, conv);
    }

    /// Add a user message to a conversation. Returns the updated conversation.
    pub fn add_user_message(
        &self,
        id: &ConversationId,
        content: impl Into<String>,
    ) -> LlmResult<Conversation> {
        let mut conv = self.get(id)?;
        conv.add_user(content);
        conv.truncate();
        self.update(conv.clone());
        Ok(conv)
    }

    /// Add an assistant message to a conversation.
    pub fn add_assistant_message(
        &self,
        id: &ConversationId,
        content: impl Into<String>,
    ) -> LlmResult<Conversation> {
        let mut conv = self.get(id)?;
        conv.add_assistant(content);
        conv.truncate();
        self.update(conv.clone());
        Ok(conv)
    }

    /// List all conversation ids.
    pub fn list(&self) -> Vec<ConversationId> {
        self.conversations
            .read()
            .keys()
            .map(|u| ConversationId(*u))
            .collect()
    }

    /// List all conversations (sorted by updated_at desc).
    pub fn list_conversations(&self) -> Vec<Conversation> {
        let mut convs: Vec<Conversation> = self.conversations.read().values().cloned().collect();
        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        convs
    }

    /// Delete a conversation.
    pub fn delete(&self, id: &ConversationId) -> bool {
        self.conversations.write().remove(&id.0).is_some()
    }
}

/// Approximate token count (4 chars ≈ 1 token for English).
/// This is a rough heuristic — real tokenization requires the model's
/// tokenizer (e.g. tiktoken for OpenAI). For Phase 1 this is sufficient
/// for context window management.
pub fn count_tokens(text: &str) -> usize {
    // Rough heuristic: 4 chars = 1 token. Whitespace-heavy text gets
    // slightly more tokens per char. We use ceil to over-estimate
    // (safer for context window limits).
    (text.len() as f64 / 4.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_starts_with_system_prompt() {
        let conv = Conversation::new("test", "You are helpful.");
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, MessageRole::System);
        assert_eq!(conv.messages[0].content, "You are helpful.");
    }

    #[test]
    fn add_user_message_auto_titles() {
        let mut conv = Conversation::new("test", "system");
        conv.add_user("How do I write a Rust function?");
        assert_eq!(conv.title.as_str(), "How do I write a Rust function?");
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[1].role, MessageRole::User);
    }

    #[test]
    fn total_tokens_sums_all_messages() {
        let mut conv = Conversation::new("test", "ab"); // ~1 token
        conv.add_user("cd"); // ~1 token
        assert!(conv.total_tokens() >= 2);
    }

    #[test]
    fn truncate_keeps_system_and_recent() {
        let mut conv = Conversation::new("test", "system");
        conv.context_window = ContextWindow {
            max_tokens: 10, // very small
            keep_system: true,
            keep_recent: 2,
        };
        // Add many messages.
        for i in 0..20 {
            conv.add_user(format!("message number {} with some padding text", i));
        }
        let removed = conv.truncate();
        assert!(removed > 0);
        // System + 2 recent.
        assert!(conv.messages.len() <= 3);
        assert_eq!(conv.messages[0].role, MessageRole::System);
    }

    #[test]
    fn conversation_engine_creates_and_lists() {
        let engine = ConversationEngine::new();
        let id1 = engine.create("test", "system 1");
        let id2 = engine.create("test", "system 2");
        assert_eq!(engine.list().len(), 2);
        assert!(engine.get(&id1).is_ok());
        assert!(engine.get(&id2).is_ok());
    }

    #[test]
    fn conversation_engine_add_messages() {
        let engine = ConversationEngine::new();
        let id = engine.create("test", "system");
        let conv = engine.add_user_message(&id, "hello").unwrap();
        assert_eq!(conv.messages.len(), 2);
        let conv = engine.add_assistant_message(&id, "hi there").unwrap();
        assert_eq!(conv.messages.len(), 3);
    }

    #[test]
    fn to_request_includes_system_and_user() {
        let mut conv = Conversation::new("test", "be helpful");
        conv.add_user("hello");
        let req = conv.to_request();
        assert_eq!(req.provider_id, "test");
        assert_eq!(req.system, Some("be helpful".to_string()));
        assert!(req.user.contains("hello"));
    }

    #[test]
    fn to_request_encodes_multi_turn_history() {
        let mut conv = Conversation::new("test", "system");
        conv.add_user("first question");
        conv.add_assistant("first answer");
        conv.add_user("second question");
        let req = conv.to_request();
        assert!(req.user.contains("first question"));
        assert!(req.user.contains("first answer"));
        assert!(req.user.contains("second question"));
    }

    #[test]
    fn token_count_is_approximate() {
        assert!(count_tokens("hello world") > 0);
        assert!(count_tokens("a") > 0);
        assert_eq!(count_tokens(""), 0);
    }
}
