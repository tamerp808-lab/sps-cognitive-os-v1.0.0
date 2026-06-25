//! Event — the atomic unit of state change in the SPS Kernel.
//!
//! # Hashing
//!
//! Every event has a 32-byte SHA-256 `hash` that covers exactly four
//! fields:
//!
//! ```text
//! hash = SHA-256(prev_hash || tick_be_bytes || event_type_utf8 || payload_canonical_json)
//! ```
//!
//! `wall_time`, `correlation_id`, `causation_tick`, `actor`, and
//! `schema_version` are **deliberately excluded** — they may differ
//! across rebuilds without affecting the deterministic core. They are
//! persisted alongside the event for display and diagnostics only.
//!
//! `payload_canonical_json` is JSON sorted by key at every level
//! (via `serde_json` with `preserve_order` *disabled* during hashing)
//! so that two events with the same logical payload produce the same
//! hash regardless of insertion order in a `Map`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::actor::Actor;
pub use crate::event_type::EventType;
use crate::KERNEL_SCHEMA_VERSION;

/// Logical sequence number. Monotonically increasing per event stream.
/// Tick 0 is reserved as the genesis sentinel (used as `prev_hash` of the
/// first real event); the first real event has tick 1.
pub type Tick = u64;

/// A 32-byte SHA-256 hash, hex-encoded for serde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventHash(#[serde(with = "hex::serde")] pub [u8; 32]);

impl EventHash {
    /// The genesis hash — `prev_hash` of the very first event in a fresh
    /// store. All zeros.
    pub const GENESIS: Self = Self([0u8; 32]);

    /// Construct from raw bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return as a slice of 32 bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Hex string representation (lowercase, 64 chars).
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from a hex string (64 chars).
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl std::fmt::Display for EventHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl std::str::FromStr for EventHash {
    type Err = hex::FromHexError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// A correlation identifier linking a chain of events back to the
/// user/agent command that initiated them.
///
/// This is a UUID v7 — time-ordered for display purposes — but it is
/// **display metadata only**. It does not participate in the hash or in
/// reducer logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Generate a fresh correlation id (UUID v7 — time-ordered).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// An event as it exists in the store — fully populated, hashed, and
/// ready to be appended. Construct via [`RawEvent::finalize`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Logical sequence number — monotonic per stream. **Primary
    /// identifier.** Used by reducers and replay ordering.
    pub tick: Tick,

    /// Hash of the previous event in the chain. `EventHash::GENESIS` for
    /// the first event.
    pub prev_hash: EventHash,

    /// SHA-256 of `(prev_hash || tick_be || event_type || payload_canonical)`.
    /// Computed by [`RawEvent::finalize`]; verified by the replay engine.
    pub hash: EventHash,

    /// Fully-qualified event type, e.g. `"goal.created"`.
    pub event_type: EventType,

    /// Strongly-typed-but-dynamic payload. Stored as `serde_json::Value`
    /// so the kernel core does not need to know about every event type;
    /// each reducer downcasts to its own typed shape.
    pub payload: serde_json::Value,

    /// Tick of the event that caused this one, if any. Used for
    /// traceability. **Display only** — not in hash, not in reducer logic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_tick: Option<Tick>,

    /// Correlation id linking back to the originating command. **Display
    /// only.**
    pub correlation_id: CorrelationId,

    /// Actor that emitted the event.
    pub actor: Actor,

    /// Schema version of the payload. Allows future migrations.
    pub schema_version: u16,

    /// Wall-clock time in milliseconds since Unix epoch. **Display only** —
    /// never read by reducers, never mixed into the hash.
    pub wall_time: u64,
}

impl Event {
    /// Return the canonical bytes that feed into the hash function.
    ///
    /// Format: `prev_hash (32 bytes raw) || tick (8 bytes big-endian) ||
    /// event_type (UTF-8) || payload (canonical JSON, keys sorted)`.
    ///
    /// This is the single source of truth for what the hash covers. If
    /// you change this function, the entire event stream becomes
    /// unverifiable — bump `KERNEL_SCHEMA_VERSION` and write a migration.
    pub fn canonical_hash_input(
        prev_hash: &EventHash,
        tick: Tick,
        event_type: &EventType,
        payload: &serde_json::Value,
    ) -> Vec<u8> {
        // Serialize payload with sorted keys (canonical form).
        let mut buf =
            serde_json::to_vec(payload).expect("serde_json::Value is always serializable");
        // serde_json::Value is a BTreeMap-backed structure when deserialized
        // without `preserve_order`, but to be safe we re-serialize through
        // canonical form here.
        let canonical_payload: serde_json::Value =
            serde_json::from_slice(&buf).expect("round-trip is infallible");
        // Re-encode with sorted keys explicitly (canonical JSON).
        buf = canonical_json_bytes(&canonical_payload);

        let mut out = Vec::with_capacity(32 + 8 + event_type.as_str().len() + buf.len());
        out.extend_from_slice(prev_hash.as_bytes());
        out.extend_from_slice(&tick.to_be_bytes());
        out.extend_from_slice(event_type.as_str().as_bytes());
        out.extend_from_slice(&buf);
        out
    }

    /// Recompute this event's hash from its fields and return it.
    ///
    /// Used by the replay verifier to check that the stored hash matches.
    pub fn recompute_hash(&self) -> EventHash {
        let bytes = Self::canonical_hash_input(
            &self.prev_hash,
            self.tick,
            &self.event_type,
            &self.payload,
        );
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        EventHash::from_bytes(out)
    }

