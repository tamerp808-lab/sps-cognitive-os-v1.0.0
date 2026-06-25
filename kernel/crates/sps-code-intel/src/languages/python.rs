//! Python parser — functions, classes, imports.

use regex::Regex;
use smol_str::SmolStr;

use crate::languages::{extract_doc_comment, LanguageParser};
use crate::symbol::{Import, Symbol, SymbolKind, SymbolLocation};

pub struct PythonParser;

impl LanguageParser for PythonParser {
    fn language(&self) -> &str { "python" }

    fn parse_symbols(&self, file: &str, source: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let fn_re = Regex::new(r"^\s*(async\s+)?def\s+(\w+)\s*\(([^)]*)\)").unwrap();
        let class_re = Regex::new(r"^\s*class\s+(\w+)").unwrap();

        // Stack of (indent, class_name). Methods inside a class have indent > class indent.
        let mut class_stack: Vec<(usize, SmolStr)> = Vec::new();

        for (i, line) in source.lines().enumerate() {
            // Skip blank lines and comments.
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let line_num = (i + 1) as u32;
            let indent = line.len() - line.trim_start().len();

            // Pop classes whose indent >= current line's indent (out of scope).
            while let Some((class_indent, _)) = class_stack.last() {
                if *class_indent >= indent {
                    class_stack.pop();
                } else {
                    break;
                }
            }

            if let Some(caps) = fn_re.captures(line) {
                let name = SmolStr::new(&caps[2]);
                let mut sym = Symbol::new(
                    name.clone(),
                    SymbolKind::Function,
                    "python",
                    SymbolLocation { file: file.into(), line: line_num, column: indent as u32 + 1, end_line: line_num },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                if let Some(params) = caps.get(3) {
                    sym.parameters = params.as_str().split(',').map(|p| SmolStr::new(p.trim())).filter(|p| !p.is_empty()).collect();
                }
                // If inside a class, set qualified name.
                if let Some((_, class_name)) = class_stack.last() {
                    sym.qualified_name = SmolStr::new(format!("{}.{}", class_name, name));
                }
                symbols.push(sym);
            } else if let Some(caps) = class_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(
                    name.clone(),
                    SymbolKind::Class,
                    "python",
                    SymbolLocation { file: file.into(), line: line_num, column: indent as u32 + 1, end_line: line_num },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                // Push BEFORE checking methods (methods come after, with higher indent).
                class_stack.push((indent, name));
                symbols.push(sym);
            }
        }
        symbols
    }

    fn parse_imports(&self, file: &str, source: &str) -> Vec<Import> {
        let from_re = Regex::new(r"^\s*from\s+(\S+)\s+import\s+(.+)").unwrap();
        let import_re = Regex::new(r"^\s*import\s+(\S+)").unwrap();
        let mut imports = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let line_num = (i + 1) as u32;
            if let Some(caps) = from_re.captures(line) {
                let what = caps[2].trim();
                let is_wildcard = what == "*";
                imports.push(Import { file: file.into(), line: line_num, path: SmolStr::new(&caps[1]), alias: None, is_wildcard });
            } else if let Some(caps) = import_re.captures(line) {
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
    fn parse_python_functions_and_classes() {
        let source = r#"
class Dog:
    def __init__(self, name):
        self.name = name

    def bark(self):
        return "Woof!"

async def fetch_data(url):
    pass
"#;
        let parser = PythonParser;
        let symbols = parser.parse_symbols("test.py", source);
        assert!(symbols.iter().any(|s| s.name == "Dog" && s.kind == SymbolKind::Class));
        let bark = symbols.iter().find(|s| s.name == "bark").unwrap();
        assert!(bark.qualified_name.contains("Dog"));
        assert!(symbols.iter().any(|s| s.name == "fetch_data"));
    }

    #[test]
    fn parse_python_imports() {
        let source = r#"
import os
from typing import List
from collections import *
"#;
        let parser = PythonParser;
        let imports = parser.parse_imports("test.py", source);
        assert_eq!(imports.len(), 3);
        assert!(imports[2].is_wildcard);
    }
}
