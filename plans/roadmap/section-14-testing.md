---
section: 14
title: Testing Framework
status: in-progress
reviewed: false
tier: 5
goal: Configurable test enforcement with dependency-aware execution and incremental test execution during compilation
spec:
  - spec/19-testing.md
sections:
  - id: "14.1"
    title: Test Requirement
    status: in-progress
  - id: "14.2"
    title: Test Declaration
    status: in-progress
  - id: "14.3"
    title: Test Attributes
    status: in-progress
  - id: "14.4"
    title: Test Functions
    status: not-started
  - id: "14.5"
    title: Assertions
    status: not-started
  - id: "14.6"
    title: Test Organization
    status: not-started
  - id: "14.7"
    title: Test Execution
    status: in-progress
  - id: "14.8"
    title: Compile-Fail Tests
    status: in-progress
  - id: "14.9"
    title: Dependency-Aware Test Execution
    status: not-started
  - id: "14.10"
    title: Test Utilities
    status: not-started
  - id: "14.11"
    title: Incremental Test Execution
    status: not-started
  - id: "14.12"
    title: Test Execution Model Implementation
    status: not-started
  - id: "14.13"
    title: Test Pass History Cache
    status: not-started
  - id: "14.14"
    title: Section Completion Checklist
    status: not-started
---

# Section 14: Testing Framework

**Goal**: Configurable test enforcement with dependency-aware execution and incremental test execution during compilation

> **SPEC**: `spec/19-testing.md`
> **DESIGN**: `design/11-testing/index.md`
> **PROPOSALS**:
> - `proposals/approved/dependency-aware-testing-proposal.md` — Dependency-aware test execution
> - `proposals/approved/incremental-test-execution-proposal.md` — Incremental test execution & explicit free-floating tests
> - `proposals/approved/test-execution-model-proposal.md` — Consolidated implementation model (data structures, algorithms, cache)

> **NOTE - Pending Syntax Changes**: The approved proposals change attribute syntax:
> - Attribute syntax: `#[skip("reason")]` → `#skip("reason")` (Section 15.1)
> See Section 15 (Approved Syntax Proposals) for details. Implement with new syntax directly to avoid migration.

---

## 14.1 Test Requirement

- [x] **Implement**: Configurable test enforcement (off/warn/error) — spec/19-testing.md § Test Requirements, design/11-testing/01-mandatory-tests.md [done] (verified 2026-03-28)
  - [x] **Implementation**: `TestEnforcement` enum (Off/Warn/Error) in `compiler/oric/src/commands/mod.rs`; `check_test_coverage()` in `compiler/oric/src/problem/semantic/test_coverage.rs`; `--test-enforcement=off|warn|error` CLI flag; E3010 error code documented
  - [ ] **Rust Tests**: No dedicated unit tests for enforcement logic
  - [ ] **Ori Tests**: `tests/spec/testing/enforcement.ori` — no Ori spec tests yet
  - [ ] **LLVM Support**: LLVM codegen for test enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test enforcement codegen

- [ ] **Implement**: Exemptions (`@main`, private helpers) — spec/19-testing.md § Exemptions
  - [ ] **Rust Tests**: `ori_types/src/check/test_coverage.rs` — exemption rules
  - [ ] **Ori Tests**: `tests/spec/testing/exemptions.ori`
  - [ ] **LLVM Support**: LLVM codegen for test exemptions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test exemptions codegen

---

## 14.2 Test Declaration

