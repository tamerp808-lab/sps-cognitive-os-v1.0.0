//! SPS Workspace — file system scanning, tree view, file management.
//!
//! Provides:
//! - **Workspace**: a root directory + scanned file tree
//! - **FileNode**: a file or directory in the tree
//! - **Scanner**: recursively scans a directory, respecting .gitignore
//! - **File operations**: read, write, delete files

#![allow(clippy::module_name_repetitions)]

pub mod scanner;
pub mod tree;
pub mod ops;
pub mod error;

pub use scanner::{WorkspaceScanner, ScanConfig};
pub use tree::{FileTree, FileNode, NodeKind};
pub use ops::{FileOps, FileContent};
pub use error::{WorkspaceError, WorkspaceResult};
