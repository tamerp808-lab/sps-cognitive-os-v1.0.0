//! File tree — hierarchical representation of the workspace.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::path::Path;

/// Kind of file node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// Directory.
    Dir,
    /// File.
    File,
}

/// A node in the file tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    /// Path relative to workspace root (e.g. "src/main.rs").
    pub path: SmolStr,
    /// Display name (e.g. "main.rs").
    pub name: SmolStr,
    /// Kind (file or dir).
    pub kind: NodeKind,
    /// File size in bytes (0 for dirs).
    #[serde(default)]
    pub size: u64,
    /// Children (for dirs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FileNode>,
    /// File extension (e.g. "rs", "ts").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<SmolStr>,
}

/// The file tree — root + all nodes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileTree {
    /// Root node.
    pub root: Option<FileNode>,
    /// Flat index: path → node (for fast lookup).
    #[serde(skip)]
    pub index: BTreeMap<String, FileNode>,
}

impl FileTree {
    /// Create an empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a tree from a list of file paths.
    pub fn from_paths(paths: &[String]) -> Self {
        let mut tree = Self::new();
        for path in paths {
            let parts: Vec<&str> = path.split('/').collect();
            let _ = tree.insert_path(&parts);
        }
        tree.rebuild_index();
        tree
    }

    /// Insert a path into the tree. Returns a reference to the leaf node.
    fn insert_path(&mut self, parts: &[&str]) -> &mut FileNode {
        if parts.is_empty() {
            return self.root.as_mut().unwrap();
        }
        if self.root.is_none() {
            self.root = Some(FileNode {
                path: SmolStr::new(""),
                name: SmolStr::new(""),
                kind: NodeKind::Dir,
                size: 0,
                children: Vec::new(),
                extension: None,
            });
        }
        let root = self.root.as_mut().unwrap();
        Self::insert_recursive(root, parts, "")
    }

    fn insert_recursive<'a>(
        node: &'a mut FileNode,
        parts: &[&str],
        parent_path: &str,
    ) -> &'a mut FileNode {
        if parts.is_empty() {
            return node;
        }
        let name = parts[0];
        let current_path = if parent_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", parent_path, name)
        };
        let is_last = parts.len() == 1;

        // Check if child already exists.
        let existing_idx = node.children.iter().position(|c| c.name == name);
        if let Some(idx) = existing_idx {
            if !is_last {
                return Self::insert_recursive(&mut node.children[idx], &parts[1..], &current_path);
            }
            return &mut node.children[idx];
        }

        // Create new node.
        let kind = if is_last { NodeKind::File } else { NodeKind::Dir };
        let extension = if is_last {
            Path::new(name)
                .extension()
                .map(|e| SmolStr::new(e.to_string_lossy().to_string()))
        } else {
            None
        };
        let new_node = FileNode {
            path: SmolStr::new(&current_path),
            name: SmolStr::new(name),
            kind,
            size: 0,
            children: Vec::new(),
            extension,
        };
        node.children.push(new_node);
        let idx = node.children.len() - 1;
        if !is_last {
            Self::insert_recursive(&mut node.children[idx], &parts[1..], &current_path)
        } else {
            &mut node.children[idx]
        }
    }

    /// Rebuild the flat index from the tree.
    pub fn rebuild_index(&mut self) {
        self.index.clear();
        if let Some(ref root) = self.root {
            Self::index_recursive(root, &mut self.index);
        }
    }

    fn index_recursive(node: &FileNode, index: &mut BTreeMap<String, FileNode>) {
        if !node.path.is_empty() {
            index.insert(node.path.to_string(), node.clone());
        }
        for child in &node.children {
            Self::index_recursive(child, index);
        }
    }

    /// Get a node by path.
    pub fn get(&self, path: &str) -> Option<&FileNode> {
        self.index.get(path)
    }

    /// List all files (not dirs) in the tree.
    pub fn files(&self) -> Vec<&FileNode> {
        self.index.values().filter(|n| n.kind == NodeKind::File).collect()
    }

    /// List all directories.
    pub fn dirs(&self) -> Vec<&FileNode> {
        self.index.values().filter(|n| n.kind == NodeKind::Dir).collect()
    }

    /// Total file count.
    pub fn file_count(&self) -> usize {
        self.files().len()
    }

    /// Total size of all files.
    pub fn total_size(&self) -> u64 {
        self.files().iter().map(|f| f.size).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tree_from_paths() {
        let paths = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "README.md".to_string(),
            "src/utils/helpers.rs".to_string(),
        ];
        let tree = FileTree::from_paths(&paths);
        assert_eq!(tree.file_count(), 4);
        assert!(tree.get("src/main.rs").is_some());
        assert!(tree.get("src/lib.rs").is_some());
        assert!(tree.get("README.md").is_some());
        assert!(tree.get("src/utils/helpers.rs").is_some());
        assert_eq!(tree.get("src/main.rs").unwrap().extension.as_deref(), Some("rs"));
    }

    #[test]
    fn tree_handles_nested_dirs() {
        let paths = vec!["a/b/c/d.rs".to_string()];
        let tree = FileTree::from_paths(&paths);
        assert_eq!(tree.file_count(), 1);
        assert!(tree.get("a").is_some());
        assert!(tree.get("a/b").is_some());
        assert!(tree.get("a/b/c").is_some());
        assert!(tree.get("a/b/c/d.rs").is_some());
    }

    #[test]
    fn tree_files_and_dirs() {
        let paths = vec!["file.txt".to_string(), "dir/file2.txt".to_string()];
        let tree = FileTree::from_paths(&paths);
        let files = tree.files();
        let dirs = tree.dirs();
        assert_eq!(files.len(), 2);
        assert!(dirs.iter().any(|d| d.name == "dir"));
    }
}
