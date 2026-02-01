# Phase 8: Workspace & Integration

**Goal**: Multi-project support and integration with other Ori tools

> **DESIGN**: `docs/ori_lang/0.1-alpha/archived-design/12-tooling/05-lsp.md` § Workspace Features, Integration Points

---

## 8.1 Multi-Root Workspace

### 8.1.1 Workspace Folders

- [ ] **Implement**: Multiple workspace folders support
  - [ ] Handle `workspace/didChangeWorkspaceFolders` notification
  - [ ] Track multiple root paths
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/folders.rs` — multi-root

- [ ] **Implement**: Per-folder configuration
  - [ ] Load `ori.toml` from each folder
  - [ ] Merge settings appropriately
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/config.rs` — per-folder

- [ ] **Implement**: Cross-folder imports
  - [ ] Resolve imports across workspace folders
  - [ ] Handle folder dependencies
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/imports.rs` — cross-folder

### 8.1.2 Workspace-Wide Operations

- [ ] **Implement**: Find all references across workspace
  - [ ] Search all folders for references
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/search.rs` — references

- [ ] **Implement**: Rename across all projects
  - [ ] Multi-file rename across folders
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/rename.rs` — workspace rename

- [ ] **Implement**: Global type checking
  - [ ] Type check entire workspace
  - [ ] Report cross-project type errors
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/typecheck.rs` — global check

---

## 8.2 Project Detection

### 8.2.1 Automatic Detection

- [ ] **Implement**: `ori.toml` detection
  - [ ] Find project root by `ori.toml` presence
  - [ ] Parse configuration
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/detection.rs` — ori.toml

- [ ] **Implement**: Directory structure detection
  - [ ] Detect by `src/` with `.ori` files
  - [ ] Fallback when no `ori.toml`
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/detection.rs` — structure

- [ ] **Implement**: Library vs binary project detection
  - [ ] Check for `@main` entry point
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/detection.rs` — project type

### 8.2.2 Standard Library Resolution

- [ ] **Implement**: Stdlib path discovery
  - [ ] Find bundled stdlib
  - [ ] Support custom stdlib path
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/stdlib.rs` — discovery

- [ ] **Implement**: Stdlib documentation loading
  - [ ] Load doc comments for stdlib items
  - [ ] Show in hover/completions
  - [ ] **Rust Tests**: `ori_lsp/src/workspace/stdlib.rs` — docs

---

## 8.3 Formatter Integration

> **DESIGN**: Format on save, format selection, format on paste

### 8.3.1 Format on Save

- [ ] **Implement**: `textDocument/formatting` request
  - [ ] Return edits to format entire document
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — full format

- [ ] **Implement**: Format on save configuration
  - [ ] `ori.formatting.formatOnSave`: true/false
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — format on save

- [ ] **Implement**: Format on save trigger
  - [ ] Format on `textDocument/willSave` if configured
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — on save

### 8.3.2 Range Formatting

- [ ] **Implement**: `textDocument/rangeFormatting` request
  - [ ] Format selected range only
  - [ ] Expand to complete statements
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — range

### 8.3.3 On-Type Formatting

- [ ] **Implement**: `textDocument/onTypeFormatting` request
  - [ ] Format on specific trigger characters
  - [ ] Configure trigger characters (e.g., `}`, `;`)
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — on type

### 8.3.4 Formatter Integration

- [ ] **Implement**: Integration with `ori_fmt` crate
  - [ ] Use shared formatting logic
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — integration

---

## 8.4 Edit Operations Integration

> **DESIGN**: LSP code actions invoke edit operations

### 8.4.1 Rename Integration

- [ ] **Implement**: `textDocument/rename` request
  - [ ] Prepare rename (validate symbol)
  - [ ] Execute rename across files
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/rename.rs` — rename

- [ ] **Implement**: `textDocument/prepareRename` request
  - [ ] Validate rename is possible
  - [ ] Return range to rename
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/rename.rs` — prepare

### 8.4.2 Refactoring API Integration

- [ ] **Implement**: Extract function via refactoring API
  - [ ] Invoke `ori refactor extract-function`
  - [ ] **Rust Tests**: `ori_lsp/src/refactoring/extract.rs` — extract func

- [ ] **Implement**: Move to module via refactoring API
  - [ ] Invoke `ori refactor move`
  - [ ] **Rust Tests**: `ori_lsp/src/refactoring/move.rs` — move

### 8.4.3 Edit Operation Commands

- [ ] **Implement**: Integration with `ori edit` commands
  - [ ] `set`, `add`, `remove`, `rename`, `move`
  - [ ] Execute via workspace/executeCommand
  - [ ] **Rust Tests**: `ori_lsp/src/commands/edit.rs` — operations

---

## 8.5 Test Runner Integration

### 8.5.1 Test Execution

- [ ] **Implement**: Run tests from code lens
  - [ ] Execute `ori test` with appropriate filters
  - [ ] Stream output to client
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/runner.rs` — lens run

- [ ] **Implement**: Test result inline display
  - [ ] Show pass/fail in source
  - [ ] Navigate to failure location
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/display.rs` — inline

### 8.5.2 Test Output

- [ ] **Implement**: Test output channel
  - [ ] Show test output in output panel
  - [ ] Parse test results for navigation
  - [ ] **Rust Tests**: `ori_lsp/src/test_integration/output.rs` — channel

---

## 8.6 Configuration

> **DESIGN**: Minimal configuration - most behavior is fixed for consistency

### 8.6.1 Available Settings

- [ ] **Implement**: Inlay hint settings
  - [ ] `ori.inlayHints.types`: true/false
  - [ ] `ori.inlayHints.captures`: true/false
  - [ ] `ori.inlayHints.defaults`: true/false
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — hint settings

- [ ] **Implement**: Diagnostic settings
  - [ ] `ori.diagnostics.delay`: milliseconds
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — diagnostic settings

- [ ] **Implement**: Testing settings
  - [ ] `ori.testing.runOnSave`: true/false
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — testing settings

- [ ] **Implement**: Formatting settings
  - [ ] `ori.formatting.formatOnSave`: true/false
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — formatting settings

### 8.6.2 Non-Configurable Behaviors

> **DESIGN**: These are fixed for consistency

- [ ] **Document**: Non-configurable aspects
  - [ ] Formatting style: always canonical
  - [ ] Diagnostic rules: always all enabled
  - [ ] Completion ranking: always semantic
  - [ ] **Documentation**: `docs/tooling/lsp/configuration.md`

### 8.6.3 Configuration Loading

- [ ] **Implement**: `workspace/didChangeConfiguration` handling
  - [ ] Reload settings on change
  - [ ] Apply without restart
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/workspace.rs` — config change

- [ ] **Implement**: Per-workspace configuration
  - [ ] Read from workspace settings
  - [ ] Fall back to user settings
  - [ ] **Rust Tests**: `ori_lsp/src/config.rs` — workspace config

---

## 8.7 Phase Completion Checklist

- [ ] All items in 8.1-8.6 have all checkboxes marked `[x]`
- [ ] Multi-root workspace support works
- [ ] Project auto-detection works
- [ ] Formatter integration (format on save, range, on type)
- [ ] Edit operations integration
- [ ] Test runner integration
- [ ] Configuration loading and update
- [ ] Run full test suite: `cargo test -p ori_lsp`

**Exit Criteria**: Full workspace support with tool integration
