//! Phase 1 — Effect System tests.

use std::sync::Arc;

use serde_json::json;
use sps_core::actor::Actor;
use sps_core::event_store::EventStore;
use sps_core::reducer::builtin::KernelMetaReducer;
use sps_core::reducer::{ReducerPipeline, ReducerRegistry};
use sps_core::storage::port::StoragePort;
use sps_effects::executors::{FsExecutor, SearchExecutor, ShellExecutor};
use sps_effects::providers::adapters::StaticAdapter;
use sps_effects::providers::llm::{LlmProvider, LlmRequest, ProviderConfig};
use sps_effects::providers::registry::ProviderRegistry;
use sps_effects::registry::EffectRegistry;
use sps_effects::{EffectManager, EffectType};
use sps_storage_memory::InMemoryStorage;

fn fresh_kernel_with_effects() -> (
    Arc<EventStore>,
    Arc<EffectRegistry>,
    Arc<ProviderRegistry>,
    Arc<EffectManager>,
) {
    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    let executors = Arc::new(EffectRegistry::new());
    let providers = Arc::new(ProviderRegistry::new());
    let manager = Arc::new(EffectManager::new(
        executors.clone(),
        providers.clone(),
        store.clone(),
    ));
    (store, executors, providers, manager)
}

#[test]
fn shell_exec_runs_real_command() {
    let (store, executors, _providers, manager) = fresh_kernel_with_effects();
    executors.register(
        "shell.exec",
        Arc::new(ShellExecutor::new(std::path::PathBuf::from("/tmp"))),
    );

    let (intent, result) = manager
        .dispatch(
            EffectType::ShellExec,
            json!({"command": "echo", "args": ["hello-phase-1"]}),
            &Actor::owner(),
            0,
        )
        .unwrap();

    assert_eq!(intent.event_type.as_str(), "effect.intent");
    assert_eq!(result.event_type.as_str(), "effect.executed");

    let output = &result.payload["output"];
    assert_eq!(output["exit_code"], 0);
    assert!(output["stdout"]
        .as_str()
        .unwrap()
        .contains("hello-phase-1"));
    assert_eq!(output["success"], true);

    // 2 events: intent + executed.
    assert_eq!(store.count().unwrap(), 2);
}

#[test]
fn fs_write_and_read_round_trip() {
    let (store, executors, _providers, manager) = fresh_kernel_with_effects();
    let tmp = tempfile::tempdir().unwrap();
    executors.register(
        "fs.write",
        Arc::new(FsExecutor::new(tmp.path().to_path_buf())),
    );
    executors.register(
        "fs.read",
        Arc::new(FsExecutor::new(tmp.path().to_path_buf())),
    );

    // Write
    let (_i, w) = manager
        .dispatch(
            EffectType::FsWrite,
            json!({"path": "test.txt", "content": "hello fs"}),
            &Actor::owner(),
            0,
        )
        .unwrap();
    assert_eq!(w.payload["output"]["bytes_written"], 8);

    // Read
    let (_i, r) = manager
        .dispatch(
            EffectType::FsRead,
            json!({"path": "test.txt"}),
            &Actor::owner(),
            0,
        )
        .unwrap();
    assert_eq!(r.payload["output"]["content"], "hello fs");
    assert_eq!(r.payload["output"]["size"], 8);

    assert_eq!(store.count().unwrap(), 4);
}

#[test]
fn fs_rejects_path_outside_workspace() {
    let (_store, executors, _providers, manager) = fresh_kernel_with_effects();
    let tmp = tempfile::tempdir().unwrap();
    executors.register(
        "fs.write",
        Arc::new(FsExecutor::new(tmp.path().to_path_buf())),
    );

    let (_i, result) = manager
        .dispatch(
            EffectType::FsWrite,
            json!({"path": "/etc/passwd", "content": "evil"}),
            &Actor::owner(),
            0,
        )
        .unwrap();
    // Should fail with governance denial (path escapes root).
    assert_eq!(result.event_type.as_str(), "effect.failed");
    let err_val = &result.payload["error"];
    // The error payload is serialized as EffectError enum variant — grab the
    // message field wherever it lives in the serialized form.
    let err_json = serde_json::to_string(err_val).unwrap();
    assert!(
        err_json.contains("escapes") || err_json.contains("canonicalize"),
        "expected governance denial, got: {}",
        err_json
    );
}

