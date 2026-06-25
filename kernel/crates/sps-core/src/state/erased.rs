//! P3D: ErasedExtension — type-erased typed extension support.
//!
//! In Phase 0, extension state lived in `CanonicalState::extensions` as
//! `BTreeMap<String, serde_json::Value>`. Every reducer read/wrote its
//! slice via `serde_json::from_value` / `to_value`. This works but is
//! slow: each dispatch pays a serialize + deserialize round-trip per
//! extension touched.
//!
//! P3D introduces a typed path: extension slices may also live in
//! `CanonicalState::typed_extensions` as `Arc<dyn ErasedExtension>`.
//! Reducers can read/write the typed value directly (no JSON). The JSON
//! form is still produced for snapshot serialization (via
//! [`CanonicalState::sync_typed_to_json`]) and rebuilt on snapshot load
//! (via [`CanonicalState::rebuild_typed_from_json`]).
//!
//! The blanket impl below means any `T` satisfying the bounds
//! (`Any + Send + Sync + Serialize + DeserializeOwned + Clone +
//! PartialEq + Debug + 'static`) is automatically an
//! `ErasedExtension`. This is the same bound the existing state
//! slices already meet, so no per-type impls are needed.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Type-erased extension trait.
pub trait ErasedExtension: Any + Send + Sync {
    /// Downcast to `&dyn Any` for typed retrieval.
    fn as_any(&self) -> &dyn Any;
    /// Serialize to a JSON value (for snapshot persistence).
    fn to_json(&self) -> serde_json::Result<Value>;
    /// Equality against another erased extension. Used by
    /// `CanonicalState::PartialEq`.
    fn eq_dyn(&self, other: &dyn ErasedExtension) -> bool;
    /// Clone into a new `Arc<dyn ErasedExtension + Send + Sync>`.
    fn clone_dyn(&self) -> Arc<dyn ErasedExtension + Send + Sync>;
    /// Debug representation (for `CanonicalState::Debug`).
    fn debug_dyn(&self) -> String;
}

impl<T> ErasedExtension for T
where
    T: Any + Send + Sync + Serialize + DeserializeOwned + Clone + PartialEq + Debug + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn to_json(&self) -> serde_json::Result<Value> {
        serde_json::to_value(self)
    }
    fn eq_dyn(&self, other: &dyn ErasedExtension) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .map(|o| self == o)
            .unwrap_or(false)
    }
    fn clone_dyn(&self) -> Arc<dyn ErasedExtension + Send + Sync> {
        Arc::new(self.clone())
    }
    fn debug_dyn(&self) -> String {
        format!("{:?}", self)
    }
}

/// Constructor function type: takes a JSON value and produces a fresh
/// `Arc<dyn ErasedExtension + Send + Sync>`. Registered per extension
/// key in [`TypedExtensionRegistry`].
pub type TypedExtensionCtor = Box<
    dyn Fn(Value) -> Result<Arc<dyn ErasedExtension + Send + Sync>, serde_json::Error>
        + Send
        + Sync,
>;

/// Registry of typed-extension constructors, keyed by extension key.
///
/// During snapshot load, [`CanonicalState::rebuild_typed_from_json`]
/// walks the JSON `extensions` map and, for each key present in this
/// registry, constructs a typed `Arc<dyn ErasedExtension>` and stores
/// it in `typed_extensions`. This lets reducers access the typed form
/// directly on snapshot resume.
///
/// [`CanonicalState::rebuild_typed_from_json`]: crate::state::CanonicalState::rebuild_typed_from_json
#[derive(Default)]
pub struct TypedExtensionRegistry {
    ctors: std::collections::BTreeMap<String, TypedExtensionCtor>,
}

impl std::fmt::Debug for TypedExtensionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedExtensionRegistry")
            .field("keys", &self.ctors.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl TypedExtensionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a constructor for the given key. The constructor
    /// deserializes a JSON value into `T` and boxes it as an erased
    /// extension.
    pub fn register<T: ErasedExtension + DeserializeOwned + 'static>(&mut self, key: &str) {
        self.ctors.insert(
            key.to_string(),
            Box::new(|v: Value| {
                let t: T = serde_json::from_value(v)?;
                Ok(Arc::new(t) as Arc<dyn ErasedExtension + Send + Sync>)
            }),
        );
    }

    /// Construct a typed extension from JSON. Returns `None` if no
    /// constructor is registered for `key`. Returns `Some(Err)` if the
    /// constructor fails (e.g. malformed JSON).
    pub fn construct(
        &self,
        key: &str,
        value: Value,
    ) -> Option<Result<Arc<dyn ErasedExtension + Send + Sync>, serde_json::Error>> {
        self.ctors.get(key).map(|f| f(value))
    }

    /// Returns `true` if a constructor is registered for `key`.
    pub fn contains(&self, key: &str) -> bool {
        self.ctors.contains_key(key)
    }

    /// Number of registered constructors.
    pub fn len(&self) -> usize {
        self.ctors.len()
    }

    /// Returns `true` if no constructors are registered.
    pub fn is_empty(&self) -> bool {
        self.ctors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Demo {
        x: u32,
        s: String,
    }

    #[test]
    fn blanket_erased_impl_works() {
        let d = Demo {
            x: 42,
            s: "hello".into(),
        };
        let erased: Arc<dyn ErasedExtension + Send + Sync> = Arc::new(d.clone());
        assert_eq!(erased.as_any().downcast_ref::<Demo>().unwrap(), &d);
        assert_eq!(erased.to_json().unwrap()["x"], 42);
        let d2: Arc<dyn ErasedExtension + Send + Sync> = Arc::new(d.clone());
        assert!(erased.eq_dyn(d2.as_ref()));
    }

    #[test]
    fn registry_round_trip() {
        let mut reg = TypedExtensionRegistry::new();
        reg.register::<Demo>("demo");
        let arc = reg
            .construct("demo", serde_json::json!({"x":7,"s":"world"}))
            .unwrap()
            .unwrap();
        assert_eq!(arc.as_any().downcast_ref::<Demo>().unwrap().x, 7);
    }
}
