---
title: "Diagnostics"
description: "Ori LSP Design — Error and Warning Reporting"
order: 2
---

# Diagnostics

Publishing parse errors, type errors, and warnings to the client.

## Overview

Diagnostics are **notifications** sent from server to client. They appear as squiggly underlines in editors.

```
textDocument/publishDiagnostics
    Server ────────────────────────► Client
```

## Diagnostic Sources

| Source | Severity | Examples |
|--------|----------|----------|
| Lexer | Error | Invalid token, unterminated string |
| Parser | Error | Missing `)`, unexpected token |
| Type checker | Error | Type mismatch, undefined variable |
| Type checker | Warning | Unused variable, unreachable code |
| Linter (future) | Warning/Hint | Style suggestions |

## Implementation

### Collecting Diagnostics

```rust
pub fn compute_diagnostics(doc: &DocumentState) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Lex errors
    let tokens = ori_lexer::lex(&doc.text);
    for error in tokens.errors {
        diagnostics.push(lex_error_to_diagnostic(&doc.text, error));
    }

    // Parse errors
    let parse_result = ori_parse::parse(&doc.text);
    for error in parse_result.errors {
        diagnostics.push(parse_error_to_diagnostic(&doc.text, error));
    }

    // Type errors (only if parsing succeeded enough)
    if let Some(ref module) = parse_result.module {
        let type_result = ori_typeck::check(module);
        for error in type_result.errors {
            diagnostics.push(type_error_to_diagnostic(&doc.text, error));
        }
        for warning in type_result.warnings {
            diagnostics.push(warning_to_diagnostic(&doc.text, warning));
        }
    }

    diagnostics
}
```

### Error Conversion

```rust
fn lex_error_to_diagnostic(text: &str, error: LexError) -> Diagnostic {
    Diagnostic {
        range: span_to_range(text, error.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.to_string())),
        source: Some("ori".to_string()),
        message: error.message,
        related_information: None,
        tags: None,
        code_description: None,
        data: None,
    }
}

fn parse_error_to_diagnostic(text: &str, error: ParseError) -> Diagnostic {
    let mut diagnostic = Diagnostic {
        range: span_to_range(text, error.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.to_string())),
        source: Some("ori".to_string()),
        message: error.message.clone(),
        related_information: None,
        tags: None,
        code_description: None,
        data: None,
    };

    // Add related information for context
    if let Some(note) = &error.note {
        diagnostic.related_information = Some(vec![
            DiagnosticRelatedInformation {
                location: Location {
                    uri: error.uri.clone(),
                    range: span_to_range(text, note.span),
                },
                message: note.message.clone(),
            }
        ]);
    }

    diagnostic
}

fn type_error_to_diagnostic(text: &str, error: TypeError) -> Diagnostic {
    Diagnostic {
        range: span_to_range(text, error.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.to_string())),
        source: Some("ori".to_string()),
        message: format_type_error(&error),
        related_information: error.related.map(|r| vec![
            DiagnosticRelatedInformation {
                location: Location {
                    uri: r.uri,
                    range: span_to_range(text, r.span),
                },
                message: r.message,
            }
        ]),
        tags: None,
        code_description: None,
        data: None,
    }
}

fn warning_to_diagnostic(text: &str, warning: Warning) -> Diagnostic {
    Diagnostic {
        range: span_to_range(text, warning.span),
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(warning.code.to_string())),
        source: Some("ori".to_string()),
        message: warning.message,
        tags: warning_tags(&warning),
        ..Default::default()
    }
}
```

### Warning Tags

LSP supports special tags for certain warning types:

```rust
fn warning_tags(warning: &Warning) -> Option<Vec<DiagnosticTag>> {
    match warning.kind {
        WarningKind::UnusedVariable |
        WarningKind::UnusedImport |
        WarningKind::UnusedFunction => {
            Some(vec![DiagnosticTag::UNNECESSARY])
        }
        WarningKind::Deprecated => {
            Some(vec![DiagnosticTag::DEPRECATED])
        }
        _ => None,
    }
}
```

