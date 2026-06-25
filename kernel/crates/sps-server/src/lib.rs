//! SPS HTTP API server.
//!
//! Exposes the kernel to the web UI via a REST + SSE API.
//!
//! # Endpoints
//!
//! - `GET  /api/health` — healthcheck
//! - `GET  /api/state` — full canonical state as JSON
//! - `GET  /api/stats` — kernel statistics
//! - `GET  /api/events?limit=N` — recent events
//! - `GET  /api/events/stream` — SSE stream of new events
//! - `POST /api/dispatch` — dispatch a raw event
//! - `GET  /api/verify` — verify hash chain
//! - `POST /api/snapshot` — take a snapshot
//! - `GET  /api/memory` — memory stats
//! - `GET  /api/memory/search?q=...` — search memories
//! - `GET  /api/agents` — list agents
//! - `POST /api/agents/dispatch` — dispatch to an agent
//! - `GET  /api/goals` — list goals
//! - `GET  /api/providers` — list configured providers
//! - `POST /api/providers` — register a new provider (URL + key + model)
//! - `DELETE /api/providers/:id` — remove a provider
//! - `POST /api/providers/:id/healthcheck` — healthcheck a provider
//! - `POST /api/llm/complete` — direct LLM completion (uses default provider)

#![allow(clippy::module_name_repetitions)]

pub mod routes;
pub mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use parking_lot::RwLock;
use sps_core::kernel::{KernelConfig, SpsKernel};
use sps_core::storage::port::StoragePort;
use sps_effects::providers::registry::ProviderRegistry;
use sps_effects::registry::EffectRegistry;
use sps_storage_sqlite::SqliteStorage;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use state::ServerState;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Database path (SQLite). Use `:memory:` for in-memory.
    pub db_path: PathBuf,
    /// HTTP listen address.
    pub listen_addr: SocketAddr,
    /// Static web UI directory (optional — serve the Next.js build).
    pub web_dir: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("~/.sps/sps.db"),
            listen_addr: "127.0.0.1:7780".parse().unwrap(),
            web_dir: None,
        }
    }
}

/// Boot the kernel against the configured storage.
///
/// P3D: registers a TypedExtensionRegistry so snapshots can rebuild
/// typed extensions on load.
pub fn boot_kernel(db_path: &PathBuf) -> Result<Arc<SpsKernel>> {
    let storage: Arc<dyn StoragePort> = if db_path.to_string_lossy() == ":memory:" {
        Arc::new(sps_storage_memory::InMemoryStorage::new())
    } else {
        let expanded = expand_home(db_path);
        if let Some(parent) = expanded.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        Arc::new(SqliteStorage::open(&expanded).context("failed to open SQLite storage")?)
    };
    let mut typed_reg = sps_core::state::TypedExtensionRegistry::new();
    register_all_typed_extensions(&mut typed_reg);
    let config = KernelConfig::default().with_typed_registry(typed_reg);
    let kernel = SpsKernel::boot_with(storage, config, |reg| {
        register_all_domain_reducers(reg);
    })
    .context("kernel boot failed")?;
    Ok(Arc::new(kernel))
}

/// Register every domain reducer from every crate.
pub fn register_all_domain_reducers(reg: &mut sps_core::reducer::ReducerRegistry) {
    sps_bus::state_ext::OwnerReducer::register(reg);
    sps_goals::reducer::GoalReducer::register(reg);
    sps_memory::reducer::MemoryReducer::register(reg);
    sps_agents::reducer::AgentReducer::register(reg);
    sps_planner::reducer::PlannerReducer::register(reg);
    sps_world::reducer::WorldReducer::register(reg);
    sps_reflection::reducer::ReflectionReducer::register(reg);
    sps_reasoning::reducer::ReasoningReducer::register(reg);
    sps_improvement::reducer::ImprovementReducer::register(reg);
    sps_execution::reducer::ExecutionReducer::register(reg);
    sps_factory::reducer::FactoryReducer::register(reg);
    sps_autonomy::reducer::AutonomyReducer::register(reg);
    sps_vectors::reducer::VectorReducer::register(reg);
}

/// P3D: Register typed-extension constructors from every domain reducer
/// that uses typed_extensions.
pub fn register_all_typed_extensions(reg: &mut sps_core::state::TypedExtensionRegistry) {
    sps_memory::reducer::MemoryReducer::register_typed_extensions(reg);
    sps_goals::reducer::GoalReducer::register_typed_extensions(reg);
    sps_execution::reducer::ExecutionReducer::register_typed_extensions(reg);
    sps_reflection::reducer::ReflectionReducer::register_typed_extensions(reg);
    sps_planner::reducer::PlannerReducer::register_typed_extensions(reg);
    sps_world::reducer::WorldReducer::register_typed_extensions(reg);
    sps_agents::reducer::AgentReducer::register_typed_extensions(reg);
    sps_autonomy::reducer::AutonomyReducer::register_typed_extensions(reg);
    sps_factory::reducer::FactoryReducer::register_typed_extensions(reg);
}

fn expand_home(path: &PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(&s[2..]);
        }
    }
    path.clone()
}

/// Build the Axum router with all routes.
pub fn build_router(state: Arc<ServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    Router::new()
        .merge(routes::health::router())
        .merge(routes::state::router())
        .merge(routes::events::router())
        .merge(routes::memory::router())
        .merge(routes::agents::router())
        .merge(routes::goals::router())
        .merge(routes::providers::router())
        .merge(routes::llm::router())
        .merge(routes::conversations::router())
        .merge(routes::stream::router())
        .merge(routes::code::router())
        .merge(routes::workspace::router())
        .merge(routes::git::router())
        .merge(routes::inline::router())
        .merge(routes::companion::router())
        // Serve the embedded web UI at /.
        .route(
            "/",
            axum::routing::get(|| async {
                axum::response::Html(include_str!("../static/index.html"))
            }),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Run the server.
pub async fn run(config: ServerConfig) -> Result<()> {
    let kernel = boot_kernel(&config.db_path)?;

    // Fix #8: Create a persistent AgentRuntime wired to the kernel.
    let agent_runtime = Arc::new(sps_agents::runtime::AgentRuntime::with_sink(
        sps_agents::runtime::AgentRuntimeConfig::default(),
        kernel.clone(),
    ));
    agent_runtime.register_builtins();

    // Fix #2 / E3: Create a persistent AutonomyGovernor + LongRunningGoalRunner
    // wired to the kernel via EventSink.
    let autonomy_governor = Arc::new(sps_autonomy::governor::AutonomyGovernor::new());
    autonomy_governor.enable();
    let goal_runner = Arc::new(sps_autonomy::governor::LongRunningGoalRunner::new(
        autonomy_governor.clone(),
    ));
    goal_runner.set_max_concurrent(10);

    let server_state = Arc::new(ServerState {
        kernel: kernel.clone(),
        providers: Arc::new(ProviderRegistry::new()),
        executors: Arc::new(EffectRegistry::new()),
        default_provider: RwLock::new(None),
        conversations: parking_lot::RwLock::new(std::collections::BTreeMap::new()),
        code_index: Arc::new(sps_code_intel::CodebaseIndex::new()),
        workspace_root: parking_lot::RwLock::new(config.web_dir.clone()),
        agent_runtime,
        autonomy_governor,
        goal_runner,
    });

    let app = build_router(server_state);
    let listener = tokio::net::TcpListener::bind(config.listen_addr)
        .await
        .context("failed to bind")?;
    tracing::info!("SPS server listening on http://{}", config.listen_addr);
    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
