//! Git history — commit log for a file or the repo.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{GitError, GitResult};

/// A single commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Full commit hash.
    pub hash: SmolStr,
    /// Short hash.
    pub short_hash: SmolStr,
    /// Author name.
    pub author: SmolStr,
    /// Author email.
    pub author_email: SmolStr,
    /// Commit date (ISO).
    pub date: SmolStr,
    /// Commit message (first line).
    pub message: SmolStr,
    /// Files changed.
    pub files_changed: u32,
    /// Insertions.
    pub insertions: u32,
    /// Deletions.
    pub deletions: u32,
}

/// File history — list of commits that touched a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileHistory {
    /// File path.
    pub file: SmolStr,
    /// Commits (newest first).
    pub commits: Vec<CommitInfo>,
}

/// Get commit history for a file.
pub fn file_history(repo: &Path, file: &str, limit: usize) -> GitResult<FileHistory> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .arg("log").arg("--format=%H|%h|%an|%ae|%aI|%s")
        .arg("--numstat").arg("-n").arg(limit.to_string())
        .arg("--").arg(file)
        .output()
        .map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let commits = parse_log(&text);
    Ok(FileHistory { file: SmolStr::new(file), commits })
}

/// Get recent commits for the repo.
pub fn log(repo: &Path, limit: usize) -> GitResult<Vec<CommitInfo>> {
    let output = Command::new("git")
        .arg("-C").arg(repo)
        .arg("log").arg("--format=%H|%h|%an|%ae|%aI|%s")
        .arg("--numstat").arg("-n").arg(limit.to_string())
        .output()
        .map_err(|e| GitError::GitNotFound(e.to_string()))?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_log(&text))
}

fn parse_log(text: &str) -> Vec<CommitInfo> {
    let mut commits = Vec::new();
    let mut current: Option<CommitInfo> = None;
    for line in text.lines() {
        if line.contains('|') && line.split('|').count() == 6 {
            // New commit line.
            if let Some(c) = current.take() { commits.push(c); }
            let parts: Vec<&str> = line.splitn(6, '|').collect();
            current = Some(CommitInfo {
                hash: SmolStr::new(parts[0]),
                short_hash: SmolStr::new(parts[1]),
                author: SmolStr::new(parts[2]),
                author_email: SmolStr::new(parts[3]),
                date: SmolStr::new(parts[4]),
                message: SmolStr::new(parts[5]),
                files_changed: 0, insertions: 0, deletions: 0,
            });
        } else if !line.is_empty() && !line.starts_with("commit ") {
            // numstat line: "insertions\tdeletions\tfile"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() == 3 {
                if let Some(ref mut c) = current {
                    c.files_changed += 1;
                    c.insertions += parts[0].parse::<u32>().unwrap_or(0);
                    c.deletions += parts[1].parse::<u32>().unwrap_or(0);
                }
            }
        }
    }
    if let Some(c) = current { commits.push(c); }
    commits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_format() {
        let text = "abcdef|abc123|Alice|alice@example.com|2024-01-01|Initial commit\n1\t0\tmain.rs\n";
        let commits = parse_log(text);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].author, "Alice");
        assert_eq!(commits[0].message, "Initial commit");
        assert_eq!(commits[0].files_changed, 1);
        assert_eq!(commits[0].insertions, 1);
    }
}
