# F: LSP Architecture Specification

This document specifies the Language Server Protocol implementation for the V2 compiler.

---

## Response Time Budget

| Operation | Budget | Strategy |
|-----------|--------|----------|
| Hover | <20ms | Cached type lookup |
| Completions | <100ms | Scope + type filtering |
| Go-to-definition | <50ms | Indexed symbol table |
| Find references | <200ms | Indexed + parallel search |
| Diagnostics | <50ms | Incremental validation |
| Rename | <500ms | Indexed + parallel rewrite |
| Signature help | <30ms | Cached function signatures |
| Document symbols | <50ms | Cached per file |
| Workspace symbols | <200ms | Indexed |
| Formatting | <100ms | Per-file, cached |
| Code actions | <100ms | Context-aware suggestions |

---

## Server Architecture

```rust
/// LSP server with incremental compilation
pub struct LspServer {
    /// Compilation database
    db: CompilerDb,

    /// Symbol index for fast lookups
    index: SymbolIndex,

    /// Open file tracking
    open_files: DashMap<Url, OpenFile>,

    /// Diagnostics channel
    diagnostics_tx: mpsc::Sender<PublishDiagnosticsParams>,

    /// Background task handles
    background_tasks: Vec<JoinHandle<()>>,
}

/// Tracked open file
pub struct OpenFile {
    /// Document version
    pub version: i32,

    /// Source content
    pub content: String,

    /// Lazily parsed module
    pub parsed: OnceCell<LazyModule>,

    /// Last diagnostics
    pub diagnostics: Vec<Diagnostic>,
}
```

---

## Lazy Module

```rust
/// Module with lazily-parsed function bodies
pub struct LazyModule {
    /// Source file reference
    file: SourceFile,

    /// Parsed signatures (always available)
    signatures: Vec<FunctionSignature>,

    /// Function body tokens (for deferred parsing)
    body_tokens: FxHashMap<FunctionId, TokenRange>,

    /// Parsed bodies (on-demand)
    bodies: RefCell<FxHashMap<FunctionId, ExprId>>,

    /// Expression arena
    arena: RefCell<ExprArena>,
}

impl LazyModule {
    /// Parse with lazy bodies for LSP
    pub fn parse_lazy(db: &dyn Db, file: SourceFile) -> Self {
        let tokens = tokens(db, file);
        let interner = db.interner();

        let mut parser = LazyParser::new(&tokens, interner);
        let mut signatures = Vec::new();
        let mut body_tokens = FxHashMap::default();

        while !parser.at_end() {
            if parser.at(TokenKind::At) {
                // Parse signature only
                let sig = parser.parse_function_signature();
                let func_id = FunctionId(signatures.len() as u32);

                // Record body token range without parsing
                let body_range = parser.skip_function_body();
                body_tokens.insert(func_id, body_range);

                signatures.push(sig);
            } else {
                parser.advance();
            }
        }

        Self {
            file,
            signatures,
            body_tokens,
            bodies: RefCell::new(FxHashMap::default()),
            arena: RefCell::new(ExprArena::new()),
        }
    }

    /// Get function body, parsing if needed
    pub fn get_body(&self, db: &dyn Db, func_id: FunctionId) -> ExprId {
        if let Some(&body) = self.bodies.borrow().get(&func_id) {
            return body;
        }

        // Parse body on demand
        let token_range = &self.body_tokens[&func_id];
        let all_tokens = tokens(db, self.file);
        let body_tokens = all_tokens.slice(token_range);

        let mut arena = self.arena.borrow_mut();
        let mut parser = Parser::new(&body_tokens, db.interner(), &mut arena);
        let body = parser.parse_expr().unwrap_or(ExprId::INVALID);

        self.bodies.borrow_mut().insert(func_id, body);
        body
    }
}
```

---

## Symbol Index

