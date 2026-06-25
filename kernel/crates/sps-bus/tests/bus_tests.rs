//! Phase 2 — Command Bus + Event Bus + Owner reducer tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use smol_str::SmolStr;
use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::event_store::EventStore;
use sps_core::reducer::builtin::KernelMetaReducer;
use sps_core::reducer::{Reducer, ReducerPipeline, ReducerRegistry};
use sps_core::storage::port::StoragePort;
use sps_bus::event_bus::EventBus;
use sps_bus::state_ext::{OwnerReducer, OwnerState};
use sps_bus::{Command, CommandBus, CommandHandler, CommandRegistry};
use sps_storage_memory::InMemoryStorage;

fn fresh_store() -> (Arc<EventStore>, Arc<dyn StoragePort>) {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());
    (store, storage)
}

// --- Owner reducer tests ---

#[test]
fn owner_profile_created_event_updates_state() {
    let mut state = sps_core::state::CanonicalState::genesis();
    let reducer = OwnerReducer;
    let event = sps_core::event::RawEvent::new(
        "owner.profile_created",
        json!({"display_name": "Zaid", "created_at": 12345}),
        Actor::owner(),
        0,
    )
    .finalize(1, sps_core::event::EventHash::GENESIS);

    reducer.reduce(&mut state, &event).unwrap();
    let owner = OwnerState::from_state(&state).unwrap();
    assert_eq!(owner.profile.display_name, "Zaid");
    assert_eq!(owner.profile.created_at, 12345);
    assert!(!owner.profile.has_password);
}

#[test]
fn owner_password_events_round_trip() {
    let mut state = sps_core::state::CanonicalState::genesis();
    let reducer = OwnerReducer;

    let e1 = RawEvent::new("owner.password_set", json!({}), Actor::owner(), 0)
        .finalize(1, sps_core::event::EventHash::GENESIS);
    reducer.reduce(&mut state, &e1).unwrap();
    let owner = OwnerState::from_state(&state).unwrap();
    assert!(owner.profile.has_password);

    let e2 = RawEvent::new("owner.password_cleared", json!({}), Actor::owner(), 0)
        .finalize(2, e1.hash);
    reducer.reduce(&mut state, &e2).unwrap();
    let owner = OwnerState::from_state(&state).unwrap();
    assert!(!owner.profile.has_password);
}

#[test]
fn owner_autonomy_toggled_event() {
    let mut state = sps_core::state::CanonicalState::genesis();
    let reducer = OwnerReducer;

    let e1 = RawEvent::new("owner.autonomy_toggled", json!({"enabled": true}), Actor::owner(), 0)
        .finalize(1, sps_core::event::EventHash::GENESIS);
    reducer.reduce(&mut state, &e1).unwrap();
    let owner = OwnerState::from_state(&state).unwrap();
    assert!(owner.profile.preferences.autonomy_enabled);

    let e2 = RawEvent::new("owner.autonomy_toggled", json!({"enabled": false}), Actor::owner(), 0)
        .finalize(2, e1.hash);
    reducer.reduce(&mut state, &e2).unwrap();
    let owner = OwnerState::from_state(&state).unwrap();
    assert!(!owner.profile.preferences.autonomy_enabled);
}

// --- Command Bus tests ---

struct EchoCommandHandler;

impl CommandHandler for EchoCommandHandler {
    fn command_type(&self) -> &str {
        "test.echo"
    }

    fn handle(
        &self,
        command: &Command,
        store: &EventStore,
        actor: &Actor,
        wall_time: u64,
    ) -> sps_core::CoreResult<Vec<u64>> {
        let raw = RawEvent::new(
            "test.echoed",
            json!({"echo": command.payload}),
            actor.clone(),
            wall_time,
        );
        let event = store.append(raw)?;
        Ok(vec![event.tick])
    }
}

#[test]
fn command_bus_dispatches_to_handler() {
    let (store, _storage) = fresh_store();
    let registry = Arc::new(CommandRegistry::new());
    registry.register(Arc::new(EchoCommandHandler));
    let bus = CommandBus::new(registry, store.clone());

    let cmd = Command::new("test.echo", json!({"msg": "hello"}));
    let ticks = bus.dispatch(&cmd).unwrap();
    assert_eq!(ticks.len(), 1);
    assert_eq!(ticks[0], 1);

    let event = store.read_by_tick(1).unwrap().unwrap();
    assert_eq!(event.event_type.as_str(), "test.echoed");
    assert_eq!(event.payload["echo"]["msg"], "hello");
}

#[test]
fn command_bus_fails_for_unknown_command() {
    let (store, _storage) = fresh_store();
    let registry = Arc::new(CommandRegistry::new());
    let bus = CommandBus::new(registry, store);

    let cmd = Command::new("unknown.command", json!({}));
    let err = bus.dispatch(&cmd).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("no handler for command type 'unknown.command'"));
}

