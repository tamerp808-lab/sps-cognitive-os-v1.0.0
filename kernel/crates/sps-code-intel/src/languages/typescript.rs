//! TypeScript / JavaScript parser.

use regex::Regex;
use smol_str::SmolStr;

use crate::languages::{extract_doc_comment, LanguageParser};
use crate::symbol::{Import, Symbol, SymbolKind, SymbolLocation};

/// TypeScript / JavaScript parser.
pub struct TypeScriptParser {
    is_javascript: bool,
}

impl TypeScriptParser {
    /// Create a new parser. If `is_javascript` is true, TS-specific syntax
    /// (interfaces, type aliases, enums) is ignored.
    pub fn new(is_javascript: bool) -> Self {
        Self { is_javascript }
    }

    fn language_name(&self) -> &str {
        if self.is_javascript {
            "javascript"
        } else {
            "typescript"
        }
    }
}

impl LanguageParser for TypeScriptParser {
    fn language(&self) -> &str {
        self.language_name()
    }

    fn parse_symbols(&self, file: &str, source: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let lang = self.language_name();
        let lines: Vec<&str> = source.lines().collect();

        // Functions: `function name(`, `async function name(`
        let fn_re = Regex::new(r"^\s*(export\s+)?(default\s+)?(async\s+)?function\s+(\w+)").unwrap();
        // Arrow functions / const functions: `const name = (...) =>`, `const name = async (...) =>`
        let arrow_re = Regex::new(r"^\s*(export\s+)?(const|let|var)\s+(\w+)\s*=?\s*(\([^)]*\)|\w+)\s*=>").unwrap();
        // Classes
        let class_re = Regex::new(r"^\s*(export\s+)?(default\s+)?(abstract\s+)?class\s+(\w+)").unwrap();
        // Interfaces (TS only)
        let iface_re = Regex::new(r"^\s*(export\s+)?interface\s+(\w+)").unwrap();
        // Type aliases (TS only)
        let type_re = Regex::new(r"^\s*(export\s+)?type\s+(\w+)\s*=").unwrap();
        // Enums (TS only)
        let enum_re = Regex::new(r"^\s*(export\s+)?(const\s+)?enum\s+(\w+)").unwrap();

        let mut current_class: Option<SmolStr> = None;
        let mut brace_depth: i32 = 0;

        for (i, line) in lines.iter().enumerate() {
            // Track class context.
            for c in line.chars() {
                match c {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            current_class = None;
                        }
                    }
                    _ => {}
                }
            }

            let line_num = (i + 1) as u32;
            let trimmed = line.trim();

            // Methods inside classes (simple heuristic: `name(...) {` or `name(...) {`).
            if current_class.is_some() && !trimmed.starts_with("function") {
                let method_re = Regex::new(r"^\s*(public\s+|private\s+|protected\s+|static\s+|async\s+|get\s+|set\s+)*(\w+)\s*\([^)]*\)\s*(:\s*\S+\s*)?\{").unwrap();
                if let Some(caps) = method_re.captures(line) {
                    let name = SmolStr::new(&caps[2]);
                    if name != "if" && name != "for" && name != "while" && name != "switch" && name != "catch" {
                        let mut sym = Symbol::new(
                            name.clone(),
                            SymbolKind::Function,
                            lang,
                            SymbolLocation {
                                file: file.into(),
                                line: line_num,
                                column: 1,
                                end_line: line_num,
                            },
                        );
                        sym.doc_comment = extract_doc_comment(source, line_num);
                        if let Some(ref cls) = current_class {
                            sym.qualified_name = SmolStr::new(format!("{}.{}", cls, name));
                        }
                        symbols.push(sym);
                    }
                }
            }

