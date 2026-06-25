//! `sps replay` — replay from genesis and compare with current state.

use anyhow::Result;
use sps_core::kernel::SpsKernel;

pub fn run(kernel: &SpsKernel) -> Result<()> {
    println!("Replaying from genesis...");
    let state = kernel.replay_from_genesis()?;
    println!("  Replayed last tick:    {}", state.last_tick());
    println!("  Replayed last hash:    {:.16}...", state.last_hash().to_hex());
    println!("  Replayed event count:  {}", state.event_count());
    println!("  Current last tick:     {}", kernel.last_tick()?);
    println!("  Current last hash:     {:.16}...", kernel.last_hash()?);
    println!("  Current event count:   {}", kernel.event_count()?);
    if state.last_tick() == kernel.last_tick()?
        && state.last_hash() == kernel.last_hash()?
        && state.event_count() == kernel.event_count()?
    {
        println!("  Replay matches current state: ✓");
    } else {
        println!("  Replay DOES NOT match current state!");
        std::process::exit(1);
    }
    Ok(())
}
