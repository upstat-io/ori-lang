# Phase 7: Tooling Integration

**Goal**: Integrate the formatter with CLI, LSP, and WASM for the playground.

> **DESIGN**: `docs/tooling/formatter/design/04-implementation/`

## Phase Status: 🔶 Partial

## 7.1 CLI Integration

### Basic Commands

- [x] **Implement**: `ori fmt <file>` — format single file
  - [x] **Tests**: CLI test for single file formatting
- [x] **Implement**: `ori fmt <directory>` — format all .ori files recursively
  - [x] **Tests**: CLI test for directory formatting
- [x] **Implement**: `ori fmt .` — format current directory
  - [x] **Tests**: CLI test for current directory
- [x] **Implement**: `ori fmt --check` — check if files are formatted (exit code)
  - [x] **Tests**: Check mode returns correct exit code

### Output Modes

- [x] **Implement**: In-place formatting (default, overwrites files)
  - [x] **Tests**: File content updated
- [x] **Implement**: `ori fmt --diff` — show diff instead of modifying
  - [x] **Tests**: Diff output format
- [x] **Implement**: `ori fmt --stdin` — read from stdin, write to stdout
  - [x] **Tests**: Stdin/stdout piping

### Error Handling

- [x] **Implement**: Handle parse errors gracefully
  - [x] **Tests**: Parse error message shown, file unchanged
- [x] **Implement**: Handle file permission errors
  - [x] **Tests**: Permission error message (via read_file helper)
- [x] **Implement**: Handle missing files
  - [x] **Tests**: Missing file error message (via read_file helper)
- [x] **Implement**: Partial success (continue on errors)
  - [x] **Tests**: Some files formatted, others skipped

### Ignore Patterns

- [x] **Implement**: `.orifmtignore` file support
  - [x] **Tests**: Ignored files not formatted
- [x] **Implement**: Default ignores (`_test/`, `target/`, etc.)
  - [x] **Tests**: Default patterns work
- [x] **Implement**: `--no-ignore` flag to format everything
  - [x] **Tests**: Flag overrides ignores

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

- [x] **Implement**: Compile `ori_fmt` to WASM (direct integration, TODO: switch to LSP)
  - [x] **Tests**: WASM module builds (`cargo check --target wasm32-unknown-unknown`)
- [x] **Implement**: JavaScript bindings (`format_ori()` function)
  - [x] **Tests**: Returns JSON with `success`, `formatted`, `error` fields
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

- [x] **Implement**: Incremental formatting (only changed regions)
  - [x] **API**: `format_incremental()`, `apply_regions()`, `FormattedRegion`, `IncrementalResult`
  - [x] **File**: `compiler/ori_fmt/src/incremental.rs`
  - [x] **Tests**: 13 integration tests in `compiler/ori_fmt/tests/incremental_tests.rs`
  - [x] **Benchmarks**: `bench_incremental_vs_full`, `bench_incremental_large_file`
  - [x] **Results**: ~30% speedup for 1000 functions (424µs vs 622µs), ~20% for 2000 functions
  - **Note**: Full speedup requires incremental parsing (future work)
- [x] **Implement**: Parallel file processing
  - [x] **Tests**: Multiple files formatted concurrently (2.4x speedup with rayon)
- [x] **Implement**: Memory-efficient large file handling
  - [x] **Tests**: Large files don't OOM (10k lines in 2.75ms)

## Completion Checklist

- [x] All CLI commands work correctly
- [ ] LSP integration complete
- [ ] WASM module builds and works
- [x] Build integration documented (in integration.md)
- [x] Performance targets met (parallel + incremental)
