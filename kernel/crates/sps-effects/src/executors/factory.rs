//! Phase 11B: Factory effect executor.
//!
//! Handles 4 factory-specific effect types:
//! - WriteFile: writes a generated file to disk (returns bytes written)
//! - RunTests: runs cargo test / npm test (returns pass/fail + output)
//! - BuildProject: runs cargo build --release / npm run build (returns success + output)
//! - PackageProject: packages the project (returns artifact path)
//!
//! All executors are deterministic in test mode (configurable via
//! FactoryExecutorConfig). In production mode, they invoke real shell
//! commands via the existing ShellExecutor.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::effect::{EffectError, EffectIntent, EffectResult};
use crate::registry::EffectExecutor;

/// Configuration for the factory executor.
#[derive(Debug, Clone)]
pub struct FactoryExecutorConfig {
    /// If true, executors return deterministic mock results without
    /// touching the filesystem or running shell commands.
    /// Default: true (safe for tests / replay).
    pub dry_run: bool,
    /// Root directory for file writes.
    pub root: std::path::PathBuf,
}

impl Default for FactoryExecutorConfig {
    fn default() -> Self {
        Self {
            dry_run: true,
            root: std::path::PathBuf::from("/tmp/sps-factory"),
        }
    }
}

/// Factory effect executor.
pub struct FactoryExecutor {
    config: FactoryExecutorConfig,
}

impl FactoryExecutor {
    /// Create a new executor with the given config.
    pub fn new(config: FactoryExecutorConfig) -> Self {
        Self { config }
    }

    /// Create a shared executor (Arc-wrapped).
    pub fn shared(config: FactoryExecutorConfig) -> Arc<Self> {
        Arc::new(Self::new(config))
    }