    /// Returns `true` iff the stored hash matches the recomputed hash.
    pub fn hash_is_valid(&self) -> bool {
        self.recompute_hash() == self.hash
    }
}

/// An event that has not yet been hashed or assigned a tick. Construct
/// one of these in user code, then call [`RawEvent::finalize`] to produce
/// an [`Event`] ready for the store.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEvent {
    /// Event type.
    pub event_type: EventType,
    /// Payload.
    pub payload: serde_json::Value,
    /// Causation tick (optional).
    pub causation_tick: Option<Tick>,
    /// Correlation id (auto-generated if absent).
    pub correlation_id: CorrelationId,
    /// Actor.
    pub actor: Actor,
    /// Wall time (ms since epoch). Caller-supplied so the kernel never
    /// calls `SystemTime::now()` inside its pure core.
    pub wall_time: u64,
}

impl RawEvent {
    /// Create a new raw event. Caller supplies the wall time.
    pub fn new(
        event_type: impl Into<EventType>,
        payload: serde_json::Value,
        actor: Actor,
        wall_time: u64,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            payload,
            causation_tick: None,
            correlation_id: CorrelationId::new(),
            actor,
            wall_time,
        }
    }

    /// Set the causation tick.
    pub fn with_causation(mut self, tick: Tick) -> Self {
        self.causation_tick = Some(tick);
        self
    }

    /// Set the correlation id.
    pub fn with_correlation(mut self, id: CorrelationId) -> Self {
        self.correlation_id = id;
        self
    }

    /// Finalize: assign tick, prev_hash, and hash. Consumes `self`.
    pub fn finalize(self, tick: Tick, prev_hash: EventHash) -> Event {
        Event {
            tick,
            prev_hash,
            hash: EventHash::GENESIS, // initial value, overwritten below
            event_type: self.event_type,
            payload: self.payload,
            causation_tick: self.causation_tick,
            correlation_id: self.correlation_id,
            actor: self.actor,
            schema_version: KERNEL_SCHEMA_VERSION,
            wall_time: self.wall_time,
        }
        .with_computed_hash()
    }
}

impl Event {
    /// Recompute and store the hash in-place. Returns `self` for chaining.
    pub fn with_computed_hash(mut self) -> Self {
        let h = self.recompute_hash();
        self.hash = h;
        self
    }
}

/// Serialize a `serde_json::Value` to bytes with all object keys sorted
/// recursively. This is the canonical form used in hashing.
fn canonical_json_bytes(value: &serde_json::Value) -> Vec<u8> {
    let canonical = canonicalize_value(value);
    serde_json::to_vec(&canonical).expect("canonical value is serializable")
}

/// Recursively sort object keys in a JSON value.
fn canonicalize_value(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Map;
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: Vec<(String, serde_json::Value)> = map
                .iter()
                .map(|(k, v)| (k.clone(), canonicalize_value(v)))
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = Map::new();
            for (k, v) in sorted {
                out.insert(k, v);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize_value).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hash_excludes_wall_time_and_correlation() {
        let payload = json!({"name": "goal-1", "priority": 5});
        let e1 = RawEvent::new("goal.created", payload.clone(), Actor::owner(), 1_000_000)
            .finalize(1, EventHash::GENESIS);
        let e2 = RawEvent::new("goal.created", payload, Actor::owner(), 9_999_999)
            .finalize(1, EventHash::GENESIS);

        // Different wall_time and correlation_id, but same hash because
        // hash only covers (prev_hash, tick, type, payload).
        assert_eq!(e1.hash, e2.hash, "hash must not depend on wall_time");
        assert_ne!(
            e1.correlation_id, e2.correlation_id,
            "correlation ids are random"
        );
        assert!(e1.hash_is_valid());
        assert!(e2.hash_is_valid());
    }

    #[test]
    fn hash_changes_with_payload() {
        let e1 = RawEvent::new(
            "goal.created",
            json!({"name": "a"}),
            Actor::owner(),
            0,
        )
        .finalize(1, EventHash::GENESIS);
        let e2 = RawEvent::new(
            "goal.created",
            json!({"name": "b"}),
            Actor::owner(),
            0,
        )
        .finalize(1, EventHash::GENESIS);
        assert_ne!(e1.hash, e2.hash);
    }

    #[test]
    fn hash_changes_with_tick() {
        let payload = json!({"x": 1});
        let e1 = RawEvent::new("t.a", payload.clone(), Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        let e2 = RawEvent::new("t.a", payload, Actor::owner(), 0)
            .finalize(2, EventHash::GENESIS);
        assert_ne!(e1.hash, e2.hash);
    }

    #[test]
    fn hash_changes_with_prev_hash() {
        let payload = json!({"x": 1});
        let e1 = RawEvent::new("t.a", payload.clone(), Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        let other_prev = EventHash::from_bytes([0xaa; 32]);
        let e2 = RawEvent::new("t.a", payload, Actor::owner(), 0)
            .finalize(1, other_prev);
        assert_ne!(e1.hash, e2.hash);
    }

    #[test]
    fn payload_key_order_does_not_affect_hash() {
        // Two payloads that differ only in key insertion order must hash
        // to the same value.
        let p1: serde_json::Value =
            serde_json::from_str(r#"{"a":1,"b":2,"c":3}"#).unwrap();
        let p2: serde_json::Value =
            serde_json::from_str(r#"{"c":3,"b":2,"a":1}"#).unwrap();
        let e1 = RawEvent::new("t.a", p1, Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        let e2 = RawEvent::new("t.a", p2, Actor::owner(), 0)
            .finalize(1, EventHash::GENESIS);
        assert_eq!(e1.hash, e2.hash, "canonical JSON must sort keys");
    }

    #[test]
    fn hash_hex_round_trips() {
        let h = EventHash::from_bytes([0xab; 32]);
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        let h2: EventHash = hex.parse().unwrap();
        assert_eq!(h, h2);
    }
}
