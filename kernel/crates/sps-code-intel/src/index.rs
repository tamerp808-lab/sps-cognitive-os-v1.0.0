//! Codebase index — in-memory index of all symbols across all files.
//!
//! The index is built by scanning files, parsing each with the appropriate
//! language parser, and storing the resulting symbols. It supports:
//!
//! - Fast lookup by name (exact + fuzzy)
//! - Find references (search for usages of a symbol name)
//! - Go-to-definition (find where a symbol is defined)
//! - File-level stats (symbol count per file)

use std::collections::BTreeMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

use crate::error::CodeIntelResult;
use crate::languages::{detect_language, parse_file, Language};
use crate::search::{fuzzy_search, SearchResult};
use crate::symbol::{Import, Symbol, SymbolKind};

/// Statistics about the indexed codebase.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total files indexed.
    pub files_indexed: usize,
    /// Total symbols.
    pub total_symbols: usize,
    /// Total imports.
    pub total_imports: usize,
    /// Symbols by kind.
    pub by_kind: BTreeMap<String, usize>,
    /// Symbols by language.
    pub by_language: BTreeMap<String, usize>,
    /// Symbols by file.
    pub by_file: BTreeMap<String, usize>,
}

/// The codebase index.
pub struct CodebaseIndex {
    /// All symbols keyed by id.
    symbols: RwLock<BTreeMap<Uuid, Symbol>>,
    /// Symbol name → ids (for fast name lookup).
    by_name: RwLock<BTreeMap<String, Vec<Uuid>>>,
    /// File → symbol ids (for file-level queries).
    by_file: RwLock<BTreeMap<String, Vec<Uuid>>>,
    /// All imports.
    imports: RwLock<Vec<Import>>,
    /// File contents cache (file → source).
    sources: RwLock<BTreeMap<String, String>>,
    /// Stats.
    stats: RwLock<IndexStats>,
}