- [x] **Implement**: Syntax `@test_name tests @target () -> void = ...` — spec/19-testing.md § Test Declaration, design/11-testing/02-test-syntax.md [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — test declaration parsing
  - [x] **Ori Tests**: All spec tests use this syntax (4181+ tests across the test suite)
  - [x] **LLVM Support**: `compile_tests()` in `compiler/ori_llvm/src/codegen/function_compiler/impls.rs`; LLVM JIT backend in `compiler/oric/src/test/runner/llvm_backend.rs`; `--backend=llvm` CLI flag [done] (verified 2026-03-28)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — no dedicated LLVM test file
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Semantics — spec/19-testing.md § Test Declaration [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — test semantics
  - [x] **Ori Tests**: All spec tests execute with correct semantics
  - [ ] **LLVM Support**: LLVM codegen for test semantics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test semantics codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Multiple targets `@test tests @a tests @b` — spec/19-testing.md § Multiple Targets [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — multiple targets parsing
  - [x] **Ori Tests**: `tests/spec/source/file_structure.ori` — test_multi tests @multi_a @multi_b @multi_c; `tests/spec/lexical/comments.ori`
  - [ ] **LLVM Support**: LLVM codegen for multiple test targets
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — multiple targets codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Explicit free-floating tests `tests _` — proposals/approved/incremental-test-execution-proposal.md [done] (verified 2026-03-28)  <!-- unblocks:0.9.1 -->
  - [x] Parser accepts `_` as target in `tests _` — `ori_parse/src/grammar/item/function/mod.rs` line 62
  - [x] AST distinguishes `Targeted(Vec<Name>)` vs `FreeFloating` — empty targets Vec = floating
  - [x] **Rust Tests**: `test_floating_with_underscore` in parser tests; `floating_tests_never_skipped` in change detection tests
  - [ ] **Ori Tests**: `tests/spec/testing/free_floating.ori` — no dedicated Ori spec tests yet
  - [ ] **LLVM Support**: LLVM codegen for free-floating tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — free-floating tests codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 14.3 Test Attributes

- [x] **Implement**: Syntax `#attribute` (new syntax) — spec/19-testing.md § Test Attributes [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — attribute parsing
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — #skip, #fail, #compile_fail all work
  - [ ] **LLVM Support**: LLVM codegen for test attribute syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test attribute syntax codegen

- [x] **Implement**: `#skip("reason")` — spec/19-testing.md § Skip Attribute [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — skip attribute handling
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori`, `tests/spec/expressions/loops.ori` — #skip used to skip unimplemented features
  - [ ] **LLVM Support**: LLVM codegen for skip attribute
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — skip attribute codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Constraints — spec/19-testing.md § Test Attributes
  - [ ] **Rust Tests**: `ori_types/src/check/test_attributes.rs` — constraint validation
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`
  - [ ] **LLVM Support**: LLVM codegen for test constraints
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test constraints codegen

- [ ] **Implement**: Semantics — spec/19-testing.md § Test Attributes
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/testing.rs` — attribute semantics
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`
  - [ ] **LLVM Support**: LLVM codegen for test attribute semantics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test attribute semantics codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 14.4 Test Functions

- [ ] **Implement**: Naming convention — spec/19-testing.md § Test Functions
  - [ ] **Rust Tests**: `ori_types/src/check/test_functions.rs` — naming validation
  - [ ] **Ori Tests**: `tests/spec/testing/naming.ori`
  - [ ] **LLVM Support**: LLVM codegen for test naming convention
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test naming codegen

- [ ] **Implement**: Test body structure — spec/19-testing.md § Test Functions
  - [ ] **Rust Tests**: `ori_types/src/infer/function.rs` — test body type checking
  - [ ] **Ori Tests**: `tests/spec/testing/body.ori`
  - [ ] **LLVM Support**: LLVM codegen for test body structure
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test body codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 14.5 Assertions

> **CROSS-REFERENCE**: Assertion built-in functions (`assert`, `assert_eq`, `assert_ne`, `assert_some`,
> `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`) are implemented in
> **Section 7 (Standard Library)**, section 7.5.
>
> This section focuses on the testing *framework* (test declarations, dependency tracking, test runner).
> The assertions themselves are always-available built-in functions from the prelude.

---

## 14.6 Test Organization

- [ ] **Implement**: Mandatory `_test/` directory — spec/19-testing.md § Test Organization
  - [ ] Compiler error (E0501) when test functions are defined outside `_test/` directories
  - [ ] Error message: "tests must be in a _test/ directory" with help suggesting correct path
  - [ ] **Rust Tests**: `ori_types/src/check/test_organization.rs` — _test/ enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/test_organization.ori`
  - [ ] **LLVM Support**: LLVM codegen for _test/ enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test organization codegen

- [ ] **Implement**: Test file discovery in `_test/` — spec/19-testing.md § Test Organization
  - [ ] Discover `.test.ori` files in `_test/` subdirectories
  - [ ] Wire test targets to source functions across directory boundary
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/import.rs` — _test/ directory handling
  - [ ] **Ori Tests**: `tests/spec/testing/test_files.ori`
  - [ ] **LLVM Support**: LLVM codegen for test file discovery
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test file discovery codegen

- [ ] **Implement**: Testing private items via `::` prefix — spec/19-testing.md § Private Items, spec/18-modules.md § Private Access
  - [ ] `::` imports work from any module (not restricted to test files)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/visibility.rs` — private access via ::
  - [ ] **Ori Tests**: `tests/spec/testing/private.ori`
  - [ ] **LLVM Support**: LLVM codegen for private item imports
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — private item imports codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Migration**: Move existing Ori spec tests to `_test/` directories
  - [ ] Audit `tests/spec/` for any tests defined alongside source
  - [ ] Move tests to corresponding `_test/` subdirectories
  - [ ] Update imports to use relative paths from `_test/`
  - [ ] Verify all tests still pass after migration

---

## 14.7 Test Execution

- [x] **Implement**: Running tests — spec/19-testing.md § Test Execution [done] (2026-02-10)
  - [x] **Rust Tests**: CLI — test runner (`ori test`, `cargo st`)
  - [x] **Ori Tests**: 900+ tests pass across the full test suite
  - [ ] **LLVM Support**: LLVM codegen for test execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test execution codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Test isolation and parallelization — spec/19-testing.md § Test Isolation
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — isolation and parallelization
  - [ ] **Ori Tests**: `tests/spec/testing/isolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for test isolation and parallelization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test isolation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Coverage enforcement — spec/19-testing.md § Coverage Enforcement
  - [ ] **Rust Tests**: `ori_types/src/check/test_coverage.rs` — coverage enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/coverage.ori`
  - [ ] **LLVM Support**: LLVM codegen for coverage enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — coverage enforcement codegen

---

## 14.8 Compile-Fail Tests

- [x] **Implement**: Compile-fail tests — spec/19-testing.md § Compile-Fail Tests, design/11-testing/03-compile-fail-tests.md [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — compile-fail harness
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — #compile_fail("type"), #compile_fail("unknown identifier"); `#fail("message")` also works
  - [ ] **LLVM Support**: LLVM codegen for compile-fail tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — compile-fail tests codegen

---

## 14.9 Dependency-Aware Test Execution

> **PROPOSAL**: `proposals/approved/dependency-aware-testing-proposal.md`

When a function changes, run tests for that function AND tests for all functions that depend on it (callers up the dependency graph). This enables fast, correct incremental testing.

### Test Execution Modes

| Mode | Command | What Runs |
|------|---------|-----------|
| Direct | `ori test --direct` | Tests for changed function only |
| Closure | `ori test` (default) | Changed + all callers (recursive) |
| Full | `ori test --full` | All tests in project |

### 14.9.1 Dependency Graph for Tests

- [ ] **Implement**: Reverse dependency lookup (function → callers)
  - [ ] **Rust Tests**: `oric/src/test/dependency_graph.rs` — reverse lookup
  - [ ] **Ori Tests**: `tests/spec/testing/dependency_graph.ori`
  - [ ] **LLVM Support**: LLVM codegen for reverse dependency lookup
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — reverse dependency lookup codegen

- [ ] **Implement**: Test registry (function → tests that target it)
  - [ ] **Rust Tests**: `oric/src/test/registry.rs` — test registry
  - [ ] **Ori Tests**: `tests/spec/testing/test_registry.ori`
  - [ ] **LLVM Support**: LLVM codegen for test registry
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test registry codegen

### 14.9.2 Reverse Closure Computation

- [ ] **Implement**: Compute reverse transitive closure of changed functions
  - [ ] **Rust Tests**: `oric/src/test/closure.rs` — reverse closure
  - [ ] **Ori Tests**: `tests/spec/testing/reverse_closure.ori`
  - [ ] **LLVM Support**: LLVM codegen for reverse transitive closure
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — reverse closure codegen

- [ ] **Implement**: Filter closure to functions with bound tests
  - [ ] **Rust Tests**: `oric/src/test/closure.rs` — closure filtering
  - [ ] **Ori Tests**: `tests/spec/testing/closure_filter.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure filtering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — closure filtering codegen

### 14.9.3 Execution Modes

- [ ] **Implement**: `--direct` mode (direct tests only)
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — direct mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_direct.ori`
  - [ ] **LLVM Support**: LLVM codegen for direct mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — direct mode codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `--closure` mode (default, changed + callers)
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — closure mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_closure.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — closure mode codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `--full` mode (all tests)
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — full mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_full.ori`
  - [ ] **LLVM Support**: LLVM codegen for full mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — full mode codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.9.4 Change Detection

- [ ] **Implement**: Detect changed functions from source diff
  - [ ] **Rust Tests**: `oric/src/test/change_detection.rs` — diff detection
  - [ ] **Ori Tests**: `tests/spec/testing/change_detection.ori`
  - [ ] **LLVM Support**: LLVM codegen for change detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — change detection codegen

- [ ] **Implement**: `--changed=@func1,@func2` explicit change specification
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — explicit changes
  - [ ] **Ori Tests**: `tests/spec/testing/explicit_changes.ori`
  - [ ] **LLVM Support**: LLVM codegen for explicit change specification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — explicit changes codegen

- [ ] **Implement**: `--dry-run` show what would run without running
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — dry run
  - [ ] **Ori Tests**: `tests/spec/testing/dry_run.ori`
  - [ ] **LLVM Support**: LLVM codegen for dry run
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — dry run codegen

### 14.9.5 Integration Test Handling

Free-floating tests (without `tests @target`) are integration tests:
- Run only in `--full` mode or when explicitly selected
- Not part of dependency closure

- [ ] **Implement**: Distinguish bound tests from free-floating tests
  - [ ] **Rust Tests**: `oric/src/test/registry.rs` — test type detection
  - [ ] **Ori Tests**: `tests/spec/testing/test_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for test type distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test type distinction codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Free-floating tests skip closure mode
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — integration test handling
  - [ ] **Ori Tests**: `tests/spec/testing/integration_tests.ori`
  - [ ] **LLVM Support**: LLVM codegen for free-floating test handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — free-floating test handling codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 14.10 Test Utilities

Identified by comparing Ori's test framework against Go and Rust test frameworks.

### 14.10.1 Filesystem Test Support

Go provides `t.TempDir()` for test isolation. Ori should have similar support.

- [ ] **Implement**: `test_tempdir()` — returns isolated temporary directory, auto-cleaned
  - [ ] **Rust Tests**: `library/std/testing.rs` — tempdir utility
  - [ ] **Ori Tests**: `tests/spec/testing/tempdir.ori`
  - [ ] **LLVM Support**: LLVM codegen for test_tempdir
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test_tempdir codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.10.2 Environment Test Support

Go provides `t.Setenv()` for test-scoped environment variables. Ori should support this via capabilities.

- [ ] **Implement**: `test_setenv(name: str, value: str)` — scoped env var, auto-restored
  - [ ] **Rust Tests**: `library/std/testing.rs` — setenv utility
  - [ ] **Ori Tests**: `tests/spec/testing/setenv.ori`
  - [ ] **LLVM Support**: LLVM codegen for test_setenv
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test_setenv codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.10.3 Test Cleanup Hooks

Go provides `t.Cleanup()` for registering cleanup functions. Ori can leverage capabilities and `with` pattern.

- [ ] **Design**: Cleanup hooks via `with` pattern or explicit registration
  - [ ] **Rust Tests**: `library/std/testing.rs` — cleanup hooks
  - [ ] **Ori Tests**: `tests/spec/testing/cleanup.ori`
  - [ ] **LLVM Support**: LLVM codegen for cleanup hooks
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — cleanup hooks codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.10.4 Helper Function Support

Go provides `t.Helper()` to mark functions as test helpers (improves stack traces).

- [ ] **Implement**: `#test_helper` attribute for better failure reporting
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/testing.rs` — helper attribute
  - [ ] **Ori Tests**: `tests/spec/testing/helper.ori`
  - [ ] **LLVM Support**: LLVM codegen for test_helper attribute
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test_helper attribute codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 14.11 Incremental Test Execution

> **PROPOSAL**: `proposals/approved/incremental-test-execution-proposal.md`

During compilation, targeted tests whose targets (or transitive dependencies) have changed are automatically executed. Free-floating tests (`tests _`) run only via explicit `ori test`.

### 14.11.1 Compilation-Integrated Test Running

- [ ] **Implement**: Run affected targeted tests during `ori check`
  - [ ] Identify changed functions (hash comparison)
  - [ ] Walk dependency graph to find affected tests
  - [ ] Execute targeted tests whose targets changed
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — incremental test integration
  - [ ] **Ori Tests**: `tests/spec/testing/incremental_basic.ori`
  - [ ] **LLVM Support**: LLVM codegen for incremental test execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — incremental test execution codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Non-blocking test failures (default)
  - [ ] Test failures reported but don't block compilation
  - [ ] "Build succeeded with N test failures" output
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — non-blocking mode
  - [ ] **Ori Tests**: `tests/spec/testing/non_blocking.ori`
  - [ ] **LLVM Support**: LLVM codegen for non-blocking test failures
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — non-blocking test failures codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.11.2 CLI Integration

| Command | Behavior |
|---------|----------|
| `ori check` | Compile + run affected targeted tests |
| `ori check --no-test` | Compile only, skip tests |
| `ori check --strict` | Fail build on test failure (for CI) |
| `ori test` | Run all tests (targeted + free-floating) |
| `ori test --only-targeted` | Run only targeted tests |

- [ ] **Implement**: `ori check` runs affected targeted tests
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — check command tests
  - [ ] **Ori Tests**: `tests/spec/testing/cli_check.ori`
  - [ ] **LLVM Support**: LLVM codegen for ori check command
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — ori check codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `--no-test` flag skips test execution
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — no-test flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_no_test.ori`
  - [ ] **LLVM Support**: LLVM codegen for --no-test flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — no-test flag codegen

- [ ] **Implement**: `--strict` flag fails build on test failure
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — strict flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_strict.ori`
  - [ ] **LLVM Support**: LLVM codegen for --strict flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — strict flag codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `--only-targeted` flag for `ori test`
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — only-targeted flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_only_targeted.ori`
  - [ ] **LLVM Support**: LLVM codegen for --only-targeted flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — only-targeted flag codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 14.11.3 Test Result Caching

- [ ] **Implement**: Hash-based test caching
  - [ ] Track hash of each function's normalized AST
  - [ ] Cache test results keyed by dependency hashes
  - [ ] Skip tests when inputs unchanged
  - [ ] **Rust Tests**: `oric/src/test/cache.rs` — caching tests
  - [ ] **Ori Tests**: `tests/spec/testing/result_caching.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash-based test caching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test caching codegen

### 14.11.4 Performance Warnings

- [ ] **Implement**: Slow targeted test warning
  - [ ] Configurable threshold (default 100ms)
  - [ ] Warning suggests `tests _` for slow tests
  - [ ] **Rust Tests**: `oric/src/commands/test.rs` — slow test warning
  - [ ] **Ori Tests**: `tests/spec/testing/slow_warning.ori`
  - [ ] **LLVM Support**: LLVM codegen for slow test warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — slow test warning codegen
  - [ ] **AOT Tests**: No AOT coverage yet

Example warning:
```
warning: targeted test @test_parse took 250ms
  --> src/parser.ori:45
  |
  | Targeted tests run during compilation.
  | Consider making this a free-floating test: tests _
  |
  = hint: targeted tests should complete in <100ms
```

---

## 14.12 Test Execution Model Implementation

> **PROPOSAL**: `proposals/approved/test-execution-model-proposal.md`

This section consolidates the implementation details from the Test Execution Model proposal, which unifies the dependency-aware and incremental test execution proposals.

### 14.12.1 Test Registry Data Structure

The `TestRegistry` tracks test-to-function relationships and caller graphs.

- [ ] **Implement**: `TestRegistry` struct
  - [ ] `tests_for: HashMap<FunctionId, Vec<TestId>>` — function → tests targeting it
  - [ ] `callers: HashMap<FunctionId, HashSet<FunctionId>>` — function → functions that call it
  - [ ] `free_floating: HashSet<TestId>` — tests with `tests _`
  - [ ] **Rust Tests**: `oric/src/test/registry.rs` — registry data structure
  - [ ] **Ori Tests**: `tests/spec/testing/registry.ori`

### 14.12.2 Content Hashing

Content hashing determines when functions have changed.

- [ ] **Implement**: Content hash computation
  - [ ] Hash function body AST (normalized: whitespace and comments stripped, source structure preserved)
  - [ ] Include parameter types and names
  - [ ] Include return type, capability requirements, generic constraints
  - [ ] **Rust Tests**: `oric/src/test/content_hash.rs` — hash computation
  - [ ] **Ori Tests**: `tests/spec/testing/content_hash.ori`

### 14.12.3 Cache Storage and Maintenance

Test results are cached for incremental builds.

- [ ] **Implement**: Cache file format
  - [ ] `.ori/cache/hashes.bin` — FunctionId → content hash
  - [ ] `.ori/cache/deps.bin` — dependency graph (callers map)
  - [ ] `.ori/cache/test-results/` — TestId → TestResult
  - [ ] Binary serialization (bincode or similar) for performance
  - [ ] **Rust Tests**: `oric/src/test/cache.rs` — cache format

- [ ] **Implement**: Test cache maintenance — prune stale entries and auto-invalidate on input hash mismatch
  - [ ] Prune entries for deleted functions on successful build completion
  - [ ] Automatic invalidation via `inputs_hash` mismatch
  - [ ] **Rust Tests**: `oric/src/test/cache.rs` — pruning logic

### 14.12.4 `--clean` Flag Behavior

- [ ] **Implement**: `ori check --clean` flag
  - [ ] Force re-execution of all targeted tests (ignore cache)
  - [ ] Still exclude free-floating tests (they always require `ori test`)
  - [ ] **Rust Tests**: `oric/src/commands/check.rs` — clean flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_clean.ori`

---

## 14.13 Test Pass History Cache

Record the last-passing git commit and timestamp for every test. When a test fails, display regression context: which commit it last passed on and when. This is a **diagnostic aid** — distinct from 14.11/14.12 which are performance optimizations (skip/reorder). No other test runner does this.

**Motivation**: When a test fails, the developer's first question is "when did this break?" Today, the answer requires `git bisect` — a manual, slow process. With pass history, the test runner answers instantly: *"last passed on `46873fe` (2026-03-20, 3 commits ago)"*. Combined with Ori's `@target` annotations, it can even show per-function regression context.

### 14.13.1 Cache Data Model

- [ ] **Implement**: `TestPassEntry` struct — `{ commit: String, timestamp: DateTime<Utc>, backend: Backend }`
  - [ ] Key: `(relative_file_path, test_name, backend)` — triple-keyed so interpreter and LLVM histories are independent
  - [ ] Commit: short SHA from `git rev-parse --short HEAD` (or `"unknown"` outside git repos)
  - [ ] Timestamp: UTC ISO-8601 string (no chrono dependency — use `std::time::SystemTime` formatted manually)
  - [ ] **Rust Tests**: `oric/src/test/pass_history/tests.rs` — entry creation, serialization round-trip

- [ ] **Implement**: `TestPassHistory` struct — in-memory representation of the full cache
  - [ ] `entries: HashMap<(PathBuf, String, Backend), TestPassEntry>` — keyed by (file, test_name, backend)
  - [ ] `load(path: &Path) -> Result<Self>` — deserialize from JSON, return empty on missing/corrupt file
  - [ ] `save(&self, path: &Path) -> Result<()>` — serialize to JSON via temp file + atomic rename
  - [ ] `record_pass(file: PathBuf, test_name: String, backend: Backend, commit: &str)` — upsert entry
  - [ ] `last_pass(file: &Path, test_name: &str, backend: Backend) -> Option<&TestPassEntry>` — lookup
  - [ ] **Rust Tests**: `oric/src/test/pass_history/tests.rs` — load/save round-trip, record_pass upsert, last_pass lookup, corrupt file recovery

### 14.13.2 Cache File Format

- [ ] **Implement**: JSON file at `.ori/test-history.json` — human-readable, debuggable
  - [ ] Version field for forward compatibility: `{ "version": 1, "tests": { ... } }`
  - [ ] Key format: `"relative/path.ori::test_name::interpreter"` (or `::llvm`)
  - [ ] Relative paths (from project root) for portability across machines
  - [ ] **Rust Tests**: `oric/src/test/pass_history/tests.rs` — JSON format validation, version handling

- [ ] **Implement**: Add `serde` and `serde_json` dependencies to `oric` Cargo.toml
  - [ ] `serde = { version = "1", features = ["derive"] }`
  - [ ] `serde_json = "1"`
  - [ ] Derive `Serialize`/`Deserialize` on `TestPassEntry` and `TestPassHistory`

- [ ] **Implement**: Add `.ori/` to `.gitignore` — cache is machine-local, not committed

### 14.13.3 Git Integration

- [ ] **Implement**: `current_git_commit() -> Option<String>` — query git for short HEAD SHA
  - [ ] Run `git rev-parse --short HEAD` via `std::process::Command`
  - [ ] Return `None` if not in a git repo or git not installed (graceful degradation)
  - [ ] Cache the result for the duration of the test run (single subprocess call)
  - [ ] **Rust Tests**: `oric/src/test/pass_history/tests.rs` — git query (integration test, `#[ignore]` if no git)

### 14.13.4 TestRunner Integration

- [ ] **Implement**: Load pass history at start of `TestRunner::run()`
  - [ ] Resolve `.ori/test-history.json` relative to project root (walk up from test path to find `.git` or `Cargo.toml`)
  - [ ] Load existing history (or empty if first run)
  - [ ] Query `current_git_commit()` once
  - [ ] Pass history + commit to `run_file_with_interner()` via parameter or shared state

- [ ] **Implement**: Record passes after each file completes
  - [ ] For each `TestOutcome::Passed` in `FileSummary`, call `history.record_pass(...)`
  - [ ] Use the resolved relative path and interner-looked-up test name

- [ ] **Implement**: Save history at end of `TestRunner::run()`
  - [ ] Create `.ori/` directory if it doesn't exist
  - [ ] Write via atomic temp file + rename
  - [ ] Failures to save are warnings (logged via `tracing::warn!`), never fatal

- [ ] **Implement**: Pass history to failure reporting path
  - [ ] `TestResult` or `FileSummary` carries `Option<TestPassEntry>` for failed tests
  - [ ] Looked up from history after test execution, before reporting

### 14.13.5 Failure Output Enhancement

- [ ] **Implement**: Enhanced failure message in `print_file_results()` (`commands/test.rs`)
  - [ ] On `FAIL`, look up `last_pass` from history
  - [ ] If found: `FAIL: test_name - error message\n        last passed: abc1234 (2026-03-20 14:30 UTC)`
  - [ ] If not found (first run or never passed): no extra line (silent)
  - [ ] **Rust Tests**: `oric/src/commands/test/tests.rs` — output formatting with history context

- [ ] **Implement**: Optional "N commits ago" annotation with `--verbose`
  - [ ] Run `git rev-list --count <last_pass_commit>..HEAD` to compute distance
  - [ ] Only in verbose mode (adds a subprocess call per failure)
  - [ ] Format: `last passed: abc1234 (2026-03-20 14:30 UTC, 3 commits ago)`
  - [ ] Graceful degradation: if git call fails, omit the "N commits ago" part
  - [ ] **Rust Tests**: `oric/src/commands/test/tests.rs` — verbose commit distance

### 14.13.6 Cache Maintenance

- [ ] **Implement**: Stale entry tolerance — never prune automatically
  - [ ] Old entries for deleted tests waste bytes but cause no harm
  - [ ] No TTL, no LRU — the cache is append/upsert only
  - [ ] Manual cleanup: `ori test --clear-history` deletes `.ori/test-history.json`

- [ ] **Implement**: `--clear-history` CLI flag
  - [ ] Deletes the cache file and starts fresh
  - [ ] **Rust Tests**: `oric/src/commands/test/tests.rs` — clear history flag

---

## 14.14 Section Completion Checklist

- [ ] All items in 14.1-14.13 have all three checkboxes marked `[ ]`
- [ ] Spec updated: `spec/19-testing.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: Tests are mandatory, dependency-aware, and run correctly
