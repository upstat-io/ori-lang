# Phase 5: Advanced Features (Weeks 17-20)

## Goal

Build production-ready advanced features:
- Signature-based invalidation with semantic hashing
- Full LSP support with response time budgets
- Lazy parsing for LSP performance
- Tiered compilation modes

**Deliverable:** Production-ready compiler with full LSP support.

---

## Week 17: Signature-Based Invalidation

### Objective

Use semantic hashing of public API signatures to minimize recompilation.

> **Design Decision:** We evaluated test-gated invalidation (using test pass/fail to gate cache invalidation) but rejected it due to:
> - Slow tests would negate incremental build benefits
> - Flaky tests would cause non-deterministic builds
> - Tests might not cover the changed code paths
>
> Instead, we use signature-based invalidation like Rust - hash the public API surface and skip downstream recompilation when signatures are unchanged.

### Semantic Hashing

```rust
use std::hash::{Hash, Hasher};
use rustc_hash::FxHasher;

/// Semantic hash of a function's observable behavior
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct SemanticHash(u64);

impl SemanticHash {
    /// Compute semantic hash from typed function
    pub fn compute(typed: &TypedFunction, db: &dyn Db) -> Self {
        let mut hasher = FxHasher::default();

        // Hash signature (always part of semantic contract)
        typed.func.name(db).hash(&mut hasher);
        for (name, ty) in typed.func.params(db) {
            name.hash(&mut hasher);
            ty.hash(&mut hasher);
        }
        typed.func.return_type(db).hash(&mut hasher);

        // Hash capabilities
        for cap in typed.func.capabilities(db) {
            cap.hash(&mut hasher);
        }

        // Hash implementation details that affect behavior
        hash_expr_semantics(&mut hasher, typed.body, db);

        SemanticHash(hasher.finish())
    }
}

fn hash_expr_semantics(hasher: &mut impl Hasher, expr: ExprId, db: &dyn Db) {
    let arena = db.current_arena();
    let node = arena.get(expr);

    // Hash expression kind (structure)
    std::mem::discriminant(&node.kind).hash(hasher);

    match &node.kind {
        // Literals: hash value
        ExprKind::Int(n) => n.hash(hasher),
        ExprKind::Float(f) => f.to_bits().hash(hasher),
        ExprKind::Bool(b) => b.hash(hasher),
        ExprKind::String(s) => s.hash(hasher),

        // Identifiers: hash name
        ExprKind::Ident(name) => name.hash(hasher),

        // Compound: recurse
        ExprKind::Binary { op, left, right } => {
            op.hash(hasher);
            hash_expr_semantics(hasher, *left, db);
            hash_expr_semantics(hasher, *right, db);
        }

        ExprKind::Call { func, args } => {
            hash_expr_semantics(hasher, *func, db);
            for arg in arena.get_list(*args) {
                hash_expr_semantics(hasher, *arg, db);
            }
        }

        // Patterns: hash kind and args
        ExprKind::Pattern { kind, args } => {
            kind.hash(hasher);
            let args_node = arena.get_pattern_args(*args);
            for (name, expr) in args_node.iter() {
                name.hash(hasher);
                hash_expr_semantics(hasher, expr, db);
            }
        }

        // ... other cases
        _ => {}
    }
}
```

### Signature Hash Computation

```rust
/// Compute signature hash for a function's public API
#[salsa::tracked]
pub fn signature_hash(db: &dyn Db, func: Function) -> SignatureHash {
    let mut hasher = FxHasher::default();

    // Hash function name
    func.name(db).hash(&mut hasher);

    // Hash parameter names and types
    for (name, ty) in func.params(db) {
        name.hash(&mut hasher);
        ty.hash(&mut hasher);
    }

    // Hash return type
    func.return_type(db).hash(&mut hasher);

    // Hash capabilities
    for cap in func.capabilities(db) {
        cap.hash(&mut hasher);
    }

    // Hash visibility
    func.visibility(db).hash(&mut hasher);

    SignatureHash(hasher.finish())
}

/// Signature hash for a type definition
#[salsa::tracked]
pub fn type_signature_hash(db: &dyn Db, ty: TypeDef) -> SignatureHash {
    let mut hasher = FxHasher::default();

    ty.name(db).hash(&mut hasher);
    ty.visibility(db).hash(&mut hasher);

    match ty.kind(db) {
        TypeDefKind::Struct { fields } => {
            for (name, field_ty) in fields {
                name.hash(&mut hasher);
                field_ty.hash(&mut hasher);
            }
        }
        TypeDefKind::Sum { variants } => {
            for variant in variants {
                variant.name.hash(&mut hasher);
                for field_ty in &variant.fields {
                    field_ty.hash(&mut hasher);
                }
            }
        }
        TypeDefKind::Alias { target } => {
            target.hash(&mut hasher);
        }
    }

    SignatureHash(hasher.finish())
}
```

