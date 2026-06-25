//! SPS CLI — entry point.
//!
//! Usage: `sps <command> [options]`
//!
//! Commands:
//!   verify      Verify the hash chain.
//!   replay      Replay from genesis and compare.
//!   snapshot    Take a snapshot.
//!   stats       Print kernel statistics.
//!   events      List recent events.
//!   provider    Manage LLM providers.
//!   memory      Memory subsystem commands.
//!   agent       Agent runtime commands.
//!   goal        Goal system commands.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sps_cli::{boot_kernel, default_db_path, open_storage};

#[derive(Parser, Debug)]
#[command(
    name = "sps",
    version,
    about = "SPS Cognitive Operating System CLI",
    long_about = "SPS is a personal AI Operating System. This CLI exposes the kernel's core functionality: event verification, replay, snapshots, memory search, agent dispatch, and more."
)]
struct Cli {
    /// Path to the SPS database file.
    #[arg(long, global = true, default_value = "~/.sps/sps.db")]
    db: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Verify the hash chain.
    Verify,
    /// Replay from genesis and compare with current state.
    Replay,
    /// Take a snapshot.
    Snapshot,
    /// Print kernel statistics.
    Stats,
    /// List recent events.
    Events {
        /// Maximum number of events to list.
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// Manage LLM providers.
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },
    /// Memory subsystem commands.
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Agent runtime commands.
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Goal system commands.
    Goal {
        #[command(subcommand)]
        action: GoalAction,
    },
}

#[derive(Subcommand, Debug)]
enum ProviderAction {
    /// List known provider kinds.
    List,
    /// Healthcheck a provider.
    Healthcheck {
        /// Provider kind (openai, openrouter, anthropic, ollama, ...).
        kind: String,
        /// API URL.
        #[arg(long)]
        url: String,
        /// API key (optional for local providers like Ollama).
        #[arg(long)]
        key: Option<String>,
        /// Model name.
        #[arg(long)]
        model: String,
    },
}

#[derive(Subcommand, Debug)]
enum MemoryAction {
    /// Print memory statistics.
    Stats,
    /// Search memories by keyword.
    Search {
        /// Search query.
        query: String,
        /// Maximum results.
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum AgentAction {
    /// List registered agents.
    List,
    /// Dispatch a task to an agent.
    Dispatch {
        /// Agent archetype (architect, developer, reviewer, tester, devops, researcher).
        archetype: String,
        /// Task title.
        title: String,
        /// Task description.
        description: String,
    },
}

#[derive(Subcommand, Debug)]
enum GoalAction {
    /// List goals.
    List,
    /// Verify a goal's completion.
    Verify {
        /// Goal id (UUID).
        goal_id: String,
    },
}

fn main() -> Result<()> {
    // Initialize tracing.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let cli = Cli::parse();

    // For commands that don't need the kernel (provider list), handle directly.
    if let Commands::Provider { action: ProviderAction::List } = &cli.command {
        println!("Known provider kinds:");
        for k in sps_cli::commands::provider::list_kinds() {
            println!("  {}", k);
        }
        return Ok(());
    }

    // For provider healthcheck, we don't need the full kernel — just the provider.
    if let Commands::Provider {
        action: ProviderAction::Healthcheck { kind, url, key, model },
    } = &cli.command
    {
        let provider = sps_cli::commands::provider::build_provider(kind, url.clone(), key.clone(), model)?;
        return sps_cli::commands::provider::healthcheck(&provider);
    }

    // All other commands need the kernel.
    let storage = open_storage(&cli.db)?;
    let kernel = boot_kernel(storage)?;

    match &cli.command {
        Commands::Verify => sps_cli::commands::verify::run(&kernel)?,
        Commands::Replay => sps_cli::commands::replay::run(&kernel)?,
        Commands::Snapshot => sps_cli::commands::snapshot::run(&kernel)?,
        Commands::Stats => sps_cli::commands::stats::run(&kernel)?,
        Commands::Events { limit } => sps_cli::commands::events::run(&kernel, *limit)?,
        Commands::Provider { action: ProviderAction::List } => {}
        Commands::Provider {
            action: ProviderAction::Healthcheck { .. },
        } => {}
        Commands::Memory { action: MemoryAction::Stats } => {
            sps_cli::commands::memory::stats(&kernel)?
        }
        Commands::Memory {
            action: MemoryAction::Search { query, limit },
        } => sps_cli::commands::memory::search(&kernel, query, *limit)?,
        Commands::Agent { action: AgentAction::List } => {
            let runtime = sps_agents::runtime::AgentRuntime::default();
            runtime.register_builtins();
            sps_cli::commands::agent::list(&runtime)?
        }
        Commands::Agent {
            action: AgentAction::Dispatch { archetype, title, description },
        } => {
            let runtime = sps_agents::runtime::AgentRuntime::default();
            runtime.register_builtins();
            sps_cli::commands::agent::dispatch(&runtime, archetype, title, description)?
        }
        Commands::Goal { action: GoalAction::List } => {
            sps_cli::commands::goal::list(&kernel)?
        }
        Commands::Goal {
            action: GoalAction::Verify { goal_id },
        } => sps_cli::commands::goal::verify(&kernel, goal_id)?,
    }

    Ok(())
}
