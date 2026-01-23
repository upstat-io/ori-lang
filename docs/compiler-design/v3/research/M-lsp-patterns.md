# M: LSP Patterns Research

This document summarizes LSP implementation patterns from rust-analyzer and clangd.

---

## rust-analyzer Architecture

### Core Design

rust-analyzer uses a three-layer architecture:

```
┌─────────────────────────────────┐
│         LSP Protocol            │  JSON-RPC, lifecycle
├─────────────────────────────────┤
│      Request Handlers           │  Hover, completion, etc.
├─────────────────────────────────┤
│   Incremental Database (Salsa)  │  Query-based analysis
└─────────────────────────────────┘
```

### Request Flow

```rust
// 1. Receive JSON-RPC request
async fn handle_request(req: Request) -> Response {
    match req.method.as_str() {
        "textDocument/hover" => {
            let params: HoverParams = serde_json::from_value(req.params)?;
            let result = handlers::hover(&db, params).await?;
            Response::success(req.id, result)
        }
        // ...
    }
}

// 2. Handler queries the database
fn hover(db: &RootDatabase, params: HoverParams) -> Option<Hover> {
    let position = params.position;
    let file_id = db.file_id(&params.text_document.uri)?;

    // Query semantic info (cached/incremental)
    let sema = Semantics::new(db);
    let token = sema.find_token_at_offset(file_id, position)?;

    // Build hover result
    let ty = sema.type_of_expr(token)?;
    Some(Hover {
        contents: format_type(ty),
        range: token.range(),
    })
}
```

### Cancellation

rust-analyzer cancels stale requests:

```rust
pub struct RequestDispatcher {
    pending: HashMap<RequestId, CancellationToken>,
}

impl RequestDispatcher {
    fn dispatch(&mut self, req: Request) {
        // Cancel previous request for same file
        if let Some(old) = self.pending_for_file(&req.file()) {
            old.cancel();
        }

        let token = CancellationToken::new();
        self.pending.insert(req.id.clone(), token.clone());

        tokio::spawn(async move {
            tokio::select! {
                result = handle_request(req, token.clone()) => {
                    send_response(result);
                }
                _ = token.cancelled() => {
                    // Request cancelled, do nothing
                }
            }
        });
    }
}
```

---

## Response Time Breakdown

### rust-analyzer Typical Response Times

| Operation | Target | Actual (p50) | Actual (p99) |
|-----------|--------|--------------|--------------|
| Hover | 20ms | 5ms | 50ms |
| Completion | 100ms | 30ms | 150ms |
| Go-to-definition | 50ms | 10ms | 80ms |
| Find references | 200ms | 50ms | 500ms |
| Rename | 500ms | 100ms | 2000ms |

### Where Time Goes

**Hover (5ms typical):**
```
Token lookup:     1ms   (AST traversal)
Type resolution:  2ms   (cached from analysis)
Formatting:       2ms   (type to string)
```

**Completion (30ms typical):**
```
Scope analysis:   5ms   (find visible items)
Type checking:   10ms   (filter by type)
Sorting:          5ms   (relevance ranking)
Formatting:      10ms   (build completion items)
```

**Go-to-definition (10ms typical):**
```
Token lookup:     1ms
Name resolution:  5ms   (cached from analysis)
Location lookup:  4ms   (symbol index)
```

---

## clangd Patterns

### AST Persistence

clangd keeps ASTs in memory for open files:

```cpp
class TUScheduler {
    // AST for each open file
    map<PathRef, unique_ptr<ParsedAST>> ASTs;

    // Background indexer
    BackgroundIndex Index;
};
```

### Preamble Caching

clangd caches preprocessor results:

```cpp
// Headers rarely change - cache the result
struct Preamble {
    string Contents;
    vector<Inclusion> Includes;
    PreprocessorState PPState;
};

// Reuse preamble if headers unchanged
Preamble& getPreamble(FileID file) {
    if (preambleValid(file)) {
        return cachedPreambles[file];
    }
    return rebuildPreamble(file);
}
```

### Application to Sigil

```rust
/// Cache parsed imports (like clangd's preamble)
pub struct ImportCache {
    /// Module → Resolved imports
    cache: DashMap<Module, ResolvedImports>,
    /// Modification times for invalidation
    mtimes: DashMap<PathBuf, SystemTime>,
}

impl ImportCache {
    pub fn get_or_resolve(&self, db: &dyn Db, module: Module) -> ResolvedImports {
        // Check cache validity
        let file = module.file(db);
        let mtime = fs::metadata(file.path(db)).ok()?.modified().ok()?;

        if let Some(entry) = self.cache.get(&module) {
            if self.mtimes.get(file.path(db)) == Some(mtime) {
                return entry.clone();
            }
        }

        // Resolve and cache
        let resolved = resolve_imports(db, module);
        self.cache.insert(module, resolved.clone());
        self.mtimes.insert(file.path(db).to_path_buf(), mtime);
        resolved
    }
}
```

---

## Incremental Updates

### rust-analyzer: VFS (Virtual File System)

```rust
/// Track file changes
pub struct Vfs {
    files: HashMap<VfsPath, FileData>,
    changes: Vec<ChangedFile>,
}

struct FileData {
    content: String,
    version: i32,
}

impl Vfs {
    /// Apply text edit
    fn apply_change(&mut self, change: TextDocumentContentChangeEvent) {
        let file = self.files.get_mut(&change.uri)?;

        match change.range {
            Some(range) => {
                // Incremental update
                let start = offset_of(file.content, range.start);
                let end = offset_of(file.content, range.end);
                file.content.replace_range(start..end, &change.text);
            }
            None => {
                // Full replacement
                file.content = change.text;
            }
        }

        file.version += 1;
        self.changes.push(ChangedFile {
            path: change.uri,
            change_kind: ChangeKind::Modify,
        });
    }
}
```

