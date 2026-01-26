# Phase 22: Tooling

**Goal**: Developer experience

> **DESIGN**: `design/12-tooling/index.md`
> **PROPOSALS**:
> - `proposals/approved/why-command-proposal.md` — Causality tracking (`sigil impact`, `sigil why`)
> - `proposals/approved/structured-diagnostics-autofix.md` — JSON output and auto-fix

---

## 22.1 Formatter

- [ ] **Implement**: `sigil fmt` command — design/12-tooling/index.md:64-69
  - [ ] **Rust Tests**: `sigilc/src/cli/fmt.rs` — fmt command
  - [ ] **Sigil Tests**: `tests/spec/tooling/fmt.si`

- [ ] **Implement**: `sigil fmt --check` for CI
  - [ ] **Rust Tests**: `sigilc/src/cli/fmt.rs` — check mode
  - [ ] **Sigil Tests**: `tests/spec/tooling/fmt_check.si`

- [ ] **Implement**: Zero-config formatting per spec
  - [ ] **Rust Tests**: `sigilc/src/fmt/rules.rs` — formatting rules
  - [ ] **Sigil Tests**: `tests/spec/tooling/fmt.si`

---

## 22.2 LSP Server

- [ ] **Implement**: Semantic addressing — design/12-tooling/index.md:25-35
  - [ ] **Rust Tests**: `sigilc/src/lsp/addressing.rs` — semantic addressing
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_addressing.si`

- [ ] **Implement**: Structured errors — design/12-tooling/index.md:36-55
  - [ ] **Rust Tests**: `sigilc/src/lsp/diagnostics.rs` — structured errors
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_errors.si`

- [ ] **Implement**: Go to definition
  - [ ] **Rust Tests**: `sigilc/src/lsp/goto_def.rs` — go to definition
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_goto.si`

- [ ] **Implement**: Find references
  - [ ] **Rust Tests**: `sigilc/src/lsp/references.rs` — find references
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_references.si`

- [ ] **Implement**: Hover information
  - [ ] **Rust Tests**: `sigilc/src/lsp/hover.rs` — hover information
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_hover.si`

- [ ] **Implement**: Completions
  - [ ] **Rust Tests**: `sigilc/src/lsp/completions.rs` — completions
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_completions.si`

- [ ] **Implement**: Diagnostics
  - [ ] **Rust Tests**: `sigilc/src/lsp/diagnostics.rs` — LSP diagnostics
  - [ ] **Sigil Tests**: `tests/spec/tooling/lsp_diagnostics.si`

---

## 22.3 Edit Operations

- [ ] **Implement**: `set`, `add`, `remove`, `rename`, `move` — design/12-tooling/index.md:56-62
  - [ ] **Rust Tests**: `sigilc/src/lsp/edit_ops.rs` — edit operations
  - [ ] **Sigil Tests**: `tests/spec/tooling/edit_ops.si`

---

## 22.4 REPL

- [ ] **Implement**: Interactive evaluation
  - [ ] **Rust Tests**: `sigilc/src/cli/repl.rs` — REPL evaluation
  - [ ] **Sigil Tests**: `tests/spec/tooling/repl.si`

- [ ] **Implement**: History and completion
  - [ ] **Rust Tests**: `sigilc/src/cli/repl.rs` — history/completion
  - [ ] **Sigil Tests**: `tests/spec/tooling/repl.si`

- [ ] **Implement**: Multi-line input
  - [ ] **Rust Tests**: `sigilc/src/cli/repl.rs` — multi-line input
  - [ ] **Sigil Tests**: `tests/spec/tooling/repl.si`

---

## 22.5 Test Runner

> **NOTE**: This section covers the TEST RUNNER CLI commands, which are largely complete.
> The TESTING FRAMEWORK features (mandatory testing, dependency-aware execution, incremental tests)
> are in Phase 14 and are not yet implemented. The test runner runs tests; the framework enforces
> testing requirements and manages test dependencies.

- [x] **Implement**: `sigil test` command — run all tests — design/11-testing/index.md
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — test command
  - [x] **Sigil Tests**: `tests/spec/tooling/test_runner.si`

- [x] **Implement**: `sigil test file.test.si` — run specific test file
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — file filtering
  - [x] **Sigil Tests**: `tests/spec/tooling/test_runner.si`