```rust
/// Fast symbol lookup index
pub struct SymbolIndex {
    /// Name → Definition locations
    definitions: DashMap<Name, Vec<DefinitionInfo>>,

    /// Name → Reference locations
    references: DashMap<Name, Vec<ReferenceInfo>>,

    /// File → Symbols in file
    file_symbols: DashMap<Url, Vec<DocumentSymbol>>,
}

#[derive(Clone)]
pub struct DefinitionInfo {
    pub file: Url,
    pub range: Range,
    pub kind: DefinitionKind,
    pub signature: Option<String>,
}

#[derive(Copy, Clone)]
pub enum DefinitionKind {
    Function,
    Type,
    Config,
    Variable,
    Parameter,
    Field,
    Pattern,
}

#[derive(Clone)]
pub struct ReferenceInfo {
    pub file: Url,
    pub range: Range,
    pub kind: ReferenceKind,
}

#[derive(Copy, Clone)]
pub enum ReferenceKind {
    Read,
    Write,
    Call,
    Type,
}
```

### Index Building

```rust
impl SymbolIndex {
    /// Build index from project (parallel)
    pub fn build(db: &dyn Db) -> Self {
        let index = Self {
            definitions: DashMap::new(),
            references: DashMap::new(),
            file_symbols: DashMap::new(),
        };

        // Index all modules in parallel
        db.all_modules()
            .par_iter()
            .for_each(|module| {
                index.index_module(db, *module);
            });

        index
    }

    /// Incremental update for single file
    pub fn update_file(&self, db: &dyn Db, file: SourceFile) {
        let file_url = url_from_path(file.path(db));

        // Remove old entries for this file
        self.definitions.retain(|_, infos| {
            infos.retain(|i| i.file != file_url);
            !infos.is_empty()
        });

        self.references.retain(|_, infos| {
            infos.retain(|i| i.file != file_url);
            !infos.is_empty()
        });

        self.file_symbols.remove(&file_url);

        // Re-index file
        let module = parsed_module(db, file);
        self.index_module(db, module);
    }
}
```

---

## Request Handlers

### Hover

```rust
impl LspServer {
    /// Handle hover request
    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let start = Instant::now();

        let file = self.get_open_file(&params.text_document.uri)?;
        let position = params.position;

        // Find token at position
        let offset = self.position_to_offset(&file.content, position)?;
        let lazy_module = file.parsed.get()?;

        // Fast path: check if in function signature (already parsed)
        for (idx, sig) in lazy_module.signatures.iter().enumerate() {
            if sig.name_span.contains(offset) {
                return Some(self.hover_for_function(sig));
            }
            if sig.params_span.contains(offset) {
                return self.hover_for_param(sig, offset);
            }
        }

        // Need to parse body for this position
        let func_id = self.find_containing_function(lazy_module, offset)?;
        let body = lazy_module.get_body(&self.db, func_id);

        // Find expression at position
        let arena = lazy_module.arena.borrow();
        let expr = self.find_expr_at_offset(&arena, body, offset)?;

        // Get type
        let ty = self.db.type_of_expr(expr)?;

        debug_assert!(start.elapsed() < Duration::from_millis(20),
            "hover took {:?}", start.elapsed());

        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                format_type_hover(ty, &self.db)
            )),
            range: Some(span_to_range(arena.get(expr).span)),
        })
    }
}
```

### Completion

