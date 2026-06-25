//! File operations — read, write, delete (within workspace boundary).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{WorkspaceError, WorkspaceResult};

/// File content (text or binary metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    /// Relative path.
    pub path: String,
    /// Content as UTF-8 string (None if binary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// File size in bytes.
    pub size: u64,
    /// Whether the file is text.
    pub is_text: bool,
    /// Line count (for text files).
    pub lines: usize,
}

/// File operations — all paths are resolved relative to the workspace root
/// and checked for boundary violations.
pub struct FileOps {
    root: PathBuf,
}

impl FileOps {
    /// Create a new FileOps bound to the given root.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolve a relative path within the workspace, checking it doesn't escape.
    pub fn resolve(&self, relative: &str) -> WorkspaceResult<PathBuf> {
        // Empty path = root.
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        let path = self.root.join(relative);
        // For paths that don't exist yet (write), canonicalize the parent.
        let parent = path.parent().unwrap_or(&self.root);
        // If parent doesn't exist, try to create it (for write operations).
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
        let canon_parent = parent
            .canonicalize()
            .map_err(|_| WorkspaceError::NotFound(parent.to_string_lossy().to_string()))?;
        let canon_root = self
            .root
            .canonicalize()
            .map_err(|_| WorkspaceError::NotAWorkspace(self.root.to_string_lossy().to_string()))?;
        if !canon_parent.starts_with(&canon_root) {
            return Err(WorkspaceError::EscapesWorkspace(relative.to_string()));
        }
        Ok(path)
    }

    /// Read a file as text.
    pub fn read(&self, relative: &str) -> WorkspaceResult<FileContent> {
        let path = self.resolve(relative)?;
        if !path.exists() {
            return Err(WorkspaceError::NotFound(relative.to_string()));
        }
        let metadata = std::fs::metadata(&path)?;
        let size = metadata.len();
        let bytes = std::fs::read(&path)?;
        let is_text = is_text_file(&bytes, relative);
        let (text, lines) = if is_text {
            let s = String::from_utf8_lossy(&bytes).to_string();
            let lines = s.lines().count();
            (Some(s), lines)
        } else {
            (None, 0)
        };
        Ok(FileContent {
            path: relative.to_string(),
            text,
            size,
            is_text,
            lines,
        })
    }

    /// Write text to a file.
    pub fn write(&self, relative: &str, content: &str) -> WorkspaceResult<u64> {
        let path = self.resolve(relative)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(content.len() as u64)
    }

    /// Delete a file.
    pub fn delete(&self, relative: &str) -> WorkspaceResult<()> {
        let path = self.resolve(relative)?;
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Check if a file exists.
    pub fn exists(&self, relative: &str) -> bool {
        self.resolve(relative).map(|p| p.exists()).unwrap_or(false)
    }

    /// List files in a directory (non-recursive).
    pub fn list_dir(&self, relative: &str) -> WorkspaceResult<Vec<DirEntry>> {
        let path = self.resolve(relative)?;
        let entries = std::fs::read_dir(&path)?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata()?;
            let rel = if relative.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", relative, name)
            };
            result.push(DirEntry {
                name,
                path: rel,
                is_dir: metadata.is_dir(),
                size: if metadata.is_dir() { 0 } else { metadata.len() },
            });
        }
        result.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        Ok(result)
    }
}

/// A directory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    /// Entry name.
    pub name: String,
    /// Relative path.
    pub path: String,
    /// Whether it's a directory.
    pub is_dir: bool,
    /// Size in bytes (0 for dirs).
    pub size: u64,
}

/// Check if a file is likely text (not binary).
fn is_text_file(bytes: &[u8], path: &str) -> bool {
    // Check extension first.
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    let text_exts = [
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h", "hpp",
        "md", "txt", "json", "yaml", "yml", "toml", "html", "css", "scss", "xml",
        "sh", "bash", "zsh", "fish", "sql", "csv", "tsv", "env", "gitignore",
        "lock", "log", "conf", "ini", "cfg",
    ];
    if text_exts.contains(&ext) {
        return true;
    }
    // Heuristic: if the first 1024 bytes contain null bytes, it's binary.
    let check_len = bytes.len().min(1024);
    for &b in &bytes[..check_len] {
        if b == 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_write_round_trip() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        ops.write("test.txt", "hello world").unwrap();
        let content = ops.read("test.txt").unwrap();
        assert_eq!(content.text.unwrap(), "hello world");
        assert_eq!(content.size, 11);
        assert!(content.is_text);
    }

    #[test]
    fn read_nonexistent_fails() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        assert!(ops.read("nonexistent.txt").is_err());
    }

    #[test]
    fn resolve_rejects_path_escaping_workspace() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        let result = ops.resolve("../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn delete_removes_file() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        ops.write("temp.txt", "temp").unwrap();
        assert!(ops.exists("temp.txt"));
        ops.delete("temp.txt").unwrap();
        assert!(!ops.exists("temp.txt"));
    }

    #[test]
    fn list_dir_returns_entries() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        ops.write("a.txt", "a").unwrap();
        ops.write("b.txt", "b").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let entries = ops.list_dir("").unwrap();
        assert_eq!(entries.len(), 3);
        // Dirs should come first.
        assert!(entries[0].is_dir);
    }

    #[test]
    fn write_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let ops = FileOps::new(dir.path().to_path_buf());
        ops.write("nested/deep/file.txt", "content").unwrap();
        assert!(ops.exists("nested/deep/file.txt"));
    }

    #[test]
    fn is_text_file_detects_binary() {
        let binary = vec![0x00, 0x01, 0x02, 0x00];
        assert!(!is_text_file(&binary, "file.bin"));
        let text = b"hello world";
        assert!(is_text_file(text, "file.txt"));
    }
}
