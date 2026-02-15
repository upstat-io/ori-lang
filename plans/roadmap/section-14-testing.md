---
section: 14
title: Testing Framework
status: in-progress
tier: 5
goal: Mandatory testing enforcement with dependency-aware execution and incremental test execution during compilation
spec:
  - spec/13-testing.md
sections:
  - id: "14.1"
    title: Test Requirement
    status: not-started
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
    title: Section Completion Checklist
    status: not-started
---

# Section 14: Testing Framework

**Goal**: Mandatory testing enforcement with dependency-aware execution and incremental test execution during compilation

> **SPEC**: `spec/13-testing.md`
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

- [ ] **Implement**: Every function must have tests — spec/13-testing.md § Test Requirements, design/11-testing/01-mandatory-tests.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — mandatory test enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/mandatory.ori`
  - [ ] **LLVM Support**: LLVM codegen for mandatory test enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — mandatory test enforcement codegen

- [ ] **Implement**: Exemptions (`@main`, private helpers) — spec/13-testing.md § Exemptions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — exemption rules
  - [ ] **Ori Tests**: `tests/spec/testing/exemptions.ori`
  - [ ] **LLVM Support**: LLVM codegen for test exemptions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test exemptions codegen

---

## 14.2 Test Declaration

- [x] **Implement**: Syntax `@test_name tests @target () -> void = ...` — spec/13-testing.md § Test Declaration, design/11-testing/02-test-syntax.md ✅ (2026-02-10)
  - [x] **Rust Tests**: Parser — test declaration parsing
  - [x] **Ori Tests**: All spec tests use this syntax (900+ tests across the test suite)
  - [ ] **LLVM Support**: LLVM codegen for test declaration syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test declaration codegen

- [x] **Implement**: Semantics — spec/13-testing.md § Test Declaration ✅ (2026-02-10)
  - [x] **Rust Tests**: Evaluator — test semantics
  - [x] **Ori Tests**: All spec tests execute with correct semantics
  - [ ] **LLVM Support**: LLVM codegen for test semantics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test semantics codegen

- [x] **Implement**: Multiple targets `@test tests @a tests @b` — spec/13-testing.md § Multiple Targets ✅ (2026-02-10)
  - [x] **Rust Tests**: Parser — multiple targets parsing
  - [x] **Ori Tests**: `tests/spec/source/file_structure.ori` — test_multi tests @multi_a @multi_b @multi_c; `tests/spec/lexical/comments.ori`
  - [ ] **LLVM Support**: LLVM codegen for multiple test targets
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — multiple targets codegen

- [ ] **Implement**: Explicit free-floating tests `tests _` — proposals/approved/incremental-test-execution-proposal.md  <!-- unblocks:0.9.1 -->
  - [ ] Parser accepts `_` as target in `tests _`
  - [ ] AST distinguishes `Targeted(Vec<Name>)` vs `FreeFloating`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/function.rs` — free-floating test parsing
  - [ ] **Ori Tests**: `tests/spec/testing/free_floating.ori`
  - [ ] **LLVM Support**: LLVM codegen for free-floating tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — free-floating tests codegen

---

## 14.3 Test Attributes

- [x] **Implement**: Syntax `#attribute` (new syntax) — spec/13-testing.md § Test Attributes ✅ (2026-02-10)
  - [x] **Rust Tests**: Parser — attribute parsing
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — #skip, #fail, #compile_fail all work
  - [ ] **LLVM Support**: LLVM codegen for test attribute syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test attribute syntax codegen

- [x] **Implement**: `#skip("reason")` — spec/13-testing.md § Skip Attribute ✅ (2026-02-10)
  - [x] **Rust Tests**: Evaluator — skip attribute handling
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori`, `tests/spec/expressions/loops.ori` — #skip used to skip unimplemented features
  - [ ] **LLVM Support**: LLVM codegen for skip attribute
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — skip attribute codegen

- [ ] **Implement**: Constraints — spec/13-testing.md § Test Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_attributes.rs` — constraint validation
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`
  - [ ] **LLVM Support**: LLVM codegen for test constraints
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test constraints codegen

- [ ] **Implement**: Semantics — spec/13-testing.md § Test Attributes
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — attribute semantics
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`
  - [ ] **LLVM Support**: LLVM codegen for test attribute semantics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test attribute semantics codegen

---

## 14.4 Test Functions

