//! Rust parser — extracts functions, structs, enums, traits, impls, macros, uses.

use regex::Regex;
use smol_str::SmolStr;

use crate::languages::{extract_doc_comment, line_at, LanguageParser};
use crate::symbol::{Import, Symbol, SymbolKind, SymbolLocation};

/// Rust source parser.
pub struct RustParser;

impl LanguageParser for RustParser {
    fn language(&self) -> &str {
        "rust"
    }

    fn parse_symbols(&self, file: &str, source: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        // Functions: `fn name(...)`, `pub fn name(...)`, `async fn name(...)`
        let fn_re = Regex::new(
            r#"^\s*(pub(?:\(\w+\))?\s+)?(async\s+)?(unsafe\s+)?(extern\s+"[^"]*"\s+)?fn\s+(\w+)"#,
        ).unwrap();
        // Structs: `struct Name`, `pub struct Name`
        let struct_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?struct\s+(\w+)").unwrap();
        // Enums
        let enum_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?enum\s+(\w+)").unwrap();
        // Traits
        let trait_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?trait\s+(\w+)").unwrap();
        // Impl blocks: `impl Type` or `impl Trait for Type`
        let impl_re = Regex::new(r"^\s*impl(?:<[^>]*>)?\s+(.+)").unwrap();
        // Type aliases: `type Name = ...`
        let type_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?type\s+(\w+)").unwrap();
        // Constants/statics
        let const_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?(const|static)\s+(\w+)").unwrap();
        // Modules: `mod name`
        let mod_re = Regex::new(r"^\s*(pub(?:\(\w+\))?\s+)?mod\s+(\w+)").unwrap();
        // Macros: `macro_rules! name`
        let macro_re = Regex::new(r"^\s*macro_rules!\s+(\w+)").unwrap();

        let mut current_impl: Option<SmolStr> = None;
        let mut brace_depth: i32 = 0;

        for (i, line) in lines.iter().enumerate() {
            // Track brace depth for impl context.
            for c in line.chars() {
                match c {
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            current_impl = None;
                        }
                    }
                    _ => {}
                }
            }

            let line_num = (i + 1) as u32;

