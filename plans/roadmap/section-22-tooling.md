---
section: 22
title: Tooling
status: in-progress
tier: 8
goal: Developer experience
sections:
  - id: "22.1"
    title: Formatter
    status: complete
  - id: "22.2"
    title: LSP Server
    status: not-started
  - id: "22.3"
    title: Edit Operations
    status: not-started
  - id: "22.4"
    title: REPL
    status: not-started
  - id: "22.5"
    title: Test Runner
    status: complete
  - id: "22.6"
    title: Causality Tracking
    status: not-started
  - id: "22.7"
    title: Structured Diagnostics
    status: not-started
  - id: "22.8"
    title: WASM Playground
    status: in-progress
  - id: "22.9"
    title: Grammar Synchronization Verification
    status: not-started
  - id: "22.10"
    title: Section Completion Checklist
    status: not-started
---

# Section 22: Tooling

**Goal**: Developer experience

> **DESIGN**: `design/12-tooling/index.md`
> **PROPOSALS**:
> - `proposals/approved/why-command-proposal.md` — Causality tracking (`ori impact`, `ori why`)
> - `proposals/approved/structured-diagnostics-autofix.md` — JSON output and auto-fix

---

## 22.1 Formatter

> **DETAILED PLAN**: `plans/ori_fmt/` — Phased implementation with tracking
> **CRATE**: `compiler/ori_fmt/` — Width calculation, rendering engine
> **DOCUMENTATION**: `docs/tooling/formatter/` — User guide, integration, troubleshooting, style guide

**Status**: ✅ Complete (CLI), 🔶 Partial (LSP/WASM in 22.2)

### Core Implementation (Complete)

- [x] **Implement**: Width calculation engine — `ori_fmt/src/width/`
  - [x] **Rust Tests**: `ori_fmt/src/width/tests.rs` — 962 tests passing

- [x] **Implement**: Two-pass rendering engine — `ori_fmt/src/formatter/`
  - [x] Width-based breaking (100 char limit)
  - [x] Always-stacked constructs (run, try, match, parallel, etc.)
  - [x] Independent breaking for nested constructs

- [x] **Implement**: Declaration formatting — `ori_fmt/src/declarations.rs`
  - [x] Functions, types, traits, impls, tests, imports, configs

- [x] **Implement**: Expression formatting — `ori_fmt/src/formatter/`
  - [x] Calls, chains, conditionals, lambdas, binary ops, bindings

- [x] **Implement**: Pattern formatting
  - [x] run, try, match, for patterns

- [x] **Implement**: Collection formatting
  - [x] Lists, maps, tuples, structs, ranges

- [x] **Implement**: Comment preservation — `ori_fmt/src/comments.rs`
  - [x] Doc comment reordering (Description → Param/Field → Warning → Example)
  - [x] @param/@field order matching declaration order

### CLI Integration (Complete)

- [x] **Implement**: `ori fmt <file>` — format single file
- [x] **Implement**: `ori fmt <directory>` — format all .ori files recursively
- [x] **Implement**: `ori fmt .` — format current directory (default)
- [x] **Implement**: `ori fmt --check` — check mode (exit 1 if unformatted)
- [x] **Implement**: `ori fmt --diff` — show diff instead of modifying
- [x] **Implement**: `ori fmt --stdin` — read from stdin, write to stdout
- [x] **Implement**: `.orifmtignore` file support with glob patterns
- [x] **Implement**: `ori fmt --no-ignore` — format everything
- [x] **Implement**: Error messages with source snippets and suggestions

### Performance (Complete)

- [x] **Implement**: Incremental formatting — `ori_fmt/src/incremental.rs`
  - [x] 13 integration tests, ~30% speedup for large files
- [x] **Implement**: Parallel file processing via rayon (2.4x speedup)
- [x] **Implement**: Memory-efficient large file handling (10k lines in 2.75ms)

### Testing (Complete)

- [x] **Rust Tests**: 440 total (215 unit, 35 golden, 5 idempotence, 171 property, 13 incremental, 1 doc)
- [x] **Golden Tests**: `tests/fmt/` — declarations, expressions, patterns, collections, comments, edge-cases

---

## 22.2 LSP Server

> **DETAILED PLAN**: `plans/ori_lsp/` — Phased implementation with tracking
> **PROPOSAL**: `proposals/approved/lsp-implementation-proposal.md` — Architecture decisions
> **CRATE**: `compiler/ori_lsp/` — LSP server implementation

