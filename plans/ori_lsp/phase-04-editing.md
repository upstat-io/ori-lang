# Phase 4: Editing Support

**Goal**: Implement real-time diagnostics and intelligent completions

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Diagnostics, Completions
> **RELATED**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/03-structured-errors.md`

---

## 4.1 Diagnostics

> **Performance Target**: < 50ms after keystroke

### 4.1.1 Diagnostic Publishing

- [ ] **Implement**: `textDocument/publishDiagnostics` notification
  - [ ] Publish diagnostics after document change
  - [ ] Debounce rapid changes (configurable delay)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/diagnostics.rs` — publishing

- [ ] **Implement**: Diagnostic delay configuration
  - [ ] `ori.diagnostics.delay`: milliseconds (default 50)
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — delay config

### 4.1.2 Precise Underlines

> **DESIGN**: Underline exactly what is wrong, not the whole line

- [ ] **Implement**: Span-to-range conversion
  - [ ] Convert AST spans to LSP Range
  - [ ] Handle multi-line spans
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/span.rs` — conversion

- [ ] **Implement**: Precise error spans
  - [ ] Type mismatch: underline mismatched expression
  - [ ] Unknown identifier: underline the identifier
  - [ ] Missing argument: underline the call site
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/span.rs` — precision

### 4.1.3 Diagnostic Levels

- [ ] **Implement**: Severity mapping
  - [ ] Error → DiagnosticSeverity.Error (red underline)
  - [ ] Warning → DiagnosticSeverity.Warning (yellow)
  - [ ] Info → DiagnosticSeverity.Information (blue)
  - [ ] Hint → DiagnosticSeverity.Hint (faded)
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/severity.rs` — mapping

- [ ] **Implement**: Diagnostic codes
  - [ ] Map Ori error codes (E0308, etc.) to LSP codes
  - [ ] Include code in diagnostic
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/codes.rs` — code mapping

### 4.1.4 Related Information

- [ ] **Implement**: DiagnosticRelatedInformation
  - [ ] Link to related locations (e.g., expected type source)
  - [ ] Show context for complex errors
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/related.rs` — related info

### 4.1.5 Test-Related Diagnostics

- [ ] **Implement**: Untested function warning
  - [ ] "warning: function has no tests"
  - [ ] Severity: Warning
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/test_coverage.rs` — warnings

- [ ] **Implement**: Failing test diagnostic
  - [ ] Show test failures as diagnostics on target function
  - [ ] Link to test location
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/test_status.rs` — failures

---

## 4.2 Quick Fixes

> **DESIGN**: Inline quick fixes with type conversion suggestions

### 4.2.1 Type Mismatch Fixes

- [ ] **Implement**: Type conversion suggestions
  - [ ] `int` → `float`: suggest `value as float`
  - [ ] `str` → `int`: suggest `value as? int`
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/fixes/type_conversion.rs`

- [ ] **Implement**: Wrapper suggestions
  - [ ] `T` → `Option<T>`: suggest `Some(value)`
  - [ ] `T` → `Result<T, E>`: suggest `Ok(value)`
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/fixes/wrappers.rs`

### 4.2.2 Missing Import Fixes

- [ ] **Implement**: Unknown type import suggestion
  - [ ] Search stdlib for matching types
  - [ ] Suggest `use std.module { Type }`
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/fixes/imports.rs`

- [ ] **Implement**: Unknown function import suggestion
  - [ ] Search stdlib for matching functions
  - [ ] Suggest appropriate use statement
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/fixes/imports.rs`

### 4.2.3 Typo Fixes

- [ ] **Implement**: Identifier typo detection
  - [ ] Levenshtein distance calculation
  - [ ] "Did you mean `similar_name`?"
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/fixes/typo.rs`

---

## 4.3 Completions

> **Performance Target**: < 100ms
> **DESIGN**: Show 10-15 items maximum, not 200. Quality over quantity.

### 4.3.1 Completion Handler

- [ ] **Implement**: `textDocument/completion` request handler
  - [ ] Return CompletionList with isIncomplete flag
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — basic handler

- [ ] **Implement**: Completion context detection
  - [ ] After `.` → method/field completion
  - [ ] After `@` → function name completion
  - [ ] After `$` → constant name completion
  - [ ] After `:` in type position → type completion
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — context

### 4.3.2 Function Completions

- [ ] **Implement**: Local function completions
  - [ ] Functions in scope
  - [ ] Show signature in detail
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — local funcs

- [ ] **Implement**: Imported function completions
  - [ ] Functions from imported modules
  - [ ] Auto-import suggestion if not imported
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — imports

- [ ] **Implement**: Method completions
  - [ ] Methods available on expression type
  - [ ] Trait methods if trait in scope
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — methods

### 4.3.3 Type Completions

- [ ] **Implement**: Type name completions
  - [ ] In type annotation positions
  - [ ] Include generic parameters
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — types

- [ ] **Implement**: Generic type completions
  - [ ] `Result<` → show `Result<T, E>`
  - [ ] `Option<` → show `Option<T>`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — generics

### 4.3.4 Pattern Completions

> **DESIGN**: Context-aware for pattern properties

- [ ] **Implement**: Pattern property completions
  - [ ] Inside `retry(` → show `.op`, `.attempts`, `.backoff`
  - [ ] Mark required vs optional
  - [ ] Show default values
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — patterns

- [ ] **Implement**: Pattern name completions
  - [ ] `run`, `try`, `match`, `recurse`, etc.
  - [ ] Based on context (expression position)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — pattern names

### 4.3.5 Import Completions

- [ ] **Implement**: Module path completions
  - [ ] `use std.` → show `math`, `io`, `json`, etc.
  - [ ] Show module contents in detail
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — modules

- [ ] **Implement**: Import item completions
  - [ ] `use std.math { ` → show `sqrt`, `abs`, etc.
  - [ ] Include type info
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — import items

### 4.3.6 Completion Ranking

- [ ] **Implement**: Semantic ranking
  - [ ] Exact prefix match first
  - [ ] Type-compatible items higher
  - [ ] Recently used items higher
  - [ ] Limit to 10-15 items
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — ranking

### 4.3.7 Completion Resolve

- [ ] **Implement**: `completionItem/resolve` request
  - [ ] Add documentation on demand
  - [ ] Add full signature details
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — resolve

---

## 4.4 Signature Help

- [ ] **Implement**: `textDocument/signatureHelp` request handler
  - [ ] Show function signature while typing arguments
  - [ ] Highlight current parameter
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/signature_help.rs` — basic

- [ ] **Implement**: Parameter highlighting
  - [ ] Track which parameter position cursor is at
  - [ ] Handle named arguments
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/signature_help.rs` — params

- [ ] **Implement**: Overload display
  - [ ] Show multiple signatures for function clauses
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/signature_help.rs` — overloads

---

## 4.5 Phase Completion Checklist

- [ ] All items in 4.1-4.4 have all checkboxes marked `[x]`
- [ ] Diagnostics publish within 50ms
- [ ] Precise underlines (not whole line)
- [ ] Quick fixes available for type mismatches, imports, typos
- [ ] Completions limited to 10-15 high-quality items
- [ ] Pattern completions show required/optional properties
- [ ] Signature help shows current parameter
- [ ] Performance: diagnostics < 50ms, completions < 100ms
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Fast, precise diagnostics with quality completions
