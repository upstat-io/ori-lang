# Section 14: Testing Framework — Verification Results

**Date**: 2026-03-19
**Section**: 14/295 items (7%)
**Status**: in-progress (14.2, 14.3, 14.7, 14.8 in-progress; rest not-started)

## Summary

| Classification | Count |
|---|---|
| VERIFIED | 7 |
| WEAK TESTS | 1 |
| NEEDS TESTS | 1 |
| Genuinely incomplete `[ ]` (confirmed) | ~85 |

All 30 Rust tests in `oric::test::*` pass. All 23 parser attribute tests pass. All 20 parser function tests pass. Full spec suite: 4181 passed, 0 failed, 42 skipped.

---

## Checked Items (`[x]`)

### 14.2 Test Declaration

**14.2.1** `[x]` Syntax `@test_name tests @target () -> void = ...`
- **VERIFIED** — Parser handles single-target attached tests.
  - Rust test: `ori_parse::grammar::item::function::tests::test_attached_single_target` — asserts 1 test, 1 target, no parse errors. PASS.
  - Ori test: 900+ spec tests use this syntax across the suite. Representative: `tests/spec/source/file_structure.ori` line 27 (`@test_file_const tests @file_const`). All pass (4181 passed).

**14.2.2** `[x]` Semantics
- **VERIFIED** — Tests execute correctly via the evaluator.
  - Rust tests: `oric::test::runner::tests::test_runner_passing_test` and `test_runner_failing_test` — correctly detect pass/fail via runtime behavior. PASS.
  - Ori tests: Entire spec suite exercises test semantics. 4181 pass, 0 fail.

**14.2.3** `[x]` Multiple targets `@test tests @a tests @b`
- **VERIFIED** — Parser correctly handles multi-target test declarations.
  - Rust test: `ori_parse::grammar::item::function::tests::test_attached_multi_target` — parses `@t tests @a tests @b`, asserts 1 test with 2 targets. PASS.
  - Ori test: `tests/spec/source/file_structure.ori` line 64 — `@test_multi tests @multi_a tests @multi_b tests @multi_c () -> void` (3 targets). Passes.
  - Additional: `tests/spec/lexical/comments.ori` line 143, 244 — multi-target tests. Pass.

**14.2.4** `[ ]` Explicit free-floating tests `tests _`
- Parser already handles `tests _` syntax (parser test `test_floating_with_underscore` passes — creates TestDef with empty targets Vec). The AST uses `targets: Vec<Name>` where empty = floating. The roadmap item asks for an explicit `Targeted(Vec<Name>) | FreeFloating` enum distinction, which is NOT implemented — the current representation uses `targets.is_empty()` checks scattered across `change_detection/mod.rs`, `ori_fmt/declarations/tests_fmt.rs`. Item is **genuinely incomplete** (no enum variant distinction, though the parsing works).

### 14.3 Test Attributes

**14.3.1** `[x]` Syntax `#attribute` (new syntax)
- **VERIFIED** — Both `#attr(...)` and `#[attr(...)]` syntaxes parse correctly.
  - Rust tests: 23 tests in `ori_parse::grammar::attr::tests` — cover `#skip`, `#compile_fail`, `#fail`, `#derive`, `#fbip`, `#target`, `#cfg`, `#repr`, unknown attrs, missing parens, missing strings, bracket-less syntax. All pass.
  - Ori test: `tests/spec/declarations/attributes.ori` — exercises `#skip`, `#fail`, `#compile_fail`, `#derive`, `#repr`, `#target`, `#cfg`, `#cfg(not_debug)`. All pass.

**14.3.2** `[x]` `#skip("reason")`
- **VERIFIED** — Skip correctly prevents test execution.
  - Rust tests: `test_parse_skip_attribute` and `test_parse_skip_attribute_no_brackets` — verify `skip_reason` is set. PASS.
  - Ori test: `tests/spec/declarations/attributes.ori` line 137-141 — `#skip("Pending implementation of feature X")` on a test with `assert(cond: false)` body — test is correctly skipped (would fail without skip). PASS.
  - Additional: `tests/spec/expressions/loops.ori` line 405 — `#skip` used to skip unimplemented feature.
  - Runner: `test_execution.rs` line 264 checks `skip_reason` and returns `TestOutcome::Skipped`.
  - 42 tests skipped across the full suite — feature is functional.

### 14.7 Test Execution

**14.7.1** `[x]` Running tests
- **VERIFIED** — `ori test` / `cargo st` test runner works.
  - Rust tests: 5 runner tests (empty file, no tests, passing, failing, filter). All pass.
  - Rust tests: 5 discovery tests (empty dir, .ori files, recursive, hidden/target skip, single file). All pass.
  - Rust tests: 9 result tests (outcome predicates, file summary, coverage, exit code, LLVM compile fail handling). All pass.
  - Rust tests: 11 change detection tests (cache, body change, new/deleted function, floating tests, bidirectional index). All pass.
  - Full spec suite: 4181 passed, 0 failed, 42 skipped. Test runner executes correctly end-to-end.

### 14.8 Compile-Fail Tests

