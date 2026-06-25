//! `sps memory` — memory subsystem commands.

use anyhow::Result;
use sps_core::kernel::SpsKernel;
use sps_memory::reducer::MemoryState;
use sps_memory::memory::MemoryKind;

pub fn stats(kernel: &SpsKernel) -> Result<()> {
    let mem_state = kernel.query(|s| MemoryState::from_state(s));
    let graph = match mem_state {
        Some(ms) => ms.graph,
        None => {
            println!("No memories recorded.");
            return Ok(());
        }
    };
    let stats = sps_memory::stats::MemoryStats::from_graph(&graph);
    println!("SPS Memory Statistics");
    println!("  Total memories:  {}", stats.total);
    println!("  Total links:     {}", stats.links);
    println!("  Avg strength:    {:.3}", stats.avg_strength);
    println!("  By kind:");
    for kind in [
        MemoryKind::Episodic,
        MemoryKind::Semantic,
        MemoryKind::Procedural,
        MemoryKind::Conceptual,
    ] {
        let count = stats.by_kind.get(kind.as_str()).copied().unwrap_or(0);
        println!("    {:<14} {}", kind.as_str(), count);
    }
    Ok(())
}

pub fn search(kernel: &SpsKernel, query: &str, limit: usize) -> Result<()> {
    let mem_state = kernel.query(|s| MemoryState::from_state(s));
    let graph = match mem_state {
        Some(ms) => ms.graph,
        None => {
            println!("No memories recorded.");
            return Ok(());
        }
    };
    let results = graph.search(query, limit);
    if results.is_empty() {
        println!("No matches for '{}'.", query);
        return Ok(());
    }
    println!("Search results for '{}' ({} matches):", query, results.len());
    for m in results {
        println!(
            "  [{}] {:<10} {} (strength: {:.2})",
            m.id, m.kind.as_str(), m.title, m.strength.0
        );
    }
    Ok(())
}