            if let Some(caps) = fn_re.captures(line) {
                let name = SmolStr::new(&caps[5]);
                let mut sym = Symbol::new(
                    name.clone(),
                    SymbolKind::Function,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                // Extract parameters from the line.
                if let Some(params_str) = extract_parens(line) {
                    sym.parameters = params_str
                        .split(',')
                        .map(|p| SmolStr::new(p.trim()))
                        .filter(|p| !p.is_empty())
                        .collect();
                }
                // Extract return type.
                if let Some(ret) = extract_return_type(line) {
                    sym.return_type = Some(SmolStr::new(ret));
                }
                // Qualified name with impl context.
                if let Some(ref impl_type) = current_impl {
                    sym.qualified_name = SmolStr::new(format!("{}::{}", impl_type, name));
                }
                symbols.push(sym);
            } else if let Some(caps) = struct_re.captures(line) {
                let name = SmolStr::new(&caps[2]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Struct,
                    "rust",
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
                let name = SmolStr::new(&caps[2]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Enum,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = trait_re.captures(line) {
                let name = SmolStr::new(&caps[2]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Trait,
                    "rust",
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
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = const_re.captures(line) {
                let name = SmolStr::new(&caps[3]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Constant,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = mod_re.captures(line) {
                let name = SmolStr::new(&caps[2]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Module,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = macro_re.captures(line) {
                let name = SmolStr::new(&caps[1]);
                let mut sym = Symbol::new(
                    name,
                    SymbolKind::Macro,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                sym.doc_comment = extract_doc_comment(source, line_num);
                symbols.push(sym);
            } else if let Some(caps) = impl_re.captures(line) {
                // `impl Type {` or `impl Trait for Type {`
                let impl_target = caps[1].trim().to_string();
                let mut target = if let Some(pos) = impl_target.find(" for ") {
                    impl_target[pos + 5..].trim().to_string()
                } else {
                    impl_target.split('<').next().unwrap_or(&impl_target).trim().to_string()
                };
                // Clean up: remove trailing `{`, `where`, etc.
                if let Some(brace_pos) = target.find('{') {
                    target = target[..brace_pos].trim().to_string();
                }
                if let Some(where_pos) = target.find(" where") {
                    target = target[..where_pos].trim().to_string();
                }
                // Set current_impl BEFORE pushing the impl symbol so that
                // functions on the SAME line (rare but possible) get the context.
                // Also handle the case where `{` is on the next line.
                if line.contains('{') {
                    current_impl = Some(SmolStr::new(&target));
                }
                let sym = Symbol::new(
                    SmolStr::new(format!("impl {}", &target)),
                    SymbolKind::Impl,
                    "rust",
                    SymbolLocation {
                        file: file.into(),
                        line: line_num,
                        column: 1,
                        end_line: line_num,
                    },
                );
                symbols.push(sym);
            }
        }
        symbols
    }

    fn parse_imports(&self, file: &str, source: &str) -> Vec<Import> {
        let use_re = Regex::new(r"^\s*use\s+(.+?);").unwrap();
        let mut imports = Vec::new();
        for (i, line) in source.lines().enumerate() {
            if let Some(caps) = use_re.captures(line) {
                let path = &caps[1];
                let is_wildcard = path.ends_with("::*");
                let path = path.trim_end_matches("::*");
                let (path, alias) = if let Some(pos) = path.find(" as ") {
                    (&path[..pos], Some(SmolStr::new(path[pos + 4..].trim())))
                } else {
                    (path, None)
                };
                imports.push(Import {
                    file: file.into(),
                    line: (i + 1) as u32,
                    path: SmolStr::new(path),
                    alias,
                    is_wildcard,
                });
            }
        }
        imports
    }
}

/// Extract content inside the first pair of parentheses.
fn extract_parens(line: &str) -> Option<String> {
    let start = line.find('(')?;
    let end = line.rfind(')')?;
    if end > start {
        Some(line[start + 1..end].to_string())
    } else {
        None
    }
}

/// Extract return type from a `fn` line (after `->`).
fn extract_return_type(line: &str) -> Option<String> {
    let pos = line.find("->")?;
    let after = &line[pos + 2..];
    let result = after.trim();
    if result.is_empty() || result.starts_with('{') {
        None
    } else {
        Some(result.trim_end_matches('{').trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_functions() {
        let source = r#"
fn hello() {}

pub fn world(x: i32) -> i32 {
    x + 1
}

async fn fetch(url: &str) -> Result<String, Error> {
    panic!("not implemented")
}
"#;
        let parser = RustParser;
        let symbols = parser.parse_symbols("test.rs", source);
        let fns: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Function).collect();
        assert_eq!(fns.len(), 3);
        assert_eq!(fns[0].name, "hello");
        assert_eq!(fns[1].name, "world");
        assert_eq!(fns[1].parameters, vec![SmolStr::new("x: i32")]);
        assert_eq!(fns[1].return_type.as_deref(), Some("i32"));
        assert_eq!(fns[2].name, "fetch");
    }

    #[test]
    fn parse_rust_structs_enums() {
        let source = r#"
struct Point { x: f64, y: f64 }

pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let parser = RustParser;
        let symbols = parser.parse_symbols("test.rs", source);
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "Color" && s.kind == SymbolKind::Enum));
    }

    #[test]
    fn parse_rust_traits_and_impls() {
        let source = r#"
trait Display {
    fn show(&self) -> String;
}

impl Display for Point {
    fn show(&self) -> String {
        format!("{},{}", self.x, self.y)
    }
}
"#;
        let parser = RustParser;
        let symbols = parser.parse_symbols("test.rs", source);
        assert!(symbols.iter().any(|s| s.name == "Display" && s.kind == SymbolKind::Trait));
        assert!(symbols.iter().any(|s| s.kind == SymbolKind::Impl));
        // The method inside impl should have qualified name "Point::show".
        // There are two `show` symbols: one in the trait (qualified_name="show")
        // and one in the impl (qualified_name="Point::show").
        let impl_shows: Vec<_> = symbols
            .iter()
            .filter(|s| s.name == "show" && s.qualified_name.contains("Point"))
            .collect();
        assert_eq!(impl_shows.len(), 1, "expected 1 impl show with Point context, got {}", impl_shows.len());
    }

    #[test]
    fn parse_rust_imports() {
        let source = r#"
use std::io;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::utils::*;
"#;
        let parser = RustParser;
        let imports = parser.parse_imports("test.rs", source);
        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].path, "std::io");
        assert!(imports[3].is_wildcard);
    }

    #[test]
    fn parse_rust_doc_comments() {
        let source = r#"
/// This is a documented function.
/// It does something useful.
fn documented() {}
"#;
        let parser = RustParser;
        let symbols = parser.parse_symbols("test.rs", source);
        let sym = symbols.iter().find(|s| s.name == "documented").unwrap();
        assert!(sym.doc_comment.as_ref().unwrap().contains("documented function"));
        assert!(sym.doc_comment.as_ref().unwrap().contains("useful"));
    }

    #[test]
    fn parse_rust_constants_and_types() {
        let source = r#"
const MAX_SIZE: usize = 1024;
static VERSION: &str = "1.0";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
"#;
        let parser = RustParser;
        let symbols = parser.parse_symbols("test.rs", source);
        assert!(symbols.iter().any(|s| s.name == "MAX_SIZE" && s.kind == SymbolKind::Constant));
        assert!(symbols.iter().any(|s| s.name == "VERSION" && s.kind == SymbolKind::Constant));
        assert!(symbols.iter().any(|s| s.name == "Result" && s.kind == SymbolKind::TypeAlias));
    }
}