- [x] **Implement**: `sigil test path/` — run all tests in directory
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — directory scanning
  - [x] **Sigil Tests**: `tests/spec/tooling/test_runner.si`

- [x] **Implement**: `sigil check file.si` — check test coverage without running — spec/13-testing.md § Coverage Enforcement
  - [x] **Rust Tests**: `sigilc/src/cli/check.rs` — coverage check
  - [x] **Sigil Tests**: `tests/spec/tooling/test_check.si`

- [x] **Implement**: Parallel test execution — spec/13-testing.md § Test Isolation
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — parallel execution
  - [x] **Sigil Tests**: `tests/spec/tooling/test_parallel.si`

- [x] **Implement**: Test filtering by name pattern (e.g., `sigil test --filter "auth"`)
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — name filtering
  - [x] **Sigil Tests**: `tests/spec/tooling/test_filter.si`

- [x] **Implement**: Test output formatting (pass/fail/skip counts, timing)
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — output formatting
  - [x] **Sigil Tests**: `tests/spec/tooling/test_output.si`

- [x] **Implement**: Verbose mode for detailed output (`--verbose`)
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — verbose mode
  - [x] **Sigil Tests**: `tests/spec/tooling/test_output.si`

- [x] **Implement**: Coverage report generation
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — coverage report
  - [x] **Sigil Tests**: `tests/spec/tooling/test_coverage.si`

- [x] **Implement**: Exit codes (0 = all pass, 1 = failures, 2 = no tests found)
  - [x] **Rust Tests**: `sigilc/src/cli/test.rs` — exit codes
  - [x] **Sigil Tests**: `tests/spec/tooling/test_exit.si`

---

## 22.6 Causality Tracking

> **PROPOSAL**: `proposals/approved/why-command-proposal.md`

Expose Salsa's dependency tracking to users for debugging and impact analysis.

### 22.6.1 `sigil impact` Command

- [ ] **Implement**: `sigil impact @target` — Show blast radius of potential change
  - [ ] Direct dependents (functions that call target)
  - [ ] Transitive dependents (recursive callers)
  - [ ] Summary count of affected functions
  - [ ] **Rust Tests**: `sigilc/src/cli/impact.rs` — impact command
  - [ ] **Sigil Tests**: `tests/spec/tooling/impact_basic.si`

- [ ] **Implement**: `sigil impact @target --verbose` — Show call sites
  - [ ] **Rust Tests**: `sigilc/src/cli/impact.rs` — verbose output
  - [ ] **Sigil Tests**: `tests/spec/tooling/impact_verbose.si`

- [ ] **Implement**: `sigil impact @Type` — Impact analysis for type changes
  - [ ] Functions using type, returning type, accepting type
  - [ ] **Rust Tests**: `sigilc/src/cli/impact.rs` — type impact
  - [ ] **Sigil Tests**: `tests/spec/tooling/impact_type.si`

### 22.6.2 `sigil why` Command

- [ ] **Implement**: `sigil why @target` — Trace why target is dirty/broken
  - [ ] Show causality chain from changed input to target
  - [ ] Concise output by default
  - [ ] **Rust Tests**: `sigilc/src/cli/why.rs` — why command
  - [ ] **Sigil Tests**: `tests/spec/tooling/why_basic.si`

- [ ] **Implement**: `sigil why @target --verbose` — Detailed causality chain
  - [ ] Full path through dependency graph
  - [ ] Source locations for each change
  - [ ] **Rust Tests**: `sigilc/src/cli/why.rs` — verbose causality
  - [ ] **Sigil Tests**: `tests/spec/tooling/why_verbose.si`

- [ ] **Implement**: `sigil why @target --diff` — Show actual code changes
  - [ ] **Rust Tests**: `sigilc/src/cli/why.rs` — diff output
  - [ ] **Sigil Tests**: `tests/spec/tooling/why_diff.si`

- [ ] **Implement**: `sigil why @target --graph` — Tree visualization
  - [ ] **Rust Tests**: `sigilc/src/cli/why.rs` — graph visualization
  - [ ] **Sigil Tests**: `tests/spec/tooling/why_graph.si`

### 22.6.3 Test Runner Integration

