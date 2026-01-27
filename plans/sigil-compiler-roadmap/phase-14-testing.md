# Phase 14: Testing Framework

**Goal**: Mandatory testing enforcement with dependency-aware execution and incremental test execution during compilation

> **SPEC**: `spec/13-testing.md`
> **DESIGN**: `design/11-testing/index.md`
> **PROPOSALS**:
> - `proposals/approved/dependency-aware-testing-proposal.md` — Dependency-aware test execution
> - `proposals/approved/incremental-test-execution-proposal.md` — Incremental test execution & explicit free-floating tests

> **NOTE - Pending Syntax Changes**: The approved proposals change attribute syntax:
> - Attribute syntax: `#[skip("reason")]` → `#skip("reason")` (Phase 15.1)
> See Phase 15 (Approved Syntax Proposals) for details. Implement with new syntax directly to avoid migration.

---

## 14.1 Test Requirement

- [ ] **Implement**: Every function must have tests — spec/13-testing.md § Test Requirements, design/11-testing/01-mandatory-tests.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — mandatory test enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/mandatory.ori`

- [ ] **Implement**: Exemptions (`@main`, private helpers) — spec/13-testing.md § Exemptions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — exemption rules
  - [ ] **Ori Tests**: `tests/spec/testing/exemptions.ori`

---

## 14.2 Test Declaration

- [ ] **Implement**: Syntax `@test_name tests @target () -> void = ...` — spec/13-testing.md § Test Declaration, design/11-testing/02-test-syntax.md
  - [ ] **Rust Tests**: `ori_parse/src/grammar/function.rs` — test declaration parsing
  - [ ] **Ori Tests**: `tests/spec/testing/declaration.ori`

- [ ] **Implement**: Semantics — spec/13-testing.md § Test Declaration
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — test semantics
  - [ ] **Ori Tests**: `tests/spec/testing/declaration.ori`

- [ ] **Implement**: Multiple targets `@test tests @a tests @b` — spec/13-testing.md § Multiple Targets
  - [ ] **Rust Tests**: `ori_parse/src/grammar/function.rs` — multiple targets parsing
  - [ ] **Ori Tests**: `tests/spec/testing/multiple_targets.ori`

- [ ] **Implement**: Explicit free-floating tests `tests _` — proposals/approved/incremental-test-execution-proposal.md
  - [ ] Parser accepts `_` as target in `tests _`
  - [ ] AST distinguishes `Targeted(Vec<Name>)` vs `FreeFloating`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/function.rs` — free-floating test parsing
  - [ ] **Ori Tests**: `tests/spec/testing/free_floating.ori`

---

## 14.3 Test Attributes

- [ ] **Implement**: Syntax `#[attribute]` — spec/13-testing.md § Test Attributes
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — attribute parsing
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`

- [ ] **Implement**: `#[skip("reason")]` — spec/13-testing.md § Skip Attribute
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — skip attribute handling
  - [ ] **Ori Tests**: `tests/spec/testing/skip.ori`

- [ ] **Implement**: Constraints — spec/13-testing.md § Test Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_attributes.rs` — constraint validation
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`

- [ ] **Implement**: Semantics — spec/13-testing.md § Test Attributes
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — attribute semantics
  - [ ] **Ori Tests**: `tests/spec/testing/attributes.ori`

---

## 14.4 Test Functions

- [ ] **Implement**: Naming convention — spec/13-testing.md § Test Functions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_functions.rs` — naming validation
  - [ ] **Ori Tests**: `tests/spec/testing/naming.ori`

- [ ] **Implement**: Test body structure — spec/13-testing.md § Test Functions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/function.rs` — test body type checking
  - [ ] **Ori Tests**: `tests/spec/testing/body.ori`

---

## 14.5 Assertions

> **CROSS-REFERENCE**: Assertion built-in functions (`assert`, `assert_eq`, `assert_ne`, `assert_some`,
> `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`) are implemented in
> **Phase 7 (Standard Library)**, section 7.5.
>
> This phase focuses on the testing *framework* (test declarations, dependency tracking, test runner).
> The assertions themselves are always-available built-in functions from the prelude.

---

## 14.6 Test Organization

- [ ] **Implement**: Inline tests — spec/13-testing.md § Test Organization
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — inline test discovery
  - [ ] **Ori Tests**: `tests/spec/testing/inline.ori`

- [ ] **Implement**: Test files `_test/` — spec/13-testing.md § Test Files, design/11-testing/index.md
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — _test/ directory handling
  - [ ] **Ori Tests**: `tests/spec/testing/test_files.ori`

- [ ] **Implement**: Testing private items — spec/13-testing.md § Private Items
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — test private access
  - [ ] **Ori Tests**: `tests/spec/testing/private.ori`

---

## 14.7 Test Execution

- [ ] **Implement**: Running tests — spec/13-testing.md § Test Execution
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — test runner
  - [ ] **Ori Tests**: `tests/spec/testing/execution.ori`

- [ ] **Implement**: Test isolation and parallelization — spec/13-testing.md § Test Isolation
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — isolation and parallelization
  - [ ] **Ori Tests**: `tests/spec/testing/isolation.ori`

