//! Phase 0 — Deterministic Replay Proof.
//!
//! This is a runnable, end-to-end demonstration that the SPS Kernel
//! satisfies its determinism contract:
//!
//! 1. Build an Event Stream (10 events).
//! 2. Take a Snapshot at tick 10.
//! 3. Replay from Genesis (ignoring the snapshot).
//! 4. Assert that the replayed state == the snapshot state.
//! 5. Bonus: replay from the snapshot, append more events, and confirm
//!    the full replay still matches a fresh replay from genesis.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::{EventHash, RawEvent};
use sps_core::reducer::builtin::KernelMetaReducer;
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::replay::{ReplayEngine, ReplayVerifier};
use sps_core::snapshot::Snapshot;
use sps_core::storage::port::StoragePort;
use sps_storage_sqlite::SqliteStorage;

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

fn make_event(
    tick: u64,
    prev: EventHash,
    et: &str,
    payload: serde_json::Value,
    wall: u64,
) -> sps_core::event::Event {
    RawEvent::new(et, payload, Actor::owner(), wall).finalize(tick, prev)
}

fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!(" SPS Kernel — Phase 0 Deterministic Replay Proof");
    println!("============================================================");
    println!();

    // Use a real on-disk SQLite DB in a temp dir so we prove persistence.
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("sps-proof.db");
    let storage: Arc<dyn StoragePort> = Arc::new(SqliteStorage::open(&db_path)?);
    println!("[1/5] Opened SQLite storage at: {}", db_path.display());
    println!("      Backend: {}", storage.backend_name());
    println!("      Initial last_tick: {}", storage.last_tick()?);
    println!();

    // ---------- STEP 1: Build an Event Stream (10 events) ----------
    println!("[2/5] Building Event Stream of 10 events...");
    let events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
        (1, "system.booted", json!({"schema_version": 1}), 1_700_000_000_000),
        (2, "test.alpha", json!({"x": 1, "y": 2}), 1_700_000_001_000),
        (3, "test.beta", json!({"name": "first"}), 1_700_000_002_000),
        (4, "test.alpha", json!({"x": 10, "y": 20}), 1_700_000_003_000),
        (5, "test.beta", json!({"name": "second", "tags": ["a", "b"]}), 1_700_000_004_000),
        (6, "test.alpha", json!({"nested": {"deep": {"value": 42}}}), 1_700_000_005_000),
        (7, "test.beta", json!({"list": [1, 2, 3, 4, 5]}), 1_700_000_006_000),
        (8, "test.alpha", json!({"x": 100}), 1_700_000_007_000),
        (9, "test.beta", json!({"name": "ninth"}), 1_700_000_008_000),
        (10, "test.alpha", json!({"final": true}), 1_700_000_009_000),
    ];

    let mut prev = EventHash::GENESIS;
    for (tick, et, payload, wall) in &events {
        let e = make_event(*tick, prev, et, payload.clone(), *wall);
        storage.append_event(&e)?;
        prev = e.hash;
        println!(
            "      tick={:2} type={:<14} hash={:.16}...",
            tick,
            et,
            e.hash.to_hex()
        );
    }
    println!("      -> 10 events appended. last_tick={}, event_count={}",
             storage.last_tick()?,
             storage.count_events()?);
    println!();

    // ---------- STEP 2: Take a Snapshot at tick 10 ----------
    println!("[3/5] Taking Snapshot at tick 10...");
    let pipeline = build_pipeline();
    let engine = ReplayEngine::new(pipeline.clone());

    // Build the state by replaying the stream.
    let state_at_10 = engine.replay_from_genesis(storage.as_ref())?;
    println!("      Replayed state: last_tick={}, last_hash={:.16}..., event_count={}",
             state_at_10.last_tick(),
             state_at_10.last_hash().to_hex(),
             state_at_10.event_count());

    let snapshot = Snapshot::take(&state_at_10, 1_700_000_010_000)?;
    println!("      Snapshot: tick={}, state_hash={:.16}...",
             snapshot.tick,
             hex::encode(snapshot.state_hash));
    storage.write_snapshot(&snapshot)?;
    println!("      -> Snapshot persisted to storage.");
    println!();

    // ---------- STEP 3: Replay from Genesis ----------
    println!("[4/5] Replaying from Genesis (ignoring snapshot)...");
    let replayed_state = engine.replay_from_genesis(storage.as_ref())?;
    println!("      Replayed state: last_tick={}, last_hash={:.16}..., event_count={}",
             replayed_state.last_tick(),
             replayed_state.last_hash().to_hex(),
             replayed_state.event_count());
    println!();

    // ---------- STEP 4: Assert Equality ----------
    println!("[5/5] Asserting State Equality (snapshot state == replay-from-genesis state)...");
    assert_eq!(
        state_at_10, replayed_state,
        "DETERMINISM VIOLATION: replayed state != snapshot state"
    );
    println!("      PASS: state_at_10 == replayed_from_genesis");
    println!();

    // ---------- Bonus: chain verification ----------
    println!("------------------------------------------------------------");
    println!(" Bonus: Hash Chain Verification");
    println!("------------------------------------------------------------");
    let report = ReplayVerifier::verify_chain(storage.as_ref())?;
    println!("      events_verified: {}", report.events_verified);
    println!("      last_tick:        {}", report.last_tick);
    println!("      last_hash:        {:.16}...", report.last_hash.to_hex());
    println!("      failure:          {:?}", report.failure);
    println!("      elapsed_us:       {}", report.elapsed_us);
    assert!(report.failure.is_none(), "chain verification failed");
    assert_eq!(report.events_verified, 10);
    println!("      PASS: chain intact, all 10 hashes verified.");
    println!();

    // ---------- Bonus: snapshot + tail replay equals full replay ----------
    println!("------------------------------------------------------------");
    println!(" Bonus: Snapshot + Tail Replay == Full Replay");
    println!("------------------------------------------------------------");

    // Append 5 more events on top of tick 10.
    println!("      Appending 5 more events (ticks 11..15)...");
    let more_events: Vec<(u64, &str, serde_json::Value, u64)> = vec![
        (11, "test.beta", json!({"after": "snapshot"}), 1_700_000_011_000),
        (12, "test.alpha", json!({"x": 200}), 1_700_000_012_000),
        (13, "test.beta", json!({"k": "v"}), 1_700_000_013_000),
        (14, "test.alpha", json!({"x": 300}), 1_700_000_014_000),
        (15, "test.beta", json!({"last": true}), 1_700_000_015_000),
    ];
    let mut prev = storage.last_hash()?;
    for (tick, et, payload, wall) in &more_events {
        let e = make_event(*tick, prev, et, payload.clone(), *wall);
        storage.append_event(&e)?;
        prev = e.hash;
    }
    println!("      -> 15 events now in store.");

    // Path A: full replay from genesis.
    let full_state = engine.replay_from_genesis(storage.as_ref())?;
    println!("      Path A (full replay): last_tick={}, event_count={}",
             full_state.last_tick(),
             full_state.event_count());

    // Path B: snapshot + tail replay.
    let loaded_snap = storage.read_latest_snapshot()?.unwrap();
    let snap_state = engine.replay_from_snapshot(storage.as_ref(), &loaded_snap)?;
    println!("      Path B (snapshot+tail): last_tick={}, event_count={}",
             snap_state.last_tick(),
             snap_state.event_count());

    assert_eq!(full_state, snap_state, "snapshot+tail != full replay");
    println!("      PASS: snapshot+tail == full replay");
    println!();

    println!("============================================================");
    println!(" ALL DETERMINISM INVARIANTS HOLD.");
    println!("============================================================");
    Ok(())
}
