---
title: "Formatting"
description: "Ori LSP Design — Code Formatting Integration"
order: 4
section: "Features"
---

# Formatting

Integrating `ori_fmt` with the LSP server for document formatting.

## Overview

Formatting is a **request** from client to server. The server returns text edits to apply.

```
textDocument/formatting
    Client ─────────────────────────► Server
           ◄───────────────────────── (TextEdit[])
```

## Implementation

### Core Logic

```rust
use ori_fmt;

pub fn format(
    docs: &DocumentManager,
    params: DocumentFormattingParams,
) -> Vec<TextEdit> {
    let uri = &params.text_document.uri;

    let doc = match docs.get(uri) {
        Some(d) => d,
        None => return vec![],
    };

    // Attempt to format
    let formatted = match ori_fmt::format(&doc.text) {
        Ok(formatted) => formatted,
        Err(_) => return vec![],  // Return empty on error
    };

    // No change needed
    if formatted == doc.text {
        return vec![];
    }

    // Return single edit replacing entire document
    vec![TextEdit {
        range: full_document_range(&doc.text),
        new_text: formatted,
    }]
}

fn full_document_range(text: &str) -> Range {
    let lines: Vec<&str> = text.lines().collect();
    let last_line = lines.len().saturating_sub(1);
    let last_col = lines.last().map(|l| l.len()).unwrap_or(0);

    Range {
        start: Position { line: 0, character: 0 },
        end: Position {
            line: last_line as u32,
            character: last_col as u32,
        },
    }
}
```

### Ignoring Client Options

LSP clients send formatting options, but Ori ignores them (zero-config):

```rust
pub fn format(
    docs: &DocumentManager,
    params: DocumentFormattingParams,
) -> Vec<TextEdit> {
    // Ignore params.options.tab_size
    // Ignore params.options.insert_spaces
    // Ori always uses 4 spaces, 100 char width

    // ...
}
```

## Error Handling

When formatting fails (e.g., parse error), we have options:

### Option A: Return Empty (Current)

```rust
let formatted = match ori_fmt::format(&doc.text) {
    Ok(f) => f,
    Err(_) => return vec![],  // No edits
};
```

Pros: Simple, non-destructive
Cons: Silent failure

### Option B: Incremental Formatting (Implemented)

Use `ori_fmt::format_incremental()` to format only declarations overlapping a changed region:

```rust
use ori_fmt::incremental::{format_incremental, IncrementalResult, apply_regions};

let result = format_incremental(
    &module,
    &comments,
    &arena,
    &interner,
    change_start,
    change_end,
);

match result {
    IncrementalResult::Regions(regions) => {
        // Apply formatted regions as edits
        regions.into_iter().map(|r| TextEdit {
            range: span_to_range(&doc.text, r.original_start, r.original_end),
            new_text: r.formatted,
        }).collect()
    }
    IncrementalResult::FullFormatNeeded => {
        // Fall back to full format (e.g., import changes)
        format_full(&doc.text)
    }
    IncrementalResult::NoChangeNeeded => vec![],
}
```

Pros: Fast for large files, natural for format-on-type
Cons: Requires successful parse of affected declarations

### Option C: Publish Diagnostic

```rust
let formatted = match ori_fmt::format(&doc.text) {
    Ok(f) => f,
    Err(e) => {
        // Publish diagnostic explaining why format failed
        let diagnostic = Diagnostic {
            range: span_to_range(&doc.text, e.span),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("ori_fmt".to_string()),
            message: format!("Cannot format: {}", e.message),
            ..Default::default()
        };

        client.publish_diagnostics(uri.clone(), vec![diagnostic], None).await;

        return vec![];
    }
};
```

## Format on Run (Playground)

The Playground formats code automatically when the user clicks Run:

```typescript
async function runCode() {
    // Format first
    const formatted = server.format(getCurrentUri());
    if (formatted) {
        editor.setValue(formatted);
    }

    // Then run
    const result = await runOri(editor.getValue());
    showOutput(result);
}
```

This matches Go Playground behavior: format is implicit, not a separate action.

## Format on Type (Future)

