# Phase 6: Semantic Features

**Goal**: Implement semantic highlighting, document outline, and structural navigation

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Semantic Highlighting, Document Outline

---

## 6.1 Semantic Highlighting

### 6.1.1 Token Types

- [ ] **Implement**: Semantic token type registration
  - [ ] Register Ori-specific token types
  - [ ] Standard types: function, variable, type, keyword, etc.
  - [ ] Custom types: config ($), pattern property, captured variable
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — types

- [ ] **Implement**: Token modifier registration
  - [ ] declaration, definition, readonly
  - [ ] static (for $constants)
  - [ ] async (for async functions)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — modifiers

### 6.1.2 Ori-Specific Highlighting

- [ ] **Implement**: Function name highlighting
  - [ ] `@function_name` → function type + bold modifier
  - [ ] Distinguish definition from call
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — functions

- [ ] **Implement**: Constant highlighting
  - [ ] `$config` → constant type + italic modifier
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — constants

- [ ] **Implement**: Type name highlighting
  - [ ] Type declarations and usages
  - [ ] Generic parameters distinct
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — types

- [ ] **Implement**: Variant highlighting
  - [ ] Sum type variants → enum member type
  - [ ] `Some`, `None`, `Ok`, `Err`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — variants

- [ ] **Implement**: Pattern property highlighting
  - [ ] `.property:` in patterns → parameter type
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — properties

### 6.1.3 Context-Sensitive Keywords

> **DESIGN**: Pattern keywords highlighted as keywords only in pattern contexts

- [ ] **Implement**: Pattern keyword detection
  - [ ] `map`, `filter`, `fold` in pattern context → keyword
  - [ ] Same identifiers elsewhere → identifier/function
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — context

- [ ] **Implement**: Built-in function highlighting
  - [ ] `run`, `try`, `match`, `recurse` → keyword in call position
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — builtins

### 6.1.4 Capture Highlighting

> **DESIGN**: Variables captured by closures are visually distinct

- [ ] **Implement**: Captured variable detection
  - [ ] Track which variables are captured by lambdas
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/captures.rs` — detection

- [ ] **Implement**: Captured variable styling
  - [ ] Different shade + underline modifier
  - [ ] Both at capture site and definition site
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — captures

### 6.1.5 Semantic Token Requests

- [ ] **Implement**: `textDocument/semanticTokens/full` request
  - [ ] Return full document tokens
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — full

- [ ] **Implement**: `textDocument/semanticTokens/range` request
  - [ ] Return tokens for visible range only
  - [ ] Optimization for large files
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — range

- [ ] **Implement**: `textDocument/semanticTokens/delta` request
  - [ ] Return incremental token updates
  - [ ] Optimization for editing
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/semantic_tokens.rs` — delta

---

## 6.2 Document Outline

> **Performance Target**: < 100ms

### 6.2.1 Hierarchical Structure

- [ ] **Implement**: Top-level sections
  - [ ] Imports section
  - [ ] Config section ($constants)
  - [ ] Types section
  - [ ] Functions section
  - [ ] Tests section
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — sections

- [ ] **Implement**: Nested structure
  - [ ] Struct fields under struct
  - [ ] Enum variants under enum
  - [ ] Impl methods under impl block
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — nesting

### 6.2.2 Coverage Indicators

> **DESIGN**: Show test coverage status in outline

- [ ] **Implement**: Test status icons
  - [ ] Checkmark: function has passing tests
  - [ ] Warning: function has no tests
  - [ ] X: function has failing tests
  - [ ] Number: count of tests
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — status

- [ ] **Implement**: Status in symbol detail
  - [ ] Include "(3 tests)" in function detail
  - [ ] Include "[!]" for untested functions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — details

---

## 6.3 Folding Ranges

- [ ] **Implement**: `textDocument/foldingRange` request
  - [ ] Return foldable regions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — basic

- [ ] **Implement**: Function body folding
  - [ ] Fold function bodies
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — functions

- [ ] **Implement**: Block expression folding
  - [ ] `run(...)`, `try(...)`, `match(...)`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — blocks

- [ ] **Implement**: Import section folding
  - [ ] Fold multiple import statements
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — imports

- [ ] **Implement**: Comment folding
  - [ ] Fold multi-line doc comments
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — comments

- [ ] **Implement**: Type definition folding
  - [ ] Struct fields, enum variants
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/folding.rs` — types

---

## 6.4 Selection Ranges

- [ ] **Implement**: `textDocument/selectionRange` request
  - [ ] Return hierarchical selection ranges
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/selection.rs` — basic

- [ ] **Implement**: Expression-based selection
  - [ ] Inner → outer expression hierarchy
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/selection.rs` — expressions

- [ ] **Implement**: Statement-based selection
  - [ ] Select let binding, then containing block
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/selection.rs` — statements

- [ ] **Implement**: Declaration-based selection
  - [ ] Select function signature, then full function
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/selection.rs` — declarations

---

## 6.5 Phase Completion Checklist

- [ ] All items in 6.1-6.4 have all checkboxes marked `[x]`
- [ ] Semantic highlighting distinguishes Ori constructs
- [ ] Context-sensitive keywords correctly colored
- [ ] Captured variables visually distinct
- [ ] Document outline shows hierarchical structure
- [ ] Coverage indicators in outline
- [ ] Folding and selection ranges work
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Rich semantic highlighting and structural navigation
