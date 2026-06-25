//! Deterministic replay integration tests.
//!
//! These are the most important tests in the entire SPS Kernel. They
//! verify the foundational invariant: **identical event streams must
//! produce identical canonical state**.
//!
//! If any of these tests fail, the kernel is broken — do not ship.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::builtin::KernelMetaReducer;
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::snapshot::Snapshot;
use sps_core::storage::port::StoragePort;
use sps_core::KERNEL_SCHEMA_VERSION;

fn build_pipeline() -> Arc<ReducerPipeline> {
    let mut reg = ReducerRegistry::new();
    for et in &[
        "system.booted",
        "system.snapshot_taken",
        "system.replay_verified",
        "test.alpha",
        "test.beta",
    ] {
        reg.register(*et, KernelMetaReducer::shared());
    }
    Arc::new(ReducerPipeline::new(Arc::new(reg)))
}

fn make_event(tick: u64, prev: EventHash, et: &str, payload: serde_json::Value, wall: u64) -> sps_core::event::Event {
    RawEvent::new(et, payload, Actor::owner(), wall)
        .finalize(tick, prev)
}

// =============================== InMemory ===============================

mod in_memory {
    use super::*;
    use sps_storage_memory::InMemoryStorage;

    fn fresh_storage() -> Arc<dyn StoragePort> {
        Arc::new(InMemoryStorage::new())
    }

