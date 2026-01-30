---
title: "Document Synchronization"
description: "Ori LSP Design — Text Document Sync Strategy"
order: 2
section: "Protocol"
---

# Document Synchronization

How the LSP server maintains document state and synchronizes with clients.

## Reference: Gleam's FileSystemProxy

Gleam uses an elegant pattern: a **FileSystemProxy** that layers in-memory edits over the real filesystem. The compiler reads through this proxy transparently, never knowing whether content is from disk or unsaved editor buffers.

```
┌─────────────────────────────────────────┐
│            FileSystemProxy              │
│  ┌─────────────┐   ┌─────────────────┐  │
│  │ In-Memory   │   │   Real          │  │
│  │ (unsaved)   │──►│   Filesystem    │  │
│  │             │   │   (fallback)    │  │
│  └─────────────┘   └─────────────────┘  │
└─────────────────────────────────────────┘
         ▲
         │ transparent to compiler
         ▼
┌─────────────────────────────────────────┐
│     Compiler (parse, typecheck)         │
└─────────────────────────────────────────┘
```

## Sync Strategy

**Full sync** (`TextDocumentSyncKind.Full = 1`):
- Simpler implementation
- Client sends entire document on each change
- Gleam uses this approach

**Why full sync**: For single-file operations (Playground), full sync is simpler and the overhead is negligible. For multi-file workspaces, we can optimize later if needed.

## FileSystemProxy Pattern (from Gleam)

```rust
/// In-memory cache layered over filesystem
/// Transparent to compiler - it just calls read()
#[derive(Clone)]
pub struct FileSystemProxy {
    /// Unsaved edits from editor (didOpen/didChange)
    memory: Arc<RwLock<HashMap<Url, String>>>,
}

impl FileSystemProxy {
    pub fn new() -> Self {
        Self {
            memory: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Read file content - checks memory first, then disk
    pub fn read(&self, uri: &Url) -> Option<String> {
        // 1. Check in-memory cache (unsaved edits)
        if let Some(content) = self.memory.read().unwrap().get(uri) {
            return Some(content.clone());
        }

        // 2. Fall back to disk (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = uri.to_file_path().ok()?;
            std::fs::read_to_string(path).ok()
        }

        #[cfg(target_arch = "wasm32")]
        None
    }

    /// Store unsaved content (didOpen or didChange)
    pub fn write_memory(&self, uri: Url, content: String) {
        self.memory.write().unwrap().insert(uri, content);
    }

    /// Remove from memory cache (didSave or didClose)
    pub fn clear_memory(&self, uri: &Url) {
        self.memory.write().unwrap().remove(uri);
    }
}
```

## Notification Handlers

```rust
fn handle_did_open(state: &mut GlobalState, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let content = params.text_document.text;

    // Store in memory cache
    state.files.write_memory(uri.clone(), content);

    // Compute and publish diagnostics
    publish_diagnostics(state, &uri);
}

fn handle_did_change(state: &mut GlobalState, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;

    // Full sync: take the last (complete) content
    if let Some(change) = params.content_changes.last() {
        state.files.write_memory(uri.clone(), change.text.clone());
    }

    // Schedule debounced diagnostics
    schedule_diagnostics(state, uri);
}

fn handle_did_save(state: &mut GlobalState, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;

    // Clear memory cache - use disk version now
    state.files.clear_memory(&uri);

    // Optionally re-publish diagnostics
}

fn handle_did_close(state: &mut GlobalState, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;

    // Clear memory cache
    state.files.clear_memory(&uri);

    // Clear diagnostics for this file
    clear_diagnostics(state, &uri);
}
```

## Change Application

Incremental changes specify a range and replacement text:

```rust
fn apply_change(doc: &mut DocumentState, change: &TextDocumentContentChangeEvent) {
    match &change.range {
        Some(range) => {
            // Incremental: replace range
            let start = position_to_offset(&doc.text, range.start);
            let end = position_to_offset(&doc.text, range.end);
            doc.text.replace_range(start..end, &change.text);
        }
        None => {
            // Full sync fallback
            doc.text = change.text.clone();
        }
    }

    // Invalidate caches
    doc.ast = None;
    doc.types = None;
    doc.diagnostics_dirty = true;
}
```

