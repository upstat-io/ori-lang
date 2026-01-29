---
title: "Document Synchronization"
description: "Ori LSP Design — Text Document Sync Strategy"
order: 2
---

# Document Synchronization

How the LSP server maintains document state and synchronizes with clients.

## Sync Strategy

**Incremental sync** (`TextDocumentSyncKind.Incremental = 2`):
- Client sends only changed ranges
- More efficient for large files
- Requires server to track document state

Alternative considered:
- **Full sync** (`TextDocumentSyncKind.Full = 1`): Simpler but sends entire document on every keystroke

## Document State

```rust
struct DocumentState {
    /// Full document text
    text: String,
    /// Document version (increments on each change)
    version: i32,
    /// Cached parse result (invalidated on change)
    ast: Option<Module>,
    /// Cached type info (invalidated on change)
    types: Option<TypeContext>,
    /// Pending diagnostic computation
    diagnostics_dirty: bool,
}

struct DocumentManager {
    documents: HashMap<Url, DocumentState>,
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

Recomputing diagnostics on every keystroke is wasteful. Debounce with a short delay:

```rust
impl DocumentManager {
    async fn on_change(&mut self, uri: &Url, changes: Vec<ContentChange>) {
        let doc = self.documents.get_mut(uri).unwrap();

        for change in changes {
            apply_change(doc, &change);
        }

        // Schedule diagnostic update (debounced)
        self.schedule_diagnostics(uri.clone(), Duration::from_millis(100));
    }

    async fn schedule_diagnostics(&mut self, uri: Url, delay: Duration) {
        // Cancel any pending diagnostic computation for this document
        self.cancel_pending_diagnostics(&uri);

        // Schedule new computation
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            self.compute_and_publish_diagnostics(&uri).await;
        });
    }
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
