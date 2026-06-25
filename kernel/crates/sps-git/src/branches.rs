//! Git branches — list, create, switch.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{GitError, GitResult};

/// A git branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Branch name.
    pub name: SmolStr,
    /// Whether this is the current branch.
    pub is_current: bool,
    /// Whether this is a remote branch.
    pub is_remote: bool,
    /// Last commit hash.
    pub last_commit: SmolStr,
}

/// Branch list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchList {
    /// All branches.
    pub branches: Vec<Branch>,
    /// Current branch name.
    pub current: Option<SmolStr>,
}

/// List all branches.
pub fn list_branches(repo: &Path) -> GitResult<BranchList> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .arg("branch").arg("--list").arg("--all").arg("-v")
        .arg("--format=%(HEAD)%(refname:short) %(objectname:short) %(upstream:short)")
        .output()
        .map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();
    let mut current = None;
    for line in text.lines() {
        let is_current = line.starts_with('*');
        let rest = line.trim_start_matches('*').trim();
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() { continue; }
        let name = SmolStr::new(parts[0]);
        let last_commit = parts.get(1).map(|s| SmolStr::new(s)).unwrap_or_default();
        let is_remote = name.contains('/');
        if is_current { current = Some(name.clone()); }
        branches.push(Branch { name, is_current, is_remote, last_commit });
    }
    Ok(BranchList { branches, current })
}

/// Create a new branch.
pub fn create_branch(repo: &Path, name: &str) -> GitResult<()> {
    let output = Command::new("git")
        .arg("-C").arg(repo).arg("checkout").arg("-b").arg(name)
        .output().map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    Ok(())
}

/// Switch to a branch.
pub fn switch_branch(repo: &Path, name: &str) -> GitResult<()> {
    let output = Command::new("git")
        .arg("-C").arg(repo).arg("checkout").arg(name)
        .output().map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::process::Command;

    #[test]
    fn list_branches_in_real_repo() {
        let dir = tempdir().unwrap();
        // Init a git repo.
        Command::new("git").arg("init").arg(dir.path()).output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.email").arg("test@test.com").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.name").arg("Test").output().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("add").arg(".").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("commit").arg("-m").arg("init").output().unwrap();

        let result = list_branches(dir.path()).unwrap();
        assert!(!result.branches.is_empty());
        assert!(result.current.is_some());
    }
}