    #[test]
    fn empty_store_verifies_and_replays_to_genesis() {
        let storage = fresh_storage();
        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none());
        assert_eq!(report.events_verified, 0);
        assert_eq!(report.last_tick, 0);
        assert_eq!(report.last_hash, EventHash::GENESIS);

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        assert_eq!(state.last_tick(), 0);
        assert_eq!(state.last_hash(), EventHash::GENESIS);
        assert_eq!(state.event_count(), 0);
    }

    #[test]
    fn append_and_replay_single_event() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({"v": KERNEL_SCHEMA_VERSION}), 1_000);
        storage.append_event(&e1).unwrap();

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none());
        assert_eq!(report.events_verified, 1);
        assert_eq!(report.last_tick, 1);
        assert_eq!(report.last_hash, e1.hash);

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        assert_eq!(state.last_tick(), 1);
        assert_eq!(state.last_hash(), e1.hash);
        assert_eq!(state.event_count(), 1);
    }

    #[test]
    fn append_chain_of_events_and_replay() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        let e2 = make_event(2, e1.hash, "test.alpha", json!({"k": "v1"}), 2);
        let e3 = make_event(3, e2.hash, "test.beta", json!({"k": "v2"}), 3);
        for e in [&e1, &e2, &e3] {
            storage.append_event(e).unwrap();
        }

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none());
        assert_eq!(report.events_verified, 3);

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        assert_eq!(state.last_tick(), 3);
        assert_eq!(state.last_hash(), e3.hash);
        assert_eq!(state.event_count(), 3);
    }

    #[test]
    fn deterministic_replay_identical_state_across_runs() {
        // The cornerstone test: build the same stream twice, replay
        // both, assert byte-identical state.
        let storage1 = fresh_storage();
        let storage2 = fresh_storage();

        let events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
            (1, "system.booted", json!({}), 100),
            (2, "test.alpha", json!({"x": 1, "y": 2}), 200),
            (3, "test.beta", json!({"z": [1, 2, 3]}), 300),
            (4, "test.alpha", json!({"nested": {"a": 1, "b": 2}}), 400),
            (5, "test.beta", json!({"list": [1, 2, {"k": "v"}]}), 500),
        ];

        let mut prev = EventHash::GENESIS;
        for (tick, et, payload, wall) in &events {
            let e = make_event(*tick, prev, et, payload.clone(), *wall);
            storage1.append_event(&e).unwrap();
            storage2.append_event(&e).unwrap();
            prev = e.hash;
        }

        let engine = ReplayEngine::new(build_pipeline());
        let state1 = engine.replay_from_genesis(storage1.as_ref()).unwrap();
        let state2 = engine.replay_from_genesis(storage2.as_ref()).unwrap();

        assert_eq!(state1, state2, "replay must be deterministic");
        assert_eq!(state1.last_tick(), 5);
        assert_eq!(state1.event_count(), 5);
    }

    #[test]
    fn wall_time_does_not_affect_hash() {
        let payloads = vec![json!({"k": "v"})];
        for (i, p) in payloads.iter().enumerate() {
            let tick = (i + 1) as u64;
            let e_a = make_event(tick, EventHash::GENESIS, "test.alpha", p.clone(), 1_000_000);
            let e_b = make_event(tick, EventHash::GENESIS, "test.alpha", p.clone(), 9_999_999);
            // Both must hash identically despite different wall times.
            assert_eq!(e_a.hash, e_b.hash, "wall_time must not affect hash");
        }
    }

    #[test]
    fn payload_key_order_does_not_affect_hash() {
        let p1: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":2,"c":3}"#).unwrap();
        let p2: serde_json::Value = serde_json::from_str(r#"{"c":3,"b":2,"a":1}"#).unwrap();
        let e1 = make_event(1, EventHash::GENESIS, "test.alpha", p1, 0);
        let e2 = make_event(1, EventHash::GENESIS, "test.alpha", p2, 0);
        assert_eq!(e1.hash, e2.hash, "canonical JSON must sort keys before hashing");
    }

    #[test]
    fn broken_chain_detected() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        let e2 = make_event(2, e1.hash, "test.alpha", json!({}), 2);
        storage.append_event(&e1).unwrap();
        storage.append_event(&e2).unwrap();

        // Corrupt: append e3 with a wrong prev_hash.
        let wrong_prev = EventHash::from_bytes([0xff; 32]);
        let e3 = make_event(3, wrong_prev, "test.beta", json!({}), 3);
        storage.append_event(&e3).unwrap();

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_some());
        match report.failure {
            Some(sps_core::replay::ReplayFailure::HashChainBroken { tick, .. }) => {
                assert_eq!(tick, 3);
            }
            other => panic!("expected HashChainBroken, got {:?}", other),
        }
    }

    #[test]
    fn tampered_hash_detected() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        // Tamper: change the stored hash to a wrong value.
        let mut tampered = e1.clone();
        tampered.hash = EventHash::from_bytes([0xee; 32]);
        // InMemoryStorage doesn't re-verify on append, so we can write it.
        storage.append_event(&tampered).unwrap();

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_some());
        match report.failure {
            Some(sps_core::replay::ReplayFailure::HashMismatch { tick, .. }) => {
                assert_eq!(tick, 1);
            }
            other => panic!("expected HashMismatch, got {:?}", other),
        }
    }

    #[test]
    fn snapshot_round_trips_and_speeds_up_replay() {
        let storage = fresh_storage();

        // Append 5 events.
        let mut prev = EventHash::GENESIS;
        for i in 1..=5 {
            let e = make_event(i, prev, "test.alpha", json!({"i": i}), i * 100);
            storage.append_event(&e).unwrap();
            prev = e.hash;
        }

        // Replay to get state at tick 5.
        let engine = ReplayEngine::new(build_pipeline());
        let state_at_5 = engine.replay_from_genesis(storage.as_ref()).unwrap();

        // Take a snapshot at tick 5.
        let snap = Snapshot::take(&state_at_5, 999).unwrap();
        storage.write_snapshot(&snap).unwrap();

        // Append 5 more events.
        for i in 6..=10 {
            let e = make_event(i, prev, "test.alpha", json!({"i": i}), i * 100);
            storage.append_event(&e).unwrap();
            prev = e.hash;
        }

        // Replay from snapshot — should reach tick 10 with same state
        // as a full replay.
        let loaded_snap = storage.read_latest_snapshot().unwrap().unwrap();
        let state_from_snap = engine.replay_from_snapshot(storage.as_ref(), &loaded_snap).unwrap();
        let state_full = engine.replay_from_genesis(storage.as_ref()).unwrap();

        assert_eq!(state_from_snap, state_full, "snapshot + tail must equal full replay");
        assert_eq!(state_from_snap.last_tick(), 10);
        assert_eq!(state_from_snap.event_count(), 10);
    }

    #[test]
    fn non_monotonic_tick_rejected() {
        let storage = fresh_storage();
        let e1 = make_event(5, EventHash::GENESIS, "test.alpha", json!({}), 1);
        storage.append_event(&e1).unwrap();
        let e2 = make_event(3, e1.hash, "test.alpha", json!({}), 2);
        let err = storage.append_event(&e2).unwrap_err();
        match err {
            sps_core::CoreError::NonMonotonicTick { prev, curr } => {
                assert_eq!(prev, 5);
                assert_eq!(curr, 3);
            }
            other => panic!("expected NonMonotonicTick, got {:?}", other),
        }
    }

    #[test]
    fn unknown_event_type_fails_replay() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "totally.unknown", json!({}), 1);
        storage.append_event(&e1).unwrap();

        let engine = ReplayEngine::new(build_pipeline());
        let err = engine.replay_from_genesis(storage.as_ref()).unwrap_err();
        match err {
            sps_core::CoreError::UnknownEventType(t) => {
                assert_eq!(t, "totally.unknown");
            }
            other => panic!("expected UnknownEventType, got {:?}", other),
        }
    }
}

// =============================== SQLite ===============================

mod sqlite {
    use super::*;
    use sps_storage_sqlite::SqliteStorage;

    fn fresh_storage() -> Arc<dyn StoragePort> {
        Arc::new(SqliteStorage::open_in_memory().unwrap())
    }

