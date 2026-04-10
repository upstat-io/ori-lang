---
section: "01"
title: "Verifier Gates & Quick Wins"
status: not-started
reviewed: false
goal: "Make AIMS and LLVM verifier failures blocking gates under verification mode, wire verify_each to env var, add function-level verify and opt -lint — so all subsequent verification tooling has enforceable failure semantics"
success_criteria:
  - "run_verify() and run_aims_verify() return Result::Err under ORI_VERIFY_ARC=1 instead of logging warnings"
  - "ORI_VERIFY_EACH=1 is registered in debug_flags.rs and wired through OptimizationConfig"
  - "fn_val.verify() runs after each function's LLVM codegen in define pass"
  - "opt -lint runs as part of codegen audit pipeline when ORI_AUDIT_CODEGEN=1"
  - "test-all.sh runs with ORI_VERIFY_EACH=1 and ORI_VERIFY_ARC=1 by default"
  - "All existing tests pass with verification gates enabled (0 regressions)"
inspired_by:
  - "Swift -enable-sil-verify-all (swift/lib/SIL/Verifier/SILVerifier.cpp) — all verifiers run every compilation"
  - "Lean4 IR Checker (lean4/src/Lean/Compiler/IR/Checker.lean) — throws on violation, not just warns"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Make ARC/AIMS Verifiers Blocking"
    status: not-started
  - id: "01.2"
    title: "Wire ORI_VERIFY_EACH and Function-Level Verify"
    status: not-started
  - id: "01.3"
    title: "Add opt -lint to Codegen Audit Pipeline"
    status: not-started
  - id: "01.4"
    title: "Enable Verification in test-all.sh and CI"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Verifier Gates & Quick Wins

**Status:** Not Started
**Goal:** Make AIMS and LLVM verifier failures blocking gates under verification mode (`ORI_VERIFY_ARC=1` / `ORI_VERIFY_EACH=1`), wire the existing `verify_each` plumbing to an env var registered in `debug_flags.rs`, add function-level IR verification after each function's codegen, and integrate `opt -lint` into the codegen audit pipeline. This section ensures that all subsequent verification tooling (Sections 02-12) has enforceable failure semantics — a verifier that detects a problem but only logs a warning is not verification, it's a suggestion.

**Success Criteria:**

- [ ] `run_verify()` and `run_aims_verify()` return errors (not warnings) when `ORI_VERIFY_ARC=1` — satisfies mission criterion: "Verifier failures become blocking gates"
- [ ] `ORI_VERIFY_EACH=1` registered in `debug_flags.rs` and wired through `OptimizationConfig` — satisfies mission criterion: "CI runs verify_each"
- [ ] `fn_val.verify(true)` called after each function in the LLVM define pass — satisfies mission criterion: "CI runs function-level verify"
- [ ] `opt -lint` integrated into codegen audit output — satisfies mission criterion: "CI runs opt -lint"
- [ ] `test-all.sh` passes with `ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1` — satisfies mission criterion: "test-all.sh updated"

**Context:** Currently, `run_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:128`) and `run_aims_verify()` (line 144) log warnings via `tracing::warn!` but never fail the compilation. FIP structural checks at `aims_pipeline/mod.rs:181` and `batch.rs:196` use `debug_assert!` which disappears in release builds. This violates `.claude/rules/arc.md` §Non-Negotiable Invariant #4: "Every active subsystem needs implementation + invariant enforcement + verification." The `verify_each` field exists at `aot/passes/config.rs:210` with a builder at line 321, but it's not wired to any env var or CLI flag — `build_optimization_config` in `oric/src/commands/build/mod.rs:158` doesn't read it. The codegen audit pipeline (`ori_llvm/src/verify/mod.rs`) runs RC balance, COW rules, ABI checks, and safety checks, but doesn't run LLVM's own `opt -lint` pass which catches UB patterns the custom checks miss.

**Reference implementations:**
- **Swift** `lib/SIL/Verifier/SILVerifier.cpp`: 7 verifiers that abort on failure (configurable via `verify-abort-on-failure` flag). The `-enable-sil-verify-all` flag runs ALL verifiers on every compilation.
- **Lean4** `src/Lean/Compiler/IR/Checker.lean`: IR checker throws compilation errors on violation (not warnings). Runs before AND after optimization passes.

**Depends on:** Nothing — this is the foundation section.

---

## 01.1 Make ARC/AIMS Verifiers Blocking

**File(s):** `compiler/ori_arc/src/pipeline/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`

The ARC/AIMS verifiers currently log warnings but never fail. Under `ORI_VERIFY_ARC=1`, verification failures must become blocking errors that halt compilation with a clear diagnostic.

