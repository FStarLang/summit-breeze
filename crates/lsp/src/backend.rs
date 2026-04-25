use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use summit_breeze_lsp::document::DocumentStore;
use summit_breeze_lsp::symbols::{CommandInfoKind, SymbolKind as SmtSymbolKind};

pub struct Backend {
    client: Client,
    store: DocumentStore,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            store: DocumentStore::new(),
        }
    }

    async fn publish_diagnostics(&self, uri: &Url) {
        let Some(doc) = self.store.get(uri) else {
            return;
        };
        let diags: Vec<Diagnostic> = doc
            .diagnostics
            .iter()
            .map(|d| Diagnostic {
                range: doc.span_to_range(d.span),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("summit-breeze".to_string()),
                message: d.message.clone(),
                ..Default::default()
            })
            .collect();
        self.client
            .publish_diagnostics(uri.clone(), diags, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.store.open(uri.clone(), params.text_document.text);
        self.publish_diagnostics(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            self.store.update(&uri, change.text);
            self.publish_diagnostics(&uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.store.close(&params.text_document.uri);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some(doc) = self.store.get(uri) else {
            return Ok(None);
        };

        let offset = doc.position_to_offset(pos);

        // Check if cursor is on a push — jump to matching pop
        for pair in &doc.index.push_pop_pairs {
            if span_contains(pair.push_span, offset) {
                if let Some(pop_span) = pair.pop_span {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        uri.clone(),
                        doc.span_to_range(pop_span),
                    ))));
                }
            }
            // Check if cursor is on a pop — jump to matching push
            if let Some(pop_span) = pair.pop_span {
                if span_contains(pop_span, offset) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        uri.clone(),
                        doc.span_to_range(pair.push_span),
                    ))));
                }
            }
        }

        // Find the reference under cursor and resolve its definition
        if let Some(sym_ref) = doc.index.ref_at(offset) {
            if let Some(def) = doc.index.resolve(&sym_ref.name, offset, sym_ref.stack_depth) {
                return Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                    uri.clone(),
                    doc.span_to_range(def.name_span),
                ))));
            }
        }

        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let Some(doc) = self.store.get(uri) else {
            return Ok(None);
        };

        let offset = doc.position_to_offset(pos);

        // Try reference under cursor first
        let resolved = doc.index.ref_at(offset).and_then(|sym_ref| {
            doc.index
                .resolve(&sym_ref.name, offset, sym_ref.stack_depth)
        });

        // Also check if cursor is on a definition site directly
        let def = resolved.or_else(|| {
            doc.index.definitions.values().flat_map(|v| v.iter()).find(|d| {
                span_contains(d.name_span, offset)
            })
        });

        let Some(def) = def else {
            return Ok(None);
        };

        // Extract the source text of the definition command
        let def_text = &doc.text[def.def_span.start as usize..def.def_span.end as usize];
        // Truncate long definitions
        let snippet = if def_text.len() > 200 {
            format!("{}…", &def_text[..200])
        } else {
            def_text.to_string()
        };

        let contents = format!("```smtlib\n{}\n```", snippet);

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: contents,
            }),
            range: None,
        }))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let Some(doc) = self.store.get(uri) else {
            return Ok(None);
        };

        let offset = doc.position_to_offset(pos);

        // Determine which definition the cursor resolves to.
        // Could be on a ref site or a def site.
        let target_def = doc
            .index
            .ref_at(offset)
            .and_then(|r| {
                doc.index
                    .resolve(&r.name, offset, r.stack_depth)
            })
            .or_else(|| {
                doc.index
                    .definitions
                    .values()
                    .flat_map(|v| v.iter())
                    .find(|d| span_contains(d.name_span, offset))
            });

        let Some(target) = target_def else {
            return Ok(None);
        };

        let name = target.name.clone();
        let target_name_start = target.name_span.start;

        let mut locations = Vec::new();

        // Include definition site if requested
        if params.context.include_declaration {
            locations.push(Location::new(uri.clone(), doc.span_to_range(target.name_span)));
        }

        // Include only references that resolve to the same definition
        for r in &doc.index.references {
            if r.name == name {
                if let Some(resolved) = doc.index.resolve(&r.name, r.span.start, r.stack_depth) {
                    if resolved.name_span.start == target_name_start {
                        locations.push(Location::new(uri.clone(), doc.span_to_range(r.span)));
                    }
                }
            }
        }

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;

        let Some(doc) = self.store.get(uri) else {
            return Ok(None);
        };

        #[allow(deprecated)]
        let symbols: Vec<DocumentSymbol> = doc
            .index
            .command_spans
            .iter()
            .filter_map(|cmd| {
                let (name, kind) = match cmd.kind {
                    CommandInfoKind::DeclareFun | CommandInfoKind::DeclareConst => (
                        cmd.name.as_deref().unwrap_or("?").to_string(),
                        SymbolKind2::FUNCTION,
                    ),
                    CommandInfoKind::DefineFun | CommandInfoKind::DefineFunRec => (
                        cmd.name.as_deref().unwrap_or("?").to_string(),
                        SymbolKind2::FUNCTION,
                    ),
                    CommandInfoKind::DeclareSort | CommandInfoKind::DefineSort => (
                        cmd.name.as_deref().unwrap_or("?").to_string(),
                        SymbolKind2::CLASS,
                    ),
                    CommandInfoKind::DeclareDatatype | CommandInfoKind::DeclareDatatypes => (
                        cmd.name.as_deref().unwrap_or("?").to_string(),
                        SymbolKind2::ENUM,
                    ),
                    CommandInfoKind::Assert => ("assert".to_string(), SymbolKind2::EVENT),
                    CommandInfoKind::CheckSat => ("check-sat".to_string(), SymbolKind2::EVENT),
                    CommandInfoKind::Push => ("push".to_string(), SymbolKind2::NAMESPACE),
                    CommandInfoKind::Pop => ("pop".to_string(), SymbolKind2::NAMESPACE),
                    CommandInfoKind::SetLogic => (
                        format!("set-logic {}", cmd.name.as_deref().unwrap_or("?")),
                        SymbolKind2::KEY,
                    ),
                    CommandInfoKind::Other => return None,
                };

                let range = doc.span_to_range(cmd.span);
                Some(DocumentSymbol {
                    name,
                    detail: None,
                    kind,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                })
            })
            .collect();

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = &params.text_document.uri;

        let Some(doc) = self.store.get(uri) else {
            return Ok(None);
        };

        let mut ranges = Vec::new();

        // Fold each top-level command that spans multiple lines
        for cmd in &doc.index.command_spans {
            let range = doc.span_to_range(cmd.span);
            if range.start.line < range.end.line {
                ranges.push(FoldingRange {
                    start_line: range.start.line,
                    start_character: Some(range.start.character),
                    end_line: range.end.line,
                    end_character: Some(range.end.character),
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: None,
                });
            }
        }

        // Fold push/pop blocks
        for pair in &doc.index.push_pop_pairs {
            if let Some(pop_span) = pair.pop_span {
                let start = doc.offset_to_position(pair.push_span.start);
                let end = doc.offset_to_position(pop_span.end);
                if start.line < end.line {
                    ranges.push(FoldingRange {
                        start_line: start.line,
                        start_character: Some(start.character),
                        end_line: end.line,
                        end_character: Some(end.character),
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
        }

        Ok(Some(ranges))
    }
}

// Rename lsp_types::SymbolKind to avoid conflict with our SymbolKind
use tower_lsp::lsp_types::SymbolKind as SymbolKind2;

fn span_contains(span: smtlib_parser::span::Span, offset: u32) -> bool {
    offset >= span.start && offset < span.end
}

