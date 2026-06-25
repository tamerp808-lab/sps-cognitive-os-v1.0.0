//! Canonical State — the single authoritative state of the SPS Kernel.
//!
//! Every field here is a pure projection of the event stream through the
//! reducer pipeline. Mutations only happen inside reducers.
//!
//! In Phase 0, the canonical state is intentionally minimal: it tracks
//! kernel metadata (last tick, last hash, event count) and an extensible
//! `extensions` map where future phases can plug in their state slices
//! without modifying this file.
//!
//! P3D adds `typed_extensions`: a map of `Arc<dyn ErasedExtension>` for
//! reducers that opt in to typed storage (avoids per-dispatch JSON
//! round-trips). The JSON `extensions` map remains the source of truth
//! for snapshot serialization; `sync_typed_to_json` /
//! `rebuild_typed_from_json` bridge the two.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::{EventHash, Tick};
use crate::state::erased::ErasedExtension;
use crate::state::slice::StateSlice;

/// Type alias for a typed extension slot.
pub type TypedExtension = Arc<dyn ErasedExtension + Send + Sync>;

/// The single canonical state of the kernel.
#[derive(Default, Serialize, Deserialize)]
pub struct CanonicalState {
    /// Kernel metadata slice.
    pub kernel: StateSlice,

    /// Extension slices for future phases. Each phase registers its
    /// slice under a stable key, e.g. `"world"`, `"goals"`, `"memory"`.
    /// Phase 0 leaves this empty.
    #[serde(default)]
    pub extensions: BTreeMap<String, Value>,

    /// Typed extension slots (P3D). Skipped during serde — the JSON
    /// form in `extensions` is the wire representation. Use
    /// [`CanonicalState::sync_typed_to_json`] before serializing and
    /// [`CanonicalState::rebuild_typed_from_json`] after deserializing.
    #[serde(skip)]
    pub typed_extensions: BTreeMap<String, TypedExtension>,
}

impl std::fmt::Debug for CanonicalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanonicalState")
            .field("kernel", &self.kernel)
            .field("extensions", &self.extensions)
            .field(
                "typed_extensions",
                &self.typed_extensions.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl CanonicalState {
    /// Create a fresh, empty state (tick 0, genesis hash).
    pub fn genesis() -> Self {
        Self::default()
    }

    /// Returns the last applied tick.
    pub fn last_tick(&self) -> Tick {
        self.kernel.last_tick
    }

    /// Returns the last applied hash.
    pub fn last_hash(&self) -> EventHash {
        self.kernel.last_hash
    }

    /// Returns the total number of applied events.
    pub fn event_count(&self) -> u64 {
        self.kernel.event_count
    }

    /// Insert/replace an extension slice. Phase 0+ helper for future phases.
    pub fn set_extension<T: Serialize>(&mut self, key: &str, value: &T) -> serde_json::Result<()> {
        let v = serde_json::to_value(value)?;
        self.extensions.insert(key.to_string(), v);
        Ok(())
    }

    /// Read an extension slice. Returns `None` if not present.
    pub fn get_extension<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.extensions
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Insert/replace a typed extension (P3D).
    pub fn set_typed_extension<T: ErasedExtension + 'static>(&mut self, key: &str, value: Arc<T>) {
        self.typed_extensions
            .insert(key.to_string(), value as Arc<dyn ErasedExtension + Send + Sync>);
    }

    /// Read a typed extension by downcasting. Returns `None` if the
    /// key is missing or the stored type does not match `T`.
    ///
    /// Note: this clones the inner value because the registry owns the
    /// `Arc<T>` and we cannot return a borrowed `&T` through the trait
    /// object. Callers that need shared ownership should use
    /// [`CanonicalState::with_typed_extension`] instead.
    pub fn get_typed_extension<T: ErasedExtension + Clone + 'static>(
        &self,
        key: &str,
    ) -> Option<Arc<T>> {
        self.typed_extensions.get(key).and_then(|v| {
            v.as_any()
                .downcast_ref::<T>()
                .map(|t| Arc::new(t.clone()))
        })
    }

    /// Mutate a typed extension in place. If the key is absent, a
    /// default `T` is constructed, mutated by `f`, and inserted.
    pub fn with_typed_extension<T: ErasedExtension + Clone + Default + 'static>(
        &mut self,
        key: &str,
        f: impl FnOnce(&mut T),
    ) {
        let mut value = match self.get_typed_extension::<T>(key) {
            Some(arc) => (*arc).clone(),
            None => T::default(),
        };
        f(&mut value);
        self.set_typed_extension(key, Arc::new(value));
    }

    /// Synchronize typed extensions into the JSON `extensions` map.
    /// Call this before serializing the state (e.g. before taking a
    /// snapshot) so the typed values are persisted as JSON.
    pub fn sync_typed_to_json(&mut self) -> serde_json::Result<()> {
        for (k, v) in &self.typed_extensions {
            let json = v.to_json()?;
            self.extensions.insert(k.clone(), json);
        }
        Ok(())
    }

    /// Rebuild typed extensions from JSON using the given registry.
    /// Call this after deserializing the state (e.g. after loading a
    /// snapshot). For each key in `extensions` that has a registered
    /// constructor, a typed `Arc<dyn ErasedExtension>` is built and
    /// stored in `typed_extensions`.
    pub fn rebuild_typed_from_json(&mut self, registry: &crate::state::TypedExtensionRegistry) {
        let keys: Vec<String> = self.extensions.keys().cloned().collect();
        for key in keys {
            if !registry.contains(&key) {
                continue;
            }
            if let Some(v) = self.extensions.get(&key).cloned() {
                if let Some(Ok(arc)) = registry.construct(&key, v) {
                    self.typed_extensions.insert(key, arc);
                }
            }
        }
    }
}