- [ ] Modify `run_verify()` (`pipeline/mod.rs:128-138`) to return `Result<(), Vec<VerifyError>>` instead of `()`. When `verify` is true and errors are found, return `Err(errors)` instead of logging and continuing. When `verify` is false (and only `debug_assertions` is active), keep the current warning behavior — debug mode is diagnostic, explicit verification mode is blocking.
  ```rust
  // Current (warns only):
  pub(crate) fn run_verify(func: &ArcFunction, phase: &str, verify: bool) {
      let enabled = verify || cfg!(debug_assertions);
      if !enabled { return; }
      let errors = crate::verify::check_function(func);
      for e in &errors {
          tracing::warn!(phase, "ARC IR verification: {e}");
      }
  }
  
  // Target (blocking under verify mode):
  pub(crate) fn run_verify(func: &ArcFunction, phase: &str, verify: bool) -> Result<(), Vec<crate::verify::VerifyError>> {
      let enabled = verify || cfg!(debug_assertions);
      if !enabled { return Ok(()); }
      let errors = crate::verify::check_function(func);
      if errors.is_empty() { return Ok(()); }
      if verify {
          // Explicit verification mode: hard error
          return Err(errors);
      }
      // debug_assertions only: warn but continue
      for e in &errors {
          tracing::warn!(phase, "ARC IR verification: {e}");
      }
      Ok(())
  }
  ```

- [ ] Apply the same pattern to `run_aims_verify()` (`pipeline/mod.rs:144-162`) — return `Result<(), Vec<VerifyError>>`, error under explicit verify mode.

- [ ] Update all call sites in `postprocess.rs` (steps 6, 7, 11) to propagate the `Result`. The `AimsPipelineResult` type may need an `errors: Vec<VerifyError>` field, or the pipeline should return `Result<AimsPipelineResult, AimsPipelineError>`.

- [ ] Replace `debug_assert!` with explicit error returns for FIP structural checks in `aims_pipeline/mod.rs` and `batch.rs`. FIP contract violations should be returned as errors when `verify_arc` is true, not hidden behind `debug_assert!`.

