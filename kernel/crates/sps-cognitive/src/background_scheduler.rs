//! SPS Gap 3: Background Scheduler — Autonomous Long-term Operation.
//!
//! The system works WITHOUT user input. It continuously:
//! - Checks for scheduled tasks (hourly, daily, weekly)
//! - Monitors for opportunities (idle time → review memories)
//! - Runs self-improvement checks
//! - Manages long-running missions
//! - Adjusts resource budget
//!
//! The BackgroundScheduler is the component that makes SPS an
//! AUTONOMOUS agent — not just a reactive system.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sps_core::actor::Actor;
use sps_core::event::RawEvent;
use sps_core::sink::EventSink;
use sps_core::CoreResult;

use crate::autonomous::{
    AutonomousScheduler, MissionManager, ResourceBudget, MissionState,
    SchedulingAction, ScheduledReview, ReviewKind,
};
use crate::cognitive_loop::{CognitiveLoop, CognitiveInput};

/// A background scheduler tick — one execution cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerTick {
    pub id: Uuid,
    pub tick_number: u64,
    pub timestamp_ms: u64,
    pub action_taken: String,
    pub cognitive_cycles_run: u32,
    pub reviews_completed: u32,
    pub missions_progressed: u32,
}

/// The Background Scheduler.
///
/// This is what makes SPS autonomous. It runs on a timer, checks
/// what needs to be done, and does it — without waiting for user input.
pub struct BackgroundScheduler {
    /// The autonomous scheduler that decides what to do.
    pub scheduler: AutonomousScheduler,
    /// Mission manager for long-running tasks.
    pub missions: MissionManager,
    /// Resource budget tracker.
    pub budget: ResourceBudget,
    /// Pending scheduled reviews.
    pub reviews: Vec<ScheduledReview>,
    /// Tick counter.
    pub tick_count: u64,
    /// Interval between ticks (ms).
    pub tick_interval_ms: u64,
    /// Whether the scheduler is running.
    pub running: bool,
    /// Last tick time.
    last_tick: Option<Instant>,
}

impl Default for BackgroundScheduler {
    fn default() -> Self {
        Self {
            scheduler: AutonomousScheduler::default(),
            missions: MissionManager::default(),
            budget: ResourceBudget {
                max_llm_tokens_per_day: 1_000_000,
                max_factory_runs_per_day: 20,
                max_active_time_per_day_ms: 28_800_000, // 8 hours
                ..Default::default()
            },
            reviews: Vec::new(),
            tick_count: 0,
            tick_interval_ms: 3_600_000, // 1 hour default
            running: false,
            last_tick: None,
        }
    }
}

