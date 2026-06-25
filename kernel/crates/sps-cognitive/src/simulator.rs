//! Scenario Simulator — simulates execution paths before committing.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A simulation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub plan_id: Uuid,
    pub outcome: SimulatedOutcome,
    pub steps_executed: u32,
    pub steps_failed: u32,
    pub final_state: SimulationState,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulatedOutcome {
    Success,
    PartialSuccess,
    Failure,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimulationState {
    pub goals_completed: u32,
    pub files_generated: u32,
    pub errors: u32,
    pub warnings: u32,
}

/// A step in the simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimStep {
    pub name: String,
    pub success_probability: f64,
    pub produces_files: u32,
    pub produces_errors: u32,
}

/// The Scenario Simulator.
pub struct ScenarioSimulator {
    /// Random seed for deterministic simulation (0 = use system entropy).
    pub seed: u64,
}

impl Default for ScenarioSimulator {
    fn default() -> Self {
        Self { seed: 42 }
    }
}

impl ScenarioSimulator {
    /// Simulate a plan execution.
    pub fn simulate(&self, plan_id: Uuid, steps: &[SimStep]) -> SimulationResult {
        let mut log = Vec::new();
        let mut state = SimulationState::default();
        let mut executed = 0u32;
        let mut failed = 0u32;
        let mut outcome = SimulatedOutcome::Success;

        // Simple deterministic PRNG (xorshift) for reproducible simulations.
        let mut rng = self.seed;
        let mut next_rand = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng as f64) / (u64::MAX as f64)
        };

        for (i, step) in steps.iter().enumerate() {
            let roll = next_rand();
            if roll < step.success_probability {
                executed += 1;
                state.goals_completed += 1;
                state.files_generated += step.produces_files;
                state.errors += step.produces_errors;
                log.push(format!("[{}/{}] ✓ {} (roll={:.2}, needed<{:.2})",
                    i + 1, steps.len(), step.name, roll, step.success_probability));
            } else {
                failed += 1;
                state.errors += 1;
                log.push(format!("[{}/{}] ✗ {} FAILED (roll={:.2}, needed<{:.2})",
                    i + 1, steps.len(), step.name, roll, step.success_probability));

                if failed > steps.len() as u32 / 2 {
                    outcome = SimulatedOutcome::Failure;
                    log.push("Simulation aborted: too many failures".into());
                    break;
                } else if outcome != SimulatedOutcome::Failure {
                    outcome = SimulatedOutcome::PartialSuccess;
                }
            }
        }

        SimulationResult {
            plan_id,
            outcome,
            steps_executed: executed,
            steps_failed: failed,
            final_state: state,
            log,
        }
    }

    /// Run multiple simulations and return the aggregate.
    pub fn monte_carlo(&self, plan_id: Uuid, steps: &[SimStep], iterations: u32) -> MonteCarloResult {
        let mut successes = 0u32;
        let mut partials = 0u32;
        let mut failures = 0u32;
        let mut total_duration = 0u64;

        for i in 0..iterations {
            // Vary the seed per iteration.
            let sim = ScenarioSimulator { seed: self.seed.wrapping_add(i as u64) };
            let result = sim.simulate(plan_id, steps);
            match result.outcome {
                SimulatedOutcome::Success => successes += 1,
                SimulatedOutcome::PartialSuccess => partials += 1,
                SimulatedOutcome::Failure => failures += 1,
            }
            total_duration += result.steps_executed as u64 * 1000;
        }

        let total = iterations.max(1) as f64;
        MonteCarloResult {
            plan_id,
            iterations,
            success_rate: successes as f64 / total,
            partial_rate: partials as f64 / total,
            failure_rate: failures as f64 / total,
            avg_steps: (total_duration as f64 / total / 1000.0) as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    pub plan_id: Uuid,
    pub iterations: u32,
    pub success_rate: f64,
    pub partial_rate: f64,
    pub failure_rate: f64,
    pub avg_steps: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_probability_steps_succeed() {
        let sim = ScenarioSimulator::default();
        let steps = vec![
            SimStep { name: "setup".into(), success_probability: 0.99, produces_files: 1, produces_errors: 0 },
            SimStep { name: "build".into(), success_probability: 0.99, produces_files: 2, produces_errors: 0 },
        ];
        let result = sim.simulate(Uuid::nil(), &steps);
        assert!(result.steps_executed > 0);
    }

    #[test]
    fn monte_carlo_aggregates() {
        let sim = ScenarioSimulator::default();
        let steps = vec![
            SimStep { name: "step1".into(), success_probability: 0.7, produces_files: 1, produces_errors: 0 },
        ];
        let mc = sim.monte_carlo(Uuid::nil(), &steps, 100);
        assert_eq!(mc.iterations, 100);
        assert!((mc.success_rate + mc.partial_rate + mc.failure_rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let sim1 = ScenarioSimulator { seed: 42 };
        let sim2 = ScenarioSimulator { seed: 42 };
        let steps = vec![
            SimStep { name: "step".into(), success_probability: 0.5, produces_files: 0, produces_errors: 0 },
        ];
        let r1 = sim1.simulate(Uuid::nil(), &steps);
        let r2 = sim2.simulate(Uuid::nil(), &steps);
        assert_eq!(r1.outcome, r2.outcome);
        assert_eq!(r1.steps_executed, r2.steps_executed);
    }
}
