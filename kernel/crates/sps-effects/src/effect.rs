//! Effect types — intent, result, error.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

/// Kinds of effects the kernel knows about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectType {
    /// LLM completion call.
    LlmComplete,
    /// Filesystem read.
    FsRead,
    /// Filesystem write.
    FsWrite,
    /// Filesystem delete.
    FsDelete,
    /// Shell command execution.
    ShellExec,
    /// Git operation.
    GitOperation,
    /// Search query.
    SearchQuery,
    /// Tool invocation.
    ToolInvoke,
    /// Provider healthcheck.
    ProviderHealthcheck,
    /// Phase 11B: Write a generated file to disk (Factory-specific).
    /// Differs from FsWrite in that it tracks project + factory_run_id.
    WriteFile,
    /// Phase 11B: Run project tests (cargo test / npm test).
    RunTests,
    /// Phase 11B: Build project (cargo build --release / npm run build).
    BuildProject,
    /// Phase 11B: Package project (tarball / docker image / etc.).
    PackageProject,
}

impl EffectType {
    /// String identifier for the effect (used as event payload discriminator).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LlmComplete => "llm.complete",
            Self::FsRead => "fs.read",
            Self::FsWrite => "fs.write",
            Self::FsDelete => "fs.delete",
            Self::ShellExec => "shell.exec",
            Self::GitOperation => "git.operation",
            Self::SearchQuery => "search.query",
            Self::ToolInvoke => "tool.invoke",
            Self::ProviderHealthcheck => "provider.healthcheck",
            Self::WriteFile => "factory.write_file",
            Self::RunTests => "factory.run_tests",
            Self::BuildProject => "factory.build_project",
            Self::PackageProject => "factory.package_project",
        }
    }
}

/// An intent to perform a side effect. Stored as the payload of an
/// `effect.intent` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectIntent {
    /// Type of effect.
    pub effect_type: EffectType,
    /// Strongly-typed input. serde_json::Value so the kernel core does
    /// not need to know about each effect type.
    pub input: serde_json::Value,
    /// Tick of the event that caused this intent (for tracing).
    pub causation_tick: Option<u64>,
}

impl EffectIntent {
    /// Create a new intent.
    pub fn new(effect_type: EffectType, input: serde_json::Value) -> Self {
        Self {
            effect_type,
            input,
            causation_tick: None,
        }
    }
}

/// The result of executing an effect. Stored as the payload of an
/// `effect.executed` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectResult {
    /// Tick of the originating `effect.intent` event.
    pub intent_tick: u64,
    /// Output of the effect (typed per effect type).
    pub output: serde_json::Value,
    /// Wall time spent executing (display only).
    pub elapsed_ms: u64,
}

/// An error produced by effect execution.
#[derive(Debug, Clone, Serialize, Deserialize, Error)]
pub enum EffectError {
    /// No executor registered for the effect type.
    #[error("no executor for effect type '{0}'")]
    NoExecutor(String),
    /// No provider configured for an LLM effect.
    #[error("no provider available for LLM effect")]
    NoProvider,
    /// The executor returned an error.
    #[error("executor error: {message}")]
    ExecutorFailed {
        /// Human-readable error message.
        message: String,
        /// Optional structured details.
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    },
    /// Governance denied the effect.
    #[error("governance denied: {0}")]
    GovernanceDenied(String),
    /// The effect was cancelled.
    #[error("cancelled")]
    Cancelled,
}

impl EffectError {
    /// Convert to a serde_json::Value for storage in event payloads.
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({"message": "unknown"}))
    }
}

/// A unique identifier for an effect (the tick of the intent event).
pub type EffectId = u64;

/// A short identifier for an effect type, used as a key in registries.
pub type EffectKey = SmolStr;