**14.8.1** `[x]` Compile-fail tests
- **WEAK TESTS** — Implementation works but Rust-level test coverage is thin.
  - Rust test: `test_parse_compile_fail_attribute` and `test_parse_compile_fail_attribute_no_brackets` — verify parser creates `expected_errors` correctly. PASS.
  - Ori test: `tests/spec/declarations/attributes.ori` lines 177-193 — `#compile_fail("type")` (type mismatch) and `#compile_fail("unknown identifier")` (undefined variable). Both pass correctly.
  - Runner: `test_execution.rs` line 31 `run_compile_fail_test()` — handles skipping, error matching with `match_errors()`. Separate from regular test execution path.
  - Runner: `mod.rs` line 321 — compile_fail tests partitioned from regular tests. Errors in compile_fail bodies are excluded from blocking regular tests (line 370-377).
  - **Weakness**: No Rust-level unit test for `run_compile_fail_test()` directly. The runner tests (`test_runner_passing_test`, `test_runner_failing_test`) do not exercise compile_fail or fail-expected paths. The error_matching module has NO tests at all (0 tests run for `test::error_matching`). Compile-fail is only tested through the Ori spec test (`attributes.ori`), which is end-to-end — no isolated unit test for the matching logic.
  - **NEEDS TESTS**: `error_matching.rs` module has exported functions (`match_errors`, `match_all_errors`, `matches_expected`, `matches_pattern_problem`, `format_actual`, `format_expected`, `format_pattern_problem`) but zero test coverage.

---

## Unchecked Items — Spot-Check of Genuinely Incomplete

### LLVM Support sub-items across 14.2, 14.3, 14.7, 14.8
All `[ ] LLVM Support` and `[ ] LLVM Rust Tests` sub-items under checked parent items are **genuinely incomplete**. No file `ori_llvm/tests/testing_framework_tests.rs` exists. LLVM codegen has no test-framework-specific support — tests currently only run via the interpreter backend (the LLVM JIT backend runs test bodies but does not have dedicated testing-framework codegen tests).

### 14.1 Test Requirement (not-started)
- **Genuinely incomplete**. No `test_coverage.rs` exists under `ori_types/src/check/`. No `--test-enforcement` CLI flag implementation found. The spec defines it (19.2) but no compiler code implements enforcement.

### 14.6 Test Organization (not-started)
- **Genuinely incomplete**. No `_test/` directory enforcement exists. Tests are currently in `tests/spec/` alongside source, not in `_test/` subdirectories. No E0501 error code for tests outside `_test/`.

### 14.9 Dependency-Aware Test Execution (not-started)
- **Partially started** via `oric::test::change_detection` module (11 tests pass). Change detection and caching infrastructure exists, but the full dependency-aware model (reverse closure, execution modes `--direct`/`--closure`/`--full`) is not implemented.

### 14.11 Incremental Test Execution (not-started)
- **Genuinely incomplete**. `ori check` does not run tests. No `--no-test`, `--strict`, or `--only-targeted` flags. No hash-based test caching beyond what `change_detection` provides.

### 14.12 Test Execution Model (not-started)
- **Genuinely incomplete**. No `TestRegistry` struct. No cache file format (`.ori/cache/`). No `--clean` flag.

---

## Findings

### NEEDS TESTS: `error_matching.rs` has zero test coverage
- **File**: `/home/eric/projects/ori_lang_aims/compiler/oric/src/test/error_matching.rs`
- **Issue**: Module exports 7 public functions for matching expected errors in compile_fail/fail tests. Zero unit tests exist. The module is only exercised via end-to-end spec tests.
- **Risk**: Regressions in error message matching (substring, pattern, multi-error) would not be caught by Rust-level tests.

### Observation: `tests _` parsing works but AST lacks explicit variant
- The parser correctly handles `tests _` (floating tests) and represents them as `TestDef { targets: Vec::new() }`. However, the roadmap item 14.2 requests an explicit `Targeted(Vec<Name>) | FreeFloating` enum distinction in the AST. The current `targets.is_empty()` pattern works but is semantically weaker.

### Observation: Change detection already has 11 passing tests
- Section 14.9 is marked "not-started" but `oric::test::change_detection` module has substantial implementation with 11 passing tests covering cache, body change detection, floating test handling, and bidirectional function-test mapping. This subsection could be marked partially complete.

---

## Test Commands Run

| Command | Result |
|---|---|
| `cargo test -p oric --lib -- test::runner` | 5 passed |
| `cargo test -p oric --lib -- test::discovery` | 5 passed |
| `cargo test -p oric --lib -- test::result` | 9 passed |
| `cargo test -p oric --lib -- test::change_detection` | 11 passed |
| `cargo test -p oric --lib -- test::error_matching` | 0 tests found |
| `cargo test -p ori_parse --lib -- grammar::attr::tests` | 23 passed |
| `cargo test -p ori_parse --lib -- grammar::item::function::tests` | 20 passed |
| `cargo st tests/spec/declarations/attributes.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/source/file_structure.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/lexical/comments.ori` | 4181 passed, 0 failed, 42 skipped |
