//! Provider registry.

use std::sync::Arc;

use parking_lot::RwLock;
use smol_str::SmolStr;

use crate::providers::llm::{LlmProvider, ProviderConfig};

/// A registered provider entry.
pub struct ProviderEntry {
    /// The provider implementation.
    pub provider: Arc<dyn LlmProvider>,
    /// The current configuration.
    pub config: ProviderConfig,
}

/// Registry of LLM providers.
#[derive(Default)]
pub struct ProviderRegistry {
    entries: RwLock<std::collections::HashMap<SmolStr, ProviderEntry>>,
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider.
    pub fn register(&self, config: ProviderConfig, provider: Arc<dyn LlmProvider>) {
        provider.configure(config.clone());
        self.entries.write().insert(config.id.clone(), ProviderEntry { provider, config });
    }

    /// Look up a provider by id.
    pub fn get(&self, id: &str) -> Option<Arc<dyn LlmProvider>> {
        self.entries.read().get(id).map(|e| e.provider.clone())
    }

    /// List all registered provider ids.
    pub fn list(&self) -> Vec<SmolStr> {
        self.entries.read().keys().cloned().collect()
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Remove a provider.
    pub fn remove(&self, id: &str) -> bool {
        self.entries.write().remove(id).is_some()
    }
}
