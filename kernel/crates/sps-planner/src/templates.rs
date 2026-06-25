//! Plan templates and template registry.

use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::plan::{Plan, PlanStep};
use sps_goals::GoalId;

/// A plan template — generates a plan from a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    /// Template name.
    pub name: SmolStr,
    /// Description.
    pub description: String,
    /// Step templates (title, parallelizable).
    pub step_templates: Vec<StepTemplate>,
}

/// A step template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTemplate {
    /// Title.
    pub title: SmolStr,
    /// Default description.
    #[serde(default)]
    pub description: String,
    /// Whether the step is parallelizable.
    #[serde(default)]
    pub parallelizable: bool,
    /// Assigned agent archetype.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<SmolStr>,
}

impl PlanTemplate {
    /// Generate a plan from this template for the given goal.
    pub fn generate(&self, goal_id: GoalId, origin_tick: u64, created_at: u64) -> Plan {
        let mut plan = Plan::new(goal_id, self.name.clone());
        plan.created_at = created_at;
        plan.origin_tick = origin_tick;
        for (i, st) in self.step_templates.iter().enumerate() {
            let idx = i as u32;
            // Linear dependency on previous step unless parallelizable.
            let depends_on = if st.parallelizable || idx == 0 {
                Vec::new()
            } else {
                vec![idx - 1]
            };
            plan.add_step(PlanStep {
                id: uuid::Uuid::now_v7(),
                title: st.title.clone(),
                description: st.description.clone(),
                index: idx,
                depends_on,
                assigned_agent: st.assigned_agent.clone(),
                parallelizable: st.parallelizable,
            });
        }
        plan
    }
}

/// Registry of plan templates.
#[derive(Default)]
pub struct TemplateRegistry {
    templates: RwLock<std::collections::HashMap<SmolStr, Arc<PlanTemplate>>>,
}

impl TemplateRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a template.
    pub fn register(&self, template: Arc<PlanTemplate>) {
        self.templates.write().insert(template.name.clone(), template);
    }

    /// Look up a template by name.
    pub fn get(&self, name: &str) -> Option<Arc<PlanTemplate>> {
        self.templates.read().get(name).cloned()
    }

    /// List all registered template names.
    pub fn list(&self) -> Vec<SmolStr> {
        self.templates.read().keys().cloned().collect()
    }
}

/// Built-in templates.
pub fn builtin_templates() -> Vec<Arc<PlanTemplate>> {
    vec![
        Arc::new(PlanTemplate {
            name: "generic.workflow".into(),
            description: "A generic 5-step workflow: analyze → plan → execute → review → finalize.".into(),
            step_templates: vec![
                StepTemplate {
                    title: "Analyze requirements".into(),
                    description: "Understand the goal and constraints.".into(),
                    parallelizable: false,
                    assigned_agent: Some("architect".into()),
                },
                StepTemplate {
                    title: "Design solution".into(),
                    description: "Architect the approach.".into(),
                    parallelizable: false,
                    assigned_agent: Some("architect".into()),
                },
                StepTemplate {
                    title: "Implement".into(),
                    description: "Write the code / execute the task.".into(),
                    parallelizable: false,
                    assigned_agent: Some("developer".into()),
                },
                StepTemplate {
                    title: "Test".into(),
                    description: "Verify the implementation.".into(),
                    parallelizable: false,
                    assigned_agent: Some("tester".into()),
                },
                StepTemplate {
                    title: "Review".into(),
                    description: "Review and finalize.".into(),
                    parallelizable: false,
                    assigned_agent: Some("reviewer".into()),
                },
            ],
        }),
        Arc::new(PlanTemplate {
            name: "research".into(),
            description: "Research workflow: gather → analyze → summarize.".into(),
            step_templates: vec![
                StepTemplate {
                    title: "Gather sources".into(),
                    description: "Collect relevant material.".into(),
                    parallelizable: true,
                    assigned_agent: Some("researcher".into()),
                },
                StepTemplate {
                    title: "Analyze findings".into(),
                    description: "Analyze the gathered material.".into(),
                    parallelizable: true,
                    assigned_agent: Some("researcher".into()),
                },
                StepTemplate {
                    title: "Summarize".into(),
                    description: "Produce a summary.".into(),
                    parallelizable: false,
                    assigned_agent: Some("researcher".into()),
                },
            ],
        }),
        Arc::new(PlanTemplate {
            name: "deployment".into(),
            description: "Deployment workflow: prepare → test → deploy → verify.".into(),
            step_templates: vec![
                StepTemplate {
                    title: "Prepare environment".into(),
                    description: "Set up the target environment.".into(),
                    parallelizable: false,
                    assigned_agent: Some("devops".into()),
                },
                StepTemplate {
                    title: "Run pre-deployment tests".into(),
                    description: "Smoke tests before going live.".into(),
                    parallelizable: false,
                    assigned_agent: Some("tester".into()),
                },
                StepTemplate {
                    title: "Deploy".into(),
                    description: "Execute the deployment.".into(),
                    parallelizable: false,
                    assigned_agent: Some("devops".into()),
                },
                StepTemplate {
                    title: "Verify deployment".into(),
                    description: "Confirm the deployment succeeded.".into(),
                    parallelizable: false,
                    assigned_agent: Some("devops".into()),
                },
            ],
        }),
    ]
}