## Position/Offset Conversion

LSP uses line/character positions. Ori spans use byte offsets.

```rust
fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut offset = 0;
    for (line_num, line) in text.lines().enumerate() {
        if line_num == pos.line as usize {
            // UTF-16 code units for character offset (LSP spec)
            return offset + utf16_to_byte_offset(line, pos.character as usize);
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut line_start = 0;

    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = i + 1;
        }
    }

    let character = byte_to_utf16_offset(&text[line_start..], offset - line_start);
    Position { line, character: character as u32 }
}
```

**Important**: LSP positions use UTF-16 code units for the character offset. Handle surrogate pairs correctly.

## Diagnostic Debouncing

Recomputing diagnostics on every keystroke is wasteful. Debounce with a short delay.

### Native (with threads)

```rust
fn schedule_diagnostics(state: &mut GlobalState, uri: Url) {
    // Cancel any pending computation for this file
    if let Some(handle) = state.pending_diagnostics.remove(&uri) {
        // Signal cancellation (drop handle or set flag)
    }

    let files = state.files.clone();
    let sender = state.task_sender.clone();
    let delay = Duration::from_millis(100);

    // Spawn delayed computation
    let handle = std::thread::spawn(move || {
        std::thread::sleep(delay);

        let diagnostics = compute_diagnostics(&files, &uri);
        sender.send(Task::PublishDiagnostics(uri, diagnostics)).ok();
    });

    state.pending_diagnostics.insert(uri, handle);
}
```

### WASM (JavaScript timers)

In WASM, use JavaScript's `setTimeout` for debouncing:

```typescript
let debounceTimer: number | null = null;

function onContentChange(uri: string, content: string) {
    server.update_document(uri, content);

    // Debounce diagnostics
    if (debounceTimer) {
        clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(() => {
        const diagnostics = server.get_diagnostics(uri);
        updateMonacoMarkers(uri, JSON.parse(diagnostics));
    }, 100);
}
```

## Cache Management

### Parse Cache

Cache the AST after parsing. Invalidate on any change.

```rust
fn get_ast(&mut self, uri: &Url) -> Option<&Module> {
    let doc = self.documents.get_mut(uri)?;

    if doc.ast.is_none() {
        let result = ori_parse::parse(&doc.text);
        doc.ast = result.module; // May have errors
    }

    doc.ast.as_ref()
}
```

### Type Cache

Cache type information. Invalidate on change or when imports change.

```rust
fn get_types(&mut self, uri: &Url) -> Option<&TypeContext> {
    let doc = self.documents.get_mut(uri)?;

    if doc.types.is_none() {
        if let Some(ast) = &doc.ast {
            let result = ori_typeck::check(ast);
            doc.types = Some(result.context);
        }
    }

    doc.types.as_ref()
}
```

### Multi-File Considerations

When a document changes, imported modules may need revalidation:

```rust
fn invalidate_dependents(&mut self, changed_uri: &Url) {
    // Find all documents that import the changed file
    for (uri, doc) in &mut self.documents {
        if doc.imports(changed_uri) {
            doc.types = None;
            doc.diagnostics_dirty = true;
        }
    }
}
```

## WASM Considerations

In the browser (Playground), there's no filesystem:

```rust
#[cfg(target_arch = "wasm32")]
impl DocumentManager {
    fn resolve_import(&self, from: &Url, path: &str) -> Option<&DocumentState> {
        // Only resolve to open documents
        // No filesystem access in browser
        let resolved = resolve_relative(from, path);
        self.documents.get(&resolved)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl DocumentManager {
    fn resolve_import(&self, from: &Url, path: &str) -> Option<DocumentState> {
        // Try open documents first
        let resolved = resolve_relative(from, path);
        if let Some(doc) = self.documents.get(&resolved) {
            return Some(doc);
        }

        // Fall back to filesystem
        if let Ok(text) = std::fs::read_to_string(resolved.to_file_path()?) {
            return Some(DocumentState::new(text));
        }

        None
    }
}
```
