//! SPS Language Server Protocol implementation.
//!
//! Provides LSP features powered by the SPS code intelligence:
//! - `textDocument/completion` — symbol name completion
//! - `textDocument/hover` — symbol info + doc comment
//! - `textDocument/definition` — go-to-definition
//! - `textDocument/references` — find all references
//! - `textDocument/documentSymbol` — symbols in current file
//! - `workspace/symbol` — workspace-wide symbol search
//!
//! # Usage
//!
//! Run the LSP server on stdio:
//! ```bash
//! sps-lsp
//! ```
//!
//! Configure VS Code to use it as a language server for Rust/TS/Python/Go.

#![allow(clippy::module_name_repetitions)]

pub mod server;
pub mod protocol;

pub use server::{SpsLanguageServer, run_stdio};
