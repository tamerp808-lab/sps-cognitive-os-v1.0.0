//! Workspace errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("path not found: {0}")]
    NotFound(String),

    #[error("path escapes workspace: {0}")]
    EscapesWorkspace(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not a workspace: {0}")]
    NotAWorkspace(String),
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;