impl Clone for CanonicalState {
    fn clone(&self) -> Self {
        Self {
            kernel: self.kernel.clone(),
            extensions: self.extensions.clone(),
            typed_extensions: self
                .typed_extensions
                .iter()
                .map(|(k, v)| (k.clone(), v.clone_dyn()))
                .collect(),
        }
    }
}

impl PartialEq for CanonicalState {
    fn eq(&self, other: &Self) -> bool {
        if self.kernel != other.kernel {
            return false;
        }
        // If either side has typed extensions, compare via typed.
        if !self.typed_extensions.is_empty() || !other.typed_extensions.is_empty() {
            if self.typed_extensions.len() != other.typed_extensions.len() {
                return false;
            }
            for (k, v) in &self.typed_extensions {
                match other.typed_extensions.get(k) {
                    Some(o) => {
                        if !v.eq_dyn(o.as_ref()) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            // Also compare JSON-only slices (no typed equivalent).
            for (k, v) in &self.extensions {
                if self.typed_extensions.contains_key(k) {
                    continue;
                }
                match other.extensions.get(k) {
                    Some(o) => {
                        if v != o {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            return true;
        }
        self.extensions == other.extensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_state_is_empty() {
        let s = CanonicalState::genesis();
        assert_eq!(s.last_tick(), 0);
        assert_eq!(s.last_hash(), EventHash::GENESIS);
        assert_eq!(s.event_count(), 0);
        assert!(s.extensions.is_empty());
    }

    #[test]
    fn extensions_round_trip() {
        let mut s = CanonicalState::genesis();
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Demo {
            n: u64,
            name: String,
        }
        let d = Demo {
            n: 7,
            name: "x".into(),
        };
        s.set_extension("demo", &d).unwrap();
        let back: Demo = s.get_extension("demo").unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn typed_extensions_sync_and_rebuild() {
        use crate::state::TypedExtensionRegistry;
        use serde::Deserialize;

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Demo {
            n: u64,
            name: String,
        }

        impl Default for Demo {
            fn default() -> Self {
                Self {
                    n: 0,
                    name: String::new(),
                }
            }
        }

        let mut s = CanonicalState::genesis();
        let d = Demo {
            n: 7,
            name: "x".into(),
        };
        s.set_typed_extension("demo", Arc::new(d.clone()));

        // Sync typed → JSON.
        s.sync_typed_to_json().unwrap();
        assert!(s.extensions.contains_key("demo"));

        // Drop typed, rebuild from JSON via registry.
        let mut reg = TypedExtensionRegistry::new();
        reg.register::<Demo>("demo");
        let mut s2 = CanonicalState::genesis();
        s2.extensions = s.extensions.clone();
        s2.rebuild_typed_from_json(&reg);
        let got = s2.get_typed_extension::<Demo>("demo").unwrap();
        assert_eq!((*got).clone(), d);
    }

    #[test]
    fn typed_partial_eq_compares_via_eq_dyn() {
        use serde::Deserialize;

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Demo {
            n: u64,
        }
        impl Default for Demo {
            fn default() -> Self {
                Self { n: 0 }
            }
        }

        let mut a = CanonicalState::genesis();
        a.set_typed_extension("demo", Arc::new(Demo { n: 7 }));
        a.sync_typed_to_json().unwrap();

        let mut b = CanonicalState::genesis();
        b.set_typed_extension("demo", Arc::new(Demo { n: 7 }));
        b.sync_typed_to_json().unwrap();

        assert_eq!(a, b);

        let mut c = CanonicalState::genesis();
        c.set_typed_extension("demo", Arc::new(Demo { n: 8 }));
        c.sync_typed_to_json().unwrap();
        assert_ne!(a, c);
    }
}
