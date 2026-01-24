// Sigil LSP Server implementation

use dashmap::DashMap;
use sigilc::{
    ast::{FunctionDef, Item, Module, TypeDef, TypeExpr},
    lexer, parser,
};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// Document state tracked by the server
struct Document {
    /// Raw source text
    text: String,
    /// Parsed module (if successful)
    module: Option<Module>,
    /// Diagnostics from last parse/check
    diagnostics: Vec<Diagnostic>,
}

/// Sigil Language Server
pub struct SigilLanguageServer {
    client: Client,
    documents: DashMap<Url, Document>,
}

impl SigilLanguageServer {
    pub fn new(client: Client) -> Self {
        SigilLanguageServer {
            client,
            documents: DashMap::new(),
        }
    }

    /// Parse a document and return diagnostics
    async fn parse_document(&self, uri: &Url, text: &str) -> (Option<Module>, Vec<Diagnostic>) {
        let filename = uri.path();
        let mut diagnostics = Vec::new();

        // Lexer phase
        let tokens = match lexer::tokenize(text, filename) {
            Ok(t) => t,
            Err(e) => {
                diagnostics.push(Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 10)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E1000".to_string())),
                    source: Some("sigil".to_string()),
                    message: format!("Lexer error: {}", e),
                    ..Default::default()
                });
                return (None, diagnostics);
            }
        };

        // Parser phase
        let mut parser = parser::Parser::new(tokens);
        let module = match parser.parse_module(filename) {
            Ok(m) => m,
            Err(e) => {
                diagnostics.push(Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 10)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E2000".to_string())),
                    source: Some("sigil".to_string()),
                    message: format!("Parse error: {}", e),
                    ..Default::default()
                });
                return (None, diagnostics);
            }
        };

        // Type checking phase
        match sigilc::type_check(module.clone()) {
            Ok(_) => {}
            Err(diag) => {
                let range = if diag.labels.is_empty() {
                    Range::new(Position::new(0, 0), Position::new(0, 10))
                } else {
                    let label = &diag.labels[0];
                    let start = offset_to_position(text, label.span.range.start);
                    let end = offset_to_position(text, label.span.range.end);
                    Range::new(start, end)
                };

                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String(format!("{:?}", diag.code))),
                    source: Some("sigil".to_string()),
                    message: diag.message,
                    ..Default::default()
                });
            }
        }

        (Some(module), diagnostics)
    }

    /// Publish diagnostics for a document
    async fn publish_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) {
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    /// Get hover info for a position
    fn get_hover_info(&self, uri: &Url, position: Position) -> Option<Hover> {
        let doc = self.documents.get(uri)?;
        let module = doc.module.as_ref()?;
        let offset = position_to_offset(&doc.text, position);

        // Find item at position
        for item in &module.items {
            if let Some(info) = self.hover_for_item(item, offset) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: info,
                    }),
                    range: None,
                });
            }
        }

        None
    }

    fn hover_for_item(&self, item: &Item, offset: usize) -> Option<String> {
        match item {
            Item::Function(fd) => {
                if fd.span.contains(&offset) {
                    Some(self.function_signature(fd))
                } else {
                    None
                }
            }
            Item::TypeDef(td) => {
                if td.span.contains(&offset) {
                    Some(self.type_signature(td))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn function_signature(&self, fd: &FunctionDef) -> String {
        let params: Vec<String> = fd
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, format_type(&p.ty)))
            .collect();

        let mut sig = format!("@{} ({}) -> {}", fd.name, params.join(", "), format_type(&fd.return_type));

        if !fd.uses_clause.is_empty() {
            sig.push_str(&format!(" uses {}", fd.uses_clause.join(", ")));
        }

        format!("```sigil\n{}\n```", sig)
    }

    fn type_signature(&self, td: &TypeDef) -> String {
        let params = if td.params.is_empty() {
            String::new()
        } else {
            format!("<{}>", td.params.join(", "))
        };

        format!("```sigil\ntype {}{}\n```", td.name, params)
    }

    /// Find definition at position
    fn find_definition(&self, uri: &Url, position: Position) -> Option<Location> {
        let doc = self.documents.get(uri)?;
        let module = doc.module.as_ref()?;
        let offset = position_to_offset(&doc.text, position);

        // Get the identifier at the position
        let name = self.get_identifier_at(&doc.text, offset)?;

        // Find definition
        for item in &module.items {
            if let Some(loc) = self.definition_for_name(item, &name, uri) {
                return Some(loc);
            }
        }

        None
    }

    fn get_identifier_at(&self, text: &str, offset: usize) -> Option<String> {
        let bytes = text.as_bytes();
        if offset >= bytes.len() {
            return None;
        }

        // Find start of identifier
        let mut start = offset;
        while start > 0 && is_ident_char(bytes[start - 1] as char) {
            start -= 1;
        }

        // Find end of identifier
        let mut end = offset;
        while end < bytes.len() && is_ident_char(bytes[end] as char) {
            end += 1;
        }

        if start == end {
            return None;
        }

        Some(text[start..end].to_string())
    }

    fn definition_for_name(&self, item: &Item, name: &str, uri: &Url) -> Option<Location> {
        match item {
            Item::Function(fd) if fd.name == name => {
                let doc = self.documents.get(uri)?;
                let start = offset_to_position(&doc.text, fd.span.start);
                let end = offset_to_position(&doc.text, fd.span.end);
                Some(Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                })
            }
            Item::TypeDef(td) if td.name == name => {
                let doc = self.documents.get(uri)?;
                let start = offset_to_position(&doc.text, td.span.start);
                let end = offset_to_position(&doc.text, td.span.end);
                Some(Location {
                    uri: uri.clone(),
                    range: Range::new(start, end),
                })
            }
            _ => None,
        }
    }

    /// Get completions at position
    fn get_completions(&self, uri: &Url, _position: Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        // Add keywords
        let keywords = [
            ("if", "if condition then expr else expr"),
            ("then", "if condition then expr"),
            ("else", "else branch"),
            ("let", "let name = value"),
            ("match", "match expr { pattern => body }"),
            ("for", "for x in collection do body"),
            ("true", "boolean true"),
            ("false", "boolean false"),
            ("nil", "nil value"),
        ];

        for (kw, doc) in keywords {
            completions.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(doc.to_string()),
                ..Default::default()
            });
        }

        // Add pattern keywords
        let patterns = [
            ("fold", "fold(over: coll, init: val, with: op)"),
            ("map", "map(over: coll, transform: fn)"),
            ("filter", "filter(over: coll, where: pred)"),
            ("collect", "collect(range: r, into: fn)"),
            ("recurse", "recurse(cond: c, base: b, step: s)"),
            ("parallel", "parallel(name: expr, ...)"),
            ("try", "try(body: expr)"),
            ("find", "find(in: coll, where: pred)"),
            ("validate", "validate(rules: [...], then: val)"),
        ];

        for (name, doc) in patterns {
            completions.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                detail: Some(doc.to_string()),
                insert_text: Some(format!("{}(\n    \n)", name)),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }

        // Add functions from current document
        if let Some(doc) = self.documents.get(uri) {
            if let Some(module) = &doc.module {
                for item in &module.items {
                    if let Item::Function(fd) = item {
                        completions.push(CompletionItem {
                            label: fd.name.clone(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some(format!("-> {}", format_type(&fd.return_type))),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        completions
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SigilLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), "@".to_string(), "$".to_string()]),
                    ..Default::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "sigil-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Sigil language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let (module, diagnostics) = self.parse_document(&uri, &text).await;

        self.documents.insert(
            uri.clone(),
            Document {
                text,
                module,
                diagnostics: diagnostics.clone(),
            },
        );

        self.publish_diagnostics(uri, diagnostics).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.content_changes.into_iter().next()
            .map(|c| c.text)
            .unwrap_or_default();

        let (module, diagnostics) = self.parse_document(&uri, &text).await;

        self.documents.insert(
            uri.clone(),
            Document {
                text,
                module,
                diagnostics: diagnostics.clone(),
            },
        );

        self.publish_diagnostics(uri, diagnostics).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        Ok(self.get_hover_info(&uri, position))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        Ok(self.find_definition(&uri, position).map(GotoDefinitionResponse::Scalar))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let completions = self.get_completions(&uri, position);
        Ok(Some(CompletionResponse::Array(completions)))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        if let Some(doc) = self.documents.get(&uri) {
            match sigilc::format::format(&doc.text) {
                Ok(formatted) => {
                    let lines = doc.text.lines().count() as u32;
                    let last_line_len = doc.text.lines().last().map_or(0, |l| l.len()) as u32;

                    Ok(Some(vec![TextEdit {
                        range: Range::new(
                            Position::new(0, 0),
                            Position::new(lines, last_line_len),
                        ),
                        new_text: formatted,
                    }]))
                }
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

// Helper functions

fn format_type(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(name) => name.clone(),
        TypeExpr::Generic(name, args) => {
            let args_str: Vec<String> = args.iter().map(format_type).collect();
            format!("{}<{}>", name, args_str.join(", "))
        }
        TypeExpr::List(inner) => format!("[{}]", format_type(inner)),
        TypeExpr::Optional(inner) => format!("?{}", format_type(inner)),
        TypeExpr::Tuple(types) => {
            let types_str: Vec<String> = types.iter().map(format_type).collect();
            format!("({})", types_str.join(", "))
        }
        TypeExpr::Function(param, ret) => {
            format!("({} -> {})", format_type(param), format_type(ret))
        }
        TypeExpr::Map(k, v) => format!("{{{}: {}}}", format_type(k), format_type(v)),
        TypeExpr::Record(fields) => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", n, format_type(t)))
                .collect();
            format!("{{ {} }}", fields_str.join(", "))
        }
        TypeExpr::DynTrait(name) => format!("dyn {}", name),
    }
}

fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut col = 0;

    for (i, c) in text.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position::new(line, col)
}

fn position_to_offset(text: &str, position: Position) -> usize {
    let mut line = 0;
    let mut col = 0;

    for (i, c) in text.char_indices() {
        if line == position.line && col == position.character {
            return i;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    text.len()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
