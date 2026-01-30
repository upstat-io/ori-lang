# Phase 2: Navigation

**Goal**: Implement code navigation features with multi-target support

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Go-to-Definition, Find References
> **RELATED**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/01-semantic-addressing.md`

---

## 2.1 Go-to-Definition

> **Performance Target**: < 50ms

### 2.1.1 Single Definition

- [ ] **Implement**: Direct jump for single definition
  - [ ] No popup when only one target
  - [ ] Jump directly to definition location
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — single target

- [ ] **Implement**: Variable definition lookup
  - [ ] Local variables → let binding site
  - [ ] Parameters → function signature
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — variables

- [ ] **Implement**: Function definition lookup
  - [ ] Functions → `@name` declaration site
  - [ ] Methods → impl block method
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — functions

- [ ] **Implement**: Type definition lookup
  - [ ] Types → `type Name = ...` declaration
  - [ ] Variants → sum type definition
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — types

### 2.1.2 Multiple Relevant Locations

- [ ] **Implement**: Multi-target picker
  - [ ] Show picker when multiple targets relevant
  - [ ] Return LocationLink array for client to display
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — multi-target

- [ ] **Implement**: Navigation targets categorization
  - [ ] Definition — where element is defined
  - [ ] Tests — test functions for this element
  - [ ] Usages — all references in codebase
  - [ ] Type definition — for typed elements
  - [ ] Implementation — for pattern usage
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — categories

### 2.1.3 Type Definition Navigation

- [ ] **Implement**: `textDocument/typeDefinition` request
  - [ ] Navigate to type definition from variable
  - [ ] Navigate to trait definition from impl
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/type_definition.rs` — type nav

---

## 2.2 Find References

> **Performance Target**: < 200ms (can show progress for large codebases)

### 2.2.1 Basic References

- [ ] **Implement**: `textDocument/references` request
  - [ ] Find all references to symbol at cursor
  - [ ] Include/exclude declaration based on client request
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — basic refs

- [ ] **Implement**: Reference location resolution
  - [ ] File path, line, column for each reference
  - [ ] Preview text for context
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — locations

### 2.2.2 Categorized Results

> **DESIGN**: Show references categorized by usage type

- [ ] **Implement**: Call site detection
  - [ ] Function calls: `fetch_data(url)`
  - [ ] Method calls: `item.process()`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — calls

- [ ] **Implement**: Test reference detection
  - [ ] `@test_name tests @target` declarations
  - [ ] Mark as "Test" category
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — tests

- [ ] **Implement**: Re-export detection
  - [ ] `pub use module { name }` statements
  - [ ] Mark as "Re-export" category
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — re-exports

- [ ] **Implement**: Type usage detection
  - [ ] Parameter type annotations: `(x: Type)`
  - [ ] Return type annotations: `-> Type`
  - [ ] Let binding types: `let x: Type`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — type usage

### 2.2.3 Filter Options

- [ ] **Implement**: Reference filtering (via command arguments)
  - [ ] All references
  - [ ] Calls only
  - [ ] Tests only
  - [ ] Definitions only
  - [ ] Current file only
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — filters

---

## 2.3 Document Symbols

> **Performance Target**: < 100ms

- [ ] **Implement**: `textDocument/documentSymbol` request
  - [ ] Return hierarchical symbol tree
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — basic

- [ ] **Implement**: Symbol kinds mapping
  - [ ] Functions (`@name`) → SymbolKind.Function
  - [ ] Types → SymbolKind.Struct / SymbolKind.Enum
  - [ ] Constants (`$name`) → SymbolKind.Constant
  - [ ] Traits → SymbolKind.Interface
  - [ ] Impl blocks → SymbolKind.Class
  - [ ] Tests → SymbolKind.Method (with "Test" detail)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — kinds

- [ ] **Implement**: Hierarchical structure
  - [ ] Top-level: imports, types, functions, tests
  - [ ] Nested: impl methods under impl block
  - [ ] Nested: struct fields under struct
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — hierarchy

- [ ] **Implement**: Symbol detail text
  - [ ] Functions: parameter types and return type
  - [ ] Types: variant/field summary
  - [ ] Constants: value if short
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/document_symbol.rs` — details

---

## 2.4 Workspace Symbols

- [ ] **Implement**: `workspace/symbol` request
  - [ ] Search symbols across all files
  - [ ] Fuzzy matching on symbol name
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/workspace_symbol.rs` — search

- [ ] **Implement**: Symbol indexing
  - [ ] Build index on workspace open
  - [ ] Update incrementally on file change
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/index.rs` — symbol index

- [ ] **Implement**: Result ranking
  - [ ] Exact match first
  - [ ] Prefix match second
  - [ ] Fuzzy match third
  - [ ] Limit results (e.g., 50 max)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/workspace_symbol.rs` — ranking

---

## 2.5 Implementation Navigation

- [ ] **Implement**: `textDocument/implementation` request
  - [ ] From trait method → all implementations
  - [ ] From type → all trait impls for that type
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/implementation.rs` — impl nav

---

## 2.6 Phase Completion Checklist

- [ ] All items in 2.1-2.5 have all checkboxes marked `[x]`
- [ ] Go-to-definition works for all identifier types
- [ ] Multi-target picker shows categorized options
- [ ] Find references returns categorized results
- [ ] Document symbols provide hierarchical outline
- [ ] Workspace symbol search works across files
- [ ] Performance: go-to-definition < 50ms, references < 200ms
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: All navigation features work with categorized, multi-target results