impl Default for CodebaseIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl CodebaseIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            symbols: RwLock::new(BTreeMap::new()),
            by_name: RwLock::new(BTreeMap::new()),
            by_file: RwLock::new(BTreeMap::new()),
            imports: RwLock::new(Vec::new()),
            sources: RwLock::new(BTreeMap::new()),
            stats: RwLock::new(IndexStats::default()),
        }
    }

    /// Index a single file. Parses the source and adds all symbols + imports.
    pub fn index_file(&self, file: &str, source: &str) -> CodeIntelResult<usize> {
        let language = detect_language(file);
        if !language.has_parser() {
            // Store the source but don't parse.
            self.sources.write().insert(file.to_string(), source.to_string());
            return Ok(0);
        }

        let (symbols, imports) = parse_file(file, source)?;

        // Remove old symbols for this file.
        self.remove_file(file);

        // Add new symbols.
        let mut count = 0;
        {
            let mut symbols_lock = self.symbols.write();
            let mut by_name = self.by_name.write();
            let mut by_file = self.by_file.write();
            let mut stats = self.stats.write();

            for sym in symbols {
                let id = sym.id;
                let name = sym.name.to_string();
                let kind_str = sym.kind.as_str().to_string();
                let lang_str = sym.language.to_string();
                let file_str = sym.location.file.to_string();

                by_name.entry(name).or_default().push(id);
                by_file.entry(file_str.clone()).or_default().push(id);
                *stats.by_kind.entry(kind_str).or_default() += 1;
                *stats.by_language.entry(lang_str).or_default() += 1;
                *stats.by_file.entry(file_str).or_default() += 1;
                stats.total_symbols += 1;
                symbols_lock.insert(id, sym);
                count += 1;
            }
        }

        // Add imports.
        {
            let mut imports_lock = self.imports.write();
            let mut stats = self.stats.write();
            for imp in imports {
                imports_lock.push(imp);
                stats.total_imports += 1;
            }
        }

        // Cache the source.
        self.sources.write().insert(file.to_string(), source.to_string());

        {
            let mut stats = self.stats.write();
            stats.files_indexed += 1;
        }

        Ok(count)
    }

    /// Remove a file and all its symbols from the index.
    pub fn remove_file(&self, file: &str) {
        let mut symbols = self.symbols.write();
        let mut by_name = self.by_name.write();
        let mut by_file = self.by_file.write();
        let mut stats = self.stats.write();

        if let Some(ids) = by_file.remove(file) {
            for id in ids {
                if let Some(sym) = symbols.remove(&id) {
                    // Remove from by_name.
                    if let Some(name_ids) = by_name.get_mut(sym.name.as_str()) {
                        name_ids.retain(|x| *x != id);
                        if name_ids.is_empty() {
                            by_name.remove(sym.name.as_str());
                        }
                    }
                    // Update stats (saturating to avoid underflow).
                    stats.total_symbols = stats.total_symbols.saturating_sub(1);
                    if let Some(count) = stats.by_kind.get_mut(sym.kind.as_str()) {
                        *count = count.saturating_sub(1);
                    }
                    if let Some(count) = stats.by_language.get_mut(sym.language.as_str()) {
                        *count = count.saturating_sub(1);
                    }
                    if let Some(count) = stats.by_file.get_mut(file) {
                        *count = count.saturating_sub(1);
                    }
                }
            }
        }
    }

    /// Find symbols by exact name.
    pub fn find_by_name(&self, name: &str) -> Vec<Symbol> {
        let by_name = self.by_name.read();
        let symbols = self.symbols.read();
        by_name
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|id| symbols.get(id).cloned())
            .collect()
    }

    /// Fuzzy search for symbols.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let symbols: Vec<Symbol> = self.symbols.read().values().cloned().collect();
        fuzzy_search(query, &symbols, limit)
    }

    /// Search with filters.
    pub fn search_filtered(
        &self,
        query: &str,
        limit: usize,
        kind: Option<SymbolKind>,
        file_filter: Option<&str>,
    ) -> Vec<SearchResult> {
        let symbols: Vec<Symbol> = self
            .symbols
            .read()
            .values()
            .filter(|s| kind.map_or(true, |k| s.kind == k))
            .filter(|s| {
                file_filter.map_or(true, |f| s.location.file.as_str().contains(f))
            })
            .cloned()
            .collect();
        fuzzy_search(query, &symbols, limit)
    }

    /// Find references to a symbol (search for its name in all indexed sources).
    pub fn find_references(&self, symbol_name: &str) -> Vec<Reference> {
        let sources = self.sources.read();
        let mut refs = Vec::new();
        for (file, source) in sources.iter() {
            for (i, line) in source.lines().enumerate() {
                if line.contains(symbol_name) {
                    refs.push(Reference {
                        file: SmolStr::new(file),
                        line: (i + 1) as u32,
                        column: line.find(symbol_name).map(|p| p as u32 + 1).unwrap_or(1),
                        context: line.trim().to_string(),
                    });
                }
            }
        }
        refs
    }

    /// Go to definition — find where a symbol is defined.
    pub fn go_to_definition(&self, name: &str) -> Vec<Symbol> {
        self.find_by_name(name)
    }

    /// List all symbols in a file.
    pub fn symbols_in_file(&self, file: &str) -> Vec<Symbol> {
        let by_file = self.by_file.read();
        let symbols = self.symbols.read();
        by_file
            .get(file)
            .into_iter()
            .flatten()
            .filter_map(|id| symbols.get(id).cloned())
            .collect()
    }

    /// Get all imports.
    pub fn imports(&self) -> Vec<Import> {
        self.imports.read().clone()
    }

    /// Get all indexed files.
    pub fn files(&self) -> Vec<String> {
        self.sources.read().keys().cloned().collect()
    }

    /// Get index statistics.
    pub fn stats(&self) -> IndexStats {
        self.stats.read().clone()
    }

    /// Get the source of a file (if indexed).
    pub fn get_source(&self, file: &str) -> Option<String> {
        self.sources.read().get(file).cloned()
    }

    /// Total symbol count.
    pub fn symbol_count(&self) -> usize {
        self.symbols.read().len()
    }

    /// Clear the entire index.
    pub fn clear(&self) {
        self.symbols.write().clear();
        self.by_name.write().clear();
        self.by_file.write().clear();
        self.imports.write().clear();
        self.sources.write().clear();
        *self.stats.write() = IndexStats::default();
    }
}

