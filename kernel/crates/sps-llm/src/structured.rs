//! Structured output — JSON mode with schema validation.
//!
//! When you need the LLM to return structured data (e.g. a goal
//! decomposition, a plan, a code review), you can request structured
//! output. This module:
//!
//! 1. Builds a system prompt that instructs the LLM to emit JSON
//!    conforming to a schema.
//! 2. Parses the LLM's response into a typed value.
//! 3. Validates the result against the schema.

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{LlmError, LlmResult};
use crate::tools::ToolSchema;

/// A JSON schema for structured output.
pub type JsonSchema = ToolSchema;

/// A structured output request — wraps a JSON schema + instructions.
#[derive(Debug, Clone)]
pub struct StructuredOutput {
    /// The schema the LLM should conform to.
    pub schema: JsonSchema,
    /// Additional instructions (prepended to the schema prompt).
    pub instructions: String,
}

impl StructuredOutput {
    /// Create a new structured output request.
    pub fn new(schema: JsonSchema, instructions: impl Into<String>) -> Self {
        Self {
            schema,
            instructions: instructions.into(),
        }
    }

    /// Build the system prompt to send to the LLM.
    pub fn system_prompt(&self) -> String {
        let schema_json = serde_json::to_string_pretty(&self.schema).unwrap_or_default();
        format!(
            "{instructions}\n\n\
             You MUST respond with a single valid JSON object that conforms to this schema. \
             Do not include any text before or after the JSON. \
             Do not wrap it in markdown code fences.\n\n\
             Schema:\n{schema}",
            instructions = self.instructions,
            schema = schema_json
        )
    }

    /// Parse and validate an LLM response against the schema.
    pub fn parse<T: DeserializeOwned>(&self, response: &str) -> LlmResult<T> {
        // Strip markdown code fences if present.
        let cleaned = response.trim();
        let cleaned = cleaned
            .strip_prefix("```json")
            .or_else(|| cleaned.strip_prefix("```"))
            .unwrap_or(cleaned)
            .trim();
        let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();

        // Parse JSON.
        let value: serde_json::Value =
            serde_json::from_str(cleaned).map_err(|e| LlmError::ParseFailure(e.to_string()))?;

        // Validate against schema (basic validation).
        self.validate(&value)?;

        // Deserialize into target type.
        serde_json::from_value(value).map_err(|e| LlmError::ParseFailure(e.to_string()))
    }

    /// Basic schema validation. Checks required fields exist.
    fn validate(&self, value: &serde_json::Value) -> LlmResult<()> {
        if self.schema.schema_type != "object" {
            return Ok(()); // Only validate objects for now.
        }
        let obj = value
            .as_object()
            .ok_or_else(|| LlmError::SchemaValidation("expected object".into()))?;
        for required in &self.schema.required {
            if !obj.contains_key(required) {
                return Err(LlmError::SchemaValidation(format!(
                    "missing required field: {}",
                    required
                )));
            }
        }
        Ok(())
    }
}

/// Convenience: request structured output and parse in one call.
pub async fn request_structured<T: DeserializeOwned + Serialize, P: crate::LlmProvider>(
    provider: &P,
    request: sps_effects::providers::llm::LlmRequest,
    output: &StructuredOutput,
) -> LlmResult<T> {
    // Override the system prompt with the structured output prompt.
    let mut req = request;
    req.system = Some(output.system_prompt());

    let completion = provider.complete(&req).map_err(LlmError::Provider)?;
    output.parse(&completion.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::BTreeMap;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct GoalDecomposition {
        title: String,
        tasks: Vec<String>,
    }

    fn make_schema() -> JsonSchema {
        JsonSchema {
            schema_type: "object".into(),
            properties: {
                let mut props = BTreeMap::new();
                props.insert(
                    "title".into(),
                    crate::tools::ToolProperty {
                        prop_type: "string".into(),
                        description: "Goal title.".into(),
                        enum_values: vec![],
                    },
                );
                props.insert(
                    "tasks".into(),
                    crate::tools::ToolProperty {
                        prop_type: "array".into(),
                        description: "List of task descriptions.".into(),
                        enum_values: vec![],
                    },
                );
                props
            },
            required: vec!["title".into(), "tasks".into()],
        }
    }

    #[test]
    fn parse_valid_json_response() {
        let schema = make_schema();
        let output = StructuredOutput::new(schema, "Decompose the goal.");
        let response = r#"{"title":"Build app","tasks":["setup","code","test"]}"#;
        let parsed: GoalDecomposition = output.parse(response).unwrap();
        assert_eq!(parsed.title, "Build app");
        assert_eq!(parsed.tasks.len(), 3);
    }

    #[test]
    fn parse_strips_markdown_fences() {
        let schema = make_schema();
        let output = StructuredOutput::new(schema, "Decompose.");
        let response = "```json\n{\"title\":\"X\",\"tasks\":[\"a\"]}\n```";
        let parsed: GoalDecomposition = output.parse(response).unwrap();
        assert_eq!(parsed.title, "X");
    }

    #[test]
    fn parse_fails_on_invalid_json() {
        let schema = make_schema();
        let output = StructuredOutput::new(schema, "Decompose.");
        let result: Result<GoalDecomposition, _> = output.parse("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn parse_fails_on_missing_required_field() {
        let schema = make_schema();
        let output = StructuredOutput::new(schema, "Decompose.");
        let result: Result<GoalDecomposition, _> = output.parse(r#"{"title":"X"}"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tasks"));
    }

    #[test]
    fn system_prompt_includes_schema_and_instructions() {
        let schema = make_schema();
        let output = StructuredOutput::new(schema, "My custom instructions.");
        let prompt = output.system_prompt();
        assert!(prompt.contains("My custom instructions."));
        assert!(prompt.contains("title"));
        assert!(prompt.contains("tasks"));
        assert!(prompt.contains("JSON"));
    }
}
