//! Generic fallback parser — just counts lines, no symbol extraction.

use crate::languages::LanguageParser;
use crate::symbol::{Import, Symbol};

pub struct GenericParser;

impl LanguageParser for GenericParser {
    fn language(&self) -> &str { "generic" }
    fn parse_symbols(&self, _file: &str, _source: &str) -> Vec<Symbol> { Vec::new() }
    fn parse_imports(&self, _file: &str, _source: &str) -> Vec<Import> { Vec::new() }
}
