//! SPS Code Intelligence — symbol extraction, codebase index, search.
//!
//! This crate provides:
//!
//! - **Symbol extraction**: parse source files to find functions, classes,
//!   structs, enums, interfaces, imports, and other definitions.
//! - **Multi-language support**: Rust, TypeScript/JavaScript, Python, Go.
//! - **Codebase index**: in-memory index of all symbols across all files,
//!   with fuzzy search.
//! - **Find references**: search for usages of a symbol across the codebase.
//! - **Go-to-definition**: locate where a symbol is defined.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │           CodebaseIndex                  │
//! │  ┌──────────────┐  ┌─────────────────┐   │
//! │  │ Symbol Table │  │  File Index     │   │
//! │  │ (by name)    │  │  (by path)      │   │
//! │  └──────────────┘  └─────────────────┘   │
//! │  ┌──────────────┐  ┌─────────────────┐   │
//! │  │ Fuzzy Search │  │ Reference Graph │   │
//! │  │ (subsequence)│  │ (who uses what) │   │
//! │  └──────────────┘  └─────────────────┘   │
//! └──────────────────────────────────────────┘
//! ```

#![allow(clippy::module_name_repetitions)]

pub mod symbol;
pub mod languages;
pub mod search;
pub mod index;
pub mod error;

pub use symbol::{Symbol, SymbolKind, SymbolLocation, Import};
pub use languages::{LanguageParser, detect_language, get_parser};
pub use index::{CodebaseIndex, IndexStats};
pub use search::{fuzzy_search, SearchQuery, SearchResult, SearchMatch};
pub use error::{CodeIntelError, CodeIntelResult};