```rust
impl LspServer {
    /// Handle completion request
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        let start = Instant::now();

        let file = self.get_open_file(&params.text_document.uri)?;
        let position = params.position;
        let offset = self.position_to_offset(&file.content, position)?;

        let mut items = Vec::new();

        // 1. Keywords (context-sensitive)
        if self.in_expression_start(file, offset) {
            items.extend(keyword_completions());
        }

        if self.in_pattern_position(file, offset) {
            items.extend(pattern_keyword_completions());
        }

        // 2. Local scope
        let scope = self.find_scope_at(file, offset)?;
        for binding in scope.all_bindings(&self.db) {
            items.push(CompletionItem {
                label: self.db.interner().resolve(binding.name).to_string(),
                kind: Some(binding_kind_to_completion(binding.kind)),
                detail: Some(format_type(binding.ty, &self.db)),
                ..Default::default()
            });
        }

        // 3. Imports
        let module = file.parsed.get()?.module;
        let imports = resolved_imports(&self.db, module);
        for import in &imports.resolved {
            for item in &import.items {
                items.push(CompletionItem {
                    label: self.db.interner().resolve(item.name).to_string(),
                    kind: Some(import_kind_to_completion(item.kind)),
                    detail: item.signature.clone(),
                    ..Default::default()
                });
            }
        }

        // 4. Type-based filtering
        if let Some(expected_ty) = self.expected_type_at(file, offset) {
            items.retain(|item| self.completion_matches_type(item, expected_ty));
        }

        debug_assert!(start.elapsed() < Duration::from_millis(100),
            "completion took {:?}", start.elapsed());

        Some(CompletionResponse::Array(items))
    }

    fn pattern_keyword_completions() -> Vec<CompletionItem> {
        vec![
            pattern_completion("map", "map(.over: ${1:items}, .transform: ${2:fn})"),
            pattern_completion("filter", "filter(.over: ${1:items}, .predicate: ${2:fn})"),
            pattern_completion("fold", "fold(.over: ${1:items}, .init: ${2:initial}, .op: ${3:fn})"),
            pattern_completion("run", "run(${1:})"),
            pattern_completion("try", "try(${1:}?)"),
            // ...
        ]
    }
}
```

### Go to Definition

```rust
impl LspServer {
    /// Handle go-to-definition request
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let start = Instant::now();

        let file = self.get_open_file(&params.text_document.uri)?;
        let offset = self.position_to_offset(&file.content, params.position)?;

        // Find identifier at position
        let name = self.find_identifier_at(file, offset)?;

        // Look up in index
        let definitions = self.index.definitions.get(&name)?;

        let result = if definitions.len() == 1 {
            GotoDefinitionResponse::Scalar(Location {
                uri: definitions[0].file.clone(),
                range: definitions[0].range,
            })
        } else {
            GotoDefinitionResponse::Array(
                definitions.iter()
                    .map(|d| Location {
                        uri: d.file.clone(),
                        range: d.range,
                    })
                    .collect()
            )
        };

        debug_assert!(start.elapsed() < Duration::from_millis(50),
            "goto_definition took {:?}", start.elapsed());

        Some(result)
    }
}
```

### Find References

```rust
impl LspServer {
    /// Handle find-references request
    pub fn find_references(&self, params: ReferenceParams) -> Option<Vec<Location>> {
        let start = Instant::now();

        let file = self.get_open_file(&params.text_document.uri)?;
        let offset = self.position_to_offset(&file.content, params.position)?;

        // Find identifier at position
        let name = self.find_identifier_at(file, offset)?;

        // Collect references
        let mut locations = Vec::new();

        // Include definition if requested
        if params.context.include_declaration {
            if let Some(defs) = self.index.definitions.get(&name) {
                for def in defs.iter() {
                    locations.push(Location {
                        uri: def.file.clone(),
                        range: def.range,
                    });
                }
            }
        }

        // Include all references
        if let Some(refs) = self.index.references.get(&name) {
            for ref_info in refs.iter() {
                locations.push(Location {
                    uri: ref_info.file.clone(),
                    range: ref_info.range,
                });
            }
        }

        debug_assert!(start.elapsed() < Duration::from_millis(200),
            "find_references took {:?}", start.elapsed());

        Some(locations)
    }
}
```

---

## Diagnostics

### Incremental Diagnostics

```rust
impl LspServer {
    /// Publish diagnostics for file
    pub fn publish_diagnostics(&self, file_url: &Url) {
        let start = Instant::now();

        let file = match self.open_files.get(file_url) {
            Some(f) => f,
            None => return,
        };

        // Get diagnostics from incremental compilation
        let source_file = self.db.source_file_for_url(file_url);
        let module = parsed_module(&self.db, source_file);
        let typed = typed_module(&self.db, module);

        let diagnostics: Vec<_> = typed.diagnostics(&self.db)
            .iter()
            .map(|d| to_lsp_diagnostic(d, file_url))
            .collect();

        self.diagnostics_tx.send(PublishDiagnosticsParams {
            uri: file_url.clone(),
            diagnostics,
            version: Some(file.version),
        }).ok();

        debug_assert!(start.elapsed() < Duration::from_millis(50),
            "diagnostics took {:?}", start.elapsed());
    }
}
```

