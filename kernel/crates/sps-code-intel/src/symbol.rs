//! Symbol types — the universal representation of code entities.
//!
//! A `Symbol` is any named definition in source code: a function, class,
//! struct, enum, interface, type alias, constant, module, etc. Each symbol
//! has a location (file + line + column) and metadata.

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use uuid::Uuid;

/// Kind of symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// Function or method.
    Function,
    /// Class.
    Class,
    /// Struct (Rust, Go).
    Struct,
    /// Enum (Rust, TypeScript, Python).
    Enum,
    /// Interface (TypeScript, Go).
    Interface,
    /// Type alias.
    TypeAlias,
    /// Module / namespace.
    Module,
    /// Constant / static.
    Constant,
    /// Variable (top-level).
    Variable,
    /// Trait (Rust).
    Trait,
    /// Implementation block (Rust).
    Impl,
    /// Macro (Rust).
    Macro,
    /// Unknown.
    Unknown,
}

impl SymbolKind {
    /// String identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::TypeAlias => "type_alias",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::Macro => "macro",
            Self::Unknown => "unknown",
        }
    }

    /// Human-readable icon (emoji).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Function => "ƒ",
            Self::Class => "C",
            Self::Struct => "S",
            Self::Enum => "E",
            Self::Interface => "I",
            Self::TypeAlias => "T",
            Self::Module => "M",
            Self::Constant => "c",
            Self::Variable => "v",
            Self::Trait => "R",
            Self::Impl => "i",
            Self::Macro => "m",
            Self::Unknown => "?",
        }
    }
}

/// Location of a symbol in source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolLocation {
    /// File path (relative to workspace root).
    pub file: SmolStr,
    /// 1-indexed line number.
    pub line: u32,
    /// 1-indexed column number.
    pub column: u32,
    /// End line (for multi-line symbols).
    pub end_line: u32,
}

/// A symbol — a named definition in source code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    /// Unique id (generated on index).
    pub id: Uuid,
    /// Symbol name (e.g. "my_function", "MyStruct").
    pub name: SmolStr,
    /// Qualified name (e.g. "module::MyStruct::my_function").
    pub qualified_name: SmolStr,
    /// Kind.
    pub kind: SymbolKind,
    /// Language.
    pub language: SmolStr,
    /// Location.
    pub location: SymbolLocation,
    /// Documentation comment (if found above the symbol).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
    /// Parameters (for functions).
    #[serde(default)]
    pub parameters: Vec<SmolStr>,
    /// Return type (if detected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<SmolStr>,
    /// Parent symbol id (e.g. for methods inside a class).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
}

impl Symbol {
    /// Create a new symbol.
    pub fn new(
        name: impl Into<SmolStr>,
        kind: SymbolKind,
        language: impl Into<SmolStr>,
        location: SymbolLocation,
    ) -> Self {
        let name = name.into();
        let qualified_name = name.clone();
        Self {
            id: Uuid::now_v7(),
            name,
            qualified_name,
            kind,
            language: language.into(),
            location,
            doc_comment: None,
            parameters: Vec::new(),
            return_type: None,
            parent_id: None,
        }
    }
}

/// An import statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    /// File that contains the import.
    pub file: SmolStr,
    /// Line number.
    pub line: u32,
    /// What is imported (module path or symbol name).
    pub path: SmolStr,
    /// Aliased name (if any, e.g. `use foo as bar`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<SmolStr>,
    /// Whether this is a wildcard import (e.g. `use foo::*`).
    pub is_wildcard: bool,
}
