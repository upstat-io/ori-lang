---
title: "Diagnostics"
description: "Ori LSP Design — Error and Warning Reporting"
order: 2
section: "Features"
---

# Diagnostics

> **Current Implementation**: Only lexer and parser errors are reported. Type checking integration, SuggestedFix, and code actions described below are not yet implemented — they represent the planned design.

Publishing parse errors, type errors, and warnings to the client.

## Reference: Go's Structured Diagnostics

Go pioneered **structured diagnostics with machine-applicable fixes**. Instead of just text messages, Go's analyzers produce:

```go
type Diagnostic struct {
    Pos            token.Pos
    End            token.Pos          // Range, not just point
    Message        string
    SuggestedFixes []SuggestedFix     // Machine-applicable fixes!
    Related        []RelatedInformation
}

type SuggestedFix struct {
    Message   string       // "Remove unused variable"
    TextEdits []TextEdit   // Non-overlapping edits
}
```

**Key insight**: By including `SuggestedFix` from day one, editors can offer quick fixes without the server implementing `textDocument/codeAction` separately.

## Overview

Diagnostics are **notifications** sent from server to client. They appear as squiggly underlines in editors.

```
textDocument/publishDiagnostics
    Server ────────────────────────► Client
```

## Diagnostic Sources

| Source | Severity | Examples | Has Fix? | Status |
|--------|----------|----------|----------|--------|
| Lexer | Error | Invalid token, unterminated string | No | ✅ Implemented |
| Parser | Error | Missing `)`, unexpected token | Sometimes | ✅ Implemented |
| Type checker | Error | Type mismatch, undefined variable | Sometimes | ❌ Not yet connected |
| Type checker | Warning | Unused variable, unreachable code | Often | ❌ Not yet connected |
| Linter (future) | Warning/Hint | Style suggestions | Usually | ❌ Not yet connected |

## SuggestedFix Support (from Go)

### Ori Diagnostic Type

Design Ori's internal diagnostic type with fixes from the start:

```rust
/// Internal diagnostic representation (before LSP conversion)
pub struct OriDiagnostic {
    pub span: Span,
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub suggestions: Vec<SuggestedFix>,  // Machine-applicable fixes
    pub related: Vec<RelatedInfo>,
}

pub struct SuggestedFix {
    pub message: String,       // "Remove unused variable `x`"
    pub edits: Vec<TextEdit>,  // The actual fix
}

pub struct TextEdit {
    pub span: Span,
    pub new_text: String,
}
```

### Example: Unused Variable

```rust
// Compiler detects unused variable
let diagnostic = OriDiagnostic {
    span: var_span,
    severity: Severity::Warning,
    code: DiagnosticCode::UnusedVariable,
    message: format!("unused variable `{}`", name),
    suggestions: vec![
        SuggestedFix {
            message: format!("Remove unused variable `{}`", name),
            edits: vec![TextEdit {
                span: declaration_span,  // Include `let` keyword
                new_text: String::new(), // Delete
            }],
        },
        SuggestedFix {
            message: format!("Prefix with underscore: `_{}`", name),
            edits: vec![TextEdit {
                span: name_span,
                new_text: format!("_{}", name),
            }],
        },
    ],
    related: vec![],
};
```

### LSP Conversion

LSP's `Diagnostic` doesn't directly include fixes. Instead, store fix data for `codeAction` requests:

```rust
fn to_lsp_diagnostic(diag: &OriDiagnostic, text: &str) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: span_to_range(text, diag.span),
        severity: Some(to_lsp_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.code.to_string())),
        source: Some("ori".to_string()),
        message: diag.message.clone(),
        related_information: to_lsp_related(&diag.related, text),
        tags: diagnostic_tags(&diag.code),
        // Store fix data for later retrieval via codeAction
        data: if diag.suggestions.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&diag.suggestions).unwrap())
        },
        ..Default::default()
    }
}
```

### Code Action Integration

When client requests code actions, retrieve fixes from diagnostic data:

```rust
fn handle_code_action(
    params: CodeActionParams,
    diagnostics_with_fixes: &HashMap<Url, Vec<OriDiagnostic>>,
) -> Vec<CodeAction> {
    let uri = &params.text_document.uri;
    let range = params.range;

    let mut actions = vec![];

    // Find diagnostics overlapping with requested range
    if let Some(diags) = diagnostics_with_fixes.get(uri) {
        for diag in diags {
            if ranges_overlap(diag.span, range) {
                for fix in &diag.suggestions {
                    actions.push(CodeAction {
                        title: fix.message.clone(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![to_lsp_diagnostic(diag)]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(fix_to_changes(uri, &fix.edits)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
            }
        }
    }

    actions
}
```

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

### Incremental Updates (Gleam Pattern)

**Key insight from Gleam**: Don't clear all diagnostics on every change. Track which files have diagnostics and only update those that were recompiled.

```rust
/// Tracks which files have diagnostics (Gleam's FeedbackBookKeeper)
pub struct DiagnosticTracker {
    files_with_errors: HashSet<Url>,
    files_with_warnings: HashSet<Url>,
}

impl DiagnosticTracker {
    /// Publish diagnostics, only clearing files that were recompiled
    pub fn publish_update(
        &mut self,
        connection: &Connection,
        compiled_files: &[Url],
        new_diagnostics: HashMap<Url, Vec<Diagnostic>>,
    ) {
        // 1. Clear diagnostics only for files that were recompiled but have no new errors
        for uri in compiled_files {
            if !new_diagnostics.contains_key(uri) {
                // File was compiled successfully, clear any old diagnostics
                if self.files_with_errors.remove(uri) || self.files_with_warnings.remove(uri) {
                    self.publish(connection, uri.clone(), vec![]);
                }
            }
        }

        // 2. Publish new diagnostics
        for (uri, diagnostics) in new_diagnostics {
            let has_errors = diagnostics.iter().any(|d| d.severity == Some(DiagnosticSeverity::ERROR));
            let has_warnings = diagnostics.iter().any(|d| d.severity == Some(DiagnosticSeverity::WARNING));

            if has_errors {
                self.files_with_errors.insert(uri.clone());
            }
            if has_warnings {
                self.files_with_warnings.insert(uri.clone());
            }

            self.publish(connection, uri, diagnostics);
        }
    }

    fn publish(&self, connection: &Connection, uri: Url, diagnostics: Vec<Diagnostic>) {
        let params = PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };
        let notification = lsp_server::Notification::new(
            "textDocument/publishDiagnostics".to_string(),
            params,
        );
        connection.sender.send(Message::Notification(notification)).ok();
    }
}
```

### On Document Change

```rust
fn handle_document_change(state: &mut GlobalState, uri: Url) {
    // Compute diagnostics
    let content = state.files.read(&uri).unwrap();
    let diagnostics = compute_diagnostics(&content);

    // Track and publish
    let mut updates = HashMap::new();
    if !diagnostics.is_empty() {
        updates.insert(uri.clone(), diagnostics);
    }

    state.diagnostic_tracker.publish_update(
        &state.connection,
        &[uri],  // Files that were "compiled"
        updates,
    );
}
```

### On Document Close

Clear diagnostics when a document is closed:

```rust
fn handle_document_close(state: &mut GlobalState, uri: Url) {
    state.files.clear_memory(&uri);

    // Clear diagnostics by publishing empty array
    state.diagnostic_tracker.publish_update(
        &state.connection,
        &[uri],
        HashMap::new(),
    );
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