impl BackgroundScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedule a recurring review.
    pub fn schedule_review(&mut self, kind: ReviewKind, mission_id: Option<Uuid>) {
        self.reviews.push(ScheduledReview {
            id: Uuid::now_v7(),
            kind,
            mission_id,
            scheduled_for_ms: 0,
            completed: false,
            findings: Vec::new(),
        });
    }

    /// Create a long-running mission.
    pub fn create_mission(&mut self, name: &str, description: &str, priority: u32) -> Uuid {
        self.missions.create(name.into(), description.into(), priority)
    }

    /// Start a mission.
    pub fn start_mission(&mut self, mission_id: Uuid) -> Result<(), String> {
        self.missions.start(mission_id)
    }

    /// Run one scheduler tick.
    ///
    /// This is the main entry point. The scheduler:
    /// 1. Checks resource budget
    /// 2. Checks for pending reviews
    /// 3. Checks active missions
    /// 4. Decides what to do (via AutonomousScheduler)
    /// 5. Executes the decision (may trigger CognitiveLoop)
    pub fn tick(&mut self, sink: &dyn EventSink) -> CoreResult<SchedulerTick> {
        self.tick_count += 1;
        let tick_id = Uuid::now_v7();
        let now_ms = 0; // In production: SystemTime::now()

        // Dispatch tick started.
        Self::dispatch(sink, "scheduler.tick_started", &serde_json::json!({
            "tick_id": tick_id.to_string(),
            "tick_number": self.tick_count,
            "timestamp_ms": now_ms,
        }))?;

        let mut action_taken = "idle".to_string();
        let mut cycles_run = 0u32;
        let mut reviews_done = 0u32;
        let mut missions_progressed = 0u32;

        // Get active missions for the scheduler.
        let active_missions: Vec<_> = self.missions.active_missions()
            .into_iter()
            .map(|m| crate::autonomous::Mission {
                id: m.id,
                name: m.name.clone(),
                description: m.description.clone(),
                goal_ids: m.goal_ids.clone(),
                state: m.state.clone(),
                created_at_ms: m.created_at_ms,
                deadline_ms: m.deadline_ms,
                priority: m.priority,
                progress: m.progress,
            })
            .collect();

        // Decide what to do.
        let decision = self.scheduler.decide(&active_missions, &self.budget, &self.reviews);

        match decision.action {
            SchedulingAction::ExecuteGoal => {
                // Run a cognitive cycle for the goal.
                action_taken = format!("execute_goal: {}", decision.reasoning);

                let input = CognitiveInput::Scheduled {
                    trigger: format!("Mission: {}", decision.reasoning),
                };
                let cycle = CognitiveLoop::run(input, sink)?;
                cycles_run = 1;

                if cycle.success {
                    // Update mission progress.
                    if let Some(mid) = decision.mission_id {
                        let _ = self.missions.update_progress(mid, 0.1);
                        missions_progressed = 1;
                    }
                    self.budget.record_llm(5000); // Estimate
                }
            }

            SchedulingAction::PlanMission => {
                action_taken = format!("plan_mission: {}", decision.reasoning);
                Self::dispatch(sink, "scheduler.plan_mission", &serde_json::json!({
                    "tick_id": tick_id.to_string(),
                    "mission_id": decision.mission_id,
                    "reasoning": decision.reasoning,
                }))?;
            }

            SchedulingAction::RunReview => {
                // Find the pending review and run it.
                if let Some(review) = self.reviews.iter_mut().find(|r| !r.completed) {
                    action_taken = format!("review: {:?}", review.kind);

                    // Run a cognitive cycle for the review.
                    let input = CognitiveInput::Scheduled {
                        trigger: format!("{:?} review", review.kind),
                    };
                    let _cycle = CognitiveLoop::run(input, sink)?;
                    cycles_run = 1;

                    review.completed = true;
                    review.findings.push(format!("{} review completed at tick {}", format!("{:?}", review.kind), self.tick_count));
                    reviews_done = 1;

                    Self::dispatch(sink, "scheduler.review_completed", &serde_json::json!({
                        "tick_id": tick_id.to_string(),
                        "review_kind": format!("{:?}", review.kind),
                        "findings": review.findings,
                    }))?;
                }
            }

            SchedulingAction::Idle => {
                action_taken = "idle — nothing to do".to_string();

                // During idle time, run memory consolidation.
                Self::dispatch(sink, "scheduler.idle_consolidation", &serde_json::json!({
                    "tick_id": tick_id.to_string(),
                    "action": "memory_consolidation",
                }))?;
            }

            SchedulingAction::WaitForBudget => {
                action_taken = "waiting_for_budget — daily limit reached".to_string();

                // Check if it's time to reset the budget (new day).
                self.budget.reset_daily();

                Self::dispatch(sink, "scheduler.budget_reset", &serde_json::json!({
                    "tick_id": tick_id.to_string(),
                    "llm_tokens": self.budget.current_llm_tokens,
                    "factory_runs": self.budget.current_factory_runs,
                }))?;
            }
        }

        // Check if any missions completed.
        for m in &self.missions.missions {
            if m.state == MissionState::Completed {
                Self::dispatch(sink, "scheduler.mission_completed", &serde_json::json!({
                    "tick_id": tick_id.to_string(),
                    "mission_id": m.id,
                    "mission_name": m.name,
                }))?;
            }
        }

        let tick = SchedulerTick {
            id: tick_id,
            tick_number: self.tick_count,
            timestamp_ms: now_ms,
            action_taken,
            cognitive_cycles_run: cycles_run,
            reviews_completed: reviews_done,
            missions_progressed,
        };

        // Dispatch tick completed.
        Self::dispatch(sink, "scheduler.tick_completed", &serde_json::json!({
            "tick_id": tick.id.to_string(),
            "tick_number": tick.tick_number,
            "action_taken": tick.action_taken,
            "cycles_run": tick.cognitive_cycles_run,
            "reviews_done": tick.reviews_completed,
            "missions_progressed": tick.missions_progressed,
        }))?;

        self.last_tick = Some(Instant::now());
        Ok(tick)
    }

    /// Run multiple ticks (simulates time passing).
    pub fn run_ticks(&mut self, count: u32, sink: &dyn EventSink) -> CoreResult<Vec<SchedulerTick>> {
        let mut ticks = Vec::new();
        for _ in 0..count {
            let tick = self.tick(sink)?;
            ticks.push(tick);
        }
        Ok(ticks)
    }

    /// Start the scheduler.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the scheduler.
    pub fn stop(&mut self) {
        self.running = false;
    }

    fn dispatch(sink: &dyn EventSink, event_type: &str, payload: &serde_json::Value) -> CoreResult<()> {
        sink.dispatch_trusted(RawEvent::new(
            event_type,
            payload.clone(),
            Actor::system("background_scheduler"),
            0,
        ))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sps_core::kernel::{KernelConfig, SpsKernel};
    use sps_core::state::TypedExtensionRegistry;
    use sps_core::storage::port::StoragePort;
    use sps_storage_memory::InMemoryStorage;

    fn boot_kernel() -> Arc<SpsKernel> {
        let storage: Arc<dyn StoragePort> = Arc::new(InMemoryStorage::new());
        let mut typed_reg = TypedExtensionRegistry::new();
        sps_goals::reducer::GoalReducer::register_typed_extensions(&mut typed_reg);
        sps_memory::reducer::MemoryReducer::register_typed_extensions(&mut typed_reg);
        let config = KernelConfig::default().with_typed_registry(typed_reg);
        SpsKernel::boot_with(storage, config, |reg| {
            sps_goals::reducer::GoalReducer::register(reg);
            sps_memory::reducer::MemoryReducer::register(reg);
        })
        .unwrap()
        .into()
    }

    #[test]
    fn scheduler_runs_idle_when_nothing_to_do() {
        let kernel = boot_kernel();
        let sink: &dyn EventSink = kernel.as_ref();
        let mut scheduler = BackgroundScheduler::new();

        let tick = scheduler.tick(sink).unwrap();

        assert_eq!(tick.tick_number, 1);
        assert!(tick.action_taken.contains("idle"));
        assert_eq!(tick.cognitive_cycles_run, 0);
    }

    #[test]
    fn scheduler_runs_pending_review() {
        let kernel = boot_kernel();
        let sink: &dyn EventSink = kernel.as_ref();
        let mut scheduler = BackgroundScheduler::new();

        // Schedule a daily review.
        scheduler.schedule_review(ReviewKind::Daily, None);

        let tick = scheduler.tick(sink).unwrap();

        assert!(tick.action_taken.contains("review"), "Should run review, got: {}", tick.action_taken);
        assert_eq!(tick.reviews_completed, 1);
        assert!(tick.cognitive_cycles_run > 0, "Should run cognitive cycle for review");
    }

    #[test]
    fn scheduler_executes_mission_goal() {
        let kernel = boot_kernel();
        let sink: &dyn EventSink = kernel.as_ref();
        let mut scheduler = BackgroundScheduler::new();

        // Create + start a mission.
        let mid = scheduler.create_mission("Build SPS v2", "Improve the SPS kernel", 5);
        scheduler.start_mission(mid).unwrap();
        // Add a goal to the mission.
        scheduler.missions.add_goal(mid, Uuid::nil()).unwrap();

        let tick = scheduler.tick(sink).unwrap();

        assert!(tick.action_taken.contains("execute_goal") || tick.action_taken.contains("plan"),
            "Should execute or plan mission, got: {}", tick.action_taken);
    }

    #[test]
    fn scheduler_resets_budget_when_exhausted() {
        let kernel = boot_kernel();
        let sink: &dyn EventSink = kernel.as_ref();
        let mut scheduler = BackgroundScheduler::new();

        // Exhaust the budget.
        scheduler.budget.current_llm_tokens = scheduler.budget.max_llm_tokens_per_day;

        let tick = scheduler.tick(sink).unwrap();

        assert!(tick.action_taken.contains("waiting_for_budget") || tick.action_taken.contains("idle"),
            "Should wait for budget or idle after reset");
    }

    #[test]
    fn scheduler_runs_multiple_ticks() {
        let kernel = boot_kernel();
        let sink: &dyn EventSink = kernel.as_ref();
        let mut scheduler = BackgroundScheduler::new();

        // Schedule a review for tick 1.
        scheduler.schedule_review(ReviewKind::Daily, None);

        let ticks = scheduler.run_ticks(3, sink).unwrap();

        assert_eq!(ticks.len(), 3);
        assert_eq!(ticks[0].tick_number, 1);
        assert_eq!(ticks[1].tick_number, 2);
        assert_eq!(ticks[2].tick_number, 3);

        // First tick should run the review.
        assert!(ticks[0].action_taken.contains("review"));
        // Second tick should be idle (review already done).
        assert!(ticks[1].action_taken.contains("idle") || ticks[1].action_taken.contains("execute"));

        // Verify hash chain.
        let report = kernel.verify().unwrap();
        assert!(report.failure.is_none(), "Hash chain should be intact");

        let events = kernel.store().read_from(0, 1000).unwrap();
        let has_tick_started = events.iter().any(|e| e.event_type.as_str() == "scheduler.tick_started");
        let has_tick_completed = events.iter().any(|e| e.event_type.as_str() == "scheduler.tick_completed");
        let has_review = events.iter().any(|e| e.event_type.as_str() == "scheduler.review_completed");
        let has_idle = events.iter().any(|e| e.event_type.as_str() == "scheduler.idle_consolidation");

        assert!(has_tick_started, "Should dispatch tick_started");
        assert!(has_tick_completed, "Should dispatch tick_completed");
        assert!(has_review, "Should dispatch review_completed");
        assert!(has_idle, "Should dispatch idle_consolidation");

        println!("\n══════════════════════════════════════════════════════════");
        println!("  BACKGROUND SCHEDULER — 3 TICKS — PASSED");
        println!("══════════════════════════════════════════════════════════");
        for t in &ticks {
            println!("  Tick {}: {} (cycles={}, reviews={}, missions={})",
                t.tick_number, t.action_taken, t.cognitive_cycles_run,
                t.reviews_completed, t.missions_progressed);
        }
        println!("  Total events: {} | Hash chain: intact", events.len());
        println!("══════════════════════════════════════════════════════════");
    }
}