- [ ] **Implement**: Coverage enforcement — spec/13-testing.md § Coverage Enforcement
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test_coverage.rs` — coverage enforcement
  - [ ] **Ori Tests**: `tests/spec/testing/coverage.ori`

---

## 14.8 Compile-Fail Tests

- [ ] **Implement**: Compile-fail tests — spec/13-testing.md § Compile-Fail Tests, design/11-testing/03-compile-fail-tests.md
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — compile-fail harness
  - [ ] **Ori Tests**: `tests/spec/testing/compile_fail.ori`

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

- [ ] **Implement**: Test registry (function → tests that target it)
  - [ ] **Rust Tests**: `oric/src/analysis/test_registry.rs` — test registry
  - [ ] **Ori Tests**: `tests/spec/testing/test_registry.ori`

### 14.9.2 Reverse Closure Computation

- [ ] **Implement**: Compute reverse transitive closure of changed functions
  - [ ] **Rust Tests**: `oric/src/analysis/closure.rs` — reverse closure
  - [ ] **Ori Tests**: `tests/spec/testing/reverse_closure.ori`

- [ ] **Implement**: Filter closure to functions with bound tests
  - [ ] **Rust Tests**: `oric/src/analysis/closure.rs` — closure filtering
  - [ ] **Ori Tests**: `tests/spec/testing/closure_filter.ori`

### 14.9.3 Execution Modes

- [ ] **Implement**: `--direct` mode (direct tests only)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — direct mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_direct.ori`

- [ ] **Implement**: `--closure` mode (default, changed + callers)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — closure mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_closure.ori`

- [ ] **Implement**: `--full` mode (all tests)
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — full mode
  - [ ] **Ori Tests**: `tests/spec/testing/mode_full.ori`

### 14.9.4 Change Detection

- [ ] **Implement**: Detect changed functions from source diff
  - [ ] **Rust Tests**: `oric/src/analysis/change_detection.rs` — diff detection
  - [ ] **Ori Tests**: `tests/spec/testing/change_detection.ori`

- [ ] **Implement**: `--changed=@func1,@func2` explicit change specification
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — explicit changes
  - [ ] **Ori Tests**: `tests/spec/testing/explicit_changes.ori`

- [ ] **Implement**: `--dry-run` show what would run without running
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — dry run
  - [ ] **Ori Tests**: `tests/spec/testing/dry_run.ori`

### 14.9.5 Integration Test Handling

Free-floating tests (without `tests @target`) are integration tests:
- Run only in `--full` mode or when explicitly selected
- Not part of dependency closure

- [ ] **Implement**: Distinguish bound tests from free-floating tests
  - [ ] **Rust Tests**: `oric/src/analysis/test_registry.rs` — test type detection
  - [ ] **Ori Tests**: `tests/spec/testing/test_types.ori`

- [ ] **Implement**: Free-floating tests skip closure mode
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — integration test handling
  - [ ] **Ori Tests**: `tests/spec/testing/integration_tests.ori`

---

## 14.10 Test Utilities

Identified by comparing Ori's test framework against Go and Rust test frameworks.

### 14.10.1 Filesystem Test Support

Go provides `t.TempDir()` for test isolation. Ori should have similar support.

- [ ] **Implement**: `test_tempdir()` — returns isolated temporary directory, auto-cleaned
  - [ ] **Rust Tests**: `library/std/testing.rs` — tempdir utility
  - [ ] **Ori Tests**: `tests/spec/testing/tempdir.ori`

### 14.10.2 Environment Test Support

Go provides `t.Setenv()` for test-scoped environment variables. Ori should support this via capabilities.

- [ ] **Implement**: `test_setenv(name: str, value: str)` — scoped env var, auto-restored
  - [ ] **Rust Tests**: `library/std/testing.rs` — setenv utility
  - [ ] **Ori Tests**: `tests/spec/testing/setenv.ori`

### 14.10.3 Test Cleanup Hooks

Go provides `t.Cleanup()` for registering cleanup functions. Ori can leverage capabilities and `with` pattern.

- [ ] **Design**: Cleanup hooks via `with` pattern or explicit registration
  - [ ] **Rust Tests**: `library/std/testing.rs` — cleanup hooks
  - [ ] **Ori Tests**: `tests/spec/testing/cleanup.ori`

### 14.10.4 Helper Function Support

Go provides `t.Helper()` to mark functions as test helpers (improves stack traces).

- [ ] **Implement**: `#test_helper` attribute for better failure reporting
  - [ ] **Rust Tests**: `oric/src/eval/testing.rs` — helper attribute
  - [ ] **Ori Tests**: `tests/spec/testing/helper.ori`

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

- [ ] **Implement**: Non-blocking test failures (default)
  - [ ] Test failures reported but don't block compilation
  - [ ] "Build succeeded with N test failures" output
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — non-blocking mode
  - [ ] **Ori Tests**: `tests/spec/testing/non_blocking.ori`

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

- [ ] **Implement**: `--no-test` flag skips test execution
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — no-test flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_no_test.ori`

- [ ] **Implement**: `--strict` flag fails build on test failure
  - [ ] **Rust Tests**: `oric/src/cli/check.rs` — strict flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_strict.ori`

- [ ] **Implement**: `--only-targeted` flag for `ori test`
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — only-targeted flag
  - [ ] **Ori Tests**: `tests/spec/testing/cli_only_targeted.ori`

### 14.11.3 Test Result Caching

- [ ] **Implement**: Hash-based test caching
  - [ ] Track hash of each function's normalized AST
  - [ ] Cache test results keyed by dependency hashes
  - [ ] Skip tests when inputs unchanged
  - [ ] **Rust Tests**: `oric/src/analysis/test_cache.rs` — caching tests
  - [ ] **Ori Tests**: `tests/spec/testing/result_caching.ori`

### 14.11.4 Performance Warnings

- [ ] **Implement**: Slow targeted test warning
  - [ ] Configurable threshold (default 100ms)
  - [ ] Warning suggests `tests _` for slow tests
  - [ ] **Rust Tests**: `oric/src/cli/test.rs` — slow test warning
  - [ ] **Ori Tests**: `tests/spec/testing/slow_warning.ori`

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

## 14.12 Phase Completion Checklist

- [ ] All items in 14.1-14.11 have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/13-testing.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: Tests are mandatory, dependency-aware, and run correctly