### Formatting (from ori_fmt Section 7.2)

- [ ] **Implement**: `textDocument/formatting` request handler
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — document formatting
- [ ] **Implement**: Return TextEdit array for changes
- [ ] **Implement**: `textDocument/rangeFormatting` request handler
  - [ ] Expand range to nearest complete construct
- [ ] **Implement**: Register format-on-save capability
- [ ] **Document**: Editor integration (VS Code, Neovim, Helix, etc.)
  - [x] Documented in `docs/tooling/formatter/integration.md` (for CLI workaround)

### Core LSP Features

- [ ] **Implement**: Semantic addressing — design/12-tooling/index.md:25-35
  - [ ] **Rust Tests**: `ori_lsp/src/analysis/addressing.rs` — semantic addressing
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_addressing.ori`

- [ ] **Implement**: Structured errors — design/12-tooling/index.md:36-55
  - [ ] **Rust Tests**: `ori_lsp/src/diagnostics/mod.rs` — structured errors
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_errors.ori`

- [ ] **Implement**: Go to definition
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/definition.rs` — go to definition
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_goto.ori`

- [ ] **Implement**: Find references
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — find references
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_references.ori`

- [ ] **Implement**: Hover information
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — hover information
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_hover.ori`

- [ ] **Implement**: Completions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — completions
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_completions.ori`

- [ ] **Implement**: Diagnostics
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/diagnostics.rs` — LSP diagnostics
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
> are in Section 14 and are not yet implemented. The test runner runs tests; the framework enforces
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

Machine-readable diagnostics with actionable fix suggestions. Enables AI agents to programmatically consume errors and auto-fix safe issues.

**Existing Infrastructure:** Core types (`Applicability`, `Suggestion`, `Substitution`) already exist in `ori_diagnostic/src/diagnostic.rs`. This section enhances the JSON emitter and adds CLI flags for auto-fix.

### 22.7.1 SourceLoc Type (Step 1)

- [ ] **Implement**: `SourceLoc` struct with line/column from byte span
  - [ ] 1-based line and column numbers
  - [ ] Unicode codepoint column (not byte offset)
  - [ ] **Rust Tests**: `ori_diagnostic/src/span_utils.rs` — source location tests

- [ ] **Implement**: Line index builder for efficient lookups
  - [ ] Build line offset table from source text
  - [ ] O(log n) span-to-location conversion
  - [ ] **Rust Tests**: `ori_diagnostic/src/span_utils.rs` — line index tests

### 22.7.2 JSON Output Enhancement (Step 2)

- [ ] **Implement**: Add file path to JSON diagnostic output
  - [ ] File path at diagnostic level
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/json.rs` — file path tests

- [ ] **Implement**: Add `start_loc`/`end_loc` to span serialization
  - [ ] Line/column for span start and end
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/json.rs` — location tests

- [ ] **Implement**: Add `structured_suggestions` to JSON output
  - [ ] Include message, substitutions, applicability
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/json.rs` — suggestions tests

- [ ] **Implement**: Add summary object to JSON output
  - [ ] Error count, warning count, fixable count
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/json.rs` — summary tests

- [ ] **Implement**: `ori check --json` CLI flag
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — JSON flag
  - [ ] **Ori Tests**: `tests/spec/tooling/json_output.ori`

### 22.7.3 Improved Human Output (Step 3)

- [ ] **Implement**: Rust-style diagnostic rendering
  - [ ] Source snippets with line numbers
  - [ ] Primary and secondary labels with underline arrows
  - [ ] Notes and help sections
  - [ ] "fix available" indicator for fixable diagnostics
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/terminal.rs` — rendering tests
  - [ ] **Ori Tests**: `tests/spec/tooling/human_output.ori`

### 22.7.4 Auto-Fix Infrastructure (Step 4)

- [ ] **Implement**: `apply_suggestions()` function
  - [ ] Apply substitutions to source text
  - [ ] Return modified source or list of changes
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/apply.rs` — apply tests

- [ ] **Implement**: Overlapping substitution handling
  - [ ] Detect overlapping spans
  - [ ] Reject conflicting fixes (error message)
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/apply.rs` — conflict tests

- [ ] **Implement**: `ori check --fix` — Apply MachineApplicable fixes
  - [ ] Only safe fixes applied automatically
  - [ ] Report number of fixes applied
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — auto-fix
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_basic.ori`

