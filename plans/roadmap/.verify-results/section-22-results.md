# Section 22: Tooling -- Verification Results

**Verified**: 2026-03-28
**Status in roadmap**: in-progress
**Actual status**: PARTIAL -- Formatter is substantially complete with 575+ passing Rust tests and 151 golden tests; Test Runner CLI is largely complete; LSP/REPL/Causality/Diagnostics enhancements/Package Management are not started; several `[ ]` items are actually done.

## Test Execution

- `cargo test -p ori_fmt`: 575 passed, 0 failed, 7 ignored doc tests (all OK)
  - 346 unit tests (lib), 36 golden, 5 property, 13 idempotence, 171 incremental, 4 width
- `cargo st tests/fmt/`: 4181 passed globally, 42 skipped (formatter golden tests included)
- `cargo test -p ori_diagnostic`: 120 passed, 0 failed, 1 ignored
- `ori fmt --check`, `ori fmt --diff`, `ori fmt --stdin`: all functional
- `ori test --filter=`, `--verbose`, `--no-parallel`, `--coverage`: all functional

---

## 22.1 Formatter

### Core Implementation (all [x] items)

- [done] Width calculation engine -- `ori_fmt/src/width/` with 6 submodules and `width/tests.rs`
  - Tests: `width/operators/tests.rs`, `width/literals/tests.rs`, `width/compounds/tests.rs`, `width/helpers/tests.rs`, `width/patterns/tests.rs`
  - VERIFIED: All tests pass

- [done] Two-pass rendering engine -- `ori_fmt/src/formatter/`
  - Width-based breaking (100 char limit), always-stacked constructs, independent breaking
  - Files: `formatter/mod.rs`, `formatter/helpers.rs`, `formatter/inline.rs`, `formatter/broken.rs`, `formatter/stacked.rs`, `formatter/literals.rs`, `formatter/patterns.rs`
  - Tests: `formatter/tests.rs`
  - VERIFIED: All tests pass

- [done] Declaration formatting -- `ori_fmt/src/declarations/`
  - Files: `functions.rs`, `types.rs`, `traits.rs`, `impls.rs`, `imports.rs`, `configs.rs`, `extern_def.rs`, `extends.rs`, `def_impls.rs`, `comments.rs`, `tests_fmt.rs`, `parsed_types/`
  - VERIFIED: All tests pass

- [done] Expression formatting -- `ori_fmt/src/formatter/`
  - Calls, conditionals, lambdas, binary ops, bindings in inline/broken/stacked formatters
  - VERIFIED: All tests pass

- [done] Pattern formatting -- `ori_fmt/src/formatter/patterns.rs`
  - run, try, match, for patterns
  - VERIFIED: All tests pass

- [done] Collection formatting
  - Lists, maps, tuples, structs, ranges in formatter/inline.rs + broken.rs
  - VERIFIED: All tests pass

- [done] Comment preservation -- `ori_fmt/src/comments/mod.rs`
  - Doc comment reordering (Description/Param/Field/Warning/Example), order matching declaration order
  - Tests: `comments/tests.rs`
  - VERIFIED: All tests pass

### Layer 4 Rule Integration

- [done] `ParenthesesRule` -- `needs_parens()` called from `formatter/helpers.rs` (lines 69, 84, 102, 117, 135, 150)
  - VERIFIED: Integrated and tested

- [todo] `ChainedElseIfRule` -- detection logic in `rules/chained_else_if.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

- [todo] `MethodChainRule` -- detection logic in `rules/method_chain.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

- [todo] `BooleanBreakRule` -- detection logic in `rules/boolean_break.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

- [todo] `ShortBodyRule` -- detection logic in `rules/short_body.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

