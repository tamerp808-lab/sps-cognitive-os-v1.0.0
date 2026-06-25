//! Workspace scanner — recursively scans a directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{WorkspaceError, WorkspaceResult};
use crate::tree::{FileNode, FileTree, NodeKind};

/// Scan configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Maximum depth (0 = unlimited).
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// File extensions to include (empty = all).
    #[serde(default)]
    pub include_extensions: Vec<String>,
    /// Directories to exclude.
    #[serde(default = "default_excludes")]
    pub exclude_dirs: Vec<String>,
    /// Maximum file size to include (bytes, 0 = unlimited).
    #[serde(default)]
    pub max_file_size: u64,
}

fn default_max_depth() -> usize { 20 }

fn default_excludes() -> Vec<String> {
    vec![
        "target".into(),
        "node_modules".into(),
        ".git".into(),
        ".cargo".into(),
        "__pycache__".into(),
        ".next".into(),
        "dist".into(),
        "build".into(),
        ".cache".into(),
    ]
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_depth: default_max_depth(),
            include_extensions: Vec::new(),
            exclude_dirs: default_excludes(),
            max_file_size: 0,
        }
    }
}

/// Workspace scanner.
pub struct WorkspaceScanner {
    config: ScanConfig,
}

impl WorkspaceScanner {
    /// Create a new scanner with the given config.
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Create a scanner with default config.
    pub fn default() -> Self {
        Self::new(ScanConfig::default())
    }

    /// Scan a directory and build a file tree.
    pub fn scan(&self, root: &Path) -> WorkspaceResult<FileTree> {
        if !root.exists() {
            return Err(WorkspaceError::NotFound(root.to_string_lossy().to_string()));
        }
        if !root.is_dir() {
            return Err(WorkspaceError::NotAWorkspace(root.to_string_lossy().to_string()));
        }

        let mut tree = FileTree::new();
        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string());

        let root_node = FileNode {
            path: SmolStr::new(""),
            name: SmolStr::new(&root_name),
            kind: NodeKind::Dir,
            size: 0,
            children: Vec::new(),
            extension: None,
        };
        tree.root = Some(root_node);

        if let Some(ref mut root_node) = tree.root {
            self.scan_dir(root, root_node, "", 0)?;
        }

        tree.rebuild_index();
        Ok(tree)
    }

    fn scan_dir(
        &self,
        dir: &Path,
        node: &mut FileNode,
        relative_path: &str,
        depth: usize,
    ) -> WorkspaceResult<()> {
        if self.config.max_depth > 0 && depth >= self.config.max_depth {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)?;
        let mut entries: Vec<_> = entries.collect::<Result<_, _>>().unwrap_or_default();
        // Sort: dirs first, then files, alphabetically.
        entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();

            // Skip excluded dirs.
            if path.is_dir() && self.config.exclude_dirs.contains(&name) {
                continue;
            }

            let rel = if relative_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", relative_path, name)
            };

            // Skip if extension filter is set and doesn't match.
            if path.is_file() && !self.config.include_extensions.is_empty() {
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !self.config.include_extensions.contains(&ext) {
                    continue;
                }
            }

            let metadata = entry.metadata()?;
            let size = metadata.len();

            // Skip large files if max_file_size is set.
            if path.is_file() && self.config.max_file_size > 0 && size > self.config.max_file_size {
                continue;
            }

            let extension = path
                .extension()
                .map(|e| SmolStr::new(e.to_string_lossy().to_string()));

            let kind = if path.is_dir() { NodeKind::Dir } else { NodeKind::File };
            let child = FileNode {
                path: SmolStr::new(&rel),
                name: SmolStr::new(&name),
                kind,
                size,
                children: Vec::new(),
                extension,
            };

            node.children.push(child);
            let idx = node.children.len() - 1;

            if path.is_dir() {
                self.scan_dir(&path, &mut node.children[idx], &rel, depth + 1)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_empty_dir() {
        let dir = tempdir().unwrap();
        let scanner = WorkspaceScanner::default();
        let tree = scanner.scan(dir.path()).unwrap();
        // Root + no children.
        assert!(tree.root.is_some());
        assert_eq!(tree.file_count(), 0);
    }

    #[test]
    fn scan_files_and_dirs() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(dir.path().join("README.md"), "# Test").unwrap();

        let scanner = WorkspaceScanner::default();
        let tree = scanner.scan(dir.path()).unwrap();
        assert_eq!(tree.file_count(), 3);
        assert!(tree.get("main.rs").is_some());
        assert!(tree.get("src/lib.rs").is_some());
        assert!(tree.get("README.md").is_some());
    }

    #[test]
    fn scan_respects_excludes() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/ignored.rs"), "ignored").unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let scanner = WorkspaceScanner::default();
        let tree = scanner.scan(dir.path()).unwrap();
        assert_eq!(tree.file_count(), 1);
        assert!(tree.get("main.rs").is_some());
        assert!(tree.get("target/ignored.rs").is_none());
    }

    #[test]
    fn scan_filters_by_extension() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "").unwrap();
        fs::write(dir.path().join("b.ts"), "").unwrap();
        fs::write(dir.path().join("c.md"), "").unwrap();

        let config = ScanConfig {
            include_extensions: vec!["rs".into()],
            ..Default::default()
        };
        let scanner = WorkspaceScanner::new(config);
        let tree = scanner.scan(dir.path()).unwrap();
        assert_eq!(tree.file_count(), 1);
        assert!(tree.get("a.rs").is_some());
        assert!(tree.get("b.ts").is_none());
    }

    #[test]
    fn scan_returns_error_for_nonexistent() {
        let scanner = WorkspaceScanner::default();
        let result = scanner.scan(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