### Incremental Parsing

Some language servers support incremental parsing:

```rust
/// Incremental parser (tree-sitter style)
pub trait IncrementalParse {
    fn parse_initial(source: &str) -> Self;

    fn apply_edit(&mut self, edit: &TextEdit) -> ParseResult;
}

impl IncrementalParse for LazyModule {
    fn apply_edit(&mut self, edit: &TextEdit) {
        // Find affected functions
        let affected = self.functions_in_range(edit.range);

        // Re-parse only affected functions
        for func_id in affected {
            self.invalidate_body(func_id);
        }
    }
}
```

---

## Completion Strategies

### Fuzzy Matching

rust-analyzer uses fuzzy matching for completions:

```rust
/// Fuzzy match score
pub fn fuzzy_match(pattern: &str, candidate: &str) -> Option<i32> {
    let mut score = 0;
    let mut pattern_idx = 0;
    let pattern_chars: Vec<char> = pattern.chars().collect();

    for (i, c) in candidate.chars().enumerate() {
        if pattern_idx < pattern_chars.len() &&
           c.eq_ignore_ascii_case(&pattern_chars[pattern_idx])
        {
            // Bonus for consecutive matches
            score += if pattern_idx > 0 && i > 0 { 2 } else { 1 };
            // Bonus for word boundary matches
            if is_word_boundary(candidate, i) { score += 3; }

            pattern_idx += 1;
        }
    }

    if pattern_idx == pattern_chars.len() {
        Some(score)
    } else {
        None
    }
}
```

### Type-Aware Completion

```rust
/// Filter completions by expected type
fn filter_by_type(
    items: &mut Vec<CompletionItem>,
    expected: TypeId,
    db: &dyn Db,
) {
    items.retain(|item| {
        match item.data {
            CompletionData::Variable { ty } => {
                types_compatible(ty, expected, db)
            }
            CompletionData::Function { ret_ty, .. } => {
                types_compatible(ret_ty, expected, db)
            }
            CompletionData::Pattern { result_ty, .. } => {
                types_compatible(result_ty, expected, db)
            }
            _ => true,  // Keep keywords, etc.
        }
    });
}
```

---

## Diagnostics Pipeline

### rust-analyzer: Flycheck

```rust
/// Background diagnostic computation
pub struct DiagnosticsManager {
    /// Current diagnostics per file
    diagnostics: DashMap<FileId, Vec<Diagnostic>>,

    /// Background computation task
    compute_task: Option<JoinHandle<()>>,
}

impl DiagnosticsManager {
    /// Schedule diagnostic computation
    fn schedule_check(&mut self, file: FileId) {
        // Debounce: wait for typing to stop
        self.debounce_timer.reset();

        // Cancel previous computation
        if let Some(task) = self.compute_task.take() {
            task.abort();
        }

        // Schedule new computation
        self.compute_task = Some(tokio::spawn(async move {
            tokio::time::sleep(DEBOUNCE_DELAY).await;
            let diags = compute_diagnostics(file);
            self.publish(file, diags);
        }));
    }
}
```

### Diagnostic Batching

```rust
/// Batch diagnostics to reduce protocol overhead
fn publish_diagnostics(&self, updates: Vec<(FileId, Vec<Diagnostic>)>) {
    // Group by file
    let mut batched: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

    for (file, diags) in updates {
        let url = file_to_url(file);
        batched.entry(url).or_default().extend(diags);
    }

    // Send one message per file
    for (uri, diagnostics) in batched {
        self.client.publish_diagnostics(PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        });
    }
}
```

---

## Index-Based Operations

### Symbol Index

```rust
/// Global symbol index for workspace-wide operations
pub struct SymbolIndex {
    /// Name → Locations
    symbols: DashMap<String, Vec<SymbolLocation>>,

    /// Trigram index for fuzzy search
    trigrams: TrigramIndex,
}

impl SymbolIndex {
    /// Workspace symbol search
    fn search(&self, query: &str) -> Vec<SymbolInformation> {
        // Use trigrams for initial filtering
        let candidates = self.trigrams.query(query);

        // Score and sort
        let mut results: Vec<_> = candidates
            .iter()
            .filter_map(|name| {
                let score = fuzzy_match(query, name)?;
                Some((name, score))
            })
            .collect();

        results.sort_by_key(|(_, score)| -score);

        // Lookup locations
        results.iter()
            .flat_map(|(name, _)| self.symbols.get(*name))
            .flatten()
            .map(|loc| loc.to_symbol_info())
            .collect()
    }
}
```

### Find All References

```rust
/// Find all references using index
fn find_references(
    db: &dyn Db,
    index: &SymbolIndex,
    name: Name,
    include_declaration: bool,
) -> Vec<Location> {
    let mut locations = Vec::new();

    // Definition(s)
    if include_declaration {
        if let Some(defs) = index.definitions.get(&name) {
            locations.extend(defs.iter().map(|d| d.location.clone()));
        }
    }

    // References
    if let Some(refs) = index.references.get(&name) {
        locations.extend(refs.iter().map(|r| r.location.clone()));
    }

    locations
}
```

---

## Key Patterns for Sigil

1. **Cancellation** - Cancel stale requests to free resources
2. **Debouncing** - Wait for typing to stop before heavy computation
3. **Layered caching** - Import cache (stable) + AST cache (per-file) + type cache (global)
4. **Fuzzy matching** - Better completion UX
5. **Type-aware filtering** - Reduce completion noise
6. **Trigram index** - Fast workspace symbol search
7. **Background indexing** - Build index without blocking requests