- [ ] **Implement**: Naming convention — spec/13-testing.md § Test Functions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_functions.rs` — naming validation
  - [ ] **Ori Tests**: `tests/spec/testing/naming.ori`
  - [ ] **LLVM Support**: LLVM codegen for test naming convention
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test naming codegen

- [ ] **Implement**: Test body structure — spec/13-testing.md § Test Functions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/function.rs` — test body type checking
  - [ ] **Ori Tests**: `tests/spec/testing/body.ori`
  - [ ] **LLVM Support**: LLVM codegen for test body structure
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test body codegen

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

- [ ] **Implement**: Mandatory `_test/` directory — spec/13-testing.md § Test Organization
  - [ ] Compiler error (E0501) when test functions are defined outside `_test/` directories
  - [ ] Error message: "tests must be in a _test/ directory" with help suggesting correct path
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_organization.rs` — _test/ enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/test_organization.ori`
  - [ ] **LLVM Support**: LLVM codegen for _test/ enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test organization codegen

- [ ] **Implement**: Test file discovery in `_test/` — spec/13-testing.md § Test Organization
  - [ ] Discover `.test.ori` files in `_test/` subdirectories
  - [ ] Wire test targets to source functions across directory boundary
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — _test/ directory handling
  - [ ] **Ori Tests**: `tests/spec/testing/test_files.ori`
  - [ ] **LLVM Support**: LLVM codegen for test file discovery
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test file discovery codegen

- [ ] **Implement**: Testing private items via `::` prefix — spec/13-testing.md § Private Items, spec/12-modules.md § Private Access
  - [ ] `::` imports work from any module (not restricted to test files)
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — private access via ::
  - [ ] **Ori Tests**: `tests/spec/testing/private.ori`
  - [ ] **LLVM Support**: LLVM codegen for private item imports
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — private item imports codegen

- [ ] **Migration**: Move existing Ori spec tests to `_test/` directories
  - [ ] Audit `tests/spec/` for any tests defined alongside source
  - [ ] Move tests to corresponding `_test/` subdirectories
  - [ ] Update imports to use relative paths from `_test/`
  - [ ] Verify all tests still pass after migration

---

## 14.7 Test Execution

- [x] **Implement**: Running tests — spec/13-testing.md § Test Execution ✅ (2026-02-10)
  - [x] **Rust Tests**: CLI — test runner (`ori test`, `cargo st`)
  - [x] **Ori Tests**: 900+ tests pass across the full test suite
  - [ ] **LLVM Support**: LLVM codegen for test execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test execution codegen

- [ ] **Implement**: Test isolation and parallelization — spec/13-testing.md § Test Isolation
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — isolation and parallelization
  - [ ] **Ori Tests**: `tests/spec/testing/isolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for test isolation and parallelization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test isolation codegen

- [ ] **Implement**: Coverage enforcement — spec/13-testing.md § Coverage Enforcement
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — coverage enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/coverage.ori`
  - [ ] **LLVM Support**: LLVM codegen for coverage enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — coverage enforcement codegen

---

## 14.8 Compile-Fail Tests

- [x] **Implement**: Compile-fail tests — spec/13-testing.md § Compile-Fail Tests, design/11-testing/03-compile-fail-tests.md ✅ (2026-02-10)
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
  - [ ] **Rust Tests**: `oric/src/analysis/dependency_graph.rs` — reverse lookup
  - [ ] **Ori Tests**: `tests/spec/testing/dependency_graph.ori`
  - [ ] **LLVM Support**: LLVM codegen for reverse dependency lookup
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — reverse dependency lookup codegen

- [ ] **Implement**: Test registry (function → tests that target it)
  - [ ] **Rust Tests**: `oric/src/analysis/test_registry.rs` — test registry
  - [ ] **Ori Tests**: `tests/spec/testing/test_registry.ori`
  - [ ] **LLVM Support**: LLVM codegen for test registry
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test registry codegen

### 14.9.2 Reverse Closure Computation

- [ ] **Implement**: Compute reverse transitive closure of changed functions
  - [ ] **Rust Tests**: `oric/src/analysis/closure.rs` — reverse closure
  - [ ] **Ori Tests**: `tests/spec/testing/reverse_closure.ori`
  - [ ] **LLVM Support**: LLVM codegen for reverse transitive closure
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — reverse closure codegen

- [ ] **Implement**: Filter closure to functions with bound tests
  - [ ] **Rust Tests**: `oric/src/analysis/closure.rs` — closure filtering
  - [ ] **Ori Tests**: `tests/spec/testing/closure_filter.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure filtering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — closure filtering codegen

### 14.9.3 Execution Modes

- [ ] **Implement**: `--direct` mode (direct tests only)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — direct mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_direct.ori`
  - [ ] **LLVM Support**: LLVM codegen for direct mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — direct mode codegen

