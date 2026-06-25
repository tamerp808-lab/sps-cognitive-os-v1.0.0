//! `sps events` — list recent events.

use anyhow::Result;
use sps_core::kernel::SpsKernel;

pub fn run(kernel: &SpsKernel, limit: usize) -> Result<()> {
    let from = 1u64;
    let events = kernel.store().read_from(from, limit)?;
    if events.is_empty() {
        println!("No events.");
        return Ok(());
    }
    println!("{:<6} {:<26} {:<16} {:<10}", "TICK", "TYPE", "HASH", "WALL_TIME");
    println!("{}", "-".repeat(70));
    for e in &events {
        println!(
            "{:<6} {:<26} {:<16} {:<10}",
            e.tick,
            e.event_type.as_str(),
            format!("{:.16}...", e.hash.to_hex()),
            e.wall_time
        );
    }
    Ok(())
}
