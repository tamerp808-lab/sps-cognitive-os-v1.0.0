//! SPS Git integration — blame, history, branches, status.
//!
//! Uses the `git` CLI (no libgit2 dependency). All operations are
//! shell-based and return structured data.

#![allow(clippy::module_name_repetitions)]

pub mod blame;
pub mod history;
pub mod branches;
pub mod status;
pub mod error;

pub use blame::{BlameInfo, BlameLine};
pub use history::{CommitInfo, FileHistory};
pub use branches::{Branch, BranchList};
pub use status::{GitStatus, StatusEntry, StatusKind};
pub use error::{GitError, GitResult};