These tags enable special rendering:
- `UNNECESSARY` → faded/dimmed text
- `DEPRECATED` → strikethrough

## Publishing

### On Document Change

```rust
impl OriLanguageServer {
    async fn on_document_change(&mut self, uri: Url, text: String) {
        // Update document state
        let doc = self.documents.update(&uri, text);

        // Compute diagnostics
        let diagnostics = compute_diagnostics(doc);

        // Publish to client
        self.client
            .publish_diagnostics(uri, diagnostics, Some(doc.version))
            .await;
    }
}
```

### On Document Close

Clear diagnostics when a document is closed:

```rust
impl OriLanguageServer {
    async fn on_document_close(&mut self, uri: Url) {
        self.documents.remove(&uri);

        // Clear diagnostics by publishing empty array
        self.client
            .publish_diagnostics(uri, vec![], None)
            .await;
    }
}
```

### Debouncing

Don't recompute on every keystroke:

```rust
impl OriLanguageServer {
    async fn schedule_diagnostics(&mut self, uri: Url) {
        // Cancel previous scheduled computation
        if let Some(handle) = self.pending_diagnostics.remove(&uri) {
            handle.abort();
        }

        // Schedule new computation with delay
        let client = self.client.clone();
        let docs = self.documents.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;

            if let Some(doc) = docs.get(&uri) {
                let diagnostics = compute_diagnostics(doc);
                client.publish_diagnostics(uri, diagnostics, Some(doc.version)).await;
            }
        });

        self.pending_diagnostics.insert(uri.clone(), handle);
    }
}
```

## Error Message Formatting

### Type Mismatch

```rust
fn format_type_mismatch(expected: &Type, actual: &Type, context: &str) -> String {
    format!(
        "type mismatch: expected `{}`, found `{}`{}",
        expected,
        actual,
        if context.is_empty() {
            String::new()
        } else {
            format!("\n  {}", context)
        }
    )
}

// Example output:
// type mismatch: expected `int`, found `str`
//   in argument `count` of function `repeat`
```

### Undefined Variable

```rust
fn format_undefined(name: &str, suggestions: &[String]) -> String {
    let mut msg = format!("cannot find `{}` in this scope", name);

    if let Some(suggestion) = suggestions.first() {
        msg.push_str(&format!("\n  help: did you mean `{}`?", suggestion));
    }

    msg
}

// Example output:
// cannot find `coutner` in this scope
//   help: did you mean `counter`?
```

## Multi-File Diagnostics

When an import changes, dependent files need re-checking:

```rust
impl OriLanguageServer {
    async fn on_file_change(&mut self, uri: Url) {
        // Update the changed file
        self.update_diagnostics(&uri).await;

        // Find dependent files
        let dependents = self.documents.files_importing(&uri);

        // Update their diagnostics too
        for dep_uri in dependents {
            self.update_diagnostics(&dep_uri).await;
        }
    }
}
```

## WASM Considerations

In WASM, diagnostics are computed synchronously and returned:

```rust
#[wasm_bindgen]
impl WasmLanguageServer {
    pub fn get_diagnostics(&self, uri: &str) -> String {
        let uri = Url::parse(uri).unwrap();
        let doc = self.inner.documents.get(&uri);

        let diagnostics = match doc {
            Some(d) => compute_diagnostics(d),
            None => vec![],
        };

        serde_json::to_string(&diagnostics).unwrap()
    }
}
```

The JavaScript side polls or uses callbacks:

```typescript
function updateDiagnostics(uri: string) {
    const diagnosticsJson = server.get_diagnostics(uri);
    const diagnostics = JSON.parse(diagnosticsJson);

    const markers = diagnostics.map(toMonacoMarker);
    monaco.editor.setModelMarkers(model, 'ori', markers);
}
```