LSP supports `textDocument/onTypeFormatting` for formatting as you type:

```typescript
interface DocumentOnTypeFormattingParams {
    textDocument: TextDocumentIdentifier;
    position: Position;
    ch: string;  // Character that triggered formatting
    options: FormattingOptions;
}
```

Potential triggers for Ori:
- `}` — format block
- `)` — format function call
- `,` — align list items
- `\n` — indent new line

This is Phase 4+ functionality.

## Format Range (Future)

LSP supports formatting a selection:

```typescript
interface DocumentRangeFormattingParams {
    textDocument: TextDocumentIdentifier;
    range: Range;
    options: FormattingOptions;
}
```

Implementation would:
1. Identify complete syntactic units in range
2. Format those units
3. Return edits only within/adjacent to range

## WASM API

```rust
#[wasm_bindgen]
impl WasmLanguageServer {
    /// Format document, returns formatted text or None on error
    pub fn format(&self, uri: &str) -> Option<String> {
        let uri = Url::parse(uri).ok()?;
        let doc = self.inner.documents.get(&uri)?;

        ori_fmt::format(&doc.text).ok()
    }

    /// Format and return as LSP TextEdit JSON (for full LSP compliance)
    pub fn format_edits(&self, uri: &str) -> String {
        let uri = match Url::parse(uri) {
            Ok(u) => u,
            Err(_) => return "[]".to_string(),
        };

        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri },
            options: Default::default(),
            work_done_progress_params: Default::default(),
        };

        let edits = format(&self.inner.documents, params);
        serde_json::to_string(&edits).unwrap()
    }
}
```

## Monaco Integration

### Formatting Provider

```typescript
monaco.languages.registerDocumentFormattingEditProvider('ori', {
    provideDocumentFormattingEdits(model, options, token) {
        const uri = model.uri.toString();
        const formatted = server.format(uri);

        if (!formatted) {
            return [];
        }

        // Single edit replacing entire content
        return [{
            range: model.getFullModelRange(),
            text: formatted,
        }];
    }
});
```

### Keyboard Shortcut

```typescript
editor.addAction({
    id: 'ori.format',
    label: 'Format Document',
    keybindings: [
        monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF,
    ],
    run: () => {
        editor.getAction('editor.action.formatDocument').run();
    },
});
```

### Format on Save (Optional)

```typescript
editor.onDidSaveModel((model) => {
    editor.getAction('editor.action.formatDocument').run();
});
```

## Performance

### Large Files

For large files, consider:

1. **Streaming**: Format and emit incrementally
2. **Caching**: Cache formatted output, invalidate on change
3. **Chunking**: Format in chunks for responsiveness

```rust
pub fn format_large(text: &str) -> Result<String, FormatError> {
    if text.len() > 100_000 {
        // Use streaming formatter
        format_streaming(text)
    } else {
        // Standard formatter
        ori_fmt::format(text)
    }
}
```

### Benchmark Target

| File Size | Target Time |
|-----------|-------------|
| < 1 KB | < 10 ms |
| 1-10 KB | < 50 ms |
| 10-100 KB | < 200 ms |
| > 100 KB | < 1 s |

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple() {
        let server = test_server("let x=1");

        let edits = server.format("file:///test.ori");

        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "let x = 1\n");
    }

    #[test]
    fn test_format_already_formatted() {
        let server = test_server("let x = 1\n");

        let edits = server.format("file:///test.ori");

        // No edits needed
        assert_eq!(edits.len(), 0);
    }

    #[test]
    fn test_format_parse_error() {
        let server = test_server("let x = ");

        let edits = server.format("file:///test.ori");

        // Returns empty on error
        assert_eq!(edits.len(), 0);
    }

    #[test]
    fn test_format_preserves_semantics() {
        let code = "let x = 1 + 2 * 3";
        let formatted = ori_fmt::format(code).unwrap();

        // Parse both and compare ASTs
        let ast1 = ori_parse::parse(code).module.unwrap();
        let ast2 = ori_parse::parse(&formatted).module.unwrap();

        assert_eq!(
            strip_spans(&ast1),
            strip_spans(&ast2)
        );
    }
}
```
