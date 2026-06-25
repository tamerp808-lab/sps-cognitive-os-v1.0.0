//! SPS FFI bridge — exposes the kernel to TypeScript via napi-rs.
//!
//! When the `napi` feature is enabled, this crate produces a Node.js
//! addon. When disabled, it builds as a pure Rust library exposing
//! the same API via plain Rust functions (useful for testing).
//!
//! # TypeScript usage (when napi feature is enabled)
//!
//! ```ts
//! import { Kernel } from '@sps/kernel';
//!
//! const kernel = Kernel.boot('/path/to/sps.db');
//! const tick = kernel.dispatchEvent('goal.created', { title: 'My goal' });
//! const state = kernel.query(); // returns JSON of canonical state
//! kernel.verify();
//! kernel.snapshot();
//! ```

#![allow(clippy::module_name_repetitions)]

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::event_store::EventStore;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::storage::port::StoragePort;
use sps_storage_sqlite::SqliteStorage;

/// A handle to a booted kernel instance. Wraps the Rust `SpsKernel`.
pub struct KernelHandle {
    kernel: Arc<RwLock<Option<SpsKernel>>>,
}

impl Default for KernelHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelHandle {
    /// Create an empty handle (no kernel booted).
    pub fn new() -> Self {
        Self {
            kernel: Arc::new(RwLock::new(None)),
        }
    }

    /// Boot the kernel against a SQLite database at the given path.
    pub fn boot(&self, db_path: &str) -> Result<(), String> {
        let path = PathBuf::from(db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let storage: Arc<dyn StoragePort> =
            Arc::new(SqliteStorage::open(&path).map_err(|e| e.to_string())?);
        let kernel = SpsKernel::boot(storage, KernelConfig::default()).map_err(|e| e.to_string())?;
        *self.kernel.write() = Some(kernel);
        Ok(())
    }

    /// Boot with an in-memory kernel (for tests / ephemeral sessions).
    pub fn boot_in_memory(&self) -> Result<(), String> {
        let storage: Arc<dyn StoragePort> = Arc::new(
            sps_storage_memory::InMemoryStorage::new(),
        );
        let kernel = SpsKernel::boot(storage, KernelConfig::default()).map_err(|e| e.to_string())?;
        *self.kernel.write() = Some(kernel);
        Ok(())
    }

    /// Dispatch a raw event. Returns the assigned tick.
    pub fn dispatch_event(
        &self,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<u64, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        let raw = RawEvent::new(event_type, payload, Actor::owner(), current_wall_time());
        let event = kernel.dispatch(raw).map_err(|e| e.to_string())?;
        Ok(event.tick)
    }

    /// Query the canonical state as JSON.
    pub fn query_state(&self) -> Result<serde_json::Value, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        let state = kernel.query(|s| serde_json::to_value(s).unwrap_or(serde_json::json!({})));
        Ok(state)
    }

    /// Verify the hash chain. Returns `true` if verification succeeded.
    pub fn verify(&self) -> Result<bool, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        let report = kernel.verify().map_err(|e| e.to_string())?;
        Ok(report.failure.is_none())
    }

    /// Take a snapshot. Returns the snapshot tick.
    pub fn snapshot(&self) -> Result<u64, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        let snap = kernel.snapshot(current_wall_time()).map_err(|e| e.to_string())?;
        Ok(snap.tick)
    }

    /// Get the last tick.
    pub fn last_tick(&self) -> Result<u64, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        kernel.last_tick().map_err(|e| e.to_string())
    }

    /// Get the event count.
    pub fn event_count(&self) -> Result<u64, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        kernel.event_count().map_err(|e| e.to_string())
    }

    /// Get the backend name (e.g. "sqlite" or "memory").
    pub fn backend_name(&self) -> Result<String, String> {
        let kernel_lock = self.kernel.read();
        let kernel = kernel_lock.as_ref().ok_or("kernel not booted")?;
        Ok(kernel.backend_name().to_string())
    }

    /// Shut down the kernel (drops the inner instance).
    pub fn shutdown(&self) {
        *self.kernel.write() = None;
    }
}

fn current_wall_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ============== napi-rs bindings (enabled with `napi` feature) ==============

#[cfg(feature = "napi")]
mod napi_bindings {
    use super::*;
    use napi_derive::napi;

    #[napi]
    pub struct Kernel {
        handle: KernelHandle,
    }

    #[napi]
    impl Kernel {
        /// Create a new (unbooted) kernel handle.
        #[napi(constructor)]
        pub fn new() -> Self {
            Self {
                handle: KernelHandle::new(),
            }
        }

        /// Boot against a SQLite database file.
        #[napi]
        pub fn boot(&self, db_path: String) -> Result<(), napi::Error> {
            self.handle.boot(&db_path).map_err(|e| napi::Error::from_reason(e))
        }

        /// Boot an in-memory kernel (for tests).
        #[napi]
        pub fn boot_in_memory(&self) -> Result<(), napi::Error> {
            self.handle.boot_in_memory().map_err(|e| napi::Error::from_reason(e))
        }

        /// Dispatch a raw event. Returns the assigned tick.
        #[napi]
        pub fn dispatch_event(
            &self,
            event_type: String,
            payload: napi::bindgen_prelude::Object,
        ) -> Result<u64, napi::Error> {
            let payload_json: serde_json::Value = serde_json::to_value(&payload)
                .unwrap_or(serde_json::Value::Null);
            self.handle
                .dispatch_event(&event_type, payload_json)
                .map_err(|e| napi::Error::from_reason(e))
        }

        /// Query the canonical state as a JSON object.
        #[napi]
        pub fn query_state(&self, env: napi::Env) -> Result<napi::JsObject, napi::Error> {
            let state = self.handle.query_state().map_err(|e| napi::Error::from_reason(e))?;
            let json_str = serde_json::to_string(&state)
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
            let mut obj = env.create_object()?;
            obj.set("state", json_str)?;
            Ok(obj)
        }

        /// Verify the hash chain. Returns true on success.
        #[napi]
        pub fn verify(&self) -> Result<bool, napi::Error> {
            self.handle.verify().map_err(|e| napi::Error::from_reason(e))
        }

        /// Take a snapshot. Returns the snapshot tick.
        #[napi]
        pub fn snapshot(&self) -> Result<u64, napi::Error> {
            self.handle.snapshot().map_err(|e| napi::Error::from_reason(e))
        }

        /// Get the last tick.
        #[napi(getter)]
        pub fn last_tick(&self) -> Result<u64, napi::Error> {
            self.handle.last_tick().map_err(|e| napi::Error::from_reason(e))
        }

        /// Get the event count.
        #[napi(getter)]
        pub fn event_count(&self) -> Result<u64, napi::Error> {
            self.handle.event_count().map_err(|e| napi::Error::from_reason(e))
        }

        /// Get the backend name.
        #[napi(getter)]
        pub fn backend_name(&self) -> Result<String, napi::Error> {
            self.handle.backend_name().map_err(|e| napi::Error::from_reason(e))
        }

        /// Shut down the kernel.
        #[napi]
        pub fn shutdown(&self) {
            self.handle.shutdown();
        }
    }
}