    #[test]
    fn empty_store_verifies_and_replays_to_genesis() {
        let storage = fresh_storage();
        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none());
        assert_eq!(report.events_verified, 0);

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        assert_eq!(state.last_tick(), 0);
        assert_eq!(state.event_count(), 0);
    }

    #[test]
    fn append_chain_and_replay() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        let e2 = make_event(2, e1.hash, "test.alpha", json!({"k": "v"}), 2);
        let e3 = make_event(3, e2.hash, "test.beta", json!({"k": "v2"}), 3);
        for e in [&e1, &e2, &e3] {
            storage.append_event(e).unwrap();
        }

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_none());
        assert_eq!(report.events_verified, 3);

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        assert_eq!(state.last_tick(), 3);
        assert_eq!(state.event_count(), 3);
    }

    #[test]
    fn deterministic_replay_identical_state_across_runs() {
        let storage1 = fresh_storage();
        let storage2 = fresh_storage();

        let events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
            (1, "system.booted", json!({}), 100),
            (2, "test.alpha", json!({"x": 1, "y": 2}), 200),
            (3, "test.beta", json!({"z": [1, 2, 3]}), 300),
            (4, "test.alpha", json!({"nested": {"a": 1, "b": 2}}), 400),
            (5, "test.beta", json!({"list": [1, 2, {"k": "v"}]}), 500),
        ];

        let mut prev = EventHash::GENESIS;
        for (tick, et, payload, wall) in &events {
            let e = make_event(*tick, prev, et, payload.clone(), *wall);
            storage1.append_event(&e).unwrap();
            storage2.append_event(&e).unwrap();
            prev = e.hash;
        }

        let engine = ReplayEngine::new(build_pipeline());
        let state1 = engine.replay_from_genesis(storage1.as_ref()).unwrap();
        let state2 = engine.replay_from_genesis(storage2.as_ref()).unwrap();

        assert_eq!(state1, state2, "replay must be deterministic across sqlite stores");
    }

    #[test]
    fn sqlite_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("sps.db");

        let events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
            (1, "system.booted", json!({}), 100),
            (2, "test.alpha", json!({"x": 1}), 200),
            (3, "test.beta", json!({"y": 2}), 300),
        ];

        // First open: append events.
        {
            let storage = Arc::new(SqliteStorage::open(&db_path).unwrap());
            let mut prev = EventHash::GENESIS;
            for (tick, et, payload, wall) in &events {
                let e = make_event(*tick, prev, et, payload.clone(), *wall);
                storage.append_event(&e).unwrap();
                prev = e.hash;
            }
        }

        // Second open: verify and replay.
        {
            let storage = Arc::new(SqliteStorage::open(&db_path).unwrap());
            let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
            assert!(report.failure.is_none());
            assert_eq!(report.events_verified, 3);

            let engine = ReplayEngine::new(build_pipeline());
            let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
            assert_eq!(state.last_tick(), 3);
            assert_eq!(state.event_count(), 3);
        }
    }

    #[test]
    fn snapshot_persists_and_loads() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        storage.append_event(&e1).unwrap();

        let engine = ReplayEngine::new(build_pipeline());
        let state = engine.replay_from_genesis(storage.as_ref()).unwrap();
        let snap = Snapshot::take(&state, 12345).unwrap();
        storage.write_snapshot(&snap).unwrap();

        let loaded = storage.read_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.tick, 1);
        assert_eq!(loaded.state_hash, snap.state_hash);
        loaded.verify().unwrap();
    }

    #[test]
    fn kv_round_trips() {
        let storage = fresh_storage();
        storage.write_kv("owner.name", b"Z").unwrap();
        let v = storage.read_kv("owner.name").unwrap().unwrap();
        assert_eq!(v, b"Z");
        let none = storage.read_kv("nonexistent").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn broken_chain_detected() {
        let storage = fresh_storage();
        let e1 = make_event(1, EventHash::GENESIS, "system.booted", json!({}), 1);
        let e2 = make_event(2, e1.hash, "test.alpha", json!({}), 2);
        storage.append_event(&e1).unwrap();
        storage.append_event(&e2).unwrap();

        let wrong_prev = EventHash::from_bytes([0xcc; 32]);
        let e3 = make_event(3, wrong_prev, "test.beta", json!({}), 3);
        storage.append_event(&e3).unwrap();

        let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
        assert!(report.failure.is_some());
    }
}

// =============================== Cross-backend ===============================

mod cross_backend {
    use super::*;
    use sps_storage_memory::InMemoryStorage;
    use sps_storage_sqlite::SqliteStorage;

    #[test]
    fn same_stream_produces_same_state_across_backends() {
        let mem = Arc::new(InMemoryStorage::new()) as Arc<dyn StoragePort>;
        let sql = Arc::new(SqliteStorage::open_in_memory().unwrap()) as Arc<dyn StoragePort>;

        let events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
            (1, "system.booted", json!({}), 1),
            (2, "test.alpha", json!({"x": 1}), 2),
            (3, "test.beta", json!({"y": 2}), 3),
            (4, "test.alpha", json!({"z": 3}), 4),
        ];

        let mut prev = EventHash::GENESIS;
        for (tick, et, payload, wall) in &events {
            let e = make_event(*tick, prev, et, payload.clone(), *wall);
            mem.append_event(&e).unwrap();
            sql.append_event(&e).unwrap();
            prev = e.hash;
        }

        let engine = ReplayEngine::new(build_pipeline());
        let state_mem = engine.replay_from_genesis(mem.as_ref()).unwrap();
        let state_sql = engine.replay_from_genesis(sql.as_ref()).unwrap();

        assert_eq!(state_mem, state_sql, "backends must produce identical state for identical stream");
    }
}
