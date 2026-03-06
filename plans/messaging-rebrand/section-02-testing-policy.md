---
section: "02"
title: "Testing Policy — Configurable Enforcement"
status: not-started
goal: "Make test enforcement a project-level configuration, not a hard compiler requirement"
depends_on: []
sections:
  - id: "02.1"
    title: "Policy Design"
    status: not-started
  - id: "02.2"
    title: "Compiler Changes"
    status: not-started
  - id: "02.3"
    title: "Default Behavior"
    status: not-started
  - id: "02.4"
    title: "Test Strategy"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Testing Policy — Configurable Enforcement

**Status:** Not Started
**Goal:** Transform test enforcement from a hard compiler error into a configurable project-level policy, while preserving all testing infrastructure (dependency-aware execution, capability-based mocking, `tests @target` syntax).

**Context:** Mandatory testing was the most polarizing feature in user group feedback. The testing infrastructure itself is innovative — dep-graph-aware execution, capability-based mocking — but the mandate felt prescriptive. Making it configurable preserves the infrastructure while removing the friction.

---

## 02.1 Policy Design

### What Changes

**Before:** Every function (except `@main`) without tests produces a compiler error. No way to opt out.

**After:** Test enforcement is a project-level setting with three modes:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `"off"` | No enforcement. Tests optional. (Default for new projects) | Scripting, prototypes, personal projects |
| `"warn"` | Missing tests produce warnings, not errors | Growing projects, gradual adoption |
| `"error"` | Missing tests are compiler errors (current behavior) | Production codebases, teams |

### Configuration

```toml
# oripk.toml (does not exist yet — config system must be built)
[project]
test-enforcement = "off"    # "off" | "warn" | "error"
```

For single-file mode (`ori run file.ori` without `oripk.toml`): always `"off"`.

### CLI Override

```bash
ori check --test-enforcement=error file.ori   # Override for CI
ori check --test-enforcement=off file.ori     # Override for quick checks
```

- [ ] Confirm three-mode design (off/warn/error)
- [ ] Confirm default is `"off"` for new projects
- [ ] Confirm CLI override flag name

---

## 02.2 Compiler Changes

### Affected Files

The test enforcement check currently lives in the `oric` crate (not `ori_types`). Changes needed:

1. **`compiler/oric/`** — Parse `test-enforcement` from `oripk.toml` config and CLI flag
2. **`compiler/oric/src/problem/semantic/mod.rs`** — `check_test_coverage()` produces `SemanticProblem::MissingTest` which emits `E3001`. Change severity from hardcoded `Diagnostic::error()` to configurable based on enforcement level
3. **`compiler/oric/src/commands/check.rs`** and **`watch.rs`** — Thread config to `check_test_coverage` call sites
4. **`compiler/ori_diagnostic/`** — No changes needed; `Diagnostic` already supports `error()`, `warning()`, and severity levels

### Implementation Approach

> **WARNING: BLOAT — `semantic/mod.rs` is 511 lines (over 500-line limit).** Before implementing
> any severity switch, extract `check_test_coverage()` and test-related diagnostic rendering
> (`MissingTest`, `TestTargetNotFound`) into `semantic/test_coverage.rs`. This is a prerequisite,
> not optional cleanup.

**Option A (Recommended): Diagnostic severity switch**
- The existing `SemanticProblem::MissingTest` diagnostic already exists (in `oric/src/problem/semantic/mod.rs`, line ~331)
- Change its severity from hardcoded `Diagnostic::error(ErrorCode::E3001)` to configurable `error | warning | suppressed`
- Controlled by a config value threaded through the `oric` commands context (check.rs, watch.rs)
- Estimated ~100-150 lines total (config enum, severity switch, call site changes)

**Option B: Separate lint pass**
- Extract `check_test_coverage()` from `oric/src/problem/semantic/mod.rs` into a separate lint pass
- More architectural change but cleaner separation
- Higher effort (~200 lines)

**Recommended:** Option A. The diagnostic already exists; changing its severity is the minimal change.

### Where the severity switch lives

Two sub-options for Option A:

**A1: Severity in `check_test_coverage()` itself** — add an `enforcement: TestEnforcement` parameter. When `off`, return empty vec. When `warn`, emit `Diagnostic::warning()` instead of `error()`. This changes `into_diagnostic()` to accept an enforcement level.

**A2: Severity at call sites (check.rs, watch.rs)** — `check_test_coverage()` still returns `Vec<SemanticProblem>`. Callers decide whether to emit as error, warning, or skip entirely. Simpler — no changes to `SemanticProblem` or its `into_diagnostic()`.

**Recommended:** A2. Keep `check_test_coverage()` pure (just finds untested functions). Callers decide severity. This follows the existing pattern where `check.rs` and `watch.rs` already control emission.

> **Implementation note:** `into_diagnostic()` hardcodes `Diagnostic::error(...)`. For A2 to work,
> either add `Diagnostic::with_severity(severity)` to override the level after construction,
> or have callers skip `into_diagnostic()` and construct diagnostics directly. The former is
> cleaner and reusable. Also note: `SemanticProblem::is_warning()` must be updated to
> conditionally include `MissingTest` when enforcement is `"warn"`.