### Module Export Hash

```rust
/// Hash all public exports from a module
#[salsa::tracked]
pub fn module_export_hash(db: &dyn Db, module: Module) -> ModuleExportHash {
    let mut hasher = FxHasher::default();

    // Hash all public function signatures
    let mut funcs: Vec<_> = module.functions(db)
        .iter()
        .filter(|f| f.visibility(db) == Visibility::Public)
        .collect();
    funcs.sort_by_key(|f| f.name(db));  // Deterministic order

    for func in funcs {
        signature_hash(db, *func).hash(&mut hasher);
    }

    // Hash all public type definitions
    let mut types: Vec<_> = module.types(db)
        .iter()
        .filter(|t| t.visibility(db) == Visibility::Public)
        .collect();
    types.sort_by_key(|t| t.name(db));

    for ty in types {
        type_signature_hash(db, *ty).hash(&mut hasher);
    }

    ModuleExportHash(hasher.finish())
}
```

### Dependency Validation

```rust
/// Check if dependent module needs revalidation
pub fn needs_revalidation(
    db: &dyn Db,
    dependent: Module,
    dependency: Module,
) -> bool {
    // Get current export hash of dependency
    let current_hash = module_export_hash(db, dependency);

    // Get cached hash from when dependent was last compiled
    let cached_hash = db.cached_dependency_hash(dependent, dependency);

    match cached_hash {
        Some(cached) => cached != current_hash,  // Revalidate if changed
        None => true,  // No cache = must revalidate
    }
}
```

### Incremental Workflow

```rust
/// Incremental compilation with signature-based invalidation
pub fn compile_incremental(db: &mut CompilerDb, changed_file: SourceFile) {
    // 1. Update file content (Salsa handles query invalidation)
    let new_text = read_file(changed_file.path(db));
    changed_file.set_text(db).to(new_text);

    // 2. Reparse changed file
    let module = parsed_module(db, changed_file);

    // 3. Compute new export hash for changed module
    let new_hash = module_export_hash(db, module);
    let old_hash = db.cached_export_hash(module);

    // 4. If public API unchanged, downstream modules skip revalidation
    if Some(new_hash) == old_hash {
        // Only internal changes - no downstream impact
        // Just type check the changed module itself
        let _ = typed_module(db, module);
        return;
    }

    // 5. Public API changed - update cache and revalidate dependents
    db.update_export_hash_cache(module, new_hash);

    for dependent_module in db.modules_depending_on(module) {
        if needs_revalidation(db, dependent_module, module) {
            // Full revalidation of dependent
            let _ = typed_module(db, dependent_module);

            // Update dependency hash cache
            db.update_dependency_hash_cache(dependent_module, module, new_hash);
        }
    }
}
```

---

## Weeks 18-19: LSP Support

### Objective

Provide responsive LSP with strict response time budgets.

### Response Time Budget

| Operation | Budget | Strategy |
|-----------|--------|----------|
| Hover | <20ms | Cached type lookup |
| Completions | <100ms | Scope + type filtering |
| Go-to-definition | <50ms | Indexed symbol table |
| Find references | <200ms | Indexed + parallel search |
| Diagnostics | <50ms | Incremental validation |
| Rename | <500ms | Indexed + parallel rewrite |

### LSP Server Structure

