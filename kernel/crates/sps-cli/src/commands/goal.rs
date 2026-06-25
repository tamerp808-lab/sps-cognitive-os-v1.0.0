//! `sps goal` — goal system commands.

use anyhow::Result;
use sps_core::kernel::SpsKernel;
use sps_goals::reducer::GoalState;
use sps_goals::hierarchy::GoalStatus;

pub fn list(kernel: &SpsKernel) -> Result<()> {
    let goal_state = kernel.query(|s| GoalState::from_state(s));
    let tree = match goal_state {
        Some(gs) => gs.tree,
        None => {
            println!("No goals recorded.");
            return Ok(());
        }
    };
    if tree.goals.is_empty() {
        println!("No goals recorded.");
        return Ok(());
    }
    println!("{:<36} {:<10} {:<8} {:<6} {}", "ID", "STATUS", "PRIORITY", "TASKS", "TITLE");
    println!("{}", "-".repeat(85));
    for g in tree.goals.values() {
        let tasks: u32 = g
            .objectives
            .iter()
            .flat_map(|o| &o.milestones)
            .map(|m| m.tasks.len() as u32)
            .sum();
        println!(
            "{:<36} {:<10} {:<8} {:<6} {}",
            g.id.to_string(),
            format!("{:?}", g.status).to_lowercase(),
            g.priority,
            tasks,
            g.title
        );
    }
    Ok(())
}

pub fn verify(kernel: &SpsKernel, goal_id: &str) -> Result<()> {
    let goal_uuid = uuid::Uuid::parse_str(goal_id)
        .map_err(|e| anyhow::anyhow!("invalid goal id: {}", e))?;
    let goal_state = kernel.query(|s| GoalState::from_state(s));
    let tree = match goal_state {
        Some(gs) => gs.tree,
        None => {
            println!("No goals recorded.");
            return Ok(());
        }
    };
    let result = tree.verify(&sps_goals::hierarchy::GoalId(goal_uuid));
    println!("Verification for goal {}:",
        goal_id
    );
    println!("  Verified:        {}", result.verified);
    println!("  Tasks total:     {}", result.tasks_total);
    println!("  Tasks completed: {}", result.tasks_completed);
    println!("  Reason:          {}", result.reason);
    Ok(())
}
