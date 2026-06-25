//! Shell executor.

use std::process::Command;
use std::sync::Arc;

use crate::effect::{EffectError, EffectIntent, EffectResult, EffectType};
use crate::registry::EffectExecutor;
use serde::{Deserialize, Serialize};

/// Shell executor. Runs real subprocess commands.
pub struct ShellExecutor {
    /// Workspace root — default cwd for commands.
    root: std::path::PathBuf,
}

impl ShellExecutor {
    /// Create a new shell executor scoped to the given root.
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ShellInput {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

impl EffectExecutor for ShellExecutor {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        if intent.effect_type != EffectType::ShellExec {
            return Err(EffectError::NoExecutor(intent.effect_type.as_str().to_string()));
        }
        let start = std::time::Instant::now();
        let input: ShellInput = serde_json::from_value(intent.input.clone()).map_err(|e| {
            EffectError::ExecutorFailed {
                message: format!("invalid shell.exec input: {}", e),
                details: None,
            }
        })?;

        let cwd = input
            .cwd
            .as_ref()
            .map(|p| std::path::PathBuf::from(p))
            .unwrap_or_else(|| self.root.clone());

        let mut cmd = Command::new(&input.command);
        cmd.args(&input.args).current_dir(&cwd);
        for (k, v) in &input.env {
            cmd.env(k, v);
        }

        let output = cmd.output().map_err(|e| EffectError::ExecutorFailed {
            message: format!("shell.exec {}: {}", input.command, e),
            details: None,
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let result_json = serde_json::json!({
            "command": input.command,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "success": output.status.success(),
        });

        Ok(EffectResult {
            intent_tick,
            output: result_json,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[allow(dead_code)]
pub fn shared(root: std::path::PathBuf) -> Arc<ShellExecutor> {
    Arc::new(ShellExecutor::new(root))
}