```rust
/// LSP server with incremental database
pub struct LspServer {
    db: CompilerDb,
    symbol_index: SymbolIndex,
    open_files: FxHashMap<Url, OpenFile>,
    diagnostics_tx: mpsc::Sender<PublishDiagnostics>,
}

struct OpenFile {
    version: i32,
    content: String,
    /// Parsed lazily on demand
    parsed: Option<Module>,
}

impl LspServer {
    /// Handle hover request
    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let start = Instant::now();

        let file = self.get_file(&params.text_document.uri)?;
        let position = params.position;

        // Find expression at position
        let expr = self.find_expr_at_position(file, position)?;

        // Get type from cache (fast path)
        let ty = self.db.cached_type(expr)?;

        let result = Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                format_type(ty, &self.db)
            )),
            range: Some(expr_range(expr)),
        };

        debug_assert!(start.elapsed() < Duration::from_millis(20),
            "hover took {:?}", start.elapsed());

        Some(result)
    }

    /// Handle completion request
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        let start = Instant::now();

        let file = self.get_file(&params.text_document.uri)?;
        let position = params.position;

        // Get scope at position
        let scope = self.find_scope_at_position(file, position)?;

        // Collect completions from scope
        let mut items = Vec::new();

        // Local variables
        for binding in scope.all_bindings() {
            items.push(CompletionItem {
                label: self.db.interner().resolve(binding.name).to_string(),
                kind: Some(binding_to_completion_kind(binding.kind)),
                detail: Some(format_type(binding.ty, &self.db)),
                ..Default::default()
            });
        }

        // Keywords (context-sensitive)
        if self.in_pattern_position(file, position) {
            items.extend(pattern_keyword_completions());
        }

        // Type-based filtering if we have expected type
        if let Some(expected) = self.expected_type_at(file, position) {
            items.retain(|item| {
                self.completion_matches_type(item, expected)
            });
        }

        debug_assert!(start.elapsed() < Duration::from_millis(100),
            "completion took {:?}", start.elapsed());

        Some(CompletionResponse::Array(items))
    }

    /// Handle go-to-definition request
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let start = Instant::now();

        let file = self.get_file(&params.text_document.uri)?;
        let position = params.position;

        // Find identifier at position
        let name = self.find_ident_at_position(file, position)?;

        // Look up in symbol index
        let definition = self.symbol_index.get_definition(name)?;

        let result = GotoDefinitionResponse::Scalar(Location {
            uri: definition.file.clone(),
            range: definition.range,
        });

        debug_assert!(start.elapsed() < Duration::from_millis(50),
            "goto_definition took {:?}", start.elapsed());

        Some(result)
    }
}
```

### Lazy Parsing for LSP

```rust
/// Lazy-parsed module for LSP
pub struct LazyModule {
    file: SourceFile,
    /// Parsed signatures (always available)
    signatures: Vec<FunctionSignature>,
    /// Parsed bodies (on demand)
    bodies: FxHashMap<FunctionId, ExprId>,
    /// Raw body tokens (for deferred parsing)
    body_tokens: FxHashMap<FunctionId, TokenRange>,
}

impl LazyModule {
    /// Parse file with lazy bodies
    pub fn parse_lazy(db: &dyn Db, file: SourceFile) -> Self {
        let tokens = tokens(db, file);
        let mut parser = LazyParser::new(&tokens, db.interner());

        let mut signatures = Vec::new();
        let mut body_tokens = FxHashMap::default();

        while !parser.at_end() {
            if parser.at(TokenKind::At) {
                // Parse function signature
                let sig = parser.parse_function_signature()?;
                let func_id = FunctionId(signatures.len() as u32);

                // Save body tokens without parsing
                let body_range = parser.skip_body()?;
                body_tokens.insert(func_id, body_range);

                signatures.push(sig);
            } else {
                parser.advance();
            }
        }

        Self {
            file,
            signatures,
            bodies: FxHashMap::default(),
            body_tokens,
        }
    }

    /// Parse body on demand
    pub fn get_body(&mut self, db: &dyn Db, func_id: FunctionId) -> ExprId {
        if let Some(&body) = self.bodies.get(&func_id) {
            return body;
        }

        // Parse body from saved tokens
        let token_range = &self.body_tokens[&func_id];
        let tokens = db.tokens_range(self.file, token_range);

        let mut parser = Parser::new(&tokens, db.interner());
        let body = parser.parse_expr().unwrap();

        self.bodies.insert(func_id, body);
        body
    }
}
```

