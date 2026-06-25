//! `sps snapshot` — take a snapshot.

use anyhow::Result;
use sps_core::kernel::SpsKernel;

fn current_wall_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn run(kernel: &SpsKernel) -> Result<()> {
    println!("Taking snapshot...");
    let snap = kernel.snapshot(current_wall_time())?;
    println!("  Tick:        {}", snap.tick);
    println!("  State hash:  {:.16}...", hex::encode(snap.state_hash));
    println!("  Wall time:   {}", snap.wall_time);
    Ok(())
}
