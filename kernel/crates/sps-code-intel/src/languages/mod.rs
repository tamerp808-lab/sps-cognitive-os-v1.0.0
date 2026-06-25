//! Language parsers.
//!
//! Each parser extracts symbols and imports from source code. The parsers
//! use regex-based extraction which is fast and doesn't require external
//! dependencies (unlike tree-sitter). This is a pragmatic trade-off:
//! regex parsing misses some edge cases but covers 90% of real code.

pub mod rust;
pub mod typescript;
pub mod python;
pub mod go;
pub mod generic;

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{CodeIntelError, CodeIntelResult};
use crate::symbol::{Import, Symbol};

/// A language parser — extracts symbols and imports from source text.
pub trait LanguageParser: Send + Sync {
    /// Language name (e.g. "rust", "typescript").
    fn language(&self) -> &str;

    /// Parse source code and extract symbols.
    fn parse_symbols(&self, file: &str, source: &str) -> Vec<Symbol>;

    /// Parse source code and extract imports.
    fn parse_imports(&self, file: &str, source: &str) -> Vec<Import>;
}

/// Detected language for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Markdown,
    Json,
    Toml,
    Yaml,
    Unknown,
}

impl Language {
    /// String identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Unknown => "unknown",
        }
    }

    /// Whether this language has a parser (can extract symbols).
    pub fn has_parser(&self) -> bool {
        matches!(
            self,
            Self::Rust | Self::TypeScript | Self::JavaScript | Self::Python | Self::Go
        )
    }
}

/// Detect language from file extension.
pub fn detect_language(path: &str) -> Language {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => Language::Rust,
        "ts" | "tsx" => Language::TypeScript,
        "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
        "py" => Language::Python,
        "go" => Language::Go,
        "md" | "markdown" => Language::Markdown,
        "json" => Language::Json,
        "toml" => Language::Toml,
        "yaml" | "yml" => Language::Yaml,
        _ => Language::Unknown,
    }
}

/// Get a parser for the given language. Returns `None` for unsupported languages.
pub fn get_parser(language: Language) -> Option<Box<dyn LanguageParser>> {
    match language {
        Language::Rust => Some(Box::new(rust::RustParser)),
        Language::TypeScript => Some(Box::new(typescript::TypeScriptParser::new(false))),
        Language::JavaScript => Some(Box::new(typescript::TypeScriptParser::new(true))),
        Language::Python => Some(Box::new(python::PythonParser)),
        Language::Go => Some(Box::new(go::GoParser)),
        _ => None,
    }
}

/// Parse a file and return its symbols + imports.
pub fn parse_file(file: &str, source: &str) -> CodeIntelResult<(Vec<Symbol>, Vec<Import>)> {
    let language = detect_language(file);
    let parser = get_parser(language).ok_or_else(|| {
        CodeIntelError::UnsupportedLanguage(format!("{:?} (file: {})", language, file))
    })?;
    let symbols = parser.parse_symbols(file, source);
    let imports = parser.parse_imports(file, source);
    Ok((symbols, imports))
}

// Helper: extract line number from byte offset.
pub(crate) fn line_at(source: &str, offset: usize) -> u32 {
    source[..offset.min(source.len())].lines().count() as u32 + 1
}

// Helper: extract doc comment from lines above a symbol.
pub(crate) fn extract_doc_comment(source: &str, line: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut doc_lines: Vec<String> = Vec::new();
    let mut i = (line as usize).saturating_sub(2); // line above the symbol
    while i < lines.len() {
        let trimmed = lines[i].trim();
        // Rust: /// or //!
        if trimmed.starts_with("///") || trimmed.starts_with("//!") {
            doc_lines.insert(0, trimmed.trim_start_matches('/').trim().to_string());
        }
        // JS/TS/Python: /** */ or #
        else if trimmed.starts_with("/**") || trimmed.starts_with("*") {
            let cleaned = trimmed
                .trim_start_matches('/')
                .trim_start_matches('*')
                .trim_end_matches('/')
                .trim()
                .trim_start_matches('*')
                .trim();
            if !cleaned.is_empty() {
                doc_lines.insert(0, cleaned.to_string());
            }
        } else if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            doc_lines.insert(0, trimmed.trim_start_matches('#').trim().to_string());
        } else if !trimmed.is_empty() {
            break;
        }
        if i == 0 {
            break;
        }
        i = i.saturating_sub(1);
    }
    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}
