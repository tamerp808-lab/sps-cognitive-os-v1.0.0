//! CLI command handlers.

pub mod verify;
pub mod replay;
pub mod snapshot;
pub mod stats;
pub mod events;
pub mod provider;
pub mod memory;
pub mod agent;
pub mod goal;

use anyhow::Result;
use sps_core::kernel::SpsKernel;

/// Get the current wall time in ms (display only).
pub fn current_wall_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Print a summary of the kernel state.
pub fn print_summary(kernel: &SpsKernel) -> Result<()> {
    println!("SPS Kernel Summary");
    println!("  Backend:    {}", kernel.backend_name());
    println!("  Last tick:  {}", kernel.last_tick()?);
    println!("  Last hash:  {:.16}...", kernel.last_hash()?);
    println!("  Events:     {}", kernel.event_count()?);
    println!("  Booted:     {}", kernel.is_booted());
    Ok(())
}
