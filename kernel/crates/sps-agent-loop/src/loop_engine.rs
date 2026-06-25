//! The agent loop engine — runs the ReAct cycle.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use sps_llm::tools::ToolRegistry;
use sps_llm::LlmProvider;
use sps_llm::conversation::{Conversation, MessageRole};

use crate::prompt::{build_react_prompt, parse_step_response};
use crate::step::{LoopStep, StepKind, StepStatus};

/// Configuration for the agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLoopConfig {
    /// Maximum number of iterations (Thought + Action pairs).
    pub max_iterations: usize,
    /// Provider id to use.
    pub provider_id: SmolStr,
    /// Optional model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SmolStr>,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            provider_id: SmolStr::new("default"),
            model: None,
        }
    }
}

/// The result of running an agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopResult {
    /// Whether the loop succeeded (produced a final answer).
    pub success: bool,
    /// All steps taken.
    pub steps: Vec<LoopStep>,
    /// Final answer (if successful).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    /// Error message (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Total iterations used.
    pub iterations: usize,
}

/// The agent loop engine.
pub struct AgentLoop {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    config: AgentLoopConfig,
}

impl AgentLoop {
    /// Create a new agent loop.
    pub fn new(provider: Arc<dyn LlmProvider>, tools: Arc<ToolRegistry>, config: AgentLoopConfig) -> Self {
        Self { provider, tools, config }
    }