- [ ] Add tests in `compiler/ori_arc/src/pipeline/tests.rs`:
  - `test_run_verify_returns_error_when_verify_true_and_errors_found`
  - `test_run_verify_warns_only_when_debug_assertions_and_no_explicit_verify`
  - `test_aims_verify_blocks_on_absent_param_has_uses`
  - `test_fip_violation_returns_error_under_verify_mode`

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 01.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ...`).

---

## 01.2 Wire ORI_VERIFY_EACH and Function-Level Verify

**File(s):** `compiler/oric/src/debug_flags.rs`, `compiler/ori_llvm/src/aot/passes/config.rs`, `compiler/oric/src/commands/build/mod.rs`, `compiler/ori_llvm/src/aot/define/mod.rs` (or equivalent function emission entry point)

Currently `verify_each` exists as a field in `OptimizationConfig` (line 210 of `config.rs`) but is never read from an env var. LLVM's module-level `module.verify()` runs at module boundaries, but function-level `fn_val.verify(true)` is not called after each function's codegen — meaning a single broken function pollutes the entire module verification with cascading errors.

- [ ] Register `ORI_VERIFY_EACH` in `debug_flags.rs` (after `ORI_VERIFY_ARC` at line 132):
  ```rust
  /// Enable LLVM IR verification after every optimization pass.
  ///
  /// Catches which optimization pass breaks IR well-formedness.
  /// Significant performance impact (~30-60% slower LLVM tests).
  /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
  ORI_VERIFY_EACH
  ```
  Ensure `check-debug-flags.sh` picks up the new flag automatically (it should — it reads the `debug_flags!` macro output).

- [ ] Wire `ORI_VERIFY_EACH` through `build_optimization_config` in `oric/src/commands/build/mod.rs` (around line 158). The `OptimizationConfig` already has `.with_verify_each(bool)` at `config.rs:321` — just connect the env var:
  ```rust
  let verify_each = std::env::var("ORI_VERIFY_EACH").is_ok();
  let opt_config = OptimizationConfig::release()
      .with_verify_each(verify_each);
  ```

- [ ] Add function-level verification in the LLVM define pass. After each function's codegen completes (all basic blocks emitted, all instructions placed), call `fn_val.verify(true)`. Find the function emission loop — likely in `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` or `compiler/ori_llvm/src/aot/define/` — and add:
  ```rust
  // After function codegen is complete:
  if fn_val.verify(true) {
      // Inkwell verify returns true on FAILURE
      return Err(CodegenError::FunctionVerificationFailed {
          function: func_name.to_string(),
      });
  }
  ```
  This catches the broken function immediately rather than cascading to module-level verification.

- [ ] Verify that `LLVM_OPT_BISECT_LIMIT` env var is respected by the optimization pipeline to support `diagnostics/opt-bisect.sh` in Section 11.

- [ ] Add tests:
  - `test_verify_each_env_var_registered_in_debug_flags`
  - `test_verify_each_wired_through_optimization_config`
  - `test_function_level_verify_catches_malformed_ir` (may need a synthetic bad IR fixture)

- [ ] **TPR checkpoint** — `/tpr-review` covering 01.1–01.2 implementation work

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 01.1's close-out, scoped to 01.2's debugging journey. Commit improvements separately using a valid conventional-commit type.

---

## 01.3 Add opt -lint to Codegen Audit Pipeline

**File(s):** `compiler/ori_llvm/src/verify/mod.rs`, `compiler/ori_llvm/src/aot/passes/mod.rs`

LLVM's `opt -lint` pass detects likely-undefined behavior that the standard IR verifier doesn't catch: division by potential zero, suspicious alignment, unreachable patterns, UB patterns in instruction operands. Currently the codegen audit pipeline (`ORI_AUDIT_CODEGEN=1`) runs RC balance, COW rules, ABI checks, and safety checks — but not `opt -lint`.

- [ ] Integrate the LLVM lint pass into the optimization pipeline. The `run_optimization_passes` function in `aot/passes/mod.rs` builds a pipeline string — add `lint` to the pipeline when `ORI_AUDIT_CODEGEN=1` is set or when a new `ORI_LLVM_LINT=1` flag is active:
  ```rust
  // In the pipeline string construction:
  if config.lint_enabled {
      pipeline.push_str(",lint");
  }
  ```
  Add `lint_enabled: bool` to `OptimizationConfig` with env var `ORI_LLVM_LINT`. Default: off. Enabled automatically when `ORI_AUDIT_CODEGEN=1`.

- [ ] Alternatively, if the LLVM lint pass is better run as a standalone analysis (not in the opt pipeline), integrate it into `audit_module()` in `verify/mod.rs` — run `opt -lint` on the module's serialized IR and parse the output into `AuditFinding`s.

- [ ] Add tests:
  - `test_opt_lint_catches_division_by_zero_pattern`
  - `test_opt_lint_integrated_with_codegen_audit`

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 01.4 Enable Verification in test-all.sh and CI

**File(s):** `test-all.sh`, `.github/workflows/ci.yml`

The verification gates from 01.1-01.3 must be ON by default in all test runs. This ensures that the entire test suite serves as a continuous verification harness.

- [ ] Update `test-all.sh` to export `ORI_VERIFY_EACH=1` and `ORI_VERIFY_ARC=1` before LLVM test suites (suites #3, #4, #7). These env vars should be set at the top of the script, not per-suite, so all LLVM-touching tests benefit:
  ```bash
  # At the top of test-all.sh, after other env setup:
  export ORI_VERIFY_EACH=1
  export ORI_VERIFY_ARC=1
  ```

- [ ] Verify that the 150-second test timeout still holds with `verify_each` enabled. The research estimates ~30-60% increase in LLVM test wall time. If any test suite exceeds 150s:
  - Identify the slow tests
  - Consider splitting the LLVM test suite into smaller shards
  - Do NOT raise the timeout — that violates CLAUDE.md §MANDATORY Test Timeouts

- [ ] Update `.github/workflows/ci.yml` to set `ORI_VERIFY_EACH=1` and `ORI_VERIFY_ARC=1` in the `env:` block for the test job:
  ```yaml
  env:
    ORI_VERIFY_EACH: "1"
    ORI_VERIFY_ARC: "1"
  ```

- [ ] Run `timeout 150 ./test-all.sh` with both flags enabled and verify 0 regressions. If any existing tests fail under verification mode, those are pre-existing bugs that verification just surfaced — file each via `/add-bug` and fix before proceeding.

- [ ] **Subsection close-out (01.4)** — MANDATORY before starting 01.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 01.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 01.N Completion Checklist

- [ ] `run_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [ ] `run_aims_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [ ] FIP checks use explicit error returns, not `debug_assert!`
- [ ] `ORI_VERIFY_EACH` registered in `debug_flags.rs` and wired through `OptimizationConfig`
- [ ] `fn_val.verify(true)` runs after each function's codegen
- [ ] `opt -lint` integrated into codegen audit or optimization pipeline
- [ ] `test-all.sh` runs with `ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1` by default
- [ ] `.github/workflows/ci.yml` sets both env vars
- [ ] All test suites pass within 150-second timeout with verification enabled
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for this section
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY after both reviews are clean. Per-subsection captures from 01.1–01.4 should already be committed; the sweep verifies they ran and adds only NEW cross-cutting items. Document the negative finding if there are no cross-cutting gaps.

**Exit Criteria:** `ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` passes with 0 failures and 0 regressions. Verification failures in ARC IR and LLVM IR are hard errors under verification mode, not warnings. `fn_val.verify()` runs per-function. `opt -lint` runs as part of codegen audit. All flags registered canonically in `debug_flags.rs`.