### Background Validation

```rust
impl LspServer {
    /// Start background validation task
    pub fn start_background_validation(&mut self) {
        let db = self.db.clone();
        let diagnostics_tx = self.diagnostics_tx.clone();
        let open_files = self.open_files.clone();

        let handle = std::thread::spawn(move || {
            loop {
                // Wait for changes
                std::thread::sleep(Duration::from_millis(100));

                // Validate all open files
                for entry in open_files.iter() {
                    let file_url = entry.key();
                    let source_file = db.source_file_for_url(file_url);

                    // Incremental type check
                    let module = parsed_module(&db, source_file);
                    let typed = typed_module(&db, module);

                    let diagnostics: Vec<_> = typed.diagnostics(&db)
                        .iter()
                        .map(|d| to_lsp_diagnostic(d, file_url))
                        .collect();

                    diagnostics_tx.send(PublishDiagnosticsParams {
                        uri: file_url.clone(),
                        diagnostics,
                        version: Some(entry.version),
                    }).ok();
                }
            }
        });

        self.background_tasks.push(handle);
    }
}
```

---

## Signature Help

```rust
impl LspServer {
    /// Handle signature help request
    pub fn signature_help(&self, params: SignatureHelpParams) -> Option<SignatureHelp> {
        let file = self.get_open_file(&params.text_document.uri)?;
        let offset = self.position_to_offset(&file.content, params.position)?;

        // Find function call context
        let (func_name, arg_index) = self.find_call_context(file, offset)?;

        // Look up function signature
        let sig = self.get_function_signature(func_name)?;

        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: sig.to_string(),
                documentation: sig.doc.clone(),
                parameters: Some(
                    sig.params.iter()
                        .map(|(name, ty)| ParameterInformation {
                            label: ParameterLabel::Simple(format!("{}: {}", name, ty)),
                            documentation: None,
                        })
                        .collect()
                ),
                active_parameter: Some(arg_index as u32),
            }],
            active_signature: Some(0),
            active_parameter: Some(arg_index as u32),
        })
    }
}
```

---

## Code Actions

```rust
impl LspServer {
    /// Handle code action request
    pub fn code_actions(&self, params: CodeActionParams) -> Option<Vec<CodeAction>> {
        let file = self.get_open_file(&params.text_document.uri)?;
        let range = params.range;

        let mut actions = Vec::new();

        // Check diagnostics in range
        for diag in &file.diagnostics {
            if !range_overlaps(range, diag.labels[0].span.into()) {
                continue;
            }

            // Add quick fixes based on diagnostic
            match diag.code {
                ErrorCode::E4001 => {
                    // Missing test - offer to generate
                    actions.push(self.action_generate_test(file, diag));
                }
                ErrorCode::E3001 => {
                    // Undefined variable - offer import
                    if let Some(action) = self.action_add_import(file, diag) {
                        actions.push(action);
                    }
                }
                ErrorCode::E2001 => {
                    // Type mismatch - offer conversion
                    if let Some(action) = self.action_convert_type(file, diag) {
                        actions.push(action);
                    }
                }
                _ => {}
            }
        }

        Some(actions)
    }
}
```

---

## Performance Monitoring

```rust
/// Track LSP operation performance
pub struct LspMetrics {
    hover_times: Histogram,
    completion_times: Histogram,
    goto_def_times: Histogram,
    references_times: Histogram,
    diagnostics_times: Histogram,
}

impl LspMetrics {
    pub fn record_hover(&self, duration: Duration) {
        self.hover_times.record(duration.as_micros() as u64);

        if duration > Duration::from_millis(20) {
            tracing::warn!("hover exceeded budget: {:?}", duration);
        }
    }

    pub fn report(&self) -> MetricsReport {
        MetricsReport {
            hover_p50: self.hover_times.percentile(50.0),
            hover_p99: self.hover_times.percentile(99.0),
            completion_p50: self.completion_times.percentile(50.0),
            completion_p99: self.completion_times.percentile(99.0),
            // ...
        }
    }
}
```
