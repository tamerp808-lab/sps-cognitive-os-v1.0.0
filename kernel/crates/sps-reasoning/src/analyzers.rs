//! Reasoning analyzers.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// A goal analysis — feasibility, ambiguity, decomposition suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalAnalysis {
    /// Goal being analyzed.
    pub goal_id: Uuid,
    /// Feasibility score (0.0–1.0).
    pub feasibility: f32,
    /// Ambiguity score (0.0–1.0).
    pub ambiguity: f32,
    /// Estimated scope (number of tasks).
    pub estimated_scope: u32,
    /// Suggestions.
    #[serde(default)]
    pub suggestions: Vec<String>,
    /// Detected risks.
    #[serde(default)]
    pub risks: Vec<String>,
}

/// A task decomposition — breaks a goal into subtasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskDecomposition {
    /// Goal being decomposed.
    pub goal_id: Uuid,
    /// Subtask descriptions.
    pub tasks: Vec<SmolStr>,
    /// Dependencies (list of (from, to) index pairs).
    #[serde(default)]
    pub dependencies: Vec<(u32, u32)>,
}

/// A conflict report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictReport {
    /// Conflicting entity ids.
    pub entities: Vec<Uuid>,
    /// Conflict description.
    pub description: String,
    /// Severity (0.0–1.0).
    pub severity: f32,
}

/// A risk assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Task being assessed.
    pub task_id: Uuid,
    /// Risk score (0.0–1.0).
    pub risk_score: f32,
    /// Risk factors.
    #[serde(default)]
    pub factors: Vec<String>,
}

/// Analyze a goal's feasibility.
pub struct GoalAnalyzer;

impl GoalAnalyzer {
    /// Analyze a goal description. Returns a [`GoalAnalysis`].
    pub fn analyze(goal_id: Uuid, description: &str) -> GoalAnalysis {
        // Heuristic analysis (no LLM in Phase 5 — that comes via Effects).
        let ambiguity = if description.len() < 20 {
            0.8
        } else if description.contains("?") {
            0.5
        } else {
            0.2
        };
        let feasibility = 1.0 - ambiguity * 0.5;
        let estimated_scope = (description.len() / 50).max(1) as u32;
        let mut suggestions = Vec::new();
        if ambiguity > 0.5 {
            suggestions.push("Clarify the goal with the user before planning.".into());
        }
        if estimated_scope > 5 {
            suggestions.push("Break this goal into milestones.".into());
        }
        GoalAnalysis {
            goal_id,
            feasibility,
            ambiguity,
            estimated_scope,
            suggestions,
            risks: Vec::new(),
        }
    }
}

/// Decompose a goal into tasks.
pub struct TaskDecomposer;

impl TaskDecomposer {
    /// Decompose a goal description into subtasks (heuristic).
    pub fn decompose(goal_id: Uuid, description: &str) -> TaskDecomposition {
        // Heuristic: split by sentences, treat each as a task.
        let tasks: Vec<SmolStr> = description
            .split('.')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(SmolStr::new)
            .collect();
        let n = tasks.len() as u32;
        // Linear dependency chain: 0 → 1 → 2 → ...
        let dependencies: Vec<(u32, u32)> = (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect();
        TaskDecomposition {
            goal_id,
            tasks,
            dependencies,
        }
    }
}

/// Solve dependencies via topological sort.
pub struct DependencySolver;

impl DependencySolver {
    /// Topological sort of tasks given dependencies.
    /// Returns the order, or an error if a cycle is detected.
    pub fn solve(
        n: u32,
        dependencies: &[(u32, u32)],
    ) -> Result<Vec<u32>, String> {
        let n = n as usize;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree = vec![0u32; n];
        for &(from, to) in dependencies {
            let from = from as usize;
            let to = to as usize;
            if from >= n || to >= n {
                return Err(format!("dependency out of range: ({}, {})", from, to));
            }
            adj[from].push(to);
            in_degree[to] += 1;
        }
        let mut queue: std::collections::VecDeque<usize> =
            (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(node) = queue.pop_front() {
            order.push(node as u32);
            for &neighbor in &adj[node] {
                in_degree[neighbor] -= 1;
                if in_degree[neighbor] == 0 {
                    queue.push_back(neighbor);
                }
            }
        }
        if order.len() != n {
            Err("cycle detected in dependencies".into())
        } else {
            Ok(order)
        }
    }
}

/// Detect conflicts between tasks/goals.
pub struct ConflictDetector;

impl ConflictDetector {
    /// Detect resource conflicts (two tasks touching the same resource).
    pub fn detect(resource_assignments: &[(Uuid, String)]) -> Vec<ConflictReport> {
        let mut resource_to_tasks: std::collections::HashMap<String, Vec<Uuid>> =
            std::collections::HashMap::new();
        for (task_id, resource) in resource_assignments {
            resource_to_tasks
                .entry(resource.clone())
                .or_default()
                .push(*task_id);
        }
        resource_to_tasks
            .into_iter()
            .filter(|(_, tasks)| tasks.len() > 1)
            .map(|(resource, tasks)| ConflictReport {
                entities: tasks,
                description: format!("multiple tasks target resource: {}", resource),
                severity: 0.7,
            })
            .collect()
    }
}

/// Assess risk per task.
pub struct RiskAnalyzer;

impl RiskAnalyzer {
    /// Assess risk based on historical failure indicators (Phase 5 heuristic).
    pub fn assess(task_id: Uuid, description: &str, complexity: u32) -> RiskAssessment {
        let mut factors = Vec::new();
        let mut risk = 0.1f32;
        if complexity > 5 {
            risk += 0.3;
            factors.push("high complexity".into());
        }
        if description.to_lowercase().contains("deploy") {
            risk += 0.2;
            factors.push("involves deployment".into());
        }
        if description.to_lowercase().contains("migrat") {
            risk += 0.3;
            factors.push("involves migration".into());
        }
        RiskAssessment {
            task_id,
            risk_score: risk.min(1.0),
            factors,
        }
    }
}

/// Optimize a plan (parallelize where possible).
pub struct PlanOptimizer;

impl PlanOptimizer {
    /// Group tasks into parallel batches based on dependencies.
    pub fn parallelize(
        n: u32,
        dependencies: &[(u32, u32)],
    ) -> Result<Vec<Vec<u32>>, String> {
        let n = n as usize;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree = vec![0u32; n];
        for &(from, to) in dependencies {
            let from = from as usize;
            let to = to as usize;
            adj[from].push(to);
            in_degree[to] += 1;
        }
        let mut batches = Vec::new();
        let mut remaining: std::collections::HashSet<usize> = (0..n).collect();
        while !remaining.is_empty() {
            let batch: Vec<u32> = remaining
                .iter()
                .filter(|&&i| in_degree[i] == 0)
                .map(|&i| i as u32)
                .collect();
            if batch.is_empty() {
                return Err("cycle detected".into());
            }
            for &node in &batch {
                let node = node as usize;
                remaining.remove(&node);
                for &neighbor in &adj[node] {
                    in_degree[neighbor] -= 1;
                }
            }
            batches.push(batch);
        }
        Ok(batches)
    }
}