// --- Event Bus tests ---

#[test]
fn event_bus_delivers_events_to_subscribers() {
    let (store, storage) = fresh_store();
    let bus = EventBus::new();

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();
    bus.subscribe(None, Arc::new(move |_event| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    }));

    // Append 3 events.
    for i in 1..=3u64 {
        let raw = RawEvent::new(
            "test.event",
            json!({"i": i}),
            Actor::owner(),
            i * 100,
        );
        store.append(raw).unwrap();
    }

    let dispatched = bus.poll(storage.as_ref()).unwrap();
    assert_eq!(dispatched, 3);
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[test]
fn event_bus_filters_by_event_type() {
    let (store, storage) = fresh_store();
    let bus = EventBus::new();

    let alpha_count = Arc::new(AtomicUsize::new(0));
    let alpha_clone = alpha_count.clone();
    bus.subscribe(
        Some("test.alpha".to_string()),
        Arc::new(move |_| {
            alpha_clone.fetch_add(1, Ordering::SeqCst);
        }),
    );

    let beta_count = Arc::new(AtomicUsize::new(0));
    let beta_clone = beta_count.clone();
    bus.subscribe(
        Some("test.beta".to_string()),
        Arc::new(move |_| {
            beta_clone.fetch_add(1, Ordering::SeqCst);
        }),
    );

    for i in 1..=4u64 {
        let et = if i % 2 == 0 { "test.alpha" } else { "test.beta" };
        let raw = RawEvent::new(et, json!({"i": i}), Actor::owner(), i * 100);
        store.append(raw).unwrap();
    }

    bus.poll(storage.as_ref()).unwrap();
    assert_eq!(alpha_count.load(Ordering::SeqCst), 2); // ticks 2, 4
    assert_eq!(beta_count.load(Ordering::SeqCst), 2); // ticks 1, 3
}

#[test]
fn event_bus_unsubscribe_works() {
    let (store, storage) = fresh_store();
    let bus = EventBus::new();

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();
    let id = bus.subscribe(None, Arc::new(move |_| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    }));

    assert_eq!(bus.subscription_count(), 1);
    assert!(bus.unsubscribe(id));
    assert_eq!(bus.subscription_count(), 0);

    let raw = RawEvent::new("test.event", json!({}), Actor::owner(), 0);
    store.append(raw).unwrap();
    bus.poll(storage.as_ref()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn event_bus_polls_only_new_events() {
    let (store, storage) = fresh_store();
    let bus = EventBus::new();

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();
    bus.subscribe(None, Arc::new(move |_| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    }));

    // First batch.
    for i in 1..=3u64 {
        let raw = RawEvent::new("test.event", json!({"i": i}), Actor::owner(), i * 100);
        store.append(raw).unwrap();
    }
    bus.poll(storage.as_ref()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 3);

    // Second batch.
    for i in 4..=5u64 {
        let raw = RawEvent::new("test.event", json!({"i": i}), Actor::owner(), i * 100);
        store.append(raw).unwrap();
    }
    bus.poll(storage.as_ref()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 5);

    // No new events.
    bus.poll(storage.as_ref()).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 5);
}

// --- Integration: CommandBus → EventStore → EventBus → ReducerPipeline ---

#[test]
fn full_pipeline_command_to_event_to_subscriber() {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    // Set up reducer pipeline with owner + kernel meta reducers.
    let mut reg = ReducerRegistry::new();
    reg.register("test.echoed", KernelMetaReducer::shared());
    OwnerReducer::register(&mut reg);
    let pipeline = Arc::new(ReducerPipeline::new(Arc::new(reg)));

    // Command bus with echo handler.
    let cmd_registry = Arc::new(CommandRegistry::new());
    cmd_registry.register(Arc::new(EchoCommandHandler));
    let cmd_bus = CommandBus::new(cmd_registry, store.clone());

    // Event bus with subscriber that runs the reducer pipeline.
    let state = Arc::new(parking_lot::RwLock::new(
        sps_core::state::CanonicalState::genesis(),
    ));
    let state_clone = state.clone();
    let pipeline_clone = pipeline.clone();
    let event_bus = EventBus::new();
    event_bus.subscribe(None, Arc::new(move |event| {
        let mut s = state_clone.write();
        let _ = pipeline_clone.apply(&mut s, event);
    }));

    // Dispatch a command.
    let cmd = Command::new("test.echo", json!({"msg": "pipeline test"}));
    let ticks = cmd_bus.dispatch(&cmd).unwrap();
    assert_eq!(ticks.len(), 1);

    // Poll the event bus — should pick up the event and apply the reducer.
    let dispatched = event_bus.poll(storage.as_ref()).unwrap();
    assert_eq!(dispatched, 1);

    // Verify state was updated.
    let s = state.read();
    assert_eq!(s.last_tick(), 1);
    assert_eq!(s.event_count(), 1);
}
