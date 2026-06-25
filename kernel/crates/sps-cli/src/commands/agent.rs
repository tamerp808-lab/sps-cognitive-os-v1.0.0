//! `sps agent` — agent runtime commands.

use anyhow::Result;
use smol_str::SmolStr;
use sps_agents::agent::AgentArchetype;
use sps_agents::runtime::AgentRuntime;

pub fn list(runtime: &AgentRuntime) -> Result<()> {
    let agents = runtime.list();
    if agents.is_empty() {
        println!("No agents registered.");
        return Ok(());
    }
    println!("{:<36} {:<12} {:<14} {}", "ID", "ARCHETYPE", "NAME", "CAPABILITIES");
    println!("{}", "-".repeat(85));
    for a in &agents {
        let caps = format!(
            "r{}w{}s{}l{}d{}",
            if a.capabilities.can_read_files { "+" } else { "-" },
            if a.capabilities.can_write_files { "+" } else { "-" },
            if a.capabilities.can_exec_shell { "+" } else { "-" },
            if a.capabilities.can_call_llm { "+" } else { "-" },
            if a.capabilities.can_delegate { "+" } else { "-" },
        );
        println!(
            "{:<36} {:<12} {:<14} {}",
            a.id.to_string(),
            a.archetype.as_str(),
            a.name,
            caps
        );
    }
    Ok(())
}

pub fn dispatch(
    runtime: &AgentRuntime,
    archetype: &str,
    title: &str,
    description: &str,
) -> Result<()> {
    let arch: AgentArchetype = archetype
        .parse()
        .map_err(|e: String| anyhow::anyhow!("{}", e))?;
    let result = runtime
        .dispatch(arch, title, description, 0, 0)
        .ok_or_else(|| anyhow::anyhow!("no agent of archetype '{}' registered", arch))?;
    println!("Dispatched to agent: {}", result.agent_id);
    println!("  Task ID:   {}", result.task_id);
    println!("  Messages:  {}", result.messages.len());
    Ok(())
}