- [ ] **Implement**: `--closure` mode (default, changed + callers)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — closure mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_closure.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — closure mode codegen

- [ ] **Implement**: `--full` mode (all tests)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — full mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_full.ori`
  - [ ] **LLVM Support**: LLVM codegen for full mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — full mode codegen

### 14.9.4 Change Detection

- [ ] **Implement**: Detect changed functions from source diff
  - [ ] **Rust Tests**: `oric/src/analysis/change_detection.rs` — diff detection
  - [ ] **Ori Tests**: `tests/spec/testing/change_detection.ori`
  - [ ] **LLVM Support**: LLVM codegen for change detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — change detection codegen

- [ ] **Implement**: `--changed=@func1,@func2` explicit change specification
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — explicit changes
  - [ ] **Ori Tests**: `tests/spec/testing/explicit_changes.ori`
  - [ ] **LLVM Support**: LLVM codegen for explicit change specification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — explicit changes codegen

- [ ] **Implement**: `--dry-run` show what would run without running
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — dry run
  - [ ] **Ori Tests**: `tests/spec/testing/dry_run.ori`
  - [ ] **LLVM Support**: LLVM codegen for dry run
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — dry run codegen

### 14.9.5 Integration Test Handling

Free-floating tests (without `tests @target`) are integration tests:
- Run only in `--full` mode or when explicitly selected
- Not part of dependency closure

- [ ] **Implement**: Distinguish bound tests from free-floating tests
  - [ ] **Rust Tests**: `oric/src/analysis/test_registry.rs` — test type detection
  - [ ] **Ori Tests**: `tests/spec/testing/test_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for test type distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test type distinction codegen

- [ ] **Implement**: Free-floating tests skip closure mode
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — integration test handling
  - [ ] **Ori Tests**: `tests/spec/testing/integration_tests.ori`
  - [ ] **LLVM Support**: LLVM codegen for free-floating test handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — free-floating test handling codegen

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

### 14.10.2 Environment Test Support

Go provides `t.Setenv()` for test-scoped environment variables. Ori should support this via capabilities.

- [ ] **Implement**: `test_setenv(name: str, value: str)` — scoped env var, auto-restored
  - [ ] **Rust Tests**: `library/std/testing.rs` — setenv utility
  - [ ] **Ori Tests**: `tests/spec/testing/setenv.ori`
  - [ ] **LLVM Support**: LLVM codegen for test_setenv
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test_setenv codegen

### 14.10.3 Test Cleanup Hooks

Go provides `t.Cleanup()` for registering cleanup functions. Ori can leverage capabilities and `with` pattern.

- [ ] **Design**: Cleanup hooks via `with` pattern or explicit registration
  - [ ] **Rust Tests**: `library/std/testing.rs` — cleanup hooks
  - [ ] **Ori Tests**: `tests/spec/testing/cleanup.ori`
  - [ ] **LLVM Support**: LLVM codegen for cleanup hooks
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — cleanup hooks codegen

### 14.10.4 Helper Function Support

Go provides `t.Helper()` to mark functions as test helpers (improves stack traces).

- [ ] **Implement**: `#test_helper` attribute for better failure reporting
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — helper attribute
  - [ ] **Ori Tests**: `tests/spec/testing/helper.ori`
  - [ ] **LLVM Support**: LLVM codegen for test_helper attribute
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test_helper attribute codegen

---

## 14.11 Incremental Test Execution

> **PROPOSAL**: `proposals/approved/incremental-test-execution-proposal.md`

During compilation, targeted tests whose targets (or transitive dependencies) have changed are automatically executed. Free-floating tests (`tests _`) run only via explicit `ori test`.

### 14.11.1 Compilation-Integrated Test Running

- [ ] **Implement**: Run affected targeted tests during `ori check`
  - [ ] Identify changed functions (hash comparison)
  - [ ] Walk dependency graph to find affected tests
  - [ ] Execute targeted tests whose targets changed
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — incremental test integration
  - [ ] **Ori Tests**: `tests/spec/testing/incremental_basic.ori`
  - [ ] **LLVM Support**: LLVM codegen for incremental test execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — incremental test execution codegen

