# Phase 7: Tooling Integration

**Goal**: Integrate the formatter with CLI, LSP, and WASM for the playground.

> **DESIGN**: `docs/tooling/formatter/design/04-implementation/`

## Phase Status: ⏳ Not Started

## 7.1 CLI Integration

### Basic Commands

- [ ] **Implement**: `ori fmt <file>` — format single file
  - [ ] **Tests**: CLI test for single file formatting
- [ ] **Implement**: `ori fmt <directory>` — format all .ori files recursively
  - [ ] **Tests**: CLI test for directory formatting
- [ ] **Implement**: `ori fmt .` — format current directory
  - [ ] **Tests**: CLI test for current directory
- [ ] **Implement**: `ori fmt --check` — check if files are formatted (exit code)
  - [ ] **Tests**: Check mode returns correct exit code

### Output Modes

- [ ] **Implement**: In-place formatting (default, overwrites files)
  - [ ] **Tests**: File content updated
- [ ] **Implement**: `ori fmt --diff` — show diff instead of modifying
  - [ ] **Tests**: Diff output format
- [ ] **Implement**: `ori fmt --stdin` — read from stdin, write to stdout
  - [ ] **Tests**: Stdin/stdout piping

### Error Handling

- [ ] **Implement**: Handle parse errors gracefully
  - [ ] **Tests**: Parse error message shown, file unchanged
- [ ] **Implement**: Handle file permission errors
  - [ ] **Tests**: Permission error message
- [ ] **Implement**: Handle missing files
  - [ ] **Tests**: Missing file error message
- [ ] **Implement**: Partial success (continue on errors)
  - [ ] **Tests**: Some files formatted, others skipped

### Ignore Patterns

- [ ] **Implement**: `.orifmtignore` file support
  - [ ] **Tests**: Ignored files not formatted
- [ ] **Implement**: Default ignores (`_test/`, `target/`, etc.)
  - [ ] **Tests**: Default patterns work
- [ ] **Implement**: `--no-ignore` flag to format everything
  - [ ] **Tests**: Flag overrides ignores

## 7.2 LSP Integration

### Format Document

- [ ] **Implement**: `textDocument/formatting` request handler
  - [ ] **Tests**: LSP test for document formatting
- [ ] **Implement**: Return TextEdit array for changes
  - [ ] **Tests**: Correct edit positions
- [ ] **Implement**: Handle partial formatting (syntax errors)
  - [ ] **Tests**: Partial format on error

### Format on Save

- [ ] **Implement**: Register format-on-save capability
  - [ ] **Tests**: Capability advertised
- [ ] **Implement**: Trigger formatting on save
  - [ ] **Tests**: Integration test with mock editor

### Format Selection

- [ ] **Implement**: `textDocument/rangeFormatting` request handler
  - [ ] **Tests**: Range formatting works
- [ ] **Implement**: Expand range to nearest complete construct
  - [ ] **Tests**: Range expansion logic

### Editor Integration

- [ ] **Document**: VS Code extension configuration
- [ ] **Document**: Neovim configuration
- [ ] **Document**: Other editors (Helix, Zed, etc.)

## 7.3 WASM Compilation

### Core WASM Module

- [ ] **Implement**: Compile `ori_fmt` to WASM
  - [ ] **Tests**: WASM module builds
- [ ] **Implement**: JavaScript bindings
  - [ ] **Tests**: JS can call format function
- [ ] **Implement**: TypeScript type definitions
  - [ ] **Tests**: Types are correct

### Playground Integration

- [ ] **Implement**: Format button in playground
  - [ ] **Tests**: Button triggers formatting
- [ ] **Implement**: Auto-format on blur (optional)
  - [ ] **Tests**: Blur triggers format
- [ ] **Implement**: Format error display
  - [ ] **Tests**: Errors shown in UI

### Performance

- [ ] **Implement**: Lazy WASM loading
  - [ ] **Tests**: WASM loads on demand
- [ ] **Implement**: Worker thread for formatting
  - [ ] **Tests**: Main thread not blocked
- [ ] **Implement**: Streaming output for large files
  - [ ] **Tests**: Large file handling

## 7.4 Build Integration

### Cargo Integration

- [ ] **Implement**: `ori_fmt` as workspace crate
  - [ ] **Tests**: `cargo build -p ori_fmt` works
- [ ] **Implement**: Feature flags for optional components
  - [ ] **Tests**: Features compile correctly

### CI Integration

- [ ] **Implement**: Format check in CI pipeline
  - [ ] **Tests**: CI fails on unformatted code
- [ ] **Document**: GitHub Actions workflow
- [ ] **Document**: GitLab CI configuration

### Pre-commit Hook

- [ ] **Implement**: Pre-commit hook script
  - [ ] **Tests**: Hook prevents unformatted commits
- [ ] **Document**: Hook installation instructions

## 7.5 Performance Optimization

- [ ] **Implement**: Incremental formatting (only changed regions)
  - [ ] **Tests**: Incremental mode faster
- [ ] **Implement**: Parallel file processing
  - [ ] **Tests**: Multiple files formatted concurrently
- [ ] **Implement**: Memory-efficient large file handling
  - [ ] **Tests**: Large files don't OOM

## Completion Checklist

- [ ] All CLI commands work correctly
- [ ] LSP integration complete
- [ ] WASM module builds and works
- [ ] Build integration documented
- [ ] Performance targets met
