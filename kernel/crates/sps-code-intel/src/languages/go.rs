//! Go parser — functions, structs, interfaces, imports.

use regex::Regex;
use smol_str::SmolStr;

use crate::languages::{extract_doc_comment, LanguageParser};
use crate::symbol::{Import, Symbol, SymbolKind, SymbolLocation};

pub struct GoParser;

impl LanguageParser for GoParser {
    fn language(&self) -> &str { "go" }

    fn parse_symbols(&self, file: &str, source: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        // Functions: `func name(`, `func (r Receiver) name(`
        let fn_re = Regex::new(r"^\s*func\s+(?:\([^)]*\)\s+)?(\w+)\s*\(([^)]*)\)").unwrap();
        let struct_re = Regex::new(r"^\s*type\s+(\w+)\s+struct\s*\{").unwrap();
        let iface_re = Regex::new(r"^\s*type\s+(\w+)\s+interface\s*\{").unwrap();
        let type_re = Regex::new(r"^\s*type\s+(\w+)\s+\S+").unwrap();

        for (i, line) in source.lines().enumerate() {
            let line_num = (i + 1) as u32;
            if let Some(caps) = fn_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(name, SymbolKind::Function, "go",
                    SymbolLocation { file: file.into(), line: line_num, column: 1, end_line: line_num });
                sym.doc_comment = extract_doc_comment(source, line_num);
                if let Some(params) = caps.get(2) {
                    sym.parameters = params.as_str().split(',').map(|p| SmolStr::new(p.trim())).filter(|p| !p.is_empty()).collect();
                }
                symbols.push(sym);
            } else if let Some(caps) = struct_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(name, SymbolKind::Struct, "go",
                    SymbolLocation { file: file.into(), line: line_num, column: 1, end_line: line_num });
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = iface_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(name, SymbolKind::Interface, "go",
                    SymbolLocation { file: file.into(), line: line_num, column: 1, end_line: line_num });
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = type_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(name, SymbolKind::TypeAlias, "go",
                    SymbolLocation { file: file.into(), line: line_num, column: 1, end_line: line_num });
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            }
        }
        symbols
    }

    fn parse_imports(&self, file: &str, source: &str) -> Vec<Import> {
        let single_re = Regex::new(r#"^\s*import\s+"([^"]+)""#).unwrap();
        let multi_re = Regex::new(r#"^\s*"([^"]+)""#).unwrap();
        let mut imports = Vec::new();
        let mut in_multi = false;
        for (i, line) in source.lines().enumerate() {
            let line_num = (i + 1) as u32;
            let trimmed = line.trim();
            if trimmed == "import (" { in_multi = true; continue; }
            if trimmed == ")" && in_multi { in_multi = false; continue; }
            if in_multi {
                if let Some(caps) = multi_re.captures(line) {
                    imports.push(Import { file: file.into(), line: line_num, path: SmolStr::new(&caps[1]), alias: None, is_wildcard: false });
                }
            } else if let Some(caps) = single_re.captures(line) {
                imports.push(Import { file: file.into(), line: line_num, path: SmolStr::new(&caps[1]), alias: None, is_wildcard: false });
            }
        }
        imports
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_go_functions() {
        let source = r#"
package main

func Add(a, b int) int {
    return a + b
}

func (s *Server) Start() error {
    return nil
}
"#;
        let parser = GoParser;
        let symbols = parser.parse_symbols("test.go", source);
        assert!(symbols.iter().any(|s| s.name == "Add" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Start" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn parse_go_types() {
        let source = r#"
type Point struct {
    X float64
    Y float64
}

type Shape interface {
    Area() float64
}
"#;
        let parser = GoParser;
        let symbols = parser.parse_symbols("test.go", source);
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "Shape" && s.kind == SymbolKind::Interface));
    }

    #[test]
    fn parse_go_imports() {
        let source = r#"
package main

import "fmt"
import (
    "os"
    "strings"
)
"#;
        let parser = GoParser;
        let imports = parser.parse_imports("test.go", source);
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().any(|i| i.path == "fmt"));
        assert!(imports.iter().any(|i| i.path == "os"));
    }
}
