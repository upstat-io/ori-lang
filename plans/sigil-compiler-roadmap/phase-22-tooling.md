# Phase 22: Tooling

**Goal**: Developer experience

> **DESIGN**: `design/12-tooling/index.md`
> **PROPOSALS**:
> - `proposals/approved/why-command-proposal.md` — Causality tracking (`ori impact`, `ori why`)
> - `proposals/approved/structured-diagnostics-autofix.md` — JSON output and auto-fix

---

## 22.1 Formatter

- [ ] **Implement**: `ori fmt` command — design/12-tooling/index.md:64-69
  - [ ] **Rust Tests**: `oric/src/cli/fmt.rs` — fmt command
  - [ ] **Ori Tests**: `tests/spec/tooling/fmt.ori`

- [ ] **Implement**: `ori fmt --check` for CI
  - [ ] **Rust Tests**: `oric/src/cli/fmt.rs` — check mode
  - [ ] **Ori Tests**: `tests/spec/tooling/fmt_check.ori`

- [ ] **Implement**: Zero-config formatting per spec
  - [ ] **Rust Tests**: `oric/src/fmt/rules.rs` — formatting rules
  - [ ] **Ori Tests**: `tests/spec/tooling/fmt.ori`

---

## 22.2 LSP Server

- [ ] **Implement**: Semantic addressing — design/12-tooling/index.md:25-35
  - [ ] **Rust Tests**: `oric/src/lsp/addressing.rs` — semantic addressing
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_addressing.ori`

- [ ] **Implement**: Structured errors — design/12-tooling/index.md:36-55
  - [ ] **Rust Tests**: `oric/src/lsp/diagnostics.rs` — structured errors
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_errors.ori`

- [ ] **Implement**: Go to definition
  - [ ] **Rust Tests**: `oric/src/lsp/goto_def.rs` — go to definition
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_goto.ori`

- [ ] **Implement**: Find references
  - [ ] **Rust Tests**: `oric/src/lsp/references.rs` — find references
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_references.ori`

- [ ] **Implement**: Hover information
  - [ ] **Rust Tests**: `oric/src/lsp/hover.rs` — hover information
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_hover.ori`

- [ ] **Implement**: Completions
  - [ ] **Rust Tests**: `oric/src/lsp/completions.rs` — completions
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_completions.ori`

- [ ] **Implement**: Diagnostics
  - [ ] **Rust Tests**: `oric/src/lsp/diagnostics.rs` — LSP diagnostics
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_diagnostics.ori`

---

## 22.3 Edit Operations

- [ ] **Implement**: `set`, `add`, `remove`, `rename`, `move` — design/12-tooling/index.md:56-62
  - [ ] **Rust Tests**: `oric/src/lsp/edit_ops.rs` — edit operations
  - [ ] **Ori Tests**: `tests/spec/tooling/edit_ops.ori`

---

## 22.4 REPL

- [ ] **Implement**: Interactive evaluation
  - [ ] **Rust Tests**: `oric/src/cli/repl.rs` — REPL evaluation
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

- [ ] **Implement**: History and completion
  - [ ] **Rust Tests**: `oric/src/cli/repl.rs` — history/completion
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

- [ ] **Implement**: Multi-line input
  - [ ] **Rust Tests**: `oric/src/cli/repl.rs` — multi-line input
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

---

## 22.5 Test Runner

> **NOTE**: This section covers the TEST RUNNER CLI commands, which are largely complete.
> The TESTING FRAMEWORK features (mandatory testing, dependency-aware execution, incremental tests)
> are in Phase 14 and are not yet implemented. The test runner runs tests; the framework enforces
> testing requirements and manages test dependencies.

- [x] **Implement**: `ori test` command — run all tests — design/11-testing/index.md
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — test command
  - [x] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori test file.test.ori` — run specific test file
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — file filtering
  - [x] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori test path/` — run all tests in directory
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — directory scanning
  - [x] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori check file.ori` — check test coverage without running — spec/13-testing.md § Coverage Enforcement
  - [x] **Rust Tests**: `oric/src/cli/check.rs` — coverage check
  - [x] **Ori Tests**: `tests/spec/tooling/test_check.ori`

- [x] **Implement**: Parallel test execution — spec/13-testing.md § Test Isolation
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — parallel execution
  - [x] **Ori Tests**: `tests/spec/tooling/test_parallel.ori`

- [x] **Implement**: Test filtering by name pattern (e.g., `ori test --filter "auth"`)
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — name filtering
  - [x] **Ori Tests**: `tests/spec/tooling/test_filter.ori`

- [x] **Implement**: Test output formatting (pass/fail/skip counts, timing)
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — output formatting
  - [x] **Ori Tests**: `tests/spec/tooling/test_output.ori`

- [x] **Implement**: Verbose mode for detailed output (`--verbose`)
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — verbose mode
  - [x] **Ori Tests**: `tests/spec/tooling/test_output.ori`

- [x] **Implement**: Coverage report generation
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — coverage report
  - [x] **Ori Tests**: `tests/spec/tooling/test_coverage.ori`

- [x] **Implement**: Exit codes (0 = all pass, 1 = failures, 2 = no tests found)
  - [x] **Rust Tests**: `oric/src/cli/test.rs` — exit codes
  - [x] **Ori Tests**: `tests/spec/tooling/test_exit.ori`

---

## 22.6 Causality Tracking

> **PROPOSAL**: `proposals/approved/why-command-proposal.md`

Expose Salsa's dependency tracking to users for debugging and impact analysis.