- [todo] `NestedForRule` -- detection logic in `rules/nested_for.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

- [todo] `LoopRule` -- detection logic in `rules/loop_rule.rs` but NOT called from rendering pipeline
  - STALE CLAIM: roadmap correctly marks `[ ]`

### CLI Integration

- [done] `ori fmt <file>` -- functional, verified
- [done] `ori fmt <directory>` -- functional, verified
- [done] `ori fmt .` -- functional, verified
- [done] `ori fmt --check` -- returns exit 1 if unformatted, verified
- [done] `ori fmt --diff` -- shows diff output, verified
- [done] `ori fmt --stdin` -- reads stdin, writes stdout, verified

- STALE ROADMAP: `.orifmtignore` marked `[ ]` but IS implemented
  - `compiler/oric/src/commands/fmt/mod.rs:297-313` -- `load_ignore_patterns()` reads `.orifmtignore`
  - `compiler/oric/src/commands/fmt/mod.rs:316` -- `is_ignored()` checks patterns
  - Supports glob patterns, comment lines (`#`), blank line skipping

- STALE ROADMAP: `--no-ignore` marked `[ ]` but IS implemented
  - `compiler/oric/src/commands/fmt/mod.rs:440` -- `--no-ignore` flag parsed
  - `compiler/oric/src/commands/fmt/mod.rs:242` -- skips loading patterns when `config.no_ignore`

- [done] Error messages with source snippets and suggestions -- `compiler/oric/src/commands/fmt/diagnostics.rs`

### Performance

- [done] Incremental formatting -- `ori_fmt/src/incremental/mod.rs`
  - Tests: `incremental/tests.rs` (171 tests)
  - VERIFIED: All tests pass

- [done] Parallel file processing via rayon -- `compiler/oric/src/commands/fmt/mod.rs:23` (`use rayon::prelude::*`)
  - `files.par_iter().for_each(...)` at line 265
  - VERIFIED: Implementation exists

- [done] Memory-efficient large file handling
  - Region-based formatting in incremental module
  - VERIFIED: Implementation exists

### User Intent Preservation

- [todo] Not implemented (design documented in roadmap but no code)

### Testing

- [done] Rust Tests: 346 unit + 5 property + 13 idempotence + 171 incremental + 4 width = 539 lib tests
- [done] Golden Tests: 151 `.ori` files in `tests/fmt/`
  - Categories: collections, comments, declarations, edge-cases, expressions, patterns

### Spec/Docs

- [done] Spec: `docs/ori_lang/v2026/spec/annex-d-formatting.md` EXISTS
- [done] Docs: `docs/tooling/formatter/` with user-guide.md, style-guide.md, integration.md, troubleshooting.md, design/

### Matrix Coverage Assessment

- Width calculations: GOOD -- separate tests for operators, literals, compounds, helpers, patterns
- Rendering: GOOD -- inline/broken/stacked paths tested via golden tests
- Rules: PARTIAL -- rule detection tested in `rules/tests.rs` but 6/7 rules not integrated so rendering-level testing is irrelevant
- CLI: GOOD -- all flags have at least manual verification
- WEAK POINT: No negative tests (e.g., "this layout must NOT collapse to inline")
- WEAK POINT: No cross-feature interaction tests (formatter + import sorting + comment preservation simultaneously)

---

## 22.2 LSP Server

- [todo] ALL items -- `compiler/ori_lsp/` contains only `.gitkeep`
- [todo] No `plans/ori_lsp/` directory exists
- NOTE: Proposal exists at `proposals/approved/lsp-implementation-proposal.md`
- NO implementation, NO tests

---

## 22.3 Edit Operations

- [todo] ALL items -- no `edit/` module in `oric/src/`
- NO implementation, NO tests

---

## 22.4 REPL

- [todo] ALL items -- no `repl` command in CLI
- NO implementation, NO tests

---

## 22.5 Test Runner

### Implemented (but roadmap marks all `[ ]`)

STALE ROADMAP: The test runner is substantially implemented but every item is marked `[ ]`.

- [done] `ori test` command -- `compiler/oric/src/commands/test.rs`, `compiler/oric/src/test/runner/mod.rs`
  - TestRunner, TestRunnerConfig, TestSummary, FileSummary, TestOutcome all implemented
  - Runs 4181 tests currently

- [done] `ori test file.test.ori` -- path argument parsed at `main.rs:138`

- [done] `ori test path/` -- directory scanning functional

- [partial] `ori check file.ori` -- type-checks and reports errors; test coverage via `--test-enforcement=off|warn|error`
  - No `--json` flag

