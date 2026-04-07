---
section: 22
title: Tooling
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 8
goal: Developer experience
sections:
  - id: "22.1"
    title: Formatter
    status: in-progress
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
    status: in-progress
  - id: "22.8"
    title: WASM Playground
    status: not-started
  - id: "22.9"
    title: Grammar Synchronization Verification
    status: not-started
  - id: "22.10"
    title: Section Completion Checklist
    status: not-started
  - id: "22.11"
    title: Package Management
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

> **CRATE**: `compiler/ori_fmt/` — Width calculation, rendering engine
> **SPEC**: `docs/ori_lang/v2026/spec/annex-d-formatting.md` — formatting spec
> **DOCUMENTATION**: `docs/tooling/formatter/` — User guide, integration, troubleshooting, style guide

**Status**: Partial

### Core Implementation (verified 2026-03-29)

- [x] **Implement**: Width calculation engine — `ori_fmt/src/width/`
  - [x] **Rust Tests**: `ori_fmt/src/width/tests.rs`

- [x] **Implement**: Two-pass rendering engine — `ori_fmt/src/formatter/`
  - [x] Width-based breaking (100 char limit)
  - [x] Always-stacked constructs (run, try, match, parallel, etc.)
  - [x] Independent breaking for nested constructs

- [x] **Implement**: Declaration formatting — `ori_fmt/src/declarations.rs`
  - [x] Functions, types, traits, impls, tests, imports, configs

- [x] **Implement**: Expression formatting — `ori_fmt/src/formatter/`
  - [x] Calls, conditionals, lambdas, binary ops, bindings

- [x] **Implement**: Pattern formatting
  - [x] run, try, match, for patterns

- [x] **Implement**: Collection formatting
  - [x] Lists, maps, tuples, structs, ranges

- [x] **Implement**: Comment preservation — `ori_fmt/src/comments.rs`
  - [x] Doc comment reordering (Description → Param/Field → Warning → Example)
  - [x] @param/@field order matching declaration order

### Layer 4 Rule Integration (Pending) (verified 2026-03-29)

6 of 7 rules have detection/decision infrastructure in `ori_fmt/src/rules/` but are **not wired into the rendering pipeline**. Only `ParenthesesRule` is integrated.

- [x] **Integrated**: `ParenthesesRule` — `needs_parens()` called from `formatter/helpers.rs` at 6 call sites
- [ ] **Wire up**: `ChainedElseIfRule` — chained `else if` always breaks (spec §16 lines 669-679)
- [ ] **Wire up**: `MethodChainRule` — all-or-nothing chain breaking
- [ ] **Wire up**: `BooleanBreakRule` — 3+ `||` clauses break with leading `||`
- [ ] **Wire up**: `ShortBodyRule` — ~20 char threshold for yield/do bodies
- [ ] **Wire up**: `NestedForRule` — Rust-style indentation for nested `for`
- [ ] **Wire up**: `LoopRule` — complex body (block/try/match/for) breaks

### CLI Integration (verified 2026-03-29)

- [x] **Implement**: `ori fmt <file>` — format single file
- [x] **Implement**: `ori fmt <directory>` — format all .ori files recursively
- [x] **Implement**: `ori fmt .` — format current directory (default)
- [x] **Implement**: `ori fmt --check` — check mode (exit 1 if unformatted)
- [x] **Implement**: `ori fmt --diff` — show diff instead of modifying
- [x] **Implement**: `ori fmt --stdin` — read from stdin, write to stdout
- [x] **Implement**: `.orifmtignore` file support with glob patterns [done] (verified 2026-03-28) — `oric/src/commands/fmt/mod.rs:297-313` `load_ignore_patterns()` reads `.orifmtignore`, supports glob patterns, comment lines, blank line skipping
- [x] **Implement**: `ori fmt --no-ignore` — format everything [done] (verified 2026-03-28) — `oric/src/commands/fmt/mod.rs:440` `--no-ignore` flag parsed, skips loading patterns when `config.no_ignore`
- [x] **Implement**: Error messages with source snippets and suggestions

