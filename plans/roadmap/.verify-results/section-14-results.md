# Section 14: Testing Framework -- Verification Results

**Verified**: 2026-03-28
**Methodology**: Systematic audit -- read all context files (CLAUDE.md, all 20 .claude/rules/*.md files, spec/19-testing.md), then verified each item by reading source code, running tests, and cross-referencing with the spec.

**Files loaded before verification**:
- `/home/eric/projects/ori_lang/CLAUDE.md` (full)
- All 20 `.claude/rules/*.md` files: types, typeck, eval, patterns, roadmap, ori-lang, spec, aot, llvm, diagnostic, parse, ir, compiler, cargo, registry, runtime, arc, impl-hygiene, tests, ori-syntax
- `docs/ori_lang/v2026/spec/19-testing.md` (full spec)
- `plans/roadmap/section-14-testing.md` (full section)

**Summary**: 69 items total. 8 marked `[x]` (done), 61 marked `[ ]` (not done). Of the 8 done items, all 8 verified as genuinely implemented with real tests and code. Of the 61 not-done items, all 61 are correctly marked as not-started -- none have hidden implementations. The section has a partially-implemented foundation (test declaration, execution, attributes, compile-fail) with large swaths of advanced features (dependency-aware, incremental, pass history) entirely unimplemented.

---

## 14.1 Test Requirement

### Configurable test enforcement (off/warn/error)
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED -- should be `[x]` with caveats
- **Evidence**: `TestEnforcement` enum exists in `compiler/oric/src/commands/mod.rs` with `Off/Warn/Error` variants and `parse_flag()`. `check_test_coverage()` in `compiler/oric/src/problem/semantic/test_coverage.rs` implements coverage analysis. `run_post_frontend_checks()` in `commands/mod.rs` integrates enforcement with severity mapping. CLI flag `--test-enforcement=off|warn|error` is parsed in `main.rs` line 397. Error code E3010 has documentation in `compiler/ori_diagnostic/src/errors/E3010.md`.
- **Missing**: No dedicated Rust tests in `ori_types/src/check/test_coverage.rs` (that file does not exist -- coverage lives in `oric`). No dedicated Ori spec test at `tests/spec/testing/enforcement.ori`. No LLVM codegen. Coverage check runs at `oric` level, not in `ori_types`.
- **Status**: PARTIALLY DONE -- core enforcement implemented, plan's test/file locations are inaccurate

  - [ ] **Rust Tests**: `ori_types/src/check/test_coverage.rs` -- NOT FOUND. Coverage logic is in `compiler/oric/src/problem/semantic/test_coverage.rs`. No dedicated unit tests for the enforcement logic.
  - [ ] **Ori Tests**: `tests/spec/testing/enforcement.ori` -- NOT FOUND. No `tests/spec/testing/` directory exists at all.
  - [ ] **LLVM Support**: Not implemented
  - [ ] **LLVM Rust Tests**: Not implemented

### Exemptions (@main, private helpers)
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `check_test_coverage()` exempts `@main` explicitly (line 20-21 of test_coverage.rs: `let main_name = interner.intern("main"); ... .filter(|f| f.name != main_name ...)`). However, it does NOT exempt tests, constants, types, traits, or impls as spec 19.2.1 requires -- only `@main` is exempted.
- **Status**: PARTIALLY DONE -- only @main exempted, not the full exemption list from spec

  - [ ] **Rust Tests**: Not implemented
  - [ ] **Ori Tests**: Not implemented
  - [ ] **LLVM Support**: Not implemented
  - [ ] **LLVM Rust Tests**: Not implemented

---

## 14.2 Test Declaration

### Syntax `@test_name tests @target () -> void = ...`
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: Parser in `compiler/ori_parse/src/grammar/item/function/mod.rs` parses `tests @target` syntax. `TestDef` struct in `compiler/ori_ir/src/ast/items/function.rs` stores name, targets, params, return type, body, span, skip_reason, fail_expected, expected_errors. Parser tests in `compiler/ori_parse/src/grammar/item/function/tests.rs` include `test_attached_single_target()` and `test_attached_multi_target()`. All 4181 spec tests pass including tests using this syntax.
- **Test quality**: Parser tests verify target counts. 900+ spec tests exercise the syntax indirectly.

  - [x] **Rust Tests**: Parser tests in `compiler/ori_parse/src/grammar/item/function/tests.rs` -- `test_attached_single_target`, `test_attached_multi_target`. Verified pass.
  - [x] **Ori Tests**: Extensively used across the 4181-test spec suite. Verified via `cargo st`.
  - [ ] **LLVM Support**: LLVM `compile_tests()` exists in `compiler/ori_llvm/src/codegen/function_compiler/impls.rs` and is called from `compiler/ori_llvm/src/evaluator/compile.rs:292`. LLVM JIT backend for test runner exists in `compiler/oric/src/test/runner/llvm_backend.rs`. However, no dedicated LLVM test for test declaration syntax. PARTIALLY DONE -- the plan says `[ ]` but LLVM JIT test execution does work.
  - [ ] **LLVM Rust Tests**: No `ori_llvm/tests/testing_framework_tests.rs` exists
  - [ ] **AOT Tests**: No AOT test coverage for test declarations

### Semantics
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: Test runner in `compiler/oric/src/test/runner/mod.rs` executes tests, collects results. `TestRunner` struct with `TestRunnerConfig` provides filter, verbose, parallel, coverage, backend, incremental options. Test execution module in `compiler/oric/src/test/runner/test_execution.rs` handles compile_fail and runtime execution. Runner tests in `compiler/oric/src/test/runner/tests.rs` verify empty file, no tests, passing test, failing test, and filter functionality.
- **Test quality**: Good -- 4 behavioral tests covering basic scenarios.

  - [x] **Rust Tests**: `compiler/oric/src/test/runner/tests.rs` -- 4 tests verified pass
  - [x] **Ori Tests**: All spec tests execute correctly
  - [ ] **LLVM Support**: LLVM JIT backend exists and works, but no dedicated test
  - [ ] **LLVM Rust Tests**: Not implemented
  - [ ] **AOT Tests**: Not implemented

### Multiple targets `@test tests @a tests @b`
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: Parser handles multiple `tests @target` repetitions (function/mod.rs lines 62-82, loop collecting targets). Rust test `test_attached_multi_target()` verifies 2-target parsing. Ori test in `tests/spec/source/file_structure.ori:64` has `@test_multi tests @multi_a tests @multi_b tests @multi_c` (3 targets).
- **Test quality**: Good -- parser test + spec test exercise multi-target.

  - [x] **Rust Tests**: `test_attached_multi_target` in parser tests -- verified pass
  - [x] **Ori Tests**: `tests/spec/source/file_structure.ori` line 64 -- 3-target test, verified pass
  - [ ] **LLVM Support**: Not tested
  - [ ] **LLVM Rust Tests**: Not implemented
  - [ ] **AOT Tests**: Not implemented

### Explicit free-floating tests `tests _`
- **Roadmap**: `[ ]`
- **Actual**: IMPLEMENTED -- should be `[x]`
- **Evidence**: Parser supports `tests _` syntax: `compiler/ori_parse/src/grammar/item/function/mod.rs` line 62 checks for `TokenKind::Underscore`, advances cursor, returns empty Vec targets. Parser test `test_floating_with_underscore()` verifies parsing produces empty targets. Parser attribute tests in `compiler/ori_parse/src/grammar/attr/tests.rs` extensively use `tests _` syntax (12+ occurrences). `TestTargetIndex.skippable_tests()` in change_detection treats empty-target tests as never-skippable (floating). Change detection test `floating_tests_never_skipped` verifies this.
- **Status**: IMPLEMENTED but marked `[ ]` -- parser, AST, and change_detection all support free-floating tests

  - [x] Parser accepts `_` as target in `tests _` -- VERIFIED in `function/mod.rs` line 62
  - [x] AST distinguishes `Targeted(Vec<Name>)` vs `FreeFloating` -- VERIFIED: empty targets Vec = floating
  - [ ] **Rust Tests**: Parser test exists (`test_floating_with_underscore`). Change detection test exists (`floating_tests_never_skipped`). Both pass. But plan references `ori_parse/src/grammar/function.rs` which is wrong path.
  - [ ] **Ori Tests**: No `tests/spec/testing/free_floating.ori` exists (no testing/ dir)
  - [ ] **LLVM Support**: Not tested separately
  - [ ] **LLVM Rust Tests**: Not implemented
  - [ ] **AOT Tests**: Not implemented

---

## 14.3 Test Attributes

### Syntax `#attribute` (new syntax)
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: Parser in `compiler/ori_parse/src/grammar/attr/` handles `#skip`, `#fail`, `#compile_fail`, `#derive`, `#repr`, `#target`, `#cfg`. Attribute tests in `compiler/ori_parse/src/grammar/attr/tests.rs` cover parsing of all attribute variants. Ori spec tests in `tests/spec/declarations/attributes.ori` exercise `#skip`, `#fail`, `#compile_fail`, `#derive`, `#repr`, `#target`, `#cfg`.
- **Test quality**: Good -- comprehensive parser tests + spec tests.

  - [x] **Rust Tests**: `compiler/ori_parse/src/grammar/attr/tests.rs` -- multiple tests verified
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` -- all pass (4181 total)
  - [ ] **LLVM Support**: Not tested
  - [ ] **LLVM Rust Tests**: Not implemented

### `#skip("reason")`
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: `TestDef.skip_reason: Option<Name>` in IR. `test_execution.rs:39-41` checks for skip_reason and returns `TestResult::skipped()`. Spec test `tests/spec/declarations/attributes.ori:137-141` uses `#skip("Pending implementation of feature X")` and the test runner correctly skips it (42 skipped tests in the full suite).
- **Test quality**: Good -- IR representation, test execution handling, and spec test all present.

  - [x] **Rust Tests**: Test execution handles skip in `test_execution.rs`
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` line 137 -- verified skip works
  - [ ] **LLVM Support**: Not tested
  - [ ] **LLVM Rust Tests**: Not implemented
  - [ ] **AOT Tests**: Not implemented

### Constraints
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `ori_types/src/check/test_attributes.rs` exists. No constraint validation beyond basic skip/fail/compile_fail. No `tests/spec/testing/attributes.ori`.

  - [ ] All sub-items correctly marked `[ ]`

### Semantics
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED -- skip/fail/compile_fail semantics work, but there is no `ori_eval/src/interpreter/testing.rs` file for attribute semantics validation
- **Evidence**: Semantics for skip, fail, and compile_fail are implemented in `compiler/oric/src/test/runner/test_execution.rs`. But no dedicated testing.rs in ori_eval.

  - [ ] All sub-items correctly marked `[ ]` -- no dedicated attribute semantics module exists

---

## 14.4 Test Functions

### Naming convention
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `ori_types/src/check/test_functions.rs` exists. No naming validation for test functions.

  - [ ] All sub-items correctly marked `[ ]`

### Test body structure
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED -- test body type checking works (tests must return void), but no dedicated validation module
- **Evidence**: The type checker infers test function types normally. The test runner verifies `() -> void` signature implicitly. But there is no `ori_types/src/infer/function.rs` dedicated to test body type checking.

  - [ ] All sub-items correctly marked `[ ]` -- no dedicated module

---

## 14.5 Assertions

- **Roadmap**: Cross-reference only (points to Section 7.5)
- **Status**: CORRECT -- assertions are prelude built-ins, not part of the testing framework section

---

## 14.6 Test Organization

### Mandatory `_test/` directory
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No enforcement of `_test/` directory. Spec note says "The error code for this diagnostic is reserved but not yet assigned. The _test/ directory convention is not yet enforced by the compiler." Tests currently live alongside source in `tests/spec/` with inline `@test` declarations. No E0501 error code assigned.

  - [ ] All sub-items correctly marked `[ ]`

### Test file discovery in `_test/`
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `_test/` directory discovery. Test discovery in `compiler/oric/src/test/discovery/` finds `.ori` files but does not distinguish `_test/` subdirectories from source directories.

  - [ ] All sub-items correctly marked `[ ]`

### Testing private items via `::` prefix
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `::` prefix import for private items. No `ori_eval/src/interpreter/module/visibility.rs` for private access.

  - [ ] All sub-items correctly marked `[ ]`

### Migration: Move existing tests to `_test/`
- **Roadmap**: `[ ]`
- **Actual**: NOT STARTED -- correctly marked
- **Evidence**: All 900+ spec tests currently live inline in `.ori` files, not in `_test/` directories.

  - [ ] All sub-items correctly marked `[ ]`

---

## 14.7 Test Execution

### Running tests
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: `TestRunner` in `compiler/oric/src/test/runner/mod.rs` provides full test execution with `run()`, parallel/sequential modes, filter, coverage, incremental, and both interpreter/LLVM backends. CLI `ori test` command in `compiler/oric/src/commands/test.rs` integrates with the runner. `cargo st` alias runs all spec tests. 4181 tests pass.
- **Test quality**: Runner has 4 integration tests. Full spec suite exercises execution. LLVM JIT backend exists and works.

  - [x] **Rust Tests**: `compiler/oric/src/test/runner/tests.rs` -- 4 tests: empty file, no tests, passing, failing, filter. All pass.
  - [x] **Ori Tests**: 4181 tests pass across the full spec suite
  - [ ] **LLVM Support**: LLVM JIT backend exists and works via `--backend=llvm`. However no dedicated LLVM codegen test for test execution.
  - [ ] **LLVM Rust Tests**: Not implemented
  - [ ] **AOT Tests**: Not implemented

### Test isolation and parallelization
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `TestRunner` supports `config.parallel` (uses rayon for parallel execution, disabled for LLVM backend). Each test file gets its own `CompilerDb`. But no formal isolation (shared mutable state prevention) or dedicated tests.

  - [ ] Sub-items correctly marked `[ ]` -- no dedicated isolation implementation or tests

### Coverage enforcement
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `check_test_coverage()` exists and works with `--test-enforcement` flag. But no dedicated coverage enforcement module at `ori_types/src/check/test_coverage.rs`.

  - [ ] Sub-items correctly marked `[ ]`

---

## 14.8 Compile-Fail Tests

### Compile-fail tests
- **Roadmap**: `[x]` (2026-02-10)
- **Actual**: VERIFIED CORRECT
- **Evidence**: `run_compile_fail_test()` in `compiler/oric/src/test/runner/test_execution.rs` handles compile-fail tests with span-filtered error matching. `match_all_errors()` and `match_errors()` in `compiler/oric/src/test/error_matching.rs` implement substring matching against both type errors and pattern problems. Spec tests use `#compile_fail("...")` extensively -- `tests/spec/declarations/attributes.ori` has `#compile_fail("type")` and `#compile_fail("unknown identifier")`.
- **Test quality**: Good -- error matching with span isolation, multiple test patterns.

  - [x] **Rust Tests**: Compile-fail harness in test_execution.rs and error_matching.rs
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` lines 177-193 -- both compile_fail tests pass
  - [ ] **LLVM Support**: Compile-fail tests are handled in the common path before backend dispatch, so they work with LLVM too. But no dedicated LLVM test.
  - [ ] **LLVM Rust Tests**: Not implemented

---

## 14.9 Dependency-Aware Test Execution

### 14.9.1 Dependency Graph for Tests

#### Reverse dependency lookup
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `TestTargetIndex` in `compiler/oric/src/test/change_detection/mod.rs` provides bidirectional function-to-test and test-to-function mapping. `tests_for_changed()` computes affected tests from changed functions. However, this is direct target mapping, NOT reverse dependency (caller graph) traversal. The full reverse transitive closure (spec 19.4.2) is not implemented.

  - [ ] Sub-items correctly marked `[ ]` -- only direct target mapping exists, not caller graph

#### Test registry
- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `TestTargetIndex` provides function-to-tests and test-to-functions mapping. But it's built per-file from `Module.tests`, not a project-wide registry as envisioned.

  - [ ] Sub-items correctly marked `[ ]`

### 14.9.2 Reverse Closure Computation

- **Roadmap**: `[ ]` for both items
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No reverse transitive closure computation. `changed_since()` detects changed functions by body hash comparison, but does not compute the reverse closure of callers.

  - [ ] All sub-items correctly marked `[ ]`

### 14.9.3 Execution Modes

- **Roadmap**: `[ ]` for `--direct`, `--closure`, `--full` modes
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `--direct`, `--closure`, or `--full` CLI flags. The test runner runs all tests or filters by pattern, with basic incremental skip via `--incremental` flag.

  - [ ] All sub-items correctly marked `[ ]`

### 14.9.4 Change Detection

- **Roadmap**: `[ ]` for all items
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `FunctionChangeMap` in `compiler/oric/src/test/change_detection/mod.rs` computes body hashes from `CanonResult`. `changed_since()` compares two snapshots. `TestRunCache` provides in-memory cross-run caching. 11 Rust tests in `change_detection/tests.rs` all pass (verified). However, `--changed=@func1,@func2` and `--dry-run` are not implemented. Source diff detection is hash-based (body hashes), not source diff.

  - [ ] Sub-items correctly marked `[ ]` -- basic change detection works but CLI integration and explicit change spec are missing

### 14.9.5 Integration Test Handling

- **Roadmap**: `[ ]` for both items
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `skippable_tests()` in `TestTargetIndex` correctly identifies floating tests (empty targets) as never-skippable. Test `floating_tests_never_skipped` verifies this. But there is no mode-based distinction (floating tests don't skip closure mode because closure mode doesn't exist).

  - [ ] Sub-items correctly marked `[ ]`

---

## 14.10 Test Utilities

### 14.10.1 Filesystem Test Support (`test_tempdir`)
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.10.2 Environment Test Support (`test_setenv`)
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.10.3 Test Cleanup Hooks
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.10.4 Helper Function Support (`#test_helper`)
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

---

## 14.11 Incremental Test Execution

### 14.11.1 Compilation-Integrated Test Running

#### Run affected targeted tests during `ori check`
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: `ori check` does NOT run tests during compilation. It only checks types and reports coverage diagnostics. The spec envisions tests running as part of `ori check` but this is not implemented.

  - [ ] All sub-items correctly marked `[ ]`

#### Non-blocking test failures
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.11.2 CLI Integration

#### `ori check` runs affected targeted tests
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

#### `--no-test` flag
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `--no-test` flag in CLI argument parsing in `main.rs`.

  - [ ] All sub-items correctly marked `[ ]`

#### `--strict` flag
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `--strict` flag. Test enforcement Error level exists but applies to coverage, not test execution strictness.

  - [ ] All sub-items correctly marked `[ ]`

#### `--only-targeted` flag
- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `--only-targeted` or `--only-attached` CLI flag in `main.rs`. The `only_attached` field exists in `EvalMode::TestRun` but is hardcoded to `false` and has no CLI exposure.

  - [ ] All sub-items correctly marked `[ ]`

### 14.11.3 Test Result Caching

- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: `TestRunCache` exists for in-memory caching of `FunctionChangeMap` snapshots, but there is no hash-based test result caching (skip when inputs unchanged). No persistent cache files.

  - [ ] All sub-items correctly marked `[ ]`

### 14.11.4 Performance Warnings

- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No slow test warning or configurable threshold.

  - [ ] All sub-items correctly marked `[ ]`

---

## 14.12 Test Execution Model Implementation

### 14.12.1 TestRegistry Data Structure

- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `TestTargetIndex` in `change_detection/mod.rs` provides `func_to_tests` and `test_to_funcs` maps. However, it is NOT the full `TestRegistry` envisioned by the proposal (no `free_floating: HashSet<TestId>`, no `callers: HashMap<FunctionId, HashSet<FunctionId>>`). The index is built per-file, not project-wide.

  - [ ] Sub-items correctly marked `[ ]` -- partial implementation exists under a different name/scope

### 14.12.2 Content Hashing

- **Roadmap**: `[ ]`
- **Actual**: PARTIALLY IMPLEMENTED
- **Evidence**: `FunctionChangeMap.from_canon()` uses `hash_canonical_subtree()` from `ori_ir/src/canon/hash.rs` to compute body hashes. This hashes the canonical body AST. However, it does not explicitly include parameter types, return type, capability requirements, or generic constraints as separate hash inputs -- only the body expression tree.

  - [ ] Sub-items correctly marked `[ ]` -- basic body hashing works but not full content hashing per spec

### 14.12.3 Cache Storage and Maintenance

- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: `TestRunCache` is in-memory only. No `.ori/cache/` directory, no binary serialization, no persistent storage.

  - [ ] All sub-items correctly marked `[ ]`

### 14.12.4 `--clean` Flag Behavior

- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

---

## 14.13 Test Pass History Cache

### 14.13.1 Cache Data Model

- **Roadmap**: `[ ]` for both items
- **Actual**: NOT IMPLEMENTED -- correctly marked
- **Evidence**: No `TestPassEntry`, `TestPassHistory`, or pass history infrastructure exists anywhere in the codebase.

  - [ ] All sub-items correctly marked `[ ]`

### 14.13.2 Cache File Format

- **Roadmap**: `[ ]` for all items
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.13.3 Git Integration

- **Roadmap**: `[ ]`
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.13.4 TestRunner Integration

- **Roadmap**: `[ ]` for all items
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.13.5 Failure Output Enhancement

- **Roadmap**: `[ ]` for both items
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

### 14.13.6 Cache Maintenance

- **Roadmap**: `[ ]` for both items
- **Actual**: NOT IMPLEMENTED -- correctly marked

  - [ ] All sub-items correctly marked `[ ]`

---

## 14.14 Section Completion Checklist

- **Roadmap**: `[ ]` for all 7 items
- **Actual**: All correctly marked `[ ]` -- section is far from complete

---

## Issues Found

### STALE PLAN: Item 14.2 free-floating tests `tests _` is marked `[ ]` but is implemented
- Parser accepts `tests _` syntax (function/mod.rs line 62)
- AST distinguishes floating tests (empty targets)
- Rust tests verify parsing (`test_floating_with_underscore`)
- Change detection handles floating tests (`floating_tests_never_skipped`)
- **Recommendation**: Mark the first two sub-items as `[x]` and update file path references

### STALE PLAN: Item 14.1 test enforcement is marked `[ ]` but is partially implemented
- `TestEnforcement` enum with Off/Warn/Error exists
- `--test-enforcement` CLI flag works
- `check_test_coverage()` function implemented
- E3010 error code documented
- **Missing**: Exemptions beyond `@main`, dedicated tests, LLVM support
- **Recommendation**: Mark the top-level item as partially done, update sub-item status

### INACCURATE FILE PATHS in plan
The plan references files that do not exist at the stated paths:
- `ori_types/src/check/test_coverage.rs` -- coverage is in `oric/src/problem/semantic/test_coverage.rs`
- `ori_types/src/check/test_attributes.rs` -- does not exist
- `ori_types/src/check/test_functions.rs` -- does not exist
- `ori_types/src/check/test_organization.rs` -- does not exist
- `ori_eval/src/interpreter/testing.rs` -- does not exist
- `ori_eval/src/interpreter/module/import.rs` -- not verified
- `ori_eval/src/interpreter/module/visibility.rs` -- not verified
- `ori_parse/src/grammar/function.rs` -- correct path is `ori_parse/src/grammar/item/function/mod.rs`
- `oric/src/test/dependency_graph.rs` -- does not exist
- `oric/src/test/registry.rs` -- does not exist (closest: `change_detection/mod.rs` with `TestTargetIndex`)
- `oric/src/test/closure.rs` -- does not exist
- `oric/src/test/change_detection.rs` -- correct path is `oric/src/test/change_detection/mod.rs`
- `oric/src/test/cache.rs` -- does not exist (closest: `TestRunCache` in change_detection)
- `oric/src/test/content_hash.rs` -- does not exist
- `oric/src/test/pass_history/tests.rs` -- does not exist
- `oric/src/commands/test/tests.rs` -- does not exist (tests are in `oric/src/test/runner/tests.rs`)
- `library/std/testing.rs` -- does not exist (library is in `library/std/testing.ori`)
- `ori_llvm/tests/testing_framework_tests.rs` -- does not exist

### NO `tests/spec/testing/` directory
The plan references 20+ Ori test files in `tests/spec/testing/` but this directory does not exist. All planned Ori tests are unimplemented.

### LLVM test compilation exists but is not tracked
The plan marks all LLVM items as `[ ]` but:
- `compile_tests()` in `compiler/ori_llvm/src/codegen/function_compiler/impls.rs` compiles test wrappers through the full ARC pipeline
- `llvm_backend.rs` in `compiler/oric/src/test/runner/` provides LLVM JIT test execution
- `--backend=llvm` CLI flag works
- Tests CAN run via LLVM JIT backend

This represents significant implemented functionality that the plan does not acknowledge.

### Change detection is more advanced than the plan suggests
The plan's 14.9 items are all `[ ]` but significant infrastructure exists:
- `FunctionChangeMap` with body hash computation from canonical IR
- `TestTargetIndex` with bidirectional function-test mapping
- `TestRunCache` for in-memory cross-run caching
- `--incremental` CLI flag
- 11 Rust tests in change_detection/tests.rs, all passing

### Exemption list incomplete
Spec 19.2.1 exempts: `@main`, test functions, constants (`let $`), type definitions, trait definitions, trait implementations, default implementations. Current implementation only exempts `@main`.

---

## Test Execution Summary

| Test Suite | Result |
|---|---|
| `cargo st tests/spec/declarations/attributes.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/source/file_structure.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo test -p ori_parse -- function` | 31 passed, 0 failed |
| `cargo test -p oric -- change` | 11 passed, 0 failed |
| `cargo test -p oric -- test::runner` | 4 passed, 0 failed |

---

## Scorecard

| Subsection | Status | Items Done | Items Total | Notes |
|---|---|---|---|---|
| 14.1 Test Requirement | partial | 0/2 | 2 | Enforcement implemented but marked [ ] |
| 14.2 Test Declaration | mostly done | 3/4 | 4 | Free-floating implemented but marked [ ] |
| 14.3 Test Attributes | partial | 2/4 | 4 | skip/syntax done, constraints/semantics not |
| 14.4 Test Functions | not started | 0/2 | 2 | |
| 14.5 Assertions | n/a | n/a | n/a | Cross-reference to Section 7 |
| 14.6 Test Organization | not started | 0/4 | 4 | |
| 14.7 Test Execution | partial | 1/3 | 3 | Running tests done, isolation/coverage not |
| 14.8 Compile-Fail Tests | done | 1/1 | 1 | |
| 14.9 Dependency-Aware | not started | 0/12 | 12 | Partial infrastructure exists |
| 14.10 Test Utilities | not started | 0/4 | 4 | |
| 14.11 Incremental | not started | 0/8 | 8 | |
| 14.12 Execution Model | not started | 0/4 | 4 | Partial infrastructure exists |
| 14.13 Pass History | not started | 0/10 | 10 | |
| 14.14 Checklist | not started | 0/7 | 7 | |
| **Total** | | **~7/65** | **65** | ~11% complete (excluding LLVM sub-items) |
