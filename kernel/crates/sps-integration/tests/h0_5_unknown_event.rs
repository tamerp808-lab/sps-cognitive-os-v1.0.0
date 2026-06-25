//! H0.5: Unknown event type — verifies the always-on KernelMetaReducer
//! handles unknown event types gracefully (tracks tick/hash/count, no panic).

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::replay::ReplayVerifier;
use sps_core::storage::port::StoragePort;
use sps_storage_memory::InMemoryStorage;

fn boot_kernel(storage: Arc<dyn StoragePort>) -> Arc<SpsKernel> {
    SpsKernel::boot_with(storage, KernelConfig::default(), |reg| {
        sps_bus::state_ext::OwnerReducer::register(reg);
        sps_goals::reducer::GoalReducer::register(reg);
        sps_memory::reducer::MemoryReducer::register(reg);
    }).unwrap().into()
}

#[test]
fn h0_5_unknown_event_type_safe() {
    println!("\n=== H0.5: Unknown event type — safe handling ===");

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let kernel = boot_kernel(storage.clone());

    // Dispatch a known event first.
    let record = sps_memory::memory::MemoryRecord {
        id: sps_memory::memory::MemoryId(uuid::Uuid::now_v7()),
        kind: sps_memory::memory::MemoryKind::Episodic,
        title: smol_str::SmolStr::new("known"),
        content: json!({}),
        tags: vec![],
        origin_tick: 0,
        created_at: 0,
    };
    kernel.dispatch(RawEvent::new("memory.created", serde_json::to_value(&record).unwrap(), Actor::owner(), 0)).unwrap();
    println!("  Step 1: dispatched known event (memory.created)");

    // Dispatch an UNKNOWN event type.
    kernel.dispatch(RawEvent::new(
        "totally.unknown.event",
        json!({"data": "test"}),
        Actor::owner(),
        0,
    )).unwrap();
    println!("  Step 2: dispatched unknown event (totally.unknown.event)");

    // Verify store.count == 2.
    let store_count = kernel.store().count().unwrap_or(0);
    assert_eq!(store_count, 2, "FAIL: expected 2 events in store, got {}", store_count);
    println!("  PASS — store.count == 2");

    // Verify meta.event_count == 2.
    let meta_count = kernel.query(|s| s.event_count());
    assert_eq!(meta_count, 2, "FAIL: meta event_count == {}, expected 2", meta_count);
    println!("  PASS — meta.event_count == 2");

    // Verify last_tick updated.
    let last_tick = kernel.query(|s| s.last_tick());
    assert_eq!(last_tick, 2, "FAIL: last_tick == {}, expected 2", last_tick);
    println!("  PASS — last_tick == 2");

    // Verify hash chain valid.
    let report = ReplayVerifier::verify_chain(storage.as_ref()).unwrap();
    assert!(report.failure.is_none(), "FAIL: hash chain broken");
    assert_eq!(report.events_verified, 2);
    println!("  PASS — hash chain verified ({} events)", report.events_verified);

    // Verify last_hash is not genesis.
    let last_hash = kernel.query(|s| s.last_hash());
    assert_ne!(last_hash, sps_core::event::EventHash::GENESIS, "FAIL: last_hash is still genesis");
    println!("  PASS — last_hash != GENESIS (updated correctly)");

    println!("\n  === H0.5 PASSED ===");
    println!("  Unknown event types are tracked by KernelMetaReducer without panic.");
}
