//! SPS Language Server — simplified for tower-lsp 0.20 compatibility.

use std::sync::Arc;
use parking_lot::RwLock;
use sps_code_intel::index::CodebaseIndex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

pub struct SpsLanguageServer {
    client: Client,
    index: Arc<CodebaseIndex>,
    root: RwLock<Option<std::path::PathBuf>>,
}

impl SpsLanguageServer {
    pub fn new(client: Client, index: Arc<CodebaseIndex>) -> Self {
        Self { client, index, root: RwLock::new(None) }
    }

    fn rel(&self, uri: &Url) -> String {
        let p = uri.path().to_string();
        if let Some(r) = self.root.read().as_ref() {
            let rs = r.to_string_lossy().to_string();
            if let Some(rel) = p.strip_prefix(&rs) { return rel.trim_start_matches('/').to_string(); }
        }
        p.rsplit('/').next().unwrap_or(&p).to_string()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SpsLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(u) = &params.root_uri { *self.root.write() = Some(std::path::PathBuf::from(u.path())); }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions { trigger_characters: Some(vec![".".into(), ":".into()]), ..Default::default() }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo { name: "SPS LSP".into(), version: Some("1.0.0".into()) }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "SPS LSP ready").await;
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        let f = self.rel(&p.text_document.uri);
        let _ = self.index.index_file(&f, &p.text_document.text);
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        let f = self.rel(&p.text_document.uri);
        if let Some(c) = p.content_changes.into_iter().next() {
            let _ = self.index.index_file(&f, &c.text);
        }
    }

    async fn completion(&self, p: CompletionParams) -> Result<Option<CompletionResponse>> {
        let f = self.rel(&p.text_document_position.text_document.uri);
        let syms = self.index.symbols_in_file(&f);
        let all: Vec<_> = self.index.search("", 100).into_iter().map(|r| r.symbol).collect();
        let items: Vec<_> = syms.iter().chain(all.iter()).map(crate::protocol::sym_to_completion).collect();
        Ok(Some(CompletionResponse::List(CompletionList { is_incomplete: true, items })))
    }

    async fn hover(&self, p: HoverParams) -> Result<Option<Hover>> {
        let f = self.rel(&p.text_document_position_params.text_document.uri);
        let l = p.text_document_position_params.position.line + 1;
        let sym = self.index.symbols_in_file(&f).into_iter().find(|s| s.location.line == l);
        Ok(sym.map(|s| crate::protocol::sym_to_hover(&s)))
    }

    async fn goto_definition(&self, p: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let f = self.rel(&p.text_document_position_params.text_document.uri);
        let l = p.text_document_position_params.position.line + 1;
        let syms = self.index.symbols_in_file(&f);
        if let Some(s) = syms.iter().find(|s| s.location.line == l) {
            let defs = self.index.go_to_definition(&s.name);
            let locs: Vec<_> = defs.iter().map(|d| Location {
                uri: Url::from_file_path(&*d.location.file).unwrap_or_else(|_| Url::parse("file:///unknown").unwrap()),
                range: Range {
                    start: Position { line: d.location.line.saturating_sub(1), character: d.location.column.saturating_sub(1) },
                    end: Position { line: d.location.end_line.saturating_sub(1), character: d.location.column.saturating_sub(1) + d.name.len() as u32 },
                },
            }).collect();
            if !locs.is_empty() { return Ok(Some(GotoDefinitionResponse::Array(locs))); }
        }
        Ok(None)
    }

    async fn references(&self, p: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let f = self.rel(&p.text_document_position.text_document.uri);
        let l = p.text_document_position.position.line + 1;
        let syms = self.index.symbols_in_file(&f);
        if let Some(s) = syms.iter().find(|s| s.location.line == l) {
            let refs = self.index.find_references(&s.name);
            return Ok(Some(refs.iter().map(crate::protocol::ref_to_loc).collect()));
        }
        Ok(None)
    }

    async fn document_symbol(&self, p: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let f = self.rel(&p.text_document.uri);
        let syms = self.index.symbols_in_file(&f);
        let ds: Vec<_> = syms.iter().map(crate::protocol::sym_to_doc).collect();
        if ds.is_empty() { Ok(None) } else { Ok(Some(DocumentSymbolResponse::Nested(ds))) }
    }

    async fn symbol(&self, p: WorkspaceSymbolParams) -> Result<Option<Vec<SymbolInformation>>> {
        let r = self.index.search(&p.query, 100);
        let si: Vec<_> = r.iter().map(|x| crate::protocol::sym_to_info(&x.symbol)).collect();
        if si.is_empty() { Ok(None) } else { Ok(Some(si)) }
    }
}

/// Run the LSP server on stdio.
pub async fn run_stdio(index: Arc<CodebaseIndex>) {
    let (service, socket) = LspService::new(|client| SpsLanguageServer::new(client, index.clone()));
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
}
