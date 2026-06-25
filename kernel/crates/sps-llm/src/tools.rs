//! Tool use / function calling.
//!
//! Tools are functions the LLM can call. Each tool has:
//! - A name
//! - A JSON schema describing its parameters
//! - A handler that executes the tool
//!
//! The `ToolRegistry` holds all available tools. When the LLM decides
//! to call a tool, the runtime:
//! 1. Parses the LLM's response for a tool call
//! 2. Looks up the tool in the registry
//! 3. Executes the tool with the parsed arguments
//! 4. Returns the result as a `ToolResult`
//! 5. Feeds the result back into the conversation

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// A tool definition — name + description + parameter schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g. "read_file", "run_command").
    pub name: String,
    /// Human-readable description (shown to the LLM).
    pub description: String,
    /// JSON schema for the parameters.
    pub schema: ToolSchema,
}

/// A simplified JSON schema for tool parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Schema type (usually "object").
    #[serde(default = "default_object")]
    pub schema_type: String,
    /// Properties.
    #[serde(default)]
    pub properties: BTreeMap<String, ToolProperty>,
    /// Required property names.
    #[serde(default)]
    pub required: Vec<String>,
}

fn default_object() -> String {
    "object".to_string()
}

/// A single property in a tool schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProperty {
    /// Property type (string, number, boolean, array, object).
    #[serde(rename = "type")]
    pub prop_type: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Enum values (if any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
}

/// A tool call from the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool name.
    pub name: String,
    /// Arguments as JSON string.
    pub arguments: String,
}

impl ToolCall {
    /// Parse arguments as a typed value.
    pub fn parse_args<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.arguments)
    }
}

/// A tool execution result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool name that was called.
    pub name: String,
    /// Whether execution succeeded.
    pub success: bool,
    /// Result content (text or JSON string).
    pub content: String,
    /// Error message (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn success(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            success: true,
            content: content.into(),
            error: None,
        }
    }

    /// Create an error result.
    pub fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            success: false,
            content: String::new(),
            error: Some(message.into()),
        }
    }
}

/// A tool handler trait. Implementations execute the tool.
pub trait Tool: Send + Sync + 'static {
    /// The tool's definition.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given arguments (JSON string).
    fn execute(&self, arguments: &str) -> ToolResult;
}

/// Registry of available tools.
pub struct ToolRegistry {
    tools: RwLock<BTreeMap<String, Arc<dyn Tool>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(BTreeMap::new()),
        }
    }

    /// Register a tool.
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.definition().name;
        self.tools.write().insert(name, tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().get(name).cloned()
    }

    /// List all tool definitions (for sending to the LLM).
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .values()
            .map(|t| t.definition())
            .collect()
    }

    /// Execute a tool call.
    pub fn execute(&self, call: &ToolCall) -> ToolResult {
        match self.get(&call.name) {
            Some(tool) => tool.execute(&call.arguments),
            None => ToolResult::error(
                &call.name,
                format!("tool '{}' not found in registry", call.name),
            ),
        }
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.read().len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.tools.read().is_empty()
    }
}

/// Parse a tool call from an LLM response. The LLM is instructed to
/// emit tool calls in the format:
///
/// ```json
/// {"tool": "tool_name", "arguments": {...}}
/// ```
///
/// This function attempts to find and parse such JSON in the response.
/// Returns `None` if no tool call is found.
pub fn parse_tool_call(response: &str) -> Option<ToolCall> {
    // Look for a JSON object with "tool" and "arguments" keys.
    // We scan for the first `{` and try to parse forward.
    let mut depth = 0i32;
    let mut start = None;
    for (i, c) in response.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        let candidate = &response[s..=i];
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
                            if let (Some(tool), Some(args)) = (
                                v.get("tool").and_then(|t| t.as_str()),
                                v.get("arguments"),
                            ) {
                                return Some(ToolCall {
                                    name: tool.to_string(),
                                    arguments: serde_json::to_string(args).unwrap_or_default(),
                                });
                            }
                        }
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }
    None
}

/// Build a system prompt that tells the LLM about available tools.
pub fn build_tool_prompt(tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    let mut prompt = String::from(
        "You have access to the following tools. To call a tool, respond with a JSON object in this exact format:\n\
         {\"tool\": \"<name>\", \"arguments\": {<args>}}\n\n\
         Available tools:\n\n",
    );
    for tool in tools {
        prompt.push_str(&format!(
            "### {}\n{}\n\nParameters:\n{}\n\n",
            tool.name,
            tool.description,
            serde_json::to_string_pretty(&tool.schema).unwrap_or_default()
        ));
    }
    prompt.push_str(
        "When you want to use a tool, emit ONLY the JSON tool call. \
         When you have a final answer for the user, respond with normal text.",
    );
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".into(),
                description: "Echoes back the input message.".into(),
                schema: ToolSchema {
                    schema_type: "object".into(),
                    properties: {
                        let mut props = BTreeMap::new();
                        props.insert(
                            "message".into(),
                            ToolProperty {
                                prop_type: "string".into(),
                                description: "The message to echo.".into(),
                                enum_values: vec![],
                            },
                        );
                        props
                    },
                    required: vec!["message".into()],
                },
            }
        }

        fn execute(&self, arguments: &str) -> ToolResult {
            #[derive(Deserialize)]
            struct Args {
                message: String,
            }
            match serde_json::from_str::<Args>(arguments) {
                Ok(args) => ToolResult::success("echo", args.message),
                Err(e) => ToolResult::error("echo", e.to_string()),
            }
        }
    }

    #[test]
    fn registry_registers_and_executes() {
        let reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        assert_eq!(reg.len(), 1);

        let call = ToolCall {
            name: "echo".into(),
            arguments: r#"{"message":"hello"}"#.into(),
        };
        let result = reg.execute(&call);
        assert!(result.success);
        assert_eq!(result.content, "hello");
    }

    #[test]
    fn registry_returns_error_for_unknown_tool() {
        let reg = ToolRegistry::new();
        let call = ToolCall {
            name: "nonexistent".into(),
            arguments: "{}".into(),
        };
        let result = reg.execute(&call);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[test]
    fn parse_tool_call_extracts_json() {
        let response = r#"I'll echo that for you. {"tool": "echo", "arguments": {"message": "hi"}}"#;
        let call = parse_tool_call(response).unwrap();
        assert_eq!(call.name, "echo");
        assert!(call.arguments.contains("hi"));
    }

    #[test]
    fn parse_tool_call_returns_none_for_no_json() {
        assert!(parse_tool_call("just a regular response").is_none());
    }

    #[test]
    fn parse_tool_call_returns_none_for_non_tool_json() {
        assert!(parse_tool_call(r#"{"foo": "bar"}"#).is_none());
    }

    #[test]
    fn build_tool_prompt_includes_all_tools() {
        let reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        let prompt = build_tool_prompt(&reg.definitions());
        assert!(prompt.contains("echo"));
        assert!(prompt.contains("Echoes back"));
        assert!(prompt.contains("message"));
    }

    #[test]
    fn tool_call_parse_args_deserializes() {
        let call = ToolCall {
            name: "echo".into(),
            arguments: r#"{"message":"hello"}"#.into(),
        };
        #[derive(Deserialize)]
        struct Args {
            message: String,
        }
        let args: Args = call.parse_args().unwrap();
        assert_eq!(args.message, "hello");
    }
}
