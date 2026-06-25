//! Prompt building for the ReAct loop.

use sps_llm::tools::{build_tool_prompt, ToolDefinition};

/// Build the system prompt for the ReAct loop.
///
/// The prompt instructs the LLM to:
/// 1. Think step by step (Thought)
/// 2. Take an action (Action) by calling a tool
/// 3. Observe the result (Observation)
/// 4. Repeat until it has a final answer
pub fn build_react_prompt(tools: &[ToolDefinition], task: &str) -> String {
    let tool_prompt = build_tool_prompt(tools);
    format!(
        "You are an autonomous agent executing a task. Use the ReAct (Reasoning + Acting) pattern.\n\n\
         ## Task\n{task}\n\n\
         ## Format\n\
         For each step, respond in ONE of these formats:\n\n\
         ### Thought\n\
         Thought: I need to <reasoning about what to do next>\n\n\
         ### Action (call a tool)\n\
         Action: {{\"tool\": \"<name>\", \"arguments\": {{<args>}}}}\n\n\
         ### Final Answer\n\
         Answer: <your complete answer to the task>\n\n\
         ## Rules\n\
         - Always start with a Thought before an Action.\n\
         - After each Action, you will receive an Observation with the tool's result.\n\
         - Use Observations to decide your next step.\n\
         - When you have enough information, give your final Answer.\n\
         - Be concise but thorough in your reasoning.\n\n\
         {tool_prompt}"
    )
}

/// Parse an LLM response to determine the step kind.
///
/// Returns `Ok(Some(step))` if a valid step was found, `Ok(None)` if
/// the response doesn't match any known format.
pub fn parse_step_response(response: &str) -> Option<crate::step::StepKind> {
    let trimmed = response.trim();

    // Check for "Thought:" prefix.
    if let Some(rest) = trimmed.strip_prefix("Thought:") {
        return Some(crate::step::StepKind::Thought {
            text: rest.trim().to_string(),
        });
    }

    // Check for "Action:" prefix.
    if let Some(rest) = trimmed.strip_prefix("Action:") {
        let rest = rest.trim();
        if let Some(call) = sps_llm::tools::parse_tool_call(rest) {
            return Some(crate::step::StepKind::Action {
                tool: call.name,
                arguments: call.arguments,
            });
        }
    }

    // Check for "Answer:" prefix.
    if let Some(rest) = trimmed.strip_prefix("Answer:") {
        return Some(crate::step::StepKind::Answer {
            text: rest.trim().to_string(),
        });
    }

    // Try to parse as a tool call directly (no prefix).
    if let Some(call) = sps_llm::tools::parse_tool_call(trimmed) {
        return Some(crate::step::StepKind::Action {
            tool: call.name,
            arguments: call.arguments,
        });
    }

    // If it looks like reasoning (multi-line text), treat as a thought.
    if trimmed.len() > 20 && !trimmed.starts_with('{') {
        return Some(crate::step::StepKind::Thought {
            text: trimmed.to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_thought() {
        let step = parse_step_response("Thought: I should search for the function first.").unwrap();
        match step {
            crate::step::StepKind::Thought { text } => {
                assert!(text.contains("search for the function"));
            }
            _ => panic!("expected Thought"),
        }
    }

    #[test]
    fn parse_action() {
        let step = parse_step_response(r#"Action: {"tool": "read_file", "arguments": {"path": "main.rs"}}"#).unwrap();
        match step {
            crate::step::StepKind::Action { tool, arguments } => {
                assert_eq!(tool, "read_file");
                assert!(arguments.contains("main.rs"));
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn parse_answer() {
        let step = parse_step_response("Answer: The function is located at line 42.").unwrap();
        match step {
            crate::step::StepKind::Answer { text } => {
                assert!(text.contains("line 42"));
            }
            _ => panic!("expected Answer"),
        }
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert!(parse_step_response("{not valid}").is_none());
    }

    #[test]
    fn build_prompt_includes_task_and_tools() {
        let tools = vec![sps_llm::tools::ToolDefinition {
            name: "test_tool".into(),
            description: "A test tool.".into(),
            schema: sps_llm::tools::ToolSchema {
                schema_type: "object".into(),
                properties: std::collections::BTreeMap::new(),
                required: vec![],
            },
        }];
        let prompt = build_react_prompt(&tools, "Find all functions in the codebase");
        assert!(prompt.contains("Find all functions"));
        assert!(prompt.contains("test_tool"));
        assert!(prompt.contains("Thought"));
        assert!(prompt.contains("Action"));
        assert!(prompt.contains("Answer"));
    }
}