    fn execute_write_file(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let path = intent.input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| EffectError::ExecutorFailed {
                message: "write_file: missing 'path'".into(),
                details: None,
            })?;
        let content = intent.input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| EffectError::ExecutorFailed {
                message: "write_file: missing 'content'".into(),
                details: None,
            })?;

        let bytes = content.len() as u64;

        if self.config.dry_run {
            // Deterministic mock: don't touch filesystem.
            return Ok(EffectResult {
                intent_tick,
                output: json!({
                    "path": path,
                    "bytes_written": bytes,
                    "dry_run": true,
                }),
                elapsed_ms: 0,
            });
        }

        // Production: write to disk.
        let full_path = self.config.root.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EffectError::ExecutorFailed {
                message: format!("write_file: mkdir failed: {}", e),
                details: None,
            })?;
        }
        std::fs::write(&full_path, content).map_err(|e| EffectError::ExecutorFailed {
            message: format!("write_file: write failed: {}", e),
            details: None,
        })?;

        Ok(EffectResult {
            intent_tick,
            output: json!({
                "path": path,
                "bytes_written": bytes,
                "dry_run": false,
            }),
            elapsed_ms: 0,
        })
    }

    fn execute_run_tests(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let project_path = intent.input.get("project_path").and_then(|v| v.as_str())
            .ok_or_else(|| EffectError::ExecutorFailed {
                message: "run_tests: missing 'project_path'".into(),
                details: None,
            })?;
        let test_framework = intent.input.get("test_framework").and_then(|v| v.as_str())
            .unwrap_or("cargo");

        if self.config.dry_run {
            // Deterministic mock: always pass.
            return Ok(EffectResult {
                intent_tick,
                output: json!({
                    "project_path": project_path,
                    "test_framework": test_framework,
                    "passed": true,
                    "tests_run": 0,
                    "tests_failed": 0,
                    "output": "dry_run mode: tests skipped",
                    "dry_run": true,
                }),
                elapsed_ms: 0,
            });
        }

        // Production: invoke cargo test / npm test.
        let cmd = match test_framework {
            "cargo" => vec!["cargo", "test"],
            "npm" => vec!["npm", "test"],
            other => return Err(EffectError::ExecutorFailed {
                message: format!("run_tests: unknown framework '{}'", other),
                details: None,
            }),
        };
        let output = std::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(project_path)
            .output()
            .map_err(|e| EffectError::ExecutorFailed {
                message: format!("run_tests: spawn failed: {}", e),
                details: None,
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let passed = output.status.success();

        Ok(EffectResult {
            intent_tick,
            output: json!({
                "project_path": project_path,
                "test_framework": test_framework,
                "passed": passed,
                "tests_run": 0, // would parse from stdout in production
                "tests_failed": if passed { 0 } else { 1 },
                "output": stdout,
                "dry_run": false,
            }),
            elapsed_ms: 0,
        })
    }

    fn execute_build_project(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let project_path = intent.input.get("project_path").and_then(|v| v.as_str())
            .ok_or_else(|| EffectError::ExecutorFailed {
                message: "build_project: missing 'project_path'".into(),
                details: None,
            })?;
        let build_system = intent.input.get("build_system").and_then(|v| v.as_str())
            .unwrap_or("cargo");

        if self.config.dry_run {
            return Ok(EffectResult {
                intent_tick,
                output: json!({
                    "project_path": project_path,
                    "build_system": build_system,
                    "success": true,
                    "artifact_path": null,
                    "output": "dry_run mode: build skipped",
                    "dry_run": true,
                }),
                elapsed_ms: 0,
            });
        }

        let cmd = match build_system {
            "cargo" => vec!["cargo", "build", "--release"],
            "npm" => vec!["npm", "run", "build"],
            other => return Err(EffectError::ExecutorFailed {
                message: format!("build_project: unknown build system '{}'", other),
                details: None,
            }),
        };
        let output = std::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(project_path)
            .output()
            .map_err(|e| EffectError::ExecutorFailed {
                message: format!("build_project: spawn failed: {}", e),
                details: None,
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let success = output.status.success();

        Ok(EffectResult {
            intent_tick,
            output: json!({
                "project_path": project_path,
                "build_system": build_system,
                "success": success,
                "artifact_path": if success { Some(format!("{}/target/release", project_path)) } else { None },
                "output": stdout,
                "dry_run": false,
            }),
            elapsed_ms: 0,
        })
    }

    fn execute_package_project(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let project_path = intent.input.get("project_path").and_then(|v| v.as_str())
            .ok_or_else(|| EffectError::ExecutorFailed {
                message: "package_project: missing 'project_path'".into(),
                details: None,
            })?;
        let format = intent.input.get("format").and_then(|v| v.as_str())
            .unwrap_or("tarball");

        if self.config.dry_run {
            return Ok(EffectResult {
                intent_tick,
                output: json!({
                    "project_path": project_path,
                    "format": format,
                    "artifact_path": format!("{}/target/{}.tar.gz", project_path, "package"),
                    "size_bytes": 0,
                    "dry_run": true,
                }),
                elapsed_ms: 0,
            });
        }

        // Production: create tarball.
        let artifact_path = format!("{}/target/package.tar.gz", project_path);
        let output = std::process::Command::new("tar")
            .args(&["-czf", &artifact_path, "-C", project_path, "."])
            .output()
            .map_err(|e| EffectError::ExecutorFailed {
                message: format!("package_project: tar failed: {}", e),
                details: None,
            })?;

        if !output.status.success() {
            return Err(EffectError::ExecutorFailed {
                message: "package_project: tar returned non-zero".into(),
                details: None,
            });
        }

        let size = std::fs::metadata(&artifact_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(EffectResult {
            intent_tick,
            output: json!({
                "project_path": project_path,
                "format": format,
                "artifact_path": artifact_path,
                "size_bytes": size,
                "dry_run": false,
            }),
            elapsed_ms: 0,
        })
    }
}

impl EffectExecutor for FactoryExecutor {
    fn name(&self) -> &'static str {
        "factory"
    }

    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        match intent.effect_type {
            crate::effect::EffectType::WriteFile => self.execute_write_file(intent, intent_tick),
            crate::effect::EffectType::RunTests => self.execute_run_tests(intent, intent_tick),
            crate::effect::EffectType::BuildProject => self.execute_build_project(intent, intent_tick),
            crate::effect::EffectType::PackageProject => self.execute_package_project(intent, intent_tick),
            _ => Err(EffectError::NoExecutor(intent.effect_type.as_str().to_string())),
        }
    }
}

/// Input for a WriteFile effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
    pub project_id: Option<uuid::Uuid>,
    pub factory_run_id: Option<uuid::Uuid>,
}

/// Input for a RunTests effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunTestsInput {
    pub project_path: String,
    pub test_framework: String,
}

/// Input for a BuildProject effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildProjectInput {
    pub project_path: String,
    pub build_system: String,
}

/// Input for a PackageProject effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageProjectInput {
    pub project_path: String,
    pub format: String,
}
