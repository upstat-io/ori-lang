# Phase 3: Information Display

**Goal**: Implement hover information and inlay hints for code understanding

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Hover Information, Inlay Hints

---

## 3.1 Basic Hover

> **Performance Target**: < 20ms

### 3.1.1 Type and Signature Display

- [ ] **Implement**: `textDocument/hover` request handler
  - [ ] Return MarkupContent with markdown
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — basic handler

- [ ] **Implement**: Function hover
  - [ ] Show signature: `@name (params) -> ReturnType`
  - [ ] Show capability requirements: `uses Http, Async`
  - [ ] Show where clauses if present
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — functions

- [ ] **Implement**: Variable hover
  - [ ] Show inferred type: `name: Type`
  - [ ] Show mutability: `let` vs `let $`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — variables

- [ ] **Implement**: Type hover
  - [ ] Show type definition summary
  - [ ] Show field list for structs
  - [ ] Show variant list for sum types
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — types

- [ ] **Implement**: Constant hover
  - [ ] Show value: `$timeout: int = 30`
  - [ ] Show doc comment if present
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — constants

### 3.1.2 Definition Location

- [ ] **Implement**: Location line in hover
  - [ ] "Defined in: src/api.ori:42"
  - [ ] Clickable link (via markdown)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — location

---

## 3.2 Expanded Hover

> **DESIGN**: Click or hold to expand with additional context

### 3.2.1 Function Body Display

- [ ] **Implement**: Function body in hover
  - [ ] Show body expression for short functions
  - [ ] Truncate long bodies with "..."
  - [ ] Syntax highlight code block
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — body display

- [ ] **Implement**: Pattern recognition
  - [ ] Detect pattern usage (retry, run, try, etc.)
  - [ ] Show pattern properties
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — patterns

### 3.2.2 Test Information

- [ ] **Implement**: Test status in hover
  - [ ] "Tests: 2/2 passing"
  - [ ] List test names
  - [ ] Show failing test details
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — test status

### 3.2.3 Usage Information

- [ ] **Implement**: "Used by" list for types
  - [ ] Functions that use this type
  - [ ] Limit to top 5 with "..." for more
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — usage list

### 3.2.4 Closure Capture Hover

- [ ] **Implement**: Capture information for lambdas
  - [ ] List captured variables
  - [ ] Show captured value/origin
  - [ ] Show capture line number
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — captures

### 3.2.5 Pattern Property Hover

- [ ] **Implement**: Pattern property info
  - [ ] Property name and type
  - [ ] Valid range if applicable
  - [ ] Default value
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — pattern props

---

## 3.3 Type & Capture Inlay Hints

### 3.3.1 Type Hints

- [ ] **Implement**: `textDocument/inlayHint` request handler
  - [ ] Return InlayHint array
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — basic handler

- [ ] **Implement**: Let binding type hints
  - [ ] Show inferred type: `doubled`: [int]` = ...`
  - [ ] Skip when type is annotated
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — let hints

- [ ] **Implement**: Lambda parameter type hints
  - [ ] Show inferred param types
  - [ ] Skip when annotated
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — lambda hints

- [ ] **Implement**: Lambda return type hints
  - [ ] Show return type after arrow
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — return hints

### 3.3.2 Closure Capture Hints

- [ ] **Implement**: Captured variable hints
  - [ ] Show `[captured: value]` after lambda
  - [ ] List all captured variables
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — capture hints

### 3.3.3 Pattern Default Value Hints

- [ ] **Implement**: Default value hints in patterns
  - [ ] Show `// default: value` for omitted properties
  - [ ] Only for required properties with defaults
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — default hints

### 3.3.4 Parameter Name Hints

- [ ] **Implement**: Named argument hints at call sites
  - [ ] Show parameter names for positional args
  - [ ] Skip for named arguments
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — param hints

---

## 3.4 Inlay Hint Configuration

> **DESIGN**: Users can toggle hints on/off

- [ ] **Implement**: Configuration options
  - [ ] `ori.inlayHints.types`: on/off
  - [ ] `ori.inlayHints.captures`: on/off
  - [ ] `ori.inlayHints.defaults`: on/off
  - [ ] `ori.inlayHints.parameterNames`: on/off
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — hint settings

- [ ] **Implement**: Dynamic configuration update
  - [ ] `workspace/didChangeConfiguration` notification
  - [ ] Update hints without restart
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/workspace.rs` — config change

- [ ] **Implement**: Hint refresh on config change
  - [ ] Re-calculate hints when settings change
  - [ ] Send `workspace/inlayHint/refresh` if supported
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/inlay_hint.rs` — refresh

---

## 3.5 Phase Completion Checklist

- [ ] All items in 3.1-3.4 have all checkboxes marked `[x]`
- [ ] Hover shows type, signature, definition location
- [ ] Expanded hover shows body, tests, captures
- [ ] Inlay hints show types, captures, defaults
- [ ] Configuration toggles work
- [ ] Performance: hover < 20ms
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Full hover information and configurable inlay hints
