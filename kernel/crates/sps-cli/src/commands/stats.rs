//! `sps stats` — print kernel statistics.

use anyhow::Result;
use sps_core::kernel::SpsKernel;

pub fn run(kernel: &SpsKernel) -> Result<()> {
    println!("SPS Kernel Statistics");
    println!("  Backend:       {}", kernel.backend_name());
    println!("  Total events:  {}", kernel.event_count()?);
    println!("  Last tick:     {}", kernel.last_tick()?);
    println!("  Last hash:     {:.16}...", kernel.last_hash()?);
    println!("  Snapshot interval: {}", kernel.snapshot_interval());
    Ok(())
}