- [ ] **Implement**: `ori check --fix --dry` — Preview fixes without applying
  - [ ] Show diff of what would change
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — dry run
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_dry.ori`

- [ ] **Implement**: `ori check --fix=all` — Also apply MaybeIncorrect fixes
  - [ ] Include MaybeIncorrect suggestions
  - [ ] Warn user to review changes
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — fix all
  - [ ] **Ori Tests**: `tests/spec/tooling/autofix_all.ori`

### 22.7.5 Upgrade Existing Diagnostics (Step 5)

- [ ] **Implement**: Convert type error suggestions to structured suggestions
  - [ ] Type conversion: `x as int`, `x as float`
  - [ ] Missing wrapper: `Some(x)`, `Ok(x)`
  - [ ] Assign `MaybeIncorrect` applicability
  - [ ] **Rust Tests**: `oric/src/reporting/type_errors.rs` — structured suggestions

- [ ] **Implement**: Convert pattern validation suggestions to structured suggestions
  - [ ] Unknown argument typos
  - [ ] Missing required arguments
  - [ ] **Rust Tests**: `ori_patterns/src/validation.rs` — structured suggestions

- [ ] **Implement**: Convert parser error suggestions to structured suggestions
  - [ ] Missing delimiters
  - [ ] Expected token fixes
  - [ ] **Rust Tests**: `ori_parse/src/error.rs` — structured suggestions

### 22.7.6 Extended Fixes (Step 6)

- [ ] **Implement**: Typo detection for identifiers (Levenshtein distance)
  - [ ] "Did you mean `similar_name`?" suggestions
  - [ ] Threshold for similarity (e.g., edit distance ≤ 2)
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/typo.rs` — typo detection

- [ ] **Implement**: MachineApplicable fixes for formatting issues
  - [ ] Missing trailing comma
  - [ ] Wrong indentation (fix to 4 spaces)
  - [ ] Inline comment moved to own line
  - [ ] Extra blank lines removed
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/formatting.rs` — formatting fixes
  - [ ] **Ori Tests**: `tests/spec/tooling/fix_formatting.ori`

- [ ] **Implement**: Import suggestions for unknown types
  - [ ] Search stdlib for matching type names
  - [ ] Suggest `use std.module { Type }`
  - [ ] **Rust Tests**: `ori_diagnostic/src/fixes/imports.rs` — import suggestions

---

## 22.8 WASM Playground

> **PROPOSAL**: `proposals/approved/wasm-playground-proposal.md`
> **CRATE**: `playground/wasm/` — WASM bindings for portable compiler subset

**Status**: ✅ Core complete, 🔶 Examples pending

### Core Implementation (Complete)

- [x] **Implement**: WASM crate with `run_ori()`, `format_ori()`, `version()` exports
- [x] **Implement**: Monaco editor integration with Ori syntax highlighting (Monarch grammar)
- [x] **Implement**: URL-based code sharing (base64 fragment)
- [x] **Implement**: Full-screen playground page (`/playground`)
- [x] **Implement**: Embedded playground on landing page
- [x] **Implement**: Basic examples (5): Hello World, Fibonacci, Factorial, List Operations, Structs

### Pending Work

- [ ] **Implement**: Extended examples (5): Sum Types, Error Handling, Iterators, Traits, Generics
  - [ ] **Ori Tests**: Example code must compile and run correctly
- [ ] **Document**: Stdlib availability and limitations (prelude only)

---

## 22.9 Grammar Synchronization Verification

> **PROPOSAL**: `proposals/approved/grammar-sync-formalization-proposal.md`

Enhance `sync-grammar` skill with operator verification checklist to catch discrepancies between grammar.ebnf and parser implementation.

### 22.8.1 Enhance sync-grammar Skill

- [ ] **Implement**: Add operator verification checklist to `.claude/commands/sync-grammar.md`
  - [ ] Checklist for each grammar operator: lexer, AST, parser, typeck, eval
  - [ ] Precedence chain verification checklist
  - [ ] Test coverage section listing operators with/without tests

- [ ] **Document**: Verification process in sync-grammar skill
  - [ ] Output format specification
  - [ ] Steps for verifying new operators

---

## 22.10 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Benchmarks
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Full tooling support