- [done] Parallel test execution -- `config.parallel = true` by default; `rayon`-based
  - `--no-parallel` flag to disable

- [done] Test filtering -- `--filter=` substring match on test names
  - Tests: `compiler/oric/src/test/runner/tests.rs:75` (`test_runner_filter`)

- [done] Test output formatting -- pass/fail/skip counts, timing, LLVM compile fail breakdown
  - `print_summary_stats()`, `print_file_results()`, `print_file_errors()`

- [done] Verbose mode -- `--verbose` / `-v` flag
  - Shows PASS/SKIP/LLVM COMPILE FAIL per test

- [done] Coverage report -- `--coverage` flag
  - `coverage_report()` returns `CoverageReport` with covered/uncovered function lists
  - Tests: `compiler/oric/src/test/result/tests.rs:128`

- [done] Exit codes -- 0=all pass, 1=failures, 2=no tests found
  - Tests: `compiler/oric/src/test/result/tests.rs:42` (`test_summary_exit_code`)

- [done] Incremental execution -- `--incremental` flag (skip unchanged tests)
- [done] Backend selection -- `--backend=llvm|interpreter`

### WEAK POINTS
- No Ori spec tests for the test runner itself (`tests/spec/tooling/test_runner.ori` does not exist)
- Rust tests exist but are limited (filter test, exit code test, coverage test)
- No test for `--verbose` output format
- No test for parallel vs sequential execution parity

---

## 22.6 Causality Tracking

- [todo] ALL items -- no `impact` or `why` commands in CLI
- NOTE: Proposal exists at `proposals/approved/why-command-proposal.md`
- NO implementation, NO tests

---

## 22.7 Structured Diagnostics

### 22.7.0 Error Code Registry Centralization

STALE ROADMAP: Items 1-3 are marked `[ ]` but are ALREADY IMPLEMENTED and TESTED.

- [done] `ErrorCode::from_str()` -- `compiler/ori_diagnostic/src/error_code/mod.rs:279`
  - Case-insensitive, returns `Option<ErrorCode>` (via `Result`)
  - Uses `ErrorCode::ALL` + `as_str()` for automatic exhaustiveness
  - Tests: `test_from_str_round_trip`, `test_from_str_case_insensitive`, `test_from_str_unknown`

- [done] `ErrorCode::all()` iterator -- `ErrorCode::ALL: &[ErrorCode]` at line 49
  - `ErrorCode::COUNT: usize` at line 52
  - Tests: `test_all_is_complete` (123 variants, duplicate check), `test_all_variants_classified` (exhaustive predicate check)

- [done] Remove `parse_error_code()` from `oric/src/commands/explain.rs`
  - `explain.rs` uses `code_str.parse::<ErrorCode>()` directly (line 7)
  - No `parse_error_code` function exists anywhere in `oric/`

- [todo] Compile-time completeness check for `ErrorDocs`
  - No test iterating `ErrorCode::all()` and checking for missing docs
  - `test_all_have_descriptions` exists but only checks `ErrorCode::description()`, NOT `ErrorDocs`

### 22.7.1 SourceLoc Type

STALE ROADMAP: Substantially implemented under different name.

- [done] Line/column from byte span -- `ori_diagnostic/src/span_utils/mod.rs`
  - `offset_to_line_col(source, offset) -> (u32, u32)` -- 1-based line and column
  - Unicode codepoint column (not byte offset) -- implemented
  - Tests: 6+ tests in `span_utils/tests.rs`

- [done] Line index builder -- `LineOffsetTable::build(source)`
  - O(log n) lookup via binary search on line offsets
  - Tests: `test_line_offset_table_*` tests

- NOTE: Not named `SourceLoc` but functionally equivalent

### 22.7.2 JSON Output Enhancement

- [partial] JSON emitter exists at `ori_diagnostic/src/emitter/json/mod.rs`
  - Has code, severity, message, labels (with byte spans), notes, suggestions, structured_suggestions
  - Tests: `emitter/json/tests.rs`

- [todo] File path NOT in diagnostic-level JSON output (only in cross-file labels)
- [todo] No `start_loc`/`end_loc` with line/column in JSON spans (only byte offsets)
- [todo] `structured_suggestions` in JSON only has `message`, missing substitution spans and applicability
- [todo] Summary is no-op (`emit_summary` does nothing)
- [todo] `ori check --json` flag NOT implemented

