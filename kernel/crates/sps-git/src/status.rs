//! Git status — working tree state.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{GitError, GitResult};

/// Kind of status change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusKind {
    /// Modified.
    Modified,
    /// Added (staged).
    Added,
    /// Deleted.
    Deleted,
    /// Renamed.
    Renamed,
    /// Untracked.
    Untracked,
    /// Copied.
    Copied,
    /// Type changed.
    TypeChanged,
    /// Conflicted.
    Conflicted,
}

/// A single status entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    /// File path.
    pub file: SmolStr,
    /// Kind of change.
    pub kind: StatusKind,
    /// Whether the file is staged.
    pub staged: bool,
}

/// Full git status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitStatus {
    /// All changed files.
    pub entries: Vec<StatusEntry>,
    /// Current branch.
    pub branch: Option<SmolStr>,
    /// Whether the working tree is clean.
    pub is_clean: bool,
}

/// Get git status.
pub fn status(repo: &Path) -> GitResult<GitStatus> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .arg("status").arg("--porcelain=v1").arg("-b")
        .output().map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut branch = None;
    for line in text.lines() {
        if line.starts_with("## ") {
            // Branch line.
            let rest = &line[3..];
            branch = SmolStr::try_from(rest.split("...").next().unwrap_or(rest).to_string()).ok();
            continue;
        }
        if line.len() < 3 { continue; }
        let x = line.chars().next().unwrap();
        let y = line.chars().nth(1).unwrap();
        let file = line[3..].trim().to_string();
        let (kind, staged) = if x == '?' && y == '?' {
            (StatusKind::Untracked, false)
        } else if x == 'A' { (StatusKind::Added, true) }
        else if x == 'M' { (StatusKind::Modified, true) }
        else if x == 'D' { (StatusKind::Deleted, true) }
        else if x == 'R' { (StatusKind::Renamed, true) }
        else if x == 'C' { (StatusKind::Copied, true) }
        else if y == 'M' { (StatusKind::Modified, false) }
        else if y == 'D' { (StatusKind::Deleted, false) }
        else { (StatusKind::Modified, false) };
        entries.push(StatusEntry { file: SmolStr::new(file), kind, staged });
    }
    let is_clean = entries.is_empty();
    Ok(GitStatus { entries, branch, is_clean })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::process::Command;

    #[test]
    fn status_of_clean_repo() {
        let dir = tempdir().unwrap();
        Command::new("git").arg("init").arg(dir.path()).output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.email").arg("t@t.com").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.name").arg("T").output().unwrap();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("add").arg(".").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("commit").arg("-m").arg("init").output().unwrap();
        let s = status(dir.path()).unwrap();
        assert!(s.is_clean);
    }

    #[test]
    fn status_with_changes() {
        let dir = tempdir().unwrap();
        Command::new("git").arg("init").arg(dir.path()).output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.email").arg("t@t.com").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("config").arg("user.name").arg("T").output().unwrap();
        std::fs::write(dir.path().join("f.txt"), "x").unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("add").arg(".").output().unwrap();
        Command::new("git").arg("-C").arg(dir.path()).arg("commit").arg("-m").arg("init").output().unwrap();
        // Modify + add untracked.
        std::fs::write(dir.path().join("f.txt"), "y").unwrap();
        std::fs::write(dir.path().join("new.txt"), "new").unwrap();
        let s = status(dir.path()).unwrap();
        assert!(!s.is_clean);
        assert_eq!(s.entries.len(), 2);
    }
}
