//! Filesystem executor.

use std::path::PathBuf;
use std::sync::Arc;

use crate::effect::{EffectError, EffectIntent, EffectResult};
use crate::registry::{map_anyhow_err, EffectExecutor};
use serde::{Deserialize, Serialize};

/// Filesystem executor. Performs real fs reads/writes.
pub struct FsExecutor {
    /// Workspace root — all paths are resolved relative to this.
    /// Operations outside this root are rejected (governance).
    root: PathBuf,
}

impl FsExecutor {
    /// Create a new executor scoped to the given root.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolve a path relative to root, ensuring it doesn't escape.
    fn resolve(&self, p: &str) -> Result<PathBuf, EffectError> {
        let candidate = if PathBuf::from(p).is_absolute() {
            PathBuf::from(p)
        } else {
            self.root.join(p)
        };
        // Security: canonicalize and check containment (best-effort).
        // For paths that don't yet exist (fs.write), we canonicalize the parent.
        let parent = candidate.parent().unwrap_or(&self.root);
        let canon_parent = parent.canonicalize().map_err(|e| EffectError::ExecutorFailed {
            message: format!("cannot canonicalize parent of {}: {}", p, e),
            details: None,
        })?;
        let joined = canon_parent.join(candidate.file_name().unwrap_or_default());
        if !joined.starts_with(&self.root) {
            return Err(EffectError::GovernanceDenied(format!(
                "path {} escapes workspace root",
                p
            )));
        }
        Ok(joined)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FsReadInput {
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FsWriteInput {
    path: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FsDeleteInput {
    path: String,
}

impl EffectExecutor for FsExecutor {
    fn name(&self) -> &'static str {
        "fs"
    }

    fn execute(&self, intent: &EffectIntent, intent_tick: u64) -> Result<EffectResult, EffectError> {
        let start = std::time::Instant::now();
        let output = match intent.effect_type {
            crate::effect::EffectType::FsRead => {
                let input: FsReadInput = serde_json::from_value(intent.input.clone())
                    .map_err(|e| EffectError::ExecutorFailed {
                        message: format!("invalid fs.read input: {}", e),
                        details: None,
                    })?;
                let path = self.resolve(&input.path)?;
                let content = std::fs::read_to_string(&path).map_err(|e| EffectError::ExecutorFailed {
                    message: format!("fs.read {}: {}", input.path, e),
                    details: None,
                })?;
                serde_json::json!({
                    "path": input.path,
                    "content": content,
                    "size": content.len(),
                })
            }
            crate::effect::EffectType::FsWrite => {
                let input: FsWriteInput = serde_json::from_value(intent.input.clone())
                    .map_err(|e| EffectError::ExecutorFailed {
                        message: format!("invalid fs.write input: {}", e),
                        details: None,
                    })?;
                let path = self.resolve(&input.path)?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(map_anyhow_err)?;
                }
                std::fs::write(&path, &input.content).map_err(|e| EffectError::ExecutorFailed {
                    message: format!("fs.write {}: {}", input.path, e),
                    details: None,
                })?;
                serde_json::json!({
                    "path": input.path,
                    "bytes_written": input.content.len(),
                })
            }
            crate::effect::EffectType::FsDelete => {
                let input: FsDeleteInput = serde_json::from_value(intent.input.clone())
                    .map_err(|e| EffectError::ExecutorFailed {
                        message: format!("invalid fs.delete input: {}", e),
                        details: None,
                    })?;
                let path = self.resolve(&input.path)?;
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).map_err(map_anyhow_err)?;
                } else {
                    std::fs::remove_file(&path).map_err(map_anyhow_err)?;
                }
                serde_json::json!({"path": input.path, "deleted": true})
            }
            _ => {
                return Err(EffectError::NoExecutor(intent.effect_type.as_str().to_string()));
            }
        };
        Ok(EffectResult {
            intent_tick,
            output,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// Convenience for the registry registration helper.
#[allow(dead_code)]
pub fn shared(root: PathBuf) -> Arc<FsExecutor> {
    Arc::new(FsExecutor::new(root))
}
