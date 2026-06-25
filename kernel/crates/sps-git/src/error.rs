//! Git errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git not found: {0}")]
    GitNotFound(String),

    #[error("not a git repository: {0}")]
    NotARepo(String),

    #[error("git command failed: {0}")]
    CommandFailed(String),

    #[error("parse error: {0}")]
    ParseError(String),
}

pub type GitResult<T> = Result<T, GitError>;