### 22.7.3 Improved Human Output

- [partial] Terminal emitter uses Ariadne for rendering -- `ori_diagnostic/src/emitter/terminal/mod.rs`
  - Has source snippets, line numbers, labels
  - Has "fix available" via suggestions field
  - Tests exist

- [todo] Not clear if secondary labels with underline arrows are fully implemented
- [todo] No explicit "fix available" indicator test

### 22.7.4 Auto-Fix Infrastructure

- [partial] Fix infrastructure exists but NO production fixes registered
  - `TextEdit`, `CodeAction`, `FixContext`, `FixRegistry` in `ori_diagnostic/src/fixes/`
  - `CodeFix` trait defined
  - Only mock fixes in tests

- [todo] `apply_suggestions()` -- not implemented
- [todo] Overlapping substitution handling -- not implemented
- [todo] `ori check --fix` -- not implemented
- [todo] `ori check --fix --dry` -- not implemented
- [todo] `ori check --fix=all` -- not implemented

### 22.7.5 Upgrade Existing Diagnostics

- [todo] ALL items -- no structured suggestion migration from `suggestions` to `structured_suggestions`
- NOTE: `with_suggestion()` has ~53 callers; `structured_suggestions` has ~7 callers

### 22.7.6 Extended Fixes

- [todo] ALL items -- no typo detection, formatting fixes, or import suggestions

---

## 22.8 WASM Playground

- [todo] ALL items -- `playground/wasm/` directory does not exist
- NOTE: Proposal exists at `proposals/approved/wasm-playground-proposal.md`

---

## 22.9 Grammar Synchronization Verification

- [todo] ALL items

---

## 22.10 Section Completion Checklist

- [todo] ALL items

---

## 22.11 Package Management

- [todo] ALL items -- `compiler/ori_pkg/` does not exist
- NOTE: Detailed plan exists at `plans/pkg_mgmt/` (11 section files + design.md)
- NO implementation code

---

## Findings Summary

### Items marked `[ ]` that should be `[x]` (STALE):

1. `.orifmtignore` file support (22.1 CLI) -- IMPLEMENTED
2. `ori fmt --no-ignore` (22.1 CLI) -- IMPLEMENTED
3. `ErrorCode::from_str()` (22.7.0) -- IMPLEMENTED with tests
4. `ErrorCode::all()` iterator (22.7.0) -- IMPLEMENTED with tests
5. Remove `parse_error_code()` (22.7.0) -- DONE
6. `SourceLoc` / line index (22.7.1) -- IMPLEMENTED as `LineOffsetTable` + `offset_to_line_col`
7. ALL Test Runner items in 22.5 -- Most are IMPLEMENTED

### Items marked `[x]` that are VERIFIED correct:
- All formatter core items (width, rendering, declarations, expressions, patterns, collections, comments)
- All formatter CLI items (fmt, directory, check, diff, stdin)
- All formatter performance items (incremental, parallel, memory)
- All formatter testing items (Rust tests, golden tests)
- ParenthesesRule integration

### BUG FOUND: None

### STALE ROADMAP DATA:
- Test Runner (22.5) has 10 `[ ]` items that are all implemented
- Error Code Registry (22.7.0) has 3 `[ ]` items that are done
- SourceLoc (22.7.1) has 2 `[ ]` items that are done (different naming)
- `.orifmtignore` and `--no-ignore` are done but marked `[ ]`

### MISSING TESTS:
- No Ori spec tests for test runner CLI (`tests/spec/tooling/test_runner.ori` does not exist)
- No negative formatter tests (layout-must-not-collapse)
- No autofix infrastructure tests beyond mocks
- No JSON emitter tests for line/column output (because feature not yet implemented)

### RISK AREAS:
- 6 formatter rules have detection logic but are not wired into rendering -- if wired incorrectly, could break existing golden tests
- JSON emitter has minimal structured suggestion support -- needs full substitution details for agent consumption
- Test runner has no Ori-level spec tests verifying its behavior
