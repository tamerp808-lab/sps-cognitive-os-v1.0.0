//! Opportunity Detector — finds goals that can be parallelized or combined.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A detected opportunity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub kind: OpportunityKind,
    pub goal_ids: Vec<Uuid>,
    pub description: String,
    pub potential_savings_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityKind {
    /// Goals with no dependencies can run in parallel.
    Parallelizable,
    /// Goals share a common sub-goal that can be done once.
    SharedSubGoal,
    /// A goal is blocked by another — can be decomposed.
    BlockedChain,
    /// A completed goal's output can be reused.
    ReusableOutput,
}

/// A goal summary for opportunity detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalSummary {
    pub id: Uuid,
    pub status: String,
    pub dependencies: Vec<Uuid>,
    pub objectives: u32,
}

/// The Opportunity Detector.
pub struct OpportunityDetector;

impl OpportunityDetector {
    /// Detect parallelization opportunities among active goals.
    pub fn detect_parallelizable(goals: &[GoalSummary]) -> Vec<Opportunity> {
        let mut opportunities = Vec::new();

        // Find goals that are active with no dependencies.
        let independent: Vec<_> = goals
            .iter()
            .filter(|g| g.status == "active" && g.dependencies.is_empty())
            .collect();

        if independent.len() >= 2 {
            let ids: Vec<_> = independent.iter().map(|g| g.id).collect();
            let savings = independent.len() as u64 * 5_000; // 5s per goal saved
            opportunities.push(Opportunity {
                kind: OpportunityKind::Parallelizable,
                goal_ids: ids,
                description: format!(
                    "{} independent goals can run in parallel",
                    independent.len()
                ),
                potential_savings_ms: savings,
            });
        }

        opportunities
    }

    /// Detect shared sub-goals (goals with common dependencies).
    pub fn detect_shared_subgoals(goals: &[GoalSummary]) -> Vec<Opportunity> {
        let mut dep_map: std::collections::HashMap<Uuid, Vec<Uuid>> = std::collections::HashMap::new();

        for g in goals {
            for dep in &g.dependencies {
                dep_map.entry(*dep).or_default().push(g.id);
            }
        }

        dep_map
            .into_iter()
            .filter(|(_, dependents)| dependents.len() >= 2)
            .map(|(dep, dependents)| Opportunity {
                kind: OpportunityKind::SharedSubGoal,
                goal_ids: dependents.clone(),
                description: format!(
                    "{} goals depend on {} — execute once, share result",
                    dependents.len(),
                    dep
                ),
                potential_savings_ms: (dependents.len() as u64 - 1) * 3_000,
            })
            .collect()
    }

    /// Detect blocked chains — goals waiting on other goals.
    pub fn detect_blocked_chains(goals: &[GoalSummary]) -> Vec<Opportunity> {
        goals
            .iter()
            .filter(|g| g.status == "active" && !g.dependencies.is_empty())
            .filter(|g| {
                // Check if any dependency is NOT completed.
                g.dependencies.iter().any(|dep| {
                    goals.iter().any(|dg| dg.id == *dep && dg.status != "completed")
                })
            })
            .map(|g| Opportunity {
                kind: OpportunityKind::BlockedChain,
                goal_ids: vec![g.id],
                description: format!(
                    "Goal {} is blocked by incomplete dependencies — consider decomposing",
                    g.id
                ),
                potential_savings_ms: 0,
            })
            .collect()
    }

    /// Detect all opportunities.
    pub fn detect_all(goals: &[GoalSummary]) -> Vec<Opportunity> {
        let mut all = Vec::new();
        all.extend(Self::detect_parallelizable(goals));
        all.extend(Self::detect_shared_subgoals(goals));
        all.extend(Self::detect_blocked_chains(goals));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_parallelizable_goals() {
        let goals = vec![
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![], objectives: 2 },
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![], objectives: 1 },
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![], objectives: 3 },
        ];
        let opps = OpportunityDetector::detect_parallelizable(&goals);
        assert_eq!(opps.len(), 1);
        assert_eq!(opps[0].kind, OpportunityKind::Parallelizable);
        assert_eq!(opps[0].goal_ids.len(), 3);
    }

    #[test]
    fn detects_shared_subgoals() {
        let dep = Uuid::now_v7();
        let goals = vec![
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![dep], objectives: 1 },
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![dep], objectives: 1 },
        ];
        let opps = OpportunityDetector::detect_shared_subgoals(&goals);
        assert_eq!(opps.len(), 1);
        assert_eq!(opps[0].kind, OpportunityKind::SharedSubGoal);
    }

    #[test]
    fn no_opportunity_for_single_goal() {
        let goals = vec![
            GoalSummary { id: Uuid::nil(), status: "active".into(), dependencies: vec![], objectives: 1 },
        ];
        let opps = OpportunityDetector::detect_parallelizable(&goals);
        assert!(opps.is_empty());
    }
}