**Note:** This requires building `oripk.toml` config parsing infrastructure from scratch. No project config system exists in the compiler today.

- [x] Locate the exact diagnostic code for "function missing tests" — **E3001** (`SemanticProblem::MissingTest` in `oric/src/problem/semantic/mod.rs`)
- [ ] **PREREQUISITE**: Split `semantic/mod.rs` — extract `check_test_coverage()` and test-related variants into `semantic/test_coverage.rs`
- [ ] Implement severity switch (approach A2): callers in check.rs/watch.rs decide severity based on `TestEnforcement` config enum; add `Diagnostic::with_severity(Severity)` to override level after construction
- [ ] Add `--test-enforcement=off|warn|error` CLI flag to clap definitions in `oric`
- [ ] Thread `TestEnforcement` config from CLI flag (and eventually `oripk.toml`) to check.rs and watch.rs call sites
- [ ] Verify `ori check` and `ori watch` respect the setting (the two callers of check_test_coverage)
- [ ] Verify `ori test` is unaffected (it does not enforce coverage, only runs tests)
- [ ] Verify `ori test --coverage` report behavior with each enforcement level
- [ ] Update `SemanticProblem::is_warning()` to conditionally include `MissingTest` when enforcement is `"warn"` (or remove dependency on `is_warning()` for test coverage problems entirely)

### Error Code Collision: E3001

**Critical issue:** E3001 is officially documented as "Unknown Pattern" (`compiler/ori_diagnostic/src/errors/E3001.md`) and registered as such in `error_code/mod.rs`. However, THREE `SemanticProblem` variants all emit E3001:
1. `MissingTest` (line 333) — function has no tests
2. `TestTargetNotFound` (line 346) — test targets unknown function
3. The original "Unknown Pattern" use (documented in E3001.md)

This is an error code collision that violates the project's error code stability rules ("once assigned, never reuse or change meaning").

**Resolution required before implementation:**
> **Note:** E3xxx is classified as "Pattern errors" by `is_pattern_error()`. Using E3010/E3011
> for test coverage preserves the status quo (they already use E3001) but is semantically
> imprecise. Decide whether to accept this or create a new range (e.g., E7xxx for "Lint/Semantic").
- [ ] Assign a dedicated error code for `MissingTest` (e.g., E3010 or E7001)
- [ ] Assign a dedicated error code for `TestTargetNotFound` (e.g., E3011 or E7002)
- [ ] Create `compiler/ori_diagnostic/src/errors/E3010.md` and `E3011.md` documenting the new codes
- [ ] Update `SemanticProblem::MissingTest` in `mod.rs` to emit the new MissingTest code instead of E3001
- [ ] Update `SemanticProblem::TestTargetNotFound` in `mod.rs` to emit the new TestTargetNotFound code instead of E3001
- [ ] Update `compiler/oric/src/reporting/tests.rs` (line 100) which asserts `ErrorCode::E3001` for MissingTest
- [ ] Review `compiler/ori_diagnostic/src/emitter/sarif/tests.rs` and `json/tests.rs` — both use E3001 in test fixtures (these use E3001 for generic test data, not MissingTest specifically, so they may not need changing — but verify)
- [ ] Update spec clause 19 error examples to reference the new code (not E0500 or E3001)

### Success Message Update

The success messages in `check.rs` (line 67) and `watch.rs` (line 163) currently print:
```
OK: path (N functions, N tests, 100% coverage)
```

When enforcement is `"off"`, this message is misleading — files with zero tests would print "100% coverage" (vacuously). Changes needed:
- [ ] When enforcement is `"off"`: print `"OK: path (N functions, N tests)"` (no coverage claim)
- [ ] When enforcement is `"warn"`: print `"OK: path (N functions, N tests, M uncovered)"` with warning count
- [ ] When enforcement is `"error"`: keep current behavior (100% coverage confirmed)

### `ori test --coverage` Impact

The `ori test --coverage` command (`commands/test.rs`) uses `TestRunner::coverage_report()` which also checks coverage and prints "MISSING COVERAGE" for uncovered functions. This should also respect the enforcement setting:
- [ ] When enforcement is `"off"`: `--coverage` still works (informational) but exit code is 0 even with gaps
- [ ] When enforcement is `"warn"`: `--coverage` reports gaps as warnings, exit code is 0
- [ ] When enforcement is `"error"`: current behavior (exit code 1 on gaps)
- [ ] Document that `--coverage` is always available regardless of enforcement level

---

## 02.3 Default Behavior

### For New Projects (`ori init`)

NOTE: `ori init` does not exist yet. It is part of the package management plan (`plans/pkg_mgmt/`). This section describes the target behavior when both `oripk.toml` and `ori init` are implemented.

```toml
[project]
name = "@user/my-project"
version = "0.1.0"
test-enforcement = "off"
```

Default is `"off"` — new users aren't confronted with "add tests or it won't compile" on their first project.

### For Existing Projects

