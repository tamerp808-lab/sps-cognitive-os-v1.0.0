//! Code intelligence errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodeIntelError {
    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("parse error: {0}")]
    ParseError(String),
}

pub type CodeIntelResult<T> = Result<T, CodeIntelError>;
