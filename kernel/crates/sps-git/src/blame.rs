//! Git blame — per-line authorship info.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{GitError, GitResult};

/// Blame info for a single line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlameLine {
    /// Line number (1-indexed).
    pub line: u32,
    /// Commit hash.
    pub commit: SmolStr,
    /// Author name.
    pub author: SmolStr,
    /// Author email.
    pub author_email: SmolStr,
    /// Commit date (ISO format).
    pub date: SmolStr,
    /// Commit summary (first line).
    pub summary: SmolStr,
    /// Line content.
    pub content: SmolStr,
}

/// Full blame info for a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlameInfo {
    /// File path.
    pub file: SmolStr,
    /// Blame per line.
    pub lines: Vec<BlameLine>,
}

/// Run `git blame` on a file.
pub fn blame(repo: &Path, file: &str) -> GitResult<BlameInfo> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("blame")
        .arg("--porcelain")
        .arg("--")
        .arg(file)
        .output()
        .map_err(|e| GitError::GitNotFound(e.to_string()))?;

    if !output.status.success() {
        return Err(GitError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let lines = parse_blame(&text);
    Ok(BlameInfo {
        file: SmolStr::new(file),
        lines,
    })
}

fn parse_blame(text: &str) -> Vec<BlameLine> {
    let mut result = Vec::new();
    let mut current_commit = String::new();
    let mut current_author = String::new();
    let mut current_email = String::new();
    let mut current_date = String::new();
    let mut current_summary = String::new();

    for line in text.lines() {
        if line.starts_with("author ") {
            current_author = line[7..].to_string();
        } else if line.starts_with("author-mail ") {
            current_email = line[12..].trim_matches('<').trim_matches('>').to_string();
        } else if line.starts_with("committer-time ") {
            // Store as timestamp — could convert to date.
            current_date = line[14..].to_string();
        } else if line.starts_with("summary ") {
            current_summary = line[8..].to_string();
        } else if line.starts_with('\t') {
            // This is the actual content line.
            let content = &line[1..];
            let line_num = result.len() as u32 + 1;
            result.push(BlameLine {
                line: line_num,
                commit: SmolStr::new(&current_commit),
                author: SmolStr::new(&current_author),
                author_email: SmolStr::new(&current_email),
                date: SmolStr::new(&current_date),
                summary: SmolStr::new(&current_summary),
                content: SmolStr::new(content),
            });
        } else if !line.is_empty() && !line.starts_with(' ') {
            // Commit hash line: "hash origline finalline"
            let parts: Vec<_> = line.split_whitespace().collect();
            if let Some(hash) = parts.first() {
                current_commit = hash.to_string();
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_blame_porcelain() {
        let text = "abc1234 1 1\nauthor Alice\nauthor-mail <alice@example.com>\nsummary Initial commit\n\tfn main() {}\n";
        let lines = parse_blame(text);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].author, "Alice");
        assert_eq!(lines[0].content, "fn main() {}");
        assert_eq!(lines[0].line, 1);
    }
}