No `test-enforcement` key in `oripk.toml` = `"off"` (not "error"). This is a breaking change from current behavior but aligns with the new philosophy. Projects that want mandatory testing add `test-enforcement = "error"` explicitly.

### Migration Path

If this is too aggressive, alternative: missing key = `"warn"` for one release cycle, then `"off"`. This gives existing users a heads-up.

- [ ] Decide: missing key = `"off"` immediately, or `"warn"` transition period
- [ ] (BLOCKED on `ori init` command) Update `ori init` template to include `test-enforcement` key with chosen default

### What Does NOT Change

- `@test tests @target` syntax — unchanged
- `tests _` floating tests — unchanged
- `ori test` command — unchanged (runs whatever tests exist)
- `ori test --only-attached` — unchanged
- Dependency-aware test execution — unchanged
- Capability-based mocking (`with...in`) — unchanged
- `#skip`, `#compile_fail`, `#fail` attributes — unchanged
- Test-driven PGO (draft proposal) — unchanged

---

## 02.4 Test Strategy

### Rust Unit Tests

Tests needed in `compiler/oric/src/problem/semantic/tests.rs`:
- [ ] `test_missing_test_is_error_when_enforcement_error` — verify E3010 (or new code) emitted as error
- [ ] `test_missing_test_is_warning_when_enforcement_warn` — verify emitted as warning
- [ ] `test_missing_test_suppressed_when_enforcement_off` — verify not emitted
- [ ] `test_check_test_coverage_exemptions_unchanged` — verify @main, tests, types still exempt

Tests needed for config parsing (location TBD — depends on oripk.toml infrastructure):
- [ ] `test_parse_test_enforcement_off` — valid TOML parses
- [ ] `test_parse_test_enforcement_warn` — valid TOML parses
- [ ] `test_parse_test_enforcement_error` — valid TOML parses
- [ ] `test_parse_test_enforcement_invalid` — invalid value produces error
- [ ] `test_missing_key_defaults_to_off` — absent key = "off"

Tests needed for CLI override:
- [ ] `test_cli_override_trumps_config` — CLI flag takes precedence over oripk.toml

### Ori Spec Tests

NOTE: Spec tests are single-file (no `oripk.toml`). To test non-default enforcement levels,
these tests must use CLI flag `--test-enforcement=X`. The spec test runner (`cargo st`) would
need to support passing this flag, or these tests should be Rust integration tests instead.

- [ ] `tests/spec/testing/enforcement_off.ori` — file with untested function compiles cleanly (default behavior)
- [ ] Test with `--test-enforcement=warn` — file with untested function produces warning (not error)
- [ ] Test with `--test-enforcement=error` — file with untested function produces error
- [ ] Decide: implement as Ori spec tests (needs test runner support) or Rust integration tests in `compiler/oric/tests/`

### Integration Tests

- [ ] Verify `./test-all.sh` passes (runs `cargo test` and `ori test`; neither calls `check_test_coverage()`, so should be unaffected — verify anyway)
- [ ] Verify `cargo st` passes in single-file mode (runs `ori test tests/`; uses the test runner, not `check_test_coverage()` — should be unaffected)

## 02.5 Completion Checklist

- [ ] Policy design confirmed (three modes: off/warn/error)
- [ ] Default behavior decided (missing key = `"off"` or `"warn"` transition)
- [ ] E3001 error code collision resolved — MissingTest and TestTargetNotFound get dedicated codes
- [ ] `semantic/mod.rs` split (currently 511 lines, over 500-line limit) — extract test coverage logic into `semantic/test_coverage.rs`
- [ ] Severity switch implemented (approach A2 + `Diagnostic::with_severity`)
- [ ] `--test-enforcement=off|warn|error` CLI flag added and functional
- [ ] (BLOCKED on `ori init` + oripk.toml infra) `ori init` generates correct default
- [ ] Single-file mode correctly defaults to `"off"`
- [ ] Success messages in check.rs/watch.rs updated for each enforcement level
- [ ] `ori test --coverage` exit code respects enforcement level (0 when `"off"`, 1 on gaps when `"error"`)
- [ ] All existing tests pass with `--test-enforcement=error` (backwards compatibility)
- [ ] New Rust unit tests pass (Section 02.4)
- [ ] New Ori spec tests pass (Section 02.4)
- [ ] `./test-all.sh` green

> **Recommended decomposition:** Implement in two sub-phases:
> 1. **CLI flag only** (`--test-enforcement=off|warn|error`) + E3001 collision fix + semantic/mod.rs split. This is self-contained and immediately useful.
> 2. **`oripk.toml` config support** — depends on building project config infrastructure (TOML parsing, file discovery, config merging). This is a larger effort that can be deferred.
>
> This decomposition avoids blocking the messaging rebrand on config infrastructure.

**Exit Criteria:** `ori check` respects the `test-enforcement` setting from CLI flag (and eventually `oripk.toml`). Default behavior for new projects is `"off"`. All existing test infrastructure (dep-graph, mocking, test runner) works identically regardless of enforcement level. MissingTest has its own stable error code.