### 22.6.1 `ori impact` Command

- [ ] **Implement**: `ori impact @target` — Show blast radius of potential change
  - [ ] Direct dependents (functions that call target)
  - [ ] Transitive dependents (recursive callers)
  - [ ] Summary count of affected functions
  - [ ] **Rust Tests**: `oric/src/cli/impact.rs` — impact command
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_basic.ori`

- [ ] **Implement**: `ori impact @target --verbose` — Show call sites
  - [ ] **Rust Tests**: `oric/src/cli/impact.rs` — verbose output
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_verbose.ori`

- [ ] **Implement**: `ori impact @Type` — Impact analysis for type changes
  - [ ] Functions using type, returning type, accepting type
  - [ ] **Rust Tests**: `oric/src/cli/impact.rs` — type impact
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_type.ori`

### 22.6.2 `ori why` Command

- [ ] **Implement**: `ori why @target` — Trace why target is dirty/broken
  - [ ] Show causality chain from changed input to target
  - [ ] Concise output by default
  - [ ] **Rust Tests**: `oric/src/cli/why.rs` — why command
  - [ ] **Ori Tests**: `tests/spec/tooling/why_basic.ori`

- [ ] **Implement**: `ori why @target --verbose` — Detailed causality chain
  - [ ] Full path through dependency graph
  - [ ] Source locations for each change
  - [ ] **Rust Tests**: `oric/src/cli/why.rs` — verbose causality
  - [ ] **Ori Tests**: `tests/spec/tooling/why_verbose.ori`

- [ ] **Implement**: `ori why @target --diff` — Show actual code changes
  - [ ] **Rust Tests**: `oric/src/cli/why.rs` — diff output
  - [ ] **Ori Tests**: `tests/spec/tooling/why_diff.ori`

- [ ] **Implement**: `ori why @target --graph` — Tree visualization
  - [ ] **Rust Tests**: `oric/src/cli/why.rs` — graph visualization
  - [ ] **Ori Tests**: `tests/spec/tooling/why_graph.ori`

### 22.6.3 Test Runner Integration

- [ ] **Implement**: Hint on test failure suggesting `ori why`
  - [ ] "Hint: This test is dirty due to changes in @X"
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — why hint
  - [ ] **Ori Tests**: `tests/spec/tooling/why_hint.ori`

---

## 22.7 Structured Diagnostics

> **PROPOSAL**: `proposals/approved/structured-diagnostics-autofix.md`

Machine-readable diagnostics with actionable fix suggestions.

### 22.7.1 Core Diagnostic Types

- [ ] **Implement**: `Fix` type with `message`, `edits`, `applicability`
  - [ ] **Rust Tests**: `ori_diagnostic/src/fix.rs` — Fix type tests

- [ ] **Implement**: `Edit` type with `span`, `replacement`
  - [ ] **Rust Tests**: `ori_diagnostic/src/edit.rs` — Edit type tests

- [ ] **Implement**: `Applicability` enum (MachineApplicable, MaybeIncorrect, HasPlaceholders, Unspecified)
  - [ ] **Rust Tests**: `ori_diagnostic/src/applicability.rs` — applicability tests

- [ ] **Implement**: `SourceLoc` with line/column from byte span
  - [ ] **Rust Tests**: `ori_diagnostic/src/source_loc.rs` — source location tests

### 22.7.2 JSON Output

- [ ] **Implement**: `ori check --json` — Machine-readable output
  - [ ] Diagnostics array with code, severity, message, span, labels, fixes
  - [ ] Summary with error/warning/fixable counts
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — JSON output
  - [ ] **Ori Tests**: `tests/spec/tooling/json_output.ori`

### 22.7.3 Improved Human Output

- [ ] **Implement**: Rust-style diagnostic rendering
  - [ ] Source snippets with line numbers
  - [ ] Primary and secondary labels with arrows
  - [ ] Notes and help sections
  - [ ] **Rust Tests**: `ori_diagnostic/src/render.rs` — diagnostic rendering
  - [ ] **Ori Tests**: `tests/spec/tooling/human_output.ori`

### 22.7.4 Auto-Fix

- [ ] **Implement**: `ori check --fix` — Apply MachineApplicable fixes
  - [ ] Only safe fixes applied automatically
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — auto-fix
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_basic.ori`

- [ ] **Implement**: `ori check --fix --dry` — Preview fixes without applying
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — dry run
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_dry.ori`

- [ ] **Implement**: `ori check --fix=all` — Also apply MaybeIncorrect fixes
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — fix all
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_all.ori`

- [ ] **Implement**: Overlapping fix handling (three-way merge or reject)
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/merge.rs` — fix merging
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_conflict.ori`

### 22.7.5 Fix Categories

- [ ] **Implement**: MachineApplicable fixes for formatting issues
  - [ ] Missing trailing comma
  - [ ] Wrong indentation
  - [ ] Inline comment moved to own line
  - [ ] Extra blank lines
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/formatting.rs` — formatting fixes
  - [ ] **Ori Tests**: `tests/spec/tooling/fix_formatting.ori`

- [ ] **Implement**: MaybeIncorrect fixes for type mismatches
  - [ ] Type conversion suggestions (`int(x)`, `float(x)`)
  - [ ] "Did you mean" for identifiers (Levenshtein distance)
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/suggestions.rs` — suggestions
  - [ ] **Ori Tests**: `tests/spec/tooling/fix_suggestions.ori`

---

## 22.8 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Benchmarks
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: Full tooling support
