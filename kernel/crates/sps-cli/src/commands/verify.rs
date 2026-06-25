//! `sps verify` — verify the hash chain.

use anyhow::Result;
use sps_core::kernel::SpsKernel;

pub fn run(kernel: &SpsKernel) -> Result<()> {
    println!("Verifying hash chain...");
    let report = kernel.verify()?;
    println!("  Events verified: {}", report.events_verified);
    println!("  Last tick:        {}", report.last_tick);
    println!("  Last hash:        {:.16}...", report.last_hash.to_hex());
    println!("  Elapsed:          {} μs", report.elapsed_us);
    if let Some(failure) = &report.failure {
        println!("  FAILURE:          {:?}", failure);
        std::process::exit(1);
    } else {
        println!("  Status:           OK ✓");
    }
    Ok(())
}