- [ ] **Implement**: Hint on test failure suggesting `sigil why`
  - [ ] "Hint: This test is dirty due to changes in @X"
  - [ ] **Rust Tests**: `sigilc/src/cli/test.rs` — why hint
  - [ ] **Sigil Tests**: `tests/spec/tooling/why_hint.si`

---

## 22.7 Structured Diagnostics

> **PROPOSAL**: `proposals/approved/structured-diagnostics-autofix.md`

Machine-readable diagnostics with actionable fix suggestions.

### 22.7.1 Core Diagnostic Types

- [ ] **Implement**: `Fix` type with `message`, `edits`, `applicability`
  - [ ] **Rust Tests**: `sigil_diagnostic/src/fix.rs` — Fix type tests

- [ ] **Implement**: `Edit` type with `span`, `replacement`
  - [ ] **Rust Tests**: `sigil_diagnostic/src/edit.rs` — Edit type tests

- [ ] **Implement**: `Applicability` enum (MachineApplicable, MaybeIncorrect, HasPlaceholders, Unspecified)
  - [ ] **Rust Tests**: `sigil_diagnostic/src/applicability.rs` — applicability tests

- [ ] **Implement**: `SourceLoc` with line/column from byte span
  - [ ] **Rust Tests**: `sigil_diagnostic/src/source_loc.rs` — source location tests

### 22.7.2 JSON Output

- [ ] **Implement**: `sigil check --json` — Machine-readable output
  - [ ] Diagnostics array with code, severity, message, span, labels, fixes
  - [ ] Summary with error/warning/fixable counts
  - [ ] **Rust Tests**: `sigilc/src/cli/check.rs` — JSON output
  - [ ] **Sigil Tests**: `tests/spec/tooling/json_output.si`

### 22.7.3 Improved Human Output

- [ ] **Implement**: Rust-style diagnostic rendering
  - [ ] Source snippets with line numbers
  - [ ] Primary and secondary labels with arrows
  - [ ] Notes and help sections
  - [ ] **Rust Tests**: `sigil_diagnostic/src/render.rs` — diagnostic rendering
  - [ ] **Sigil Tests**: `tests/spec/tooling/human_output.si`

### 22.7.4 Auto-Fix

- [ ] **Implement**: `sigil check --fix` — Apply MachineApplicable fixes
  - [ ] Only safe fixes applied automatically
  - [ ] **Rust Tests**: `sigilc/src/cli/check.rs` — auto-fix
  - [ ] **Sigil Tests**: `tests/spec/tooling/autofix_basic.si`

- [ ] **Implement**: `sigil check --fix --dry` — Preview fixes without applying
  - [ ] **Rust Tests**: `sigilc/src/cli/check.rs` — dry run
  - [ ] **Sigil Tests**: `tests/spec/tooling/autofix_dry.si`

- [ ] **Implement**: `sigil check --fix=all` — Also apply MaybeIncorrect fixes
  - [ ] **Rust Tests**: `sigilc/src/cli/check.rs` — fix all
  - [ ] **Sigil Tests**: `tests/spec/tooling/autofix_all.si`

- [ ] **Implement**: Overlapping fix handling (three-way merge or reject)
  - [ ] **Rust Tests**: `sigil_diagnostic/src/fixes/merge.rs` — fix merging
  - [ ] **Sigil Tests**: `tests/spec/tooling/autofix_conflict.si`

### 22.7.5 Fix Categories

- [ ] **Implement**: MachineApplicable fixes for formatting issues
  - [ ] Missing trailing comma
  - [ ] Wrong indentation
  - [ ] Inline comment moved to own line
  - [ ] Extra blank lines
  - [ ] **Rust Tests**: `sigil_diagnostic/src/fixes/formatting.rs` — formatting fixes
  - [ ] **Sigil Tests**: `tests/spec/tooling/fix_formatting.si`

- [ ] **Implement**: MaybeIncorrect fixes for type mismatches
  - [ ] Type conversion suggestions (`int(x)`, `float(x)`)
  - [ ] "Did you mean" for identifiers (Levenshtein distance)
  - [ ] **Rust Tests**: `sigil_diagnostic/src/fixes/suggestions.rs` — suggestions
  - [ ] **Sigil Tests**: `tests/spec/tooling/fix_suggestions.si`

---

## 22.8 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Benchmarks
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: Full tooling support
