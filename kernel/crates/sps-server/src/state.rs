//! Shared server state.

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;
use smol_str::SmolStr;
use sps_agents::runtime::AgentRuntime;
use sps_autonomy::governor::{AutonomyGovernor, LongRunningGoalRunner};
use sps_core::kernel::SpsKernel;
use sps_core::sink::EventSink;
use sps_effects::providers::registry::ProviderRegistry;
use sps_effects::registry::EffectRegistry;
use sps_llm::conversation::{Conversation, ConversationEngine, ConversationId};
use sps_code_intel::CodebaseIndex;

/// State shared across all route handlers.
pub struct ServerState {
    /// The booted kernel.
    pub kernel: Arc<SpsKernel>,
    /// LLM provider registry.
    pub providers: Arc<ProviderRegistry>,
    /// Effect executor registry.
    pub executors: Arc<EffectRegistry>,
    /// Default provider id (for direct LLM completion).
    pub default_provider: RwLock<Option<SmolStr>>,
    /// Active conversations keyed by id.
    pub conversations: RwLock<BTreeMap<ConversationId, (ConversationEngine, Conversation)>>,
    /// Code intelligence index.
    pub code_index: Arc<CodebaseIndex>,
    /// Workspace root path (for file scanning).
    pub workspace_root: RwLock<Option<std::path::PathBuf>>,
    /// Agent runtime — wired to the kernel via EventSink (Fix #8).
    /// Persistent across requests (Fix #8c). Agent IDs are stable.
    pub agent_runtime: Arc<AgentRuntime>,
    /// Autonomy governor — owns config (status, max_concurrent_goals,
    /// sandbox_paths). Singleton shared across all HTTP requests.
    pub autonomy_governor: Arc<AutonomyGovernor>,
    /// Long-running goal runner — event-sourced producer for goal
    /// activation/deactivation (Fix #2 / E2). Wired to the kernel via
    /// EventSink. The runner is a thin dispatcher: it does pre-dispatch
    /// validation (is_enabled, soft capacity) and then dispatches events
    /// through the kernel's EventSink. AutonomyState is the authoritative
    /// source of truth; the runner's in-memory cache is best-effort only.
    pub goal_runner: Arc<LongRunningGoalRunner>,
}

impl ServerState {
    /// Get the default provider id, if set.
    pub fn default_provider(&self) -> Option<SmolStr> {
        self.default_provider.read().clone()
    }

    /// Set the default provider id.
    pub fn set_default_provider(&self, id: impl Into<SmolStr>) {
        *self.default_provider.write() = Some(id.into());
    }
}
