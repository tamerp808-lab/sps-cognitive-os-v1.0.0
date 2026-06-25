//! Git executor — runs git commands via the shell executor's pattern.

use std::process::Command;
use std::sync::Arc;

use crate::effect::{EffectError, EffectIntent, EffectResult, EffectType};
use crate::registry::EffectExecutor;
use serde::{Deserialize, Serialize};

/// Git executor.
pub struct GitExecutor {
    /// Workspace root — the repo path.
    root: std::path::PathBuf,
}

impl GitExecutor {
    /// Create a new git executor.
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GitInput {
    operation: String, // "status", "log", "add", "commit", etc.
    #[serde(default)]
    args: Vec<String>,
}

impl EffectExecutor for GitExecutor {
    fn name(&self) -> &'static str {
        "git"
    }

    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        if intent.effect_type != EffectType::GitOperation {
            return Err(EffectError::NoExecutor(intent.effect_type.as_str().to_string()));
        }
        let start = std::time::Instant::now();
        let input: GitInput = serde_json::from_value(intent.input.clone()).map_err(|e| {
            EffectError::ExecutorFailed {
                message: format!("invalid git.operation input: {}", e),
                details: None,
            }
        })?;

        let mut cmd = Command::new("git");
        cmd.arg(&input.operation).args(&input.args).current_dir(&self.root);

        let output = cmd.output().map_err(|e| EffectError::ExecutorFailed {
            message: format!("git {} {}: {}", input.operation, input.args.join(" "), e),
            details: None,
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(EffectResult {
            intent_tick,
            output: serde_json::json!({
                "operation": input.operation,
                "args": input.args,
                "exit_code": exit_code,
                "stdout": stdout,
                "stderr": stderr,
                "success": output.status.success(),
            }),
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[allow(dead_code)]
pub fn shared(root: std::path::PathBuf) -> Arc<GitExecutor> {
    Arc::new(GitExecutor::new(root))
}