### Performance (verified 2026-03-29)

- [x] **Implement**: Incremental formatting — `ori_fmt/src/incremental.rs`
- [x] **Implement**: Parallel file processing via rayon
- [x] **Implement**: Memory-efficient large file handling

### User Intent Preservation (Design Needed)

When the user deliberately writes a stacked/multi-line layout that happens to fit within 100 chars, the formatter should **not** collapse it to inline. Width should be a lower priority than user intent.

**Approach**: Detect user-intended breaks via span gaps — if there's a newline in the source between two parts of a construct (e.g., between `then "A"` and `else`), treat the construct as "user-stacked" and preserve the broken layout regardless of width. The formatter already has spans on every expression; checking `source[expr1.span.end..keyword.span.start].contains('\n')` would detect this.

**Key scenarios to cover**:
- Chained `if/else if` that fits on one line but user wrote multi-line
- Method chains that fit but user broke at each `.`
- Function calls with args that fit but user put one-per-line
- Binary expressions that fit but user broke before operators
- Any construct where "fits" ≠ "readable"

**Not yet designed**: Need to distinguish "user deliberately broke this" from "editor auto-wrapped" or "pasted with random formatting". Idempotence must hold — running `ori fmt` twice must produce the same output.

**QA**: ~400 additional scenarios needed to cover edge cases from the syntax revamp and intent preservation.

### Testing (verified 2026-03-29)

- [x] **Rust Tests**: unit, golden, idempotence, property, incremental — 171 tests pass
- [x] **Golden Tests**: `tests/fmt/` — 100+ `.ori` files covering declarations, expressions, patterns, collections, comments, edge-cases

> **WEAK TESTS**: No Ori spec tests for formatter. `tests/spec/tooling/` directory does not exist. All testing is Rust-side only.

---

## 22.2 LSP Server (verified 2026-03-29)

> **DETAILED PLAN**: `plans/ori_lsp/` — Phased implementation with tracking
> **PROPOSAL**: `proposals/approved/lsp-implementation-proposal.md` — Architecture decisions
> **CRATE**: `compiler/ori_lsp/` — LSP server implementation (does not exist yet)
>
> **NOTE**: `tools/ori-lsp/` exists as a standalone prototype but is fully excluded from the workspace. It imports from old APIs (`oric::ast`, `oric::type_check`, `oric::format::format`) that no longer exist. This prototype is non-functional dead code.

### Formatting (from ori_fmt Section 7.2)

- [ ] **Implement**: `textDocument/formatting` request handler
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/formatting.rs` — document formatting
- [ ] **Implement**: Return TextEdit array for changes
- [ ] **Implement**: `textDocument/rangeFormatting` request handler
  - [ ] Expand range to nearest complete construct
- [ ] **Implement**: Register format-on-save capability
- [ ] **Document**: Editor integration (VS Code, Neovim, Helix, etc.)
  - [ ] Documented in `docs/tooling/formatter/integration.md` (for CLI workaround)

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

- [ ] **Implement**: Find references — LSP `textDocument/references` handler in `ori_lsp`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/references.rs` — find references
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_references.ori`

- [ ] **Implement**: Hover information — LSP `textDocument/hover` with type signatures and docs in `ori_lsp`
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/hover.rs` — hover information
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_hover.ori`

- [ ] **Implement**: Completions
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/completion.rs` — completions
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_completions.ori`

- [ ] **Implement**: Diagnostics
  - [ ] **Rust Tests**: `ori_lsp/src/handlers/diagnostics.rs` — LSP diagnostics
  - [ ] **Ori Tests**: `tests/spec/tooling/lsp_diagnostics.ori`

---

## 22.3 Edit Operations (verified 2026-03-29)

- [ ] **Implement**: `set`, `add`, `remove`, `rename`, `move` — design/12-tooling/index.md:56-62
  - [ ] **Rust Tests**: `oric/src/edit/edit_ops.rs` — edit operations
  - [ ] **Ori Tests**: `tests/spec/tooling/edit_ops.ori`

---

## 22.4 REPL (verified 2026-03-29)

- [ ] **Implement**: REPL interactive evaluation — `ori repl` command with expression eval and result display
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs` — REPL evaluation
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