/// A reference to a symbol (a usage somewhere in the codebase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// File path.
    pub file: SmolStr,
    /// Line number.
    pub line: u32,
    /// Column.
    pub column: u32,
    /// The line content (for context).
    pub context: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_rust_file() {
        let idx = CodebaseIndex::new();
        let source = r#"
use std::io;

fn main() {}

struct Point { x: f64, y: f64 }

impl Point {
    fn distance(&self) -> f64 { 0.0 }
}
"#;
        let count = idx.index_file("test.rs", source).unwrap();
        assert!(count >= 4); // main, Point, impl, distance
        assert!(idx.symbol_count() >= 4);
    }

    #[test]
    fn find_by_name_returns_matching_symbols() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn hello() {} fn world() {}").unwrap();
        let results = idx.find_by_name("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "hello");
    }

    #[test]
    fn search_fuzzy_matches() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn hello_world() {} fn goodbye() {}").unwrap();
        let results = idx.search("hel", 10);
        assert!(results.iter().any(|r| r.symbol.name == "hello_world"));
        assert!(!results.iter().any(|r| r.symbol.name == "goodbye"));
    }

    #[test]
    fn find_references_searches_all_files() {
        let idx = CodebaseIndex::new();
        idx.index_file("a.rs", "fn hello() {}\nhello();\nhello();").unwrap();
        idx.index_file("b.rs", "let x = hello();").unwrap();
        let refs = idx.find_references("hello");
        assert!(refs.len() >= 3); // definition + 2 calls in a.rs + 1 call in b.rs
    }

    #[test]
    fn go_to_definition_finds_symbol() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn my_function() {}").unwrap();
        let defs = idx.go_to_definition("my_function");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].location.line, 1);
    }

    #[test]
    fn remove_file_clears_symbols() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn hello() {}").unwrap();
        assert_eq!(idx.symbol_count(), 1);
        idx.remove_file("test.rs");
        assert_eq!(idx.symbol_count(), 0);
    }

    #[test]
    fn stats_track_counts() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn a() {}\nstruct B {}").unwrap();
        idx.index_file("test.ts", "function c() {}\nclass D {}").unwrap();
        let stats = idx.stats();
        assert_eq!(stats.files_indexed, 2);
        assert!(stats.total_symbols >= 4, "expected >=4 symbols, got {}", stats.total_symbols);
        let fn_count = stats.by_kind.get("function").copied().unwrap_or(0);
        assert!(fn_count >= 2, "expected >=2 functions, got {}", fn_count);
        let rust_count = stats.by_language.get("rust").copied().unwrap_or(0);
        assert!(rust_count >= 2, "expected >=2 rust symbols, got {}", rust_count);
        let ts_count = stats.by_language.get("typescript").copied().unwrap_or(0);
        assert!(ts_count >= 2, "expected >=2 ts symbols, got {}", ts_count);
    }

    #[test]
    fn unsupported_language_still_caches_source() {
        let idx = CodebaseIndex::new();
        idx.index_file("README.md", "# Hello").unwrap();
        assert_eq!(idx.symbol_count(), 0);
        assert!(idx.get_source("README.md").is_some());
    }

    #[test]
    fn search_with_kind_filter() {
        let idx = CodebaseIndex::new();
        idx.index_file("test.rs", "fn hello() {}\nstruct Hello {}").unwrap();
        let results = idx.search_filtered("hello", 10, Some(SymbolKind::Struct), None);
        assert_eq!(results.len(), 1, "expected 1 struct, got {}", results.len());
        assert_eq!(results[0].symbol.kind, SymbolKind::Struct);
    }

    #[test]
    fn index_multiple_files() {
        let idx = CodebaseIndex::new();
        idx.index_file("a.rs", "fn func_a() {}").unwrap();
        idx.index_file("b.rs", "fn func_b() {}").unwrap();
        idx.index_file("c.rs", "fn func_c() {}").unwrap();
        let stats = idx.stats();
        assert_eq!(stats.files_indexed, 3);
        assert_eq!(idx.files().len(), 3);
    }
}
