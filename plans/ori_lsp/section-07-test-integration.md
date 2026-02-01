# Phase 7: Test Integration

**Goal**: Implement test-first visibility with inline status, code lens, and test explorer

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Test Integration
> **RELATED**: Phase 14 (Testing Framework) in main roadmap

---

## 7.1 Inline Test Status

> **DESIGN**: Show test status next to functions

### 7.1.1 Test Status Computation

- [ ] **Implement**: Test status cache
  - [ ] Cache test results per function
  - [ ] Invalidate on source change
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/cache.rs` — status cache

- [ ] **Implement**: Test discovery
  - [ ] Find tests targeting each function (`@test tests @target`)
  - [ ] Count passing/failing tests
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/discovery.rs` — discovery

### 7.1.2 Status Display

- [ ] **Implement**: Inline diagnostics for test status
  - [ ] "3/3 tests passing" as hint diagnostic
  - [ ] "1/3 tests failing" as warning diagnostic
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/status.rs` — diagnostics

- [ ] **Implement**: Failing test details in hover
  - [ ] Show which tests failed
  - [ ] Show failure message
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/status.rs` — hover

---

## 7.2 Code Lens

### 7.2.1 Function Code Lens

- [ ] **Implement**: `textDocument/codeLens` request handler
  - [ ] Return code lens for functions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — handler

- [ ] **Implement**: "Run Tests" lens on functions
  - [ ] Show above function declaration
  - [ ] Execute tests targeting this function
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — run tests

- [ ] **Implement**: "Debug" lens on functions
  - [ ] Launch debugger for function
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — debug

- [ ] **Implement**: "Coverage" lens on functions
  - [ ] Toggle coverage highlighting
  - [ ] Show coverage percentage
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — coverage

### 7.2.2 Test Code Lens

- [ ] **Implement**: "Run" lens on test functions
  - [ ] Execute single test
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — run single

- [ ] **Implement**: "Debug" lens on test functions
  - [ ] Debug single test
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — debug test

- [ ] **Implement**: "Go to Target" lens on test functions
  - [ ] Navigate to target function
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — go to target

### 7.2.3 Code Lens Resolve

- [ ] **Implement**: `codeLens/resolve` request
  - [ ] Compute command lazily
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/code_lens.rs` — resolve

---

## 7.3 Test Explorer

### 7.3.1 Test Discovery Protocol

- [ ] **Implement**: Test adapter protocol support
  - [ ] VS Code Test Explorer integration
  - [ ] Custom test item provider
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/explorer.rs` — protocol

- [ ] **Implement**: Test tree structure
  - [ ] Root: workspace
  - [ ] Children: test files
  - [ ] Children: individual tests
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/explorer.rs` — tree

### 7.3.2 Test Execution

- [ ] **Implement**: Run test(s) command
  - [ ] Run selected tests
  - [ ] Stream results
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/runner.rs` — run

- [ ] **Implement**: Debug test(s) command
  - [ ] Launch debugger for selected tests
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/runner.rs` — debug

- [ ] **Implement**: Cancel test run
  - [ ] Interrupt running tests
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/runner.rs` — cancel

### 7.3.3 Test Results

- [ ] **Implement**: Test result reporting
  - [ ] Pass/fail status
  - [ ] Failure message and location
  - [ ] Duration
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/results.rs` — reporting

- [ ] **Implement**: Test output capture
  - [ ] Capture stdout/stderr
  - [ ] Show in test results
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/results.rs` — output

---

## 7.4 Coverage Display

### 7.4.1 Coverage Data

- [ ] **Implement**: Coverage data collection
  - [ ] Track which lines executed during tests
  - [ ] Store per-function coverage
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/coverage.rs` — collection

- [ ] **Implement**: Coverage percentage calculation
  - [ ] Lines covered / total lines
  - [ ] Per-function and per-file
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/coverage.rs` — calculation

### 7.4.2 Coverage Visualization

- [ ] **Implement**: Line coverage decorations
  - [ ] Green: covered lines
  - [ ] Red: uncovered lines
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/coverage.rs` — decorations

- [ ] **Implement**: Coverage toggle command
  - [ ] Show/hide coverage overlay
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/coverage.rs` — toggle

### 7.4.3 Coverage Gutters

- [ ] **Implement**: Gutter coverage indicators
  - [ ] Small colored markers in gutter
  - [ ] Hover for coverage details
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/coverage.rs` — gutters

---

## 7.5 Test Commands

### 7.5.1 Command Palette Integration

- [ ] **Implement**: "Ori: Run All Tests" command
  - [ ] Run all tests in workspace
  - [ ] **Rust Tests**: `ori_lsp/src/commands/test.rs` — run all

- [ ] **Implement**: "Ori: Run Tests in Current File" command
  - [ ] Run tests in active file
  - [ ] **Rust Tests**: `ori_lsp/src/commands/test.rs` — run file

- [ ] **Implement**: "Ori: Run Test at Cursor" command
  - [ ] Run test under cursor
  - [ ] **Rust Tests**: `ori_lsp/src/commands/test.rs` — run cursor

- [ ] **Implement**: "Ori: Run Failed Tests" command
  - [ ] Re-run previously failed tests
  - [ ] **Rust Tests**: `ori_lsp/src/commands/test.rs` — run failed

- [ ] **Implement**: "Ori: Run Tests for @function" command
  - [ ] Run tests targeting specific function
  - [ ] **Rust Tests**: `ori_lsp/src/commands/test.rs` — run for target

---

## 7.6 Phase Completion Checklist

- [ ] All items in 7.1-7.5 have all checkboxes marked `[x]`
- [ ] Inline test status shows next to functions
- [ ] Code lens provides Run/Debug/Coverage actions
- [ ] Test explorer shows hierarchical test tree
- [ ] Test execution with streaming results
- [ ] Coverage visualization with line decorations
- [ ] Command palette test commands work
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Full test-first visibility with IDE integration