#[test]
fn llm_complete_via_static_provider() {
    let (_store, _executors, providers, manager) = fresh_kernel_with_effects();
    let static_provider = Arc::new(StaticAdapter::new("test-provider", " canned LLM response "));
    let config = ProviderConfig {
        id: "test-provider".into(),
        name: "Test Provider".into(),
        api_url: "http://localhost".into(),
        api_key: None,
        model_name: "static-model".into(),
        metadata: Default::default(),
    };
    providers.register(config, static_provider);

    let request = LlmRequest {
        provider_id: "test-provider".into(),
        model: None,
        system: Some("You are a test.".into()),
        user: "Say hello.".into(),
        max_tokens: Some(100),
        temperature: Some(0.7),
    };

    let (intent, result) = manager
        .dispatch(
            EffectType::LlmComplete,
            serde_json::to_value(&request).unwrap(),
            &Actor::owner(),
            0,
        )
        .unwrap();

    assert_eq!(intent.event_type.as_str(), "effect.intent");
    assert_eq!(result.event_type.as_str(), "effect.executed");
    assert_eq!(
        result.payload["output"]["text"],
        " canned LLM response "
    );
}

#[test]
fn llm_complete_fails_without_provider() {
    let (_store, _executors, _providers, manager) = fresh_kernel_with_effects();

    let request = LlmRequest {
        provider_id: "nonexistent".into(),
        model: None,
        system: None,
        user: "hi".into(),
        max_tokens: None,
        temperature: None,
    };

    let (_i, result) = manager
        .dispatch(
            EffectType::LlmComplete,
            serde_json::to_value(&request).unwrap(),
            &Actor::owner(),
            0,
        )
        .unwrap();

    assert_eq!(result.event_type.as_str(), "effect.failed");
    let err_json = serde_json::to_string(&result.payload["error"]).unwrap();
    assert!(err_json.contains("NoProvider"), "got: {}", err_json);
}

#[test]
fn search_returns_matching_documents() {
    let (_store, executors, _providers, manager) = fresh_kernel_with_effects();
    executors.register("search.query", Arc::new(SearchExecutor::new()));

    let (_i, r) = manager
        .dispatch(
            EffectType::SearchQuery,
            json!({
                "query": "rust",
                "documents": [
                    "Rust is a systems language",
                    "Python is dynamic",
                    "rust has ownership"
                ],
                "limit": 10
            }),
            &Actor::owner(),
            0,
        )
        .unwrap();

    assert_eq!(r.event_type.as_str(), "effect.executed");
    let hits = r.payload["output"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2); // "Rust is a systems language" + "rust has ownership"
}

#[test]
fn effect_results_are_deterministic_on_replay() {
    // After execution, the recorded effect.executed event contains the
    // exact output. Replay must reproduce it byte-for-byte.
    use sps_core::replay::ReplayEngine;

    let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
    let store = Arc::new(EventStore::new(storage.clone()).unwrap());

    let executors = Arc::new(EffectRegistry::new());
    executors.register(
        "shell.exec",
        Arc::new(ShellExecutor::new(std::path::PathBuf::from("/tmp"))),
    );
    let providers = Arc::new(ProviderRegistry::new());
    let manager = Arc::new(EffectManager::new(
        executors.clone(),
        providers.clone(),
        store.clone(),
    ));

    // Dispatch a shell command.
    let (_i, result) = manager
        .dispatch(
            EffectType::ShellExec,
            json!({"command": "echo", "args": ["replay-test"]}),
            &Actor::owner(),
            1234,
        )
        .unwrap();

    // Now replay from genesis. The recorded effect.executed event's
    // payload (including the stdout "replay-test") must be reproduced
    // exactly — the executor is NOT re-invoked.
    let pipeline = {
        let mut reg = ReducerRegistry::new();
        for et in &[
            "effect.intent",
            "effect.executed",
            "effect.failed",
        ] {
            reg.register(*et, KernelMetaReducer::shared());
        }
        Arc::new(ReducerPipeline::new(Arc::new(reg)))
    };

    let engine = ReplayEngine::new(pipeline);
    let state = engine.replay_from_genesis(storage.as_ref()).unwrap();

    // State must reflect 2 applied events.
    assert_eq!(state.event_count(), 2);
    assert_eq!(state.last_tick(), 2);
    assert_eq!(state.last_hash(), result.hash);
}

#[test]
fn unknown_effect_type_records_failure() {
    let (_store, executors, _providers, manager) = fresh_kernel_with_effects();
    // No executors registered.

    let (_i, result) = manager
        .dispatch(
            EffectType::ShellExec,
            json!({"command": "echo", "args": ["x"]}),
            &Actor::owner(),
            0,
        )
        .unwrap();

    assert_eq!(result.event_type.as_str(), "effect.failed");
    let err_json = serde_json::to_string(&result.payload["error"]).unwrap();
    assert!(err_json.contains("NoExecutor"), "got: {}", err_json);
    assert!(err_json.contains("shell.exec"), "got: {}", err_json);
}
