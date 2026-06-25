//! SPS LSP — simplified protocol helpers.

use sps_code_intel::symbol::{Symbol as SpsSymbol, SymbolKind as SpsSymbolKind};
use tower_lsp::lsp_types::*;

pub fn to_lsp_kind(kind: SpsSymbolKind) -> SymbolKind {
    match kind {
        SpsSymbolKind::Function => SymbolKind::FUNCTION,
        SpsSymbolKind::Class => SymbolKind::CLASS,
        SpsSymbolKind::Struct => SymbolKind::STRUCT,
        SpsSymbolKind::Enum => SymbolKind::ENUM,
        SpsSymbolKind::Interface => SymbolKind::INTERFACE,
        SpsSymbolKind::TypeAlias => SymbolKind::TYPE_PARAMETER,
        SpsSymbolKind::Module => SymbolKind::MODULE,
        SpsSymbolKind::Constant => SymbolKind::CONSTANT,
        SpsSymbolKind::Variable => SymbolKind::VARIABLE,
        SpsSymbolKind::Trait => SymbolKind::INTERFACE,
        SpsSymbolKind::Impl => SymbolKind::OBJECT,
        SpsSymbolKind::Macro => SymbolKind::FUNCTION,
        SpsSymbolKind::Unknown => SymbolKind::VARIABLE,
    }
}

pub fn sym_to_doc(sym: &SpsSymbol) -> DocumentSymbol {
    let pos = |l: u32, c: u32| Position { line: l.saturating_sub(1), character: c.saturating_sub(1) };
    let r = Range { start: pos(sym.location.line, sym.location.column), end: pos(sym.location.end_line, sym.location.column + sym.name.len() as u32) };
    DocumentSymbol {
        name: sym.name.to_string(),
        detail: Some(sym.qualified_name.to_string()),
        kind: to_lsp_kind(sym.kind),
        tags: None,
        range: r,
        selection_range: r,
        children: None,
        deprecated: None,
    }
}

pub fn sym_to_info(sym: &SpsSymbol) -> SymbolInformation {
    let pos = |l: u32, c: u32| Position { line: l.saturating_sub(1), character: c.saturating_sub(1) };
    SymbolInformation {
        name: sym.name.to_string(),
        kind: to_lsp_kind(sym.kind),
        tags: None,
        deprecated: None,
        location: Location {
            uri: Url::from_file_path(&*sym.location.file).unwrap_or_else(|_| Url::parse("file:///unknown").unwrap()),
            range: Range { start: pos(sym.location.line, sym.location.column), end: pos(sym.location.end_line, sym.location.column + sym.name.len() as u32) },
        },
        container_name: None,
    }
}

pub fn ref_to_loc(rf: &sps_code_intel::index::Reference) -> Location {
    Location {
        uri: Url::from_file_path(&*rf.file).unwrap_or_else(|_| Url::parse("file:///unknown").unwrap()),
        range: Range {
            start: Position { line: rf.line.saturating_sub(1), character: rf.column.saturating_sub(1) },
            end: Position { line: rf.line.saturating_sub(1), character: rf.column.saturating_sub(1) + 10 },
        },
    }
}

pub fn sym_to_completion(sym: &SpsSymbol) -> CompletionItem {
    CompletionItem {
        label: sym.name.to_string(),
        kind: Some(match sym.kind {
            SpsSymbolKind::Function => CompletionItemKind::FUNCTION,
            SpsSymbolKind::Class | SpsSymbolKind::Struct => CompletionItemKind::CLASS,
            SpsSymbolKind::Enum => CompletionItemKind::ENUM,
            SpsSymbolKind::Interface | SpsSymbolKind::Trait => CompletionItemKind::INTERFACE,
            SpsSymbolKind::Constant => CompletionItemKind::CONSTANT,
            SpsSymbolKind::Module => CompletionItemKind::MODULE,
            SpsSymbolKind::TypeAlias => CompletionItemKind::TYPE_PARAMETER,
            _ => CompletionItemKind::VARIABLE,
        }),
        detail: Some(sym.qualified_name.to_string()),
        documentation: sym.doc_comment.as_ref().map(|d| Documentation::String(d.clone())),
        ..Default::default()
    }
}

pub fn sym_to_hover(sym: &SpsSymbol) -> Hover {
    let mut md = format!("**{}** ({})\n```{}\n{}\n```", sym.name, sym.kind.as_str(), sym.language, sym.qualified_name);
    if let Some(ref doc) = sym.doc_comment { md.push_str(&format!("\n\n---\n{}", doc)); }
    if !sym.parameters.is_empty() { md.push_str(&format!("\n\n**Params:** {}", sym.parameters.join(", "))); }
    if let Some(ref ret) = sym.return_type { md.push_str(&format!("\n**Returns:** {}", ret)); }
    Hover {
        contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: md }),
        range: None,
    }
}
