//! Autonomy sandbox — restricts autonomous effects to a workspace.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A sandbox boundary — paths the autonomous runner may touch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SandboxBoundary {
    /// Allowed roots (absolute paths).
    pub allowed_roots: Vec<PathBuf>,
    /// Denied paths (absolute — overrides allowed_roots).
    pub denied_paths: Vec<PathBuf>,
}

impl Default for SandboxBoundary {
    fn default() -> Self {
        Self {
            allowed_roots: vec![],
            denied_paths: vec![],
        }
    }
}

impl SandboxBoundary {
    /// Create a new boundary with the given allowed roots.
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            allowed_roots,
            denied_paths: vec![],
        }
    }

    /// Check whether a path is within the boundary.
    pub fn contains(&self, path: &PathBuf) -> bool {
        // Denied takes precedence.
        for denied in &self.denied_paths {
            if path.starts_with(denied) {
                return false;
            }
        }
        if self.allowed_roots.is_empty() {
            return false; // nothing allowed
        }
        for allowed in &self.allowed_roots {
            if path.starts_with(allowed) {
                return true;
            }
        }
        false
    }

    /// Add an allowed root.
    pub fn allow(&mut self, root: PathBuf) {
        self.allowed_roots.push(root);
    }

    /// Add a denied path.
    pub fn deny(&mut self, path: PathBuf) {
        self.denied_paths.push(path);
    }
}

/// A sandbox violation — an effect attempted to touch a path outside
/// the boundary.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SandboxViolation {
    /// Path is outside the allowed roots.
    #[error("path {:?} is outside the sandbox boundary", path)]
    OutsideBoundary {
        /// The offending path.
        path: PathBuf,
    },
    /// Path is explicitly denied.
    #[error("path {:?} is denied by the sandbox", path)]
    Denied {
        /// The offending path.
        path: PathBuf,
    },
}

/// The sandbox — enforces the boundary on every effect.
pub struct AutonomySandbox {
    boundary: parking_lot::RwLock<SandboxBoundary>,
}

impl Default for AutonomySandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomySandbox {
    /// Create a new sandbox with an empty boundary.
    pub fn new() -> Self {
        Self {
            boundary: parking_lot::RwLock::new(SandboxBoundary::default()),
        }
    }

    /// Create a sandbox with the given boundary.
    pub fn with_boundary(boundary: SandboxBoundary) -> Self {
        Self {
            boundary: parking_lot::RwLock::new(boundary),
        }
    }

    /// Check a path against the sandbox. Returns Ok(()) if allowed.
    pub fn check(&self, path: &PathBuf) -> Result<(), SandboxViolation> {
        let boundary = self.boundary.read();
        if !boundary.contains(path) {
            // Determine if it's denied or just outside.
            for denied in &boundary.denied_paths {
                if path.starts_with(denied) {
                    return Err(SandboxViolation::Denied {
                        path: path.clone(),
                    });
                }
            }
            return Err(SandboxViolation::OutsideBoundary {
                path: path.clone(),
            });
        }
        Ok(())
    }

    /// Update the boundary.
    pub fn set_boundary(&self, boundary: SandboxBoundary) {
        *self.boundary.write() = boundary;
    }

    /// Get the current boundary.
    pub fn boundary(&self) -> SandboxBoundary {
        self.boundary.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_allows_paths_within_root() {
        let boundary = SandboxBoundary::new(vec![PathBuf::from("/workspace")]);
        let sandbox = AutonomySandbox::with_boundary(boundary);
        sandbox
            .check(&PathBuf::from("/workspace/src/main.rs"))
            .expect("path within root should be allowed");
    }

    #[test]
    fn sandbox_rejects_paths_outside_root() {
        let boundary = SandboxBoundary::new(vec![PathBuf::from("/workspace")]);
        let sandbox = AutonomySandbox::with_boundary(boundary);
        let result = sandbox.check(&PathBuf::from("/etc/passwd"));
        assert!(result.is_err());
    }

    #[test]
    fn sandbox_denied_paths_override_allowed() {
        let mut boundary = SandboxBoundary::new(vec![PathBuf::from("/workspace")]);
        boundary.deny(PathBuf::from("/workspace/secrets"));
        let sandbox = AutonomySandbox::with_boundary(boundary);
        let result = sandbox.check(&PathBuf::from("/workspace/secrets/key.pem"));
        assert!(matches!(result, Err(SandboxViolation::Denied { .. })));
    }
}
