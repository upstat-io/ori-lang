# Phase 5: Code Actions

**Goal**: Implement refactoring and code transformation actions

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Code Actions
> **RELATED**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/02-edit-operations.md`
> **RELATED**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/07-refactoring-api.md`

---

## 5.1 Function-Level Actions

### 5.1.1 Test Actions

- [ ] **Implement**: "Run tests for @function" action
  - [ ] Available on function declarations
  - [ ] Execute tests targeting this function
  - [ ] Show results inline or in test panel
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — run tests

- [ ] **Implement**: "Go to tests" action
  - [ ] Navigate to test file/location
  - [ ] Show picker if multiple tests
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — go to tests

- [ ] **Implement**: "Generate test skeleton" action
  - [ ] Create test file if needed
  - [ ] Generate `@test_func tests @target () -> void = ...`
  - [ ] Include assertion placeholder
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — generate test

### 5.1.2 Refactoring Actions

- [ ] **Implement**: "Rename symbol" action
  - [ ] Rename function across all files
  - [ ] Preview changes before applying
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — rename

- [ ] **Implement**: "Extract to module" action
  - [ ] Move function to new/existing module
  - [ ] Update imports automatically
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — extract module

- [ ] **Implement**: "Inline function" action
  - [ ] Replace call sites with function body
  - [ ] Only for simple, single-expression functions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — inline

### 5.1.3 Debug Actions

- [ ] **Implement**: "Debug this function" action
  - [ ] Set up debug configuration
  - [ ] Launch debugger with breakpoint
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — debug

---

## 5.2 Expression-Level Actions

### 5.2.1 Extract Refactorings

- [ ] **Implement**: "Extract to function" action
  - [ ] Create new function from selected expression
  - [ ] Detect captured variables as parameters
  - [ ] Generate function name suggestion
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — extract func

- [ ] **Implement**: "Extract to variable" action
  - [ ] Create `let` binding for expression
  - [ ] Place binding at appropriate scope
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — extract var

### 5.2.2 Inline Refactorings

- [ ] **Implement**: "Inline variable" action
  - [ ] Replace variable uses with its value
  - [ ] Remove binding if unused after inlining
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — inline var

### 5.2.3 Transformation Actions

- [ ] **Implement**: "Convert to fold" action
  - [ ] Transform loop accumulation to fold pattern
  - [ ] Available when pattern detected
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — to fold

- [ ] **Implement**: "Convert to map" action
  - [ ] Transform loop transformation to map pattern
  - [ ] Available when pattern detected
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — to map

- [ ] **Implement**: "Convert to filter" action
  - [ ] Transform conditional loop to filter pattern
  - [ ] Available when pattern detected
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — to filter

---

## 5.3 Error-Level Actions

### 5.3.1 Quick Fix Actions

- [ ] **Implement**: Quick fix from diagnostic
  - [ ] Convert quick fix suggestions to code actions
  - [ ] Mark with `CodeActionKind.QuickFix`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — quick fix

- [ ] **Implement**: "See error documentation" action
  - [ ] Open documentation for error code
  - [ ] Available on errors with documentation
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — docs link

### 5.3.2 Import Actions

- [ ] **Implement**: "Add import" action
  - [ ] Add missing import statement
  - [ ] Insert at appropriate location
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — add import

- [ ] **Implement**: "Organize imports" action
  - [ ] Sort imports alphabetically
  - [ ] Group by std vs local
  - [ ] Remove unused imports
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — organize

---

## 5.4 Test-Centric Actions

> **DESIGN**: Test-first visibility is a core design principle

### 5.4.1 Test Navigation

- [ ] **Implement**: "Show test coverage" action
  - [ ] Highlight which lines are covered by tests
  - [ ] Show coverage percentage
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — coverage

### 5.4.2 Test Creation

- [ ] **Implement**: "Add test case" action
  - [ ] Add new test for existing function
  - [ ] Generate unique test name
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — add test

- [ ] **Implement**: "Create compile-fail test" action
  - [ ] Generate `#compile_fail("expected error")` test
  - [ ] Pre-fill expected error
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — compile fail

### 5.4.3 Test Execution

- [ ] **Implement**: "Run test at cursor" action
  - [ ] Execute single test
  - [ ] Show result inline
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — run single

- [ ] **Implement**: "Debug test at cursor" action
  - [ ] Debug single test
  - [ ] Set breakpoint at test start
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — debug test

---

## 5.5 Code Action Handler

- [ ] **Implement**: `textDocument/codeAction` request handler
  - [ ] Return available actions for range
  - [ ] Filter by requested kinds
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — handler

- [ ] **Implement**: Code action kinds categorization
  - [ ] `quickfix` for error fixes
  - [ ] `refactor` for refactoring actions
  - [ ] `refactor.extract` for extraction
  - [ ] `refactor.inline` for inlining
  - [ ] `source.organizeImports` for imports
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — kinds

- [ ] **Implement**: `codeAction/resolve` request
  - [ ] Compute edit lazily for expensive actions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_action.rs` — resolve

---

## 5.6 Edit Operations Integration

> **RELATED**: Integration with `ori edit` command operations

- [ ] **Implement**: Workspace edit generation
  - [ ] Multi-file edits for refactorings
  - [ ] Document changes with annotations
  - [ ] **Rust Tests**: `ori_lsp/src/edit/workspace.rs` — multi-file

- [ ] **Implement**: Text edit utilities
  - [ ] Insert, replace, delete operations
  - [ ] Preserve formatting
  - [ ] **Rust Tests**: `ori_lsp/src/edit/text.rs` — utilities

- [ ] **Implement**: Import insertion logic
  - [ ] Find correct insertion point
  - [ ] Maintain alphabetical order
  - [ ] Handle grouping
  - [ ] **Rust Tests**: `ori_lsp/src/edit/imports.rs` — insertion

---

## 5.7 Phase Completion Checklist

- [ ] All items in 5.1-5.6 have all checkboxes marked `[x]`
- [ ] Test actions work (run, go to, generate)
- [ ] Refactoring actions work (rename, extract, inline)
- [ ] Quick fix actions derived from diagnostics
- [ ] Code action kinds correctly categorized
- [ ] Multi-file edits work correctly
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Comprehensive code actions for refactoring and test management