            if let Some(caps) = fn_re.captures(line) {
                let name = SmolStr::new(&caps[4]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Function,
                    lang,
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                if let Some(params) = extract_parens(line) {
                    sym.parameters = params
                        .split(',')
                        .map(|p| SmolStr::new(p.trim()))
                        .filter(|p| !p.is_empty())
                        .collect();
                }
                symbols.push(sym);
            } else if let Some(caps) = arrow_re.captures(line) {
                let name = SmolStr::new(&caps[3]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Function,
                    lang,
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = class_re.captures(line) {
                let name = SmolStr::new(&caps[4]);
                let mut sym = Symbol::new(
                    name.clone(),
                    SymbolKind::Class,
                    lang,
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                if line.contains('{') {
                    current_class = Some(name);
                }
                symbols.push(sym);
            } else if !self.is_javascript {
                if let Some(caps) = iface_re.captures(line) {
                    let name = SmolStr::new(&caps[2]);
                    let mut sym = Symbol::new(
                        name,
                        SymbolKind::Interface,
                        lang,
                        SymbolLocation {
                            file: file.into(),
                            line: line_num,
                            column: 1,
                            end_line: line_num,
                        },
                    );
                    sym.doc_comment = extract_doc_comment(source, line_num);
                    symbols.push(sym);
                } else if let Some(caps) = type_re.captures(line) {
                    let name = SmolStr::new(&caps[2]);
                    let mut sym = Symbol::new(
                        name,
                        SymbolKind::TypeAlias,
                        lang,
                        SymbolLocation {
                            file: file.into(),
                            line: line_num,
                            column: 1,
                            end_line: line_num,
                        },
                    );
                    sym.doc_comment = extract_doc_comment(source, line_num);
                    symbols.push(sym);
                } else if let Some(caps) = enum_re.captures(line) {
                    let name = SmolStr::new(&caps[3]);
                    let mut sym = Symbol::new(
                        name,
                        SymbolKind::Enum,
                        lang,
                        SymbolLocation {
                            file: file.into(),
                            line: line_num,
                            column: 1,
                            end_line: line_num,
                        },
                    );
                    sym.doc_comment = extract_doc_comment(source, line_num);
                    symbols.push(sym);
                }
            }
        }
        symbols
    }

    fn parse_imports(&self, file: &str, source: &str) -> Vec<Import> {
        let import_re = Regex::new(r#"^\s*import\s+(.+?)\s+from\s+['"]([^'"]+)['"]"#).unwrap();
        let import_side_re = Regex::new(r#"^\s*import\s+['"]([^'"]+)['"]"#).unwrap();
        let require_re = Regex::new(r#"^\s*(?:const|let|var)\s+.+?=\s*require\(['"]([^'"]+)['"]\)"#).unwrap();
        let mut imports = Vec::new();
        for (i, line) in source.lines().enumerate() {
            if let Some(caps) = import_re.captures(line) {
                imports.push(Import {
                    file: file.into(),
                    line: (i + 1) as u32,
                    path: SmolStr::new(&caps[2]),
                    alias: None,
                    is_wildcard: caps[1].contains('*'),
                });
            } else if let Some(caps) = import_side_re.captures(line) {
                imports.push(Import {
                    file: file.into(),
                    line: (i + 1) as u32,
                    path: SmolStr::new(&caps[1]),
                    alias: None,
                    is_wildcard: false,
                });
            } else if let Some(caps) = require_re.captures(line) {
                imports.push(Import {
                    file: file.into(),
                    line: (i + 1) as u32,
                    path: SmolStr::new(&caps[1]),
                    alias: None,
                    is_wildcard: false,
                });
            }
        }
        imports
    }
}

fn extract_parens(line: &str) -> Option<String> {
    let start = line.find('(')?;
    let end = line.rfind(')')?;
    if end > start {
        Some(line[start + 1..end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ts_functions() {
        let source = r#"
function add(a: number, b: number): number {
    return a + b;
}

async function fetchData(url: string): Promise<Response> {
    return fetch(url);
}

const greet = (name: string) => `Hello, ${name}`;
"#;
        let parser = TypeScriptParser::new(false);
        let symbols = parser.parse_symbols("test.ts", source);
        let fns: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Function).collect();
        assert_eq!(fns.len(), 3);
        assert_eq!(fns[0].name, "add");
        assert_eq!(fns[0].parameters.len(), 2);
    }

    #[test]
    fn parse_ts_classes_and_methods() {
        let source = r#"
class Animal {
    constructor(name: string) {}

    public speak(): string {
        return "..."
    }

    private eat(): void {}
}
"#;
        let parser = TypeScriptParser::new(false);
        let symbols = parser.parse_symbols("test.ts", source);
        assert!(symbols.iter().any(|s| s.name == "Animal" && s.kind == SymbolKind::Class));
        let methods: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function && s.qualified_name.contains('.'))
            .collect();
        assert!(methods.len() >= 2);
    }

    #[test]
    fn parse_ts_interfaces_and_types() {
        let source = r#"
interface User {
    id: number;
    name: string;
}

type Status = "active" | "inactive";
"#;
        let parser = TypeScriptParser::new(false);
        let symbols = parser.parse_symbols("test.ts", source);
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Interface));
        assert!(symbols.iter().any(|s| s.name == "Status" && s.kind == SymbolKind::TypeAlias));
    }

    #[test]
    fn parse_ts_imports() {
        let source = r#"
import { useState, useEffect } from "react";
import * as fs from "fs";
import path from "path";
"#;
        let parser = TypeScriptParser::new(false);
        let imports = parser.parse_imports("test.ts", source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "react");
        assert!(imports[1].is_wildcard);
    }

    #[test]
    fn js_parser_ignores_ts_syntax() {
        let source = r#"
interface Foo {}
type Bar = string;
function baz() {}
"#;
        let parser = TypeScriptParser::new(true);
        let symbols = parser.parse_symbols("test.js", source);
        // Interface and type should be ignored in JS mode.
        assert!(!symbols.iter().any(|s| s.kind == SymbolKind::Interface));
        assert!(!symbols.iter().any(|s| s.kind == SymbolKind::TypeAlias));
        assert!(symbols.iter().any(|s| s.name == "baz"));
    }
}