### Symbol Index

```rust
/// Index for fast symbol lookup
pub struct SymbolIndex {
    /// Name → Definition locations
    definitions: DashMap<Name, Vec<DefinitionLocation>>,
    /// Name → Reference locations
    references: DashMap<Name, Vec<ReferenceLocation>>,
}

#[derive(Clone)]
pub struct DefinitionLocation {
    pub file: Url,
    pub range: Range,
    pub kind: DefinitionKind,
}

impl SymbolIndex {
    /// Build index from project
    pub fn build(db: &dyn Db) -> Self {
        let index = Self {
            definitions: DashMap::new(),
            references: DashMap::new(),
        };

        // Index all modules in parallel
        db.all_modules()
            .par_iter()
            .for_each(|module| {
                index.index_module(db, *module);
            });

        index
    }

    /// Index single module
    fn index_module(&self, db: &dyn Db, module: Module) {
        let file_url = url_for_file(module.file(db));

        // Index function definitions
        for func in module.functions(db) {
            self.definitions
                .entry(func.name(db))
                .or_default()
                .push(DefinitionLocation {
                    file: file_url.clone(),
                    range: func.name_span(db).into(),
                    kind: DefinitionKind::Function,
                });
        }

        // Index type definitions
        for ty in module.types(db) {
            self.definitions
                .entry(ty.name(db))
                .or_default()
                .push(DefinitionLocation {
                    file: file_url.clone(),
                    range: ty.name_span(db).into(),
                    kind: DefinitionKind::Type,
                });
        }

        // Index all identifier references
        let arena = module.expr_arena(db);
        for expr_id in arena.all_exprs() {
            if let ExprKind::Ident(name) = arena.get(expr_id).kind {
                self.references
                    .entry(name)
                    .or_default()
                    .push(ReferenceLocation {
                        file: file_url.clone(),
                        range: arena.get(expr_id).span.into(),
                    });
            }
        }
    }

    /// Update index for changed file
    pub fn update_file(&self, db: &dyn Db, file: SourceFile) {
        let file_url = url_for_file(file);

        // Remove old entries for this file
        self.definitions.retain(|_, locs| {
            locs.retain(|loc| loc.file != file_url);
            !locs.is_empty()
        });

        self.references.retain(|_, locs| {
            locs.retain(|loc| loc.file != file_url);
            !locs.is_empty()
        });

        // Re-index file
        let module = parsed_module(db, file);
        self.index_module(db, module);
    }
}
```

---

## Week 20: Tiered Compilation

### Objective

Provide different compilation modes for different use cases.

### Compilation Tiers

```rust
/// Compilation tier selection
#[derive(Copy, Clone)]
pub enum CompilationTier {
    /// Syntax validation only - fastest
    Check,
    /// Full type checking, minimal optimization
    Build,
    /// Full optimization for release
    Release,
}

impl CompilationTier {
    pub fn from_args(args: &Args) -> Self {
        if args.check {
            CompilationTier::Check
        } else if args.release || args.opt {
            CompilationTier::Release
        } else {
            CompilationTier::Build
        }
    }
}

/// Tier-aware compilation
pub fn compile(db: &dyn Db, tier: CompilationTier) -> CompileResult {
    match tier {
        CompilationTier::Check => compile_check(db),
        CompilationTier::Build => compile_build(db),
        CompilationTier::Release => compile_release(db),
    }
}
```

### Check Mode (Fast Feedback)