- [ ] **Implement**: History and completion
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs` — history/completion
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

- [ ] **Implement**: Multi-line input
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs` — multi-line input
  - [ ] **Ori Tests**: `tests/spec/tooling/repl.ori`

---

## 22.5 Test Runner (verified 2026-03-29)

> **NOTE**: This section covers the TEST RUNNER CLI commands, which are largely complete.
> The TESTING FRAMEWORK features (configurable test enforcement, dependency-aware execution, incremental tests)
> are in Section 14 and are not yet implemented. The test runner runs tests; the framework enforces
> testing requirements (when enabled) and manages test dependencies.

- [x] **Implement**: `ori test` command — run all tests — design/11-testing/index.md [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `oric/src/commands/test.rs`, `oric/src/test/runner/mod.rs` — TestRunner, TestRunnerConfig, TestSummary, FileSummary, TestOutcome all implemented. Runs 4181 tests.
  - [ ] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori test file.test.ori` — run specific test file [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `main.rs:138` — path argument parsed
  - [ ] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori test path/` — run all tests in directory [done] (verified 2026-03-28)
  - [x] **Rust Tests**: Directory scanning functional
  - [ ] **Ori Tests**: `tests/spec/tooling/test_runner.ori`

- [x] **Implement**: `ori check file.ori` — check test coverage without running — spec/19-testing.md § Coverage Enforcement [done] (verified 2026-03-28)
  - [x] **Rust Tests**: Type-checks and reports errors; test coverage via `--test-enforcement=off|warn|error`
  - [ ] **Ori Tests**: `tests/spec/tooling/test_check.ori`
  - Note: No `--json` flag yet

- [x] **Implement**: Parallel test execution — spec/19-testing.md § Test Isolation [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `config.parallel = true` by default; rayon-based. `--no-parallel` flag to disable.
  - [ ] **Ori Tests**: `tests/spec/tooling/test_parallel.ori`

- [x] **Implement**: Test filtering by name pattern (e.g., `ori test --filter "auth"`) [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `oric/src/test/runner/tests.rs:75` — `test_runner_filter` (substring match on test names)
  - [ ] **Ori Tests**: `tests/spec/tooling/test_filter.ori`

- [x] **Implement**: Test output formatting (pass/fail/skip counts, timing) [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `print_summary_stats()`, `print_file_results()`, `print_file_errors()` — LLVM compile fail breakdown
  - [ ] **Ori Tests**: `tests/spec/tooling/test_output.ori`

- [x] **Implement**: Verbose mode for detailed output (`--verbose`) [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `--verbose` / `-v` flag shows PASS/SKIP/LLVM COMPILE FAIL per test
  - [ ] **Ori Tests**: `tests/spec/tooling/test_output.ori`

- [x] **Implement**: Coverage report generation [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `--coverage` flag, `coverage_report()` returns `CoverageReport` with covered/uncovered function lists. Test at `oric/src/test/result/tests.rs:128`.
  - [ ] **Ori Tests**: `tests/spec/tooling/test_coverage.ori`

- [x] **Implement**: Exit codes (0 = all pass, 1 = failures, 2 = no tests found) [done] (verified 2026-03-28)
  - [x] **Rust Tests**: `oric/src/test/result/tests.rs:42` — `test_summary_exit_code`
  - [ ] **Ori Tests**: `tests/spec/tooling/test_exit.ori`

> **WEAK TESTS**: No Ori-side spec tests exist. All 11 planned `tests/spec/tooling/test_*.ori` files are unchecked. `tests/spec/tooling/` directory does not exist. Rust-side testing is solid (5 runner tests, 8 result tests) but no end-to-end Ori tests verify CLI commands.

---

## 22.6 Causality Tracking (verified 2026-03-29)

> **PROPOSAL**: `proposals/approved/why-command-proposal.md`

Expose Salsa's dependency tracking to users for debugging and impact analysis.

### 22.6.1 `ori impact` Command

- [ ] **Implement**: `ori impact @target` — Show blast radius of potential change
  - [ ] Direct dependents (functions that call target)
  - [ ] Transitive dependents (recursive callers)
  - [ ] Summary count of affected functions
  - [ ] **Rust Tests**: `oric/src/commands/impact.rs` — impact command
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_basic.ori`

- [ ] **Implement**: `ori impact @target --verbose` — Show call sites
  - [ ] **Rust Tests**: `oric/src/commands/impact.rs` — verbose output
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_verbose.ori`

- [ ] **Implement**: `ori impact @Type` — Impact analysis for type changes
  - [ ] Functions using type, returning type, accepting type
  - [ ] **Rust Tests**: `oric/src/commands/impact.rs` — type impact
  - [ ] **Ori Tests**: `tests/spec/tooling/impact_type.ori`

### 22.6.2 `ori why` Command

- [ ] **Implement**: `ori why @target` — Trace why target is dirty/broken
  - [ ] Show causality chain from changed input to target
  - [ ] Concise output by default
  - [ ] **Rust Tests**: `oric/src/commands/why.rs` — why command
  - [ ] **Ori Tests**: `tests/spec/tooling/why_basic.ori`

- [ ] **Implement**: `ori why @target --verbose` — Detailed causality chain
  - [ ] Full path through dependency graph
  - [ ] Source locations for each change
  - [ ] **Rust Tests**: `oric/src/commands/why.rs` — verbose causality
  - [ ] **Ori Tests**: `tests/spec/tooling/why_verbose.ori`

- [ ] **Implement**: `ori why @target --diff` — Show actual code changes
  - [ ] **Rust Tests**: `oric/src/commands/why.rs` — diff output
  - [ ] **Ori Tests**: `tests/spec/tooling/why_diff.ori`

- [ ] **Implement**: `ori why @target --graph` — Tree visualization
  - [ ] **Rust Tests**: `oric/src/commands/why.rs` — graph visualization
  - [ ] **Ori Tests**: `tests/spec/tooling/why_graph.ori`

### 22.6.3 Test Runner Integration

- [ ] **Implement**: Hint on test failure suggesting `ori why`
  - [ ] "Hint: This test is dirty due to changes in @X"
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — why hint
  - [ ] **Ori Tests**: `tests/spec/tooling/why_hint.ori`

---

## 22.7 Structured Diagnostics (verified 2026-03-29)

> **PROPOSAL**: `proposals/approved/structured-diagnostics-autofix.md`

Machine-readable diagnostics with actionable fix suggestions. Enables AI agents to programmatically consume errors and auto-fix safe issues.

**Existing Infrastructure:** Core types (`Applicability`, `Suggestion`, `Substitution`) already exist in `ori_diagnostic/src/diagnostic.rs`. This section enhances the JSON emitter and adds CLI flags for auto-fix.

### 22.7.0 Error Code Registry Centralization (Step 0) (verified 2026-03-29)

Currently `ErrorCode` has three manually-synchronized representations: the enum definition, `as_str()`, and `parse_error_code()` in `oric`. Adding a new error code requires updating all three plus the `DOCS` array — and forgetting one (as happened with E2019) silently breaks `ori explain`.

- [x] **Implement**: `ErrorCode::from_str()` in `ori_diagnostic/src/error_code/mod.rs` [done] (verified 2026-03-28)
  - [x] Reverse lookup from string (e.g., `"E2019"`) to `ErrorCode` variant
  - [x] Case-insensitive matching (e.g., `"e2019"` works)
  - [x] Returns `Option<ErrorCode>` (via `Result`)
  - [x] Uses `ErrorCode::ALL` + `as_str()` for automatic exhaustiveness
  - [x] **Rust Tests**: `test_from_str_round_trip`, `test_from_str_case_insensitive`, `test_from_str_unknown`

- [x] **Implement**: `ErrorCode::all()` iterator in `ori_diagnostic/src/error_code/mod.rs` [done] (verified 2026-03-28)
  - [x] `ErrorCode::ALL: &[ErrorCode]` at line 49, `ErrorCode::COUNT: usize` at line 52
  - [x] **Rust Tests**: `test_all_is_complete` (123 variants, duplicate check), `test_all_variants_classified` (exhaustive predicate check)

- [x] **Refactor**: Remove `parse_error_code()` from `oric/src/commands/explain.rs` [done] (verified 2026-03-28)
  - [x] `explain.rs` uses `code_str.parse::<ErrorCode>()` directly (line 7)
  - [x] No `parse_error_code` function exists anywhere in `oric/`
  - [ ] **Rust Tests**: `oric/tests/phases/` — `ori explain E2019` integration test

- [ ] **Implement**: Compile-time completeness check for `ErrorDocs`
  - [ ] Test that iterates `ErrorCode::all()` and warns on missing docs
  - [ ] Explicit opt-out list for codes without docs yet (E4xxx, E5xxx, E6xxx)
  - [ ] **Rust Tests**: `ori_diagnostic/src/errors/tests.rs` — completeness check

### 22.7.1 SourceLoc Type (Step 1) (verified 2026-03-29)

- [x] **Implement**: `SourceLoc` struct with line/column from byte span [done] (verified 2026-03-28) — implemented as `offset_to_line_col()` in `ori_diagnostic/src/span_utils/mod.rs`
  - [x] 1-based line and column numbers
  - [x] Unicode codepoint column (not byte offset)
  - [x] **Rust Tests**: `span_utils/tests.rs` — 6+ tests

- [x] **Implement**: Line index builder for efficient lookups [done] (verified 2026-03-28) — `LineOffsetTable::build(source)`
  - [x] Build line offset table from source text
  - [x] O(log n) span-to-location conversion via binary search on line offsets
  - [x] **Rust Tests**: `test_line_offset_table_*` tests

### 22.7.2 JSON Output Enhancement (Step 2) (verified 2026-03-29)

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

### 22.7.3 Improved Human Output (Step 3) (verified 2026-03-29)

- [ ] **Implement**: Rust-style diagnostic rendering
  - [ ] Source snippets with line numbers
  - [ ] Primary and secondary labels with underline arrows
  - [ ] Notes and help sections
  - [ ] "fix available" indicator for fixable diagnostics
  - [ ] **Rust Tests**: `ori_diagnostic/src/emitter/terminal.rs` — rendering tests
  - [ ] **Ori Tests**: `tests/spec/tooling/human_output.ori`

### 22.7.4 Auto-Fix Infrastructure (Step 4) (verified 2026-03-29)

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

### 22.7.5 Upgrade Existing Diagnostics (Step 5) (verified 2026-03-29)

> **Hygiene note:** `Diagnostic` currently has dual suggestion fields: `suggestions: Vec<String>` (~53 callers of `with_suggestion()`) and `structured_suggestions: Vec<Suggestion>` (~7 callers). Emitters must check both. After migrating all callers below, remove the `suggestions` field and `with_suggestion()` method entirely, leaving `structured_suggestions` as the single path.

- [ ] **Implement**: Convert type error suggestions to structured suggestions
  - [ ] Type conversion: `x as int`, `x as float`
  - [ ] Missing wrapper: `Some(x)`, `Ok(x)`
  - [ ] Assign `MaybeIncorrect` applicability
  - [ ] **Rust Tests**: `oric/src/reporting/typeck/tests.rs` — structured suggestions

- [ ] **Implement**: Convert pattern validation suggestions to structured suggestions
  - [ ] Unknown argument typos
  - [ ] Missing required arguments
  - [ ] **Rust Tests**: `ori_patterns/src/validation.rs` — structured suggestions

- [ ] **Implement**: Convert parser error suggestions to structured suggestions
  - [ ] Missing delimiters
  - [ ] Expected token fixes
  - [ ] **Rust Tests**: `ori_parse/src/error.rs` — structured suggestions

### 22.7.6 Extended Fixes (Step 6) (verified 2026-03-29)

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

## 22.8 WASM Playground (verified 2026-03-29)

> **PROPOSAL**: `proposals/approved/wasm-playground-proposal.md`
> **CRATE**: `playground/wasm/` — WASM bindings for portable compiler subset (does not exist yet)

**Status**: Not started

### Core Implementation

- [ ] **Implement**: WASM crate with `run_ori()`, `format_ori()`, `version()` exports
- [ ] **Implement**: Monaco editor integration with Ori syntax highlighting (Monarch grammar)
- [ ] **Implement**: URL-based code sharing (base64 fragment)
- [ ] **Implement**: Full-screen playground page (`/playground`)
- [ ] **Implement**: Embedded playground on landing page
- [ ] **Implement**: Basic examples (5): Hello World, Fibonacci, Factorial, List Operations, Structs

### Pending Work

- [ ] **Implement**: Extended examples (5): Sum Types, Error Handling, Iterators, Traits, Generics
  - [ ] **Ori Tests**: Example code must compile and run correctly
- [ ] **Document**: Stdlib availability and limitations (prelude only)

---

## 22.9 Grammar Synchronization Verification (verified 2026-03-29)

> **PROPOSAL**: `proposals/approved/grammar-sync-formalization-proposal.md`

Enhance `sync-grammar` skill with operator verification checklist to catch discrepancies between grammar.ebnf and parser implementation.

### 22.9.1 Enhance sync-grammar Skill

- [ ] **Implement**: Add operator verification checklist to `.claude/commands/sync-grammar.md`
  - [ ] Checklist for each grammar operator: lexer, AST, parser, typeck, eval
  - [ ] Precedence chain verification checklist
  - [ ] Test coverage section listing operators with/without tests

- [ ] **Document**: Verification process in sync-grammar skill
  - [ ] Output format specification
  - [ ] Steps for verifying new operators

---

## 22.11 Package Management (verified 2026-03-29)

> **DETAILED PLAN**: `plans/pkg_mgmt/` — Phased implementation with tracking
> **DESIGN**: `plans/pkg_mgmt/design.md` — Full specification

Package management for Ori projects with registry support.

### Core Features

- [ ] **Implement**: Manifest parsing (`oripk.toml`)
  - [ ] Package metadata, dependencies, features
  - [ ] **Rust Tests**: `ori_pkg/src/manifest_tests.rs`

- [ ] **Implement**: Lock file (`oripk.lock`)
  - [ ] Checksum-based integrity verification
  - [ ] **Rust Tests**: `ori_pkg/src/lock_tests.rs`

- [ ] **Implement**: Version resolution — exact-version-only dependency solving in `ori_pkg`
  - [ ] Exact versions only, single version policy
  - [ ] **Rust Tests**: `ori_pkg/src/resolution_tests.rs`

- [ ] **Implement**: Package cache — global download cache at `~/.ori/cache/` in `ori_pkg`
  - [ ] Global cache at `~/.ori/cache/`
  - [ ] **Rust Tests**: `ori_pkg/src/cache_tests.rs`

### CLI Commands

- [ ] **Implement**: `ori install` — Install dependencies
- [ ] **Implement**: `ori add <pkg>` — Add a dependency
- [ ] **Implement**: `ori remove <pkg>` — Remove a dependency
- [ ] **Implement**: `ori upgrade` — Upgrade dependencies
- [ ] **Implement**: `ori sync` — Sync to lock file
- [ ] **Implement**: `ori check` — Verify dependencies
- [ ] **Implement**: `ori publish` — Publish to registry
- [ ] **Implement**: `ori search <query>` — Search registry

### Registry

- [ ] **Implement**: Registry protocol (Cloudflare-based)
- [ ] **Implement**: Registry client — HTTP client for package registry API in `ori_pkg`
- [ ] **Deploy**: Production registry infrastructure

---

## 22.10 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage
- [ ] Benchmarks
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Full tooling support