- [ ] **Implement**: Non-blocking test failures (default)
  - [ ] Test failures reported but don't block compilation
  - [ ] "Build succeeded with N test failures" output
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — non-blocking mode
  - [ ] **Ori Tests**: `tests/spec/testing/non_blocking.ori`
  - [ ] **LLVM Support**: LLVM codegen for non-blocking test failures
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — non-blocking test failures codegen

### 14.11.2 CLI Integration

| Command | Behavior |
|---------|----------|
| `ori check` | Compile + run affected targeted tests |
| `ori check --no-test` | Compile only, skip tests |
| `ori check --strict` | Fail build on test failure (for CI) |
| `ori test` | Run all tests (targeted + free-floating) |
| `ori test --only-targeted` | Run only targeted tests |

- [ ] **Implement**: `ori check` runs affected targeted tests
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — check command tests
  - [ ] **Ori Tests**: `tests/spec/testing/cli_check.ori`
  - [ ] **LLVM Support**: LLVM codegen for ori check command
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — ori check codegen

- [ ] **Implement**: `--no-test` flag skips test execution
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — no-test flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_no_test.ori`
  - [ ] **LLVM Support**: LLVM codegen for --no-test flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — no-test flag codegen

- [ ] **Implement**: `--strict` flag fails build on test failure
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — strict flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_strict.ori`
  - [ ] **LLVM Support**: LLVM codegen for --strict flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — strict flag codegen

- [ ] **Implement**: `--only-targeted` flag for `ori test`
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — only-targeted flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_only_targeted.ori`
  - [ ] **LLVM Support**: LLVM codegen for --only-targeted flag
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — only-targeted flag codegen

### 14.11.3 Test Result Caching

- [ ] **Implement**: Hash-based test caching
  - [ ] Track hash of each function's normalized AST
  - [ ] Cache test results keyed by dependency hashes
  - [ ] Skip tests when inputs unchanged
  - [ ] **Rust Tests**: `oric/src/analysis/test_cache.rs` — caching tests
  - [ ] **Ori Tests**: `tests/spec/testing/result_caching.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash-based test caching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — test caching codegen

### 14.11.4 Performance Warnings

- [ ] **Implement**: Slow targeted test warning
  - [ ] Configurable threshold (default 100ms)
  - [ ] Warning suggests `tests _` for slow tests
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — slow test warning
  - [ ] **Ori Tests**: `tests/spec/testing/slow_warning.ori`
  - [ ] **LLVM Support**: LLVM codegen for slow test warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_framework_tests.rs` — slow test warning codegen

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
  - [ ] **Rust Tests**: `oric/src/analysis/test_registry.rs` — registry data structure
  - [ ] **Ori Tests**: `tests/spec/testing/registry.ori`

### 14.12.2 Content Hashing

Content hashing determines when functions have changed.

- [ ] **Implement**: Content hash computation
  - [ ] Hash function body AST (normalized: whitespace and comments stripped, source structure preserved)
  - [ ] Include parameter types and names
  - [ ] Include return type, capability requirements, generic constraints
  - [ ] **Rust Tests**: `oric/src/analysis/content_hash.rs` — hash computation
  - [ ] **Ori Tests**: `tests/spec/testing/content_hash.ori`

### 14.12.3 Cache Storage and Maintenance

Test results are cached for incremental builds.

- [ ] **Implement**: Cache file format
  - [ ] `.ori/cache/hashes.bin` — FunctionId → content hash
  - [ ] `.ori/cache/deps.bin` — dependency graph (callers map)
  - [ ] `.ori/cache/test-results/` — TestId → TestResult
  - [ ] Binary serialization (bincode or similar) for performance
  - [ ] **Rust Tests**: `oric/src/cache/test_cache.rs` — cache format

- [ ] **Implement**: Cache maintenance
  - [ ] Prune entries for deleted functions on successful build completion
  - [ ] Automatic invalidation via `inputs_hash` mismatch
  - [ ] **Rust Tests**: `oric/src/cache/test_cache.rs` — pruning logic

### 14.12.4 `--clean` Flag Behavior

- [ ] **Implement**: `ori check --clean` flag
  - [ ] Force re-execution of all targeted tests (ignore cache)
  - [ ] Still exclude free-floating tests (they always require `ori test`)
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — clean flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_clean.ori`

---

## 14.13 Section Completion Checklist

- [ ] All items in 14.1-14.12 have all three checkboxes marked `[ ]`
- [ ] Spec updated: `spec/13-testing.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: Tests are mandatory, dependency-aware, and run correctly