```rust
/// Check mode: syntax + basic type validation
fn compile_check(db: &dyn Db) -> CompileResult {
    let files = db.all_files();

    // Parse all files
    let parse_results: Vec<_> = files
        .par_iter()
        .map(|f| {
            let module = parsed_module(db, *f);
            (f, module, module.diagnostics(db))
        })
        .collect();

    // Collect parse errors
    let mut diagnostics: Vec<_> = parse_results
        .iter()
        .flat_map(|(_, _, d)| d.iter().cloned())
        .collect();

    // Basic type check (no full inference)
    for (file, module, _) in &parse_results {
        let type_diags = basic_type_check(db, *module);
        diagnostics.extend(type_diags);
    }

    CompileResult {
        success: !diagnostics.iter().any(|d| d.is_error()),
        diagnostics,
        output: None,
    }
}

/// Basic type checking without full inference
fn basic_type_check(db: &dyn Db, module: Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for func in module.functions(db) {
        // Check declared types are valid
        for (_, ty) in func.params(db) {
            if !is_valid_type(db, *ty) {
                diagnostics.push(unknown_type_error(*ty, func.span(db)));
            }
        }

        // Check return type is valid
        if !is_valid_type(db, func.return_type(db)) {
            diagnostics.push(unknown_type_error(func.return_type(db), func.span(db)));
        }
    }

    diagnostics
}
```

### Build Mode (Development)

```rust
/// Build mode: full type checking, minimal optimization
fn compile_build(db: &dyn Db) -> CompileResult {
    // Full parse
    let modules = parse_project(db, &db.all_files());

    // Full type check
    let type_result = type_check_project(db);
    if type_result.has_errors() {
        return CompileResult {
            success: false,
            diagnostics: type_result.diagnostics,
            output: None,
        };
    }

    // Test coverage check
    let coverage = check_test_coverage(db);
    if !coverage.complete {
        return CompileResult {
            success: false,
            diagnostics: coverage.missing_test_errors(),
            output: None,
        };
    }

    // Codegen with minimal optimization
    let generated = codegen_project(db, &type_result.modules);

    // Compile C code
    let output = compile_c(&generated, OptLevel::Debug)?;

    CompileResult {
        success: true,
        diagnostics: type_result.diagnostics,
        output: Some(output),
    }
}
```

### Release Mode (Production)

```rust
/// Release mode: full optimization
fn compile_release(db: &dyn Db) -> CompileResult {
    // Build mode first
    let build_result = compile_build(db);
    if !build_result.success {
        return build_result;
    }

    // Run tests (required for release)
    let test_result = run_all_tests(db);
    if !test_result.all_passed() {
        return CompileResult {
            success: false,
            diagnostics: test_result.failure_diagnostics(),
            output: None,
        };
    }

    // Additional optimizations
    let typed_modules = db.all_typed_modules();

    // Pattern fusion
    let fused = apply_pattern_fusion(db, &typed_modules);

    // Dead code elimination
    let live = dead_code_elimination(db, &fused);

    // Optimized codegen
    let generated = codegen_optimized(db, &live);

    // Compile with optimizations
    let output = compile_c(&generated, OptLevel::Release)?;

    CompileResult {
        success: true,
        diagnostics: vec![],
        output: Some(output),
    }
}
```

---

## Phase 5 Deliverables Checklist

### Week 17: Signature-Based Invalidation
- [ ] `SignatureHash` computation for functions
- [ ] `type_signature_hash` for type definitions
- [ ] `module_export_hash` query
- [ ] Dependency validation with signature comparison
- [ ] Export hash caching

### Weeks 18-19: LSP Support
- [ ] `LspServer` structure
- [ ] Hover (<20ms)
- [ ] Completions (<100ms)
- [ ] Go-to-definition (<50ms)
- [ ] Find references (<200ms)
- [ ] Diagnostics (<50ms)
- [ ] Lazy parsing for bodies
- [ ] Symbol index with parallel build

### Week 20: Tiered Compilation
- [ ] `CompilationTier` enum
- [ ] Check mode (syntax + basic types)
- [ ] Build mode (full types, minimal opt)
- [ ] Release mode (full optimization)
- [ ] Pattern fusion in release mode

### Tests
- [ ] Signature hashing correctness tests
- [ ] LSP response time tests
- [ ] Tiered compilation output tests
- [ ] Incremental compilation tests

---

## Next Phase

With advanced features complete, proceed to [Phase 6: Formatter](08-phase-6-formatter.md) to build the CST-based formatter.