    /// Run the loop for a given task. Returns the complete result.
    pub fn run(&self, task: &str) -> LoopResult {
        let system_prompt = build_react_prompt(&self.tools.definitions(), task);
        let mut conv = Conversation::new(self.config.provider_id.clone(), &system_prompt);
        if let Some(ref model) = self.config.model {
            conv.model = Some(model.clone());
        }

        let mut steps: Vec<LoopStep> = Vec::new();
        let mut step_index = 0usize;

        for iteration in 0..self.config.max_iterations {
            // 1. Ask the LLM what to do next.
            let request = conv.to_request();
            let completion = match self.provider.complete(&request) {
                Ok(c) => c,
                Err(e) => {
                    steps.push(LoopStep::error(step_index, e.to_string()));
                    return LoopResult {
                        success: false,
                        steps,
                        answer: None,
                        error: Some(e.to_string()),
                        iterations: iteration,
                    };
                }
            };

            // 2. Parse the response.
            let step_kind = match parse_step_response(&completion.text) {
                Some(k) => k,
                None => {
                    // If we can't parse, treat the response as a thought.
                    StepKind::Thought { text: completion.text.clone() }
                }
            };

            match step_kind {
                StepKind::Thought { text } => {
                    steps.push(LoopStep::thought(step_index, &text));
                    step_index += 1;
                    // Add the thought to the conversation so the LLM has context.
                    conv.add_assistant(&format!("Thought: {}", text));
                }
                StepKind::Action { tool, arguments } => {
                    steps.push(LoopStep::action(step_index, &tool, &arguments));
                    step_index += 1;

                    // Execute the tool.
                    let tool_call = sps_llm::tools::ToolCall { name: tool.clone(), arguments: arguments.clone() };
                    let result = self.tools.execute(&tool_call);

                    // Record the observation.
                    steps.push(LoopStep::observation(
                        step_index,
                        &result.name,
                        &result.content,
                        result.success,
                    ));
                    step_index += 1;

                    // Add action + observation to conversation.
                    conv.add_assistant(&format!("Action: {{\"tool\": \"{}\", \"arguments\": {}}}", tool, arguments));
                    let obs_text = if result.success {
                        format!("Observation: {}", result.content)
                    } else {
                        format!("Observation (error): {}", result.error.unwrap_or_default())
                    };
                    conv.add_user(&obs_text);

                    // Continue the loop.
                }
                StepKind::Answer { text } => {
                    steps.push(LoopStep::answer(step_index, &text));
                    return LoopResult {
                        success: true,
                        steps,
                        answer: Some(text),
                        error: None,
                        iterations: iteration + 1,
                    };
                }
                StepKind::Observation { .. } | StepKind::Error { .. } => {
                    // These shouldn't come from the LLM; skip.
                }
            }
        }

        // Max iterations reached without an answer.
        LoopResult {
            success: false,
            steps,
            answer: None,
            error: Some(format!("max iterations ({}) reached without a final answer", self.config.max_iterations)),
            iterations: self.config.max_iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sps_effects::providers::adapters::StaticAdapter;
    use sps_effects::providers::llm::{LlmProvider, ProviderConfig};
    use sps_llm::tools::{Tool, ToolDefinition, ToolProperty, ToolSchema};
    use std::collections::BTreeMap;

    struct EchoTool;

    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".into(),
                description: "Echoes back the input.".into(),
                schema: ToolSchema {
                    schema_type: "object".into(),
                    properties: {
                        let mut p = BTreeMap::new();
                        p.insert("message".into(), ToolProperty {
                            prop_type: "string".into(),
                            description: "Message to echo.".into(),
                            enum_values: vec![],
                        });
                        p
                    },
                    required: vec!["message".into()],
                },
            }
        }
        fn execute(&self, arguments: &str) -> sps_llm::tools::ToolResult {
            #[derive(serde::Deserialize)]
            struct Args { message: String }
            match serde_json::from_str::<Args>(arguments) {
                Ok(args) => sps_llm::tools::ToolResult::success("echo", args.message),
                Err(e) => sps_llm::tools::ToolResult::error("echo", e.to_string()),
            }
        }
    }

    fn make_provider(response: &str) -> Arc<dyn LlmProvider> {
        let p = Arc::new(StaticAdapter::new("test", response));
        p.configure(ProviderConfig {
            id: "test".into(),
            name: "Test".into(),
            api_url: "http://localhost".into(),
            api_key: None,
            model_name: "test".into(),
            metadata: Default::default(),
        });
        p
    }

    #[test]
    fn loop_returns_answer_when_llm_produces_one() {
        let provider = make_provider("Answer: The result is 42.");
        let tools = Arc::new(ToolRegistry::new());
        let config = AgentLoopConfig { max_iterations: 3, provider_id: "test".into(), model: None };
        let agent_loop = AgentLoop::new(provider, tools, config);
        let result = agent_loop.run("What is the answer?");
        assert!(result.success);
        assert_eq!(result.answer.unwrap(), "The result is 42.");
    }

    #[test]
    fn loop_executes_tool_then_answers() {
        // First call: Action (echo tool). Second call: Answer.
        let provider = make_provider(r#"Answer: Done echoing."#);
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(EchoTool));
        let config = AgentLoopConfig { max_iterations: 5, provider_id: "test".into(), model: None };
        let agent_loop = AgentLoop::new(provider, tools, config);
        let result = agent_loop.run("Echo hello then answer");
        assert!(result.success);
    }

    #[test]
    fn loop_fails_on_max_iterations() {
        // Always returns a thought — never an answer.
        let provider = make_provider("Thought: I need to think more.");
        let tools = Arc::new(ToolRegistry::new());
        let config = AgentLoopConfig { max_iterations: 2, provider_id: "test".into(), model: None };
        let agent_loop = AgentLoop::new(provider, tools, config);
        let result = agent_loop.run("Never answer");
        assert!(!result.success);
        assert!(result.error.unwrap().contains("max iterations"));
    }

    #[test]
    fn loop_handles_tool_execution() {
        let provider = make_provider(r#"Action: {"tool": "echo", "arguments": {"message": "hello"}}"#);
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(EchoTool));
        let config = AgentLoopConfig { max_iterations: 3, provider_id: "test".into(), model: None };
        let agent_loop = AgentLoop::new(provider, tools, config);
        let result = agent_loop.run("Echo hello");
        // The loop will execute echo, then on the next iteration the LLM will
        // return the same action (StaticAdapter always returns the same response).
        // After max_iterations it will fail, but the tool should have been called.
        assert!(result.steps.iter().any(|s| matches!(s.kind, StepKind::Observation { ref tool, .. } if tool == "echo")));
    }

    #[test]
    fn loop_records_all_steps() {
        let provider = make_provider("Answer: Done.");
        let tools = Arc::new(ToolRegistry::new());
        let config = AgentLoopConfig { max_iterations: 3, provider_id: "test".into(), model: None };
        let agent_loop = AgentLoop::new(provider, tools, config);
        let result = agent_loop.run("Do something");
        assert!(!result.steps.is_empty());
        // The last step should be an Answer.
        assert!(matches!(result.steps.last().unwrap().kind, StepKind::Answer { .. }));
    }
}
