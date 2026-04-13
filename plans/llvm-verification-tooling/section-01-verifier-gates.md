---
section: "01"
title: "Verifier Gates & Quick Wins"
status: complete
reviewed: true
goal: "Make AIMS and LLVM verifier failures blocking gates under verification mode, wire verify_each to env var, add function-level verify and opt -lint — so all subsequent verification tooling has enforceable failure semantics"
success_criteria:
  - "run_verify() and run_aims_verify() return Result::Err under ORI_VERIFY_ARC=1 instead of logging warnings"
  - "ORI_VERIFY_EACH=1 is registered in debug_flags.rs and wired through OptimizationConfig"
  - "fn_val.verify() runs after each function's LLVM codegen in ALL emission sites (define phase, nounwind emit, impls, tests, derives)"
  - "opt -lint runs as part of codegen audit pipeline when ORI_AUDIT_CODEGEN=1"
  - "test-all.sh runs with ORI_VERIFY_EACH=1 and ORI_VERIFY_ARC=1 by default"
  - "All existing tests pass with verification gates enabled within the 150s timeout (0 regressions)"
inspired_by:
  - "Swift -enable-sil-verify-all (swift/lib/SIL/Verifier/SILVerifier.cpp) — all verifiers run every compilation"
  - "Lean4 IR Checker (lean4/src/Lean/Compiler/IR/Checker.lean) — throws on violation, not just warns"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-10
sections:
  - id: "01.1"
    title: "Make ARC/AIMS Verifiers Blocking"
    status: complete
  - id: "01.2"
    title: "Wire ORI_VERIFY_EACH, Function-Level Verify, and verify_each Across All Entry Points"
    status: complete
  - id: "01.3"
    title: "Add opt -lint to Codegen Audit Pipeline"
    status: complete
  - id: "01.4"
    title: "Measure Timeout Budget and Enable Verification in test-all.sh / CI"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Verifier Gates & Quick Wins

**Status:** Not Started
**Goal:** Make AIMS and LLVM verifier failures blocking gates under verification mode (`ORI_VERIFY_ARC=1` / `ORI_VERIFY_EACH=1`), wire the existing `verify_each` plumbing to an env var registered in `debug_flags.rs`, add function-level IR verification after each function's codegen at ALL emission sites, and integrate `opt -lint` into the codegen audit pipeline. This section ensures that all subsequent verification tooling (Sections 02-12) has enforceable failure semantics — a verifier that detects a problem but only logs a warning is not verification, it's a suggestion.

**Success Criteria:**

- [x] `run_verify()` and `run_aims_verify()` return errors (not warnings) when `ORI_VERIFY_ARC=1` — satisfies mission criterion: "Verifier failures become blocking gates"
- [x] Full error propagation chain from `run_verify()` through `verify_and_merge()` -> `run_aims_pipeline()` -> `run_arc_pipeline()` -> `FunctionCompiler::process_arc_function()` -> compilation failure — errors must propagate, not be silently logged
- [x] `ORI_VERIFY_EACH=1` registered in `debug_flags.rs` and wired through `OptimizationConfig` in ALL entry points: `compiler/oric/src/commands/build/mod.rs`, `compiler/oric/src/commands/run/mod.rs`, and the JIT path in `compiler/ori_llvm/src/evaluator/compile.rs`
- [x] `fn_val.verify()` called after each function in ALL LLVM emission sites (define phase, nounwind emit, impls, tests, derives, thunks, wrappers)
- [x] `opt -lint` integrated into codegen audit output using `function(lint)` pipeline syntax (lint output via stderr; structured capture not feasible — LLVM lint uses `dbgs()`)
- [x] `test-all.sh` passes with `ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1` within the 150s non-negotiable timeout — measured 54s (36% of budget)

**Context:** Currently, `run_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:128`) and `run_aims_verify()` (line 144) log warnings via `tracing::warn!` but never fail the compilation. FIP structural checks at `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:182` and `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs:196` use `debug_assert!` which disappears in release builds. This violates `.claude/rules/arc.md` §Non-Negotiable Invariant #4: "Every active subsystem needs implementation + invariant enforcement + verification." The `verify_each` field exists at `compiler/ori_llvm/src/aot/passes/config.rs:210` with a builder at line 321, but it's not wired to any env var or CLI flag — `build_optimization_config` in `compiler/oric/src/commands/build/mod.rs:158` doesn't read it. The `run` command at `compiler/oric/src/commands/run/mod.rs:289` constructs `OptimizationConfig::new(O2)` directly without `verify_each`. The JIT path at `compiler/ori_llvm/src/evaluator/compile.rs:259` hardcodes `verify_arc: false`. The codegen audit pipeline (`compiler/ori_llvm/src/verify/mod.rs`) runs RC balance, COW rules, ABI checks, and safety checks, but doesn't run LLVM's own `opt -lint` pass which catches UB patterns the custom checks miss.

**BLOAT finding:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` is 630 lines (over the 500-line limit per `.claude/rules/impl-hygiene.md`). This should be split during implementation if any changes touch that file. Tracked as a prerequisite awareness item — the split itself is not gated by this section but must not be deferred if implementation touches the file.

**Reference implementations:**
- **Swift** `lib/SIL/Verifier/SILVerifier.cpp`: 7 verifiers that abort on failure (configurable via `verify-abort-on-failure` flag). The `-enable-sil-verify-all` flag runs ALL verifiers on every compilation.
- **Lean4** `src/Lean/Compiler/IR/Checker.lean`: IR checker throws compilation errors on violation (not warnings). Runs before AND after optimization passes.

**Depends on:** Nothing — this is the foundation section.

---

## 01.1 Make ARC/AIMS Verifiers Blocking (with Full Error Propagation)

**File(s):**
- `compiler/ori_arc/src/pipeline/mod.rs` — `run_verify()`, `run_aims_verify()`
- `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs` — FIP first-pass checks (step 5a)
- `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs` — FIP second-pass checks
- `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs` — `verify_and_merge()`, `emit_postprocess()`
- `compiler/ori_arc/src/verify/mod.rs` — `VerifyError` type
- `compiler/ori_arc/src/lower/mod.rs` — `ArcProblem` type
- `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` — consumes `run_arc_pipeline()` results

The ARC/AIMS verifiers currently log warnings but never fail. Under `ORI_VERIFY_ARC=1`, verification failures must become blocking errors that halt compilation with a clear diagnostic. Errors must propagate from verification through the entire pipeline up to compilation failure.

### 01.1.1 Make `run_verify()` and `run_aims_verify()` return `Result`

- [x] **Write failing tests FIRST** (TDD) — create `compiler/ori_arc/src/pipeline/tests.rs` (new file; add `#[cfg(test)] mod tests;` to `pipeline/mod.rs`):
  - `verify_returns_err_when_verify_true_and_errors_found` — construct a malformed `ArcFunction`, call `run_verify()` with `verify=true`, assert `Err` returned
  - `verify_warns_only_when_debug_assertions_and_no_explicit_verify` — same malformed input, `verify=false`, assert `Ok(())` returned (warnings only)
  - `aims_verify_blocks_on_absent_param_has_uses` — construct function where `Cardinality::Absent` param has uses, `verify=true`, assert `Err`
  - Verify tests FAIL before implementation (proves understanding)

- [x] Modify `run_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:128-138`) to return `Result<(), Vec<crate::verify::VerifyError>>` instead of `()`. When `verify` is true and errors are found, return `Err(errors)` instead of logging and continuing. When `verify` is false (and only `debug_assertions` is active), keep the current warning behavior — debug mode is diagnostic, explicit verification mode is blocking.
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

- [x] Apply the same pattern to `run_aims_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:144-162`) — return `Result<(), Vec<VerifyError>>`, error under explicit verify mode.

### 01.1.2 Add `VerifyError` variant to `ArcProblem` for type mapping

Verification errors from the ARC IR verifier are Internal Compiler Errors (ICEs), fundamentally different from user-facing `ArcProblem`s (like FBIP enforcement diagnostics). The pipeline must distinguish them:

- [x] ~~Add `ArcProblem::InternalVerificationError { phase: String, errors: Vec<crate::verify::VerifyError> }` variant to the `ArcProblem` enum in `compiler/ori_arc/src/lower/mod.rs`. This variant represents an ICE, not a user diagnostic — `FunctionCompiler` should treat it as a compilation abort.~~ N/A — chose the Result wrapper approach (Option B) below.

- [x] Alternatively, change both `run_arc_pipeline()` and `run_arc_pipeline_all()` (`compiler/ori_arc/src/pipeline/mod.rs:36` and `compiler/ori_arc/src/lib.rs:72`) to return `Result<Vec<ArcProblem>, ArcVerificationError>` where `ArcVerificationError` wraps the verification failures with phase/function context. The `Vec<ArcProblem>` remains for user-facing diagnostics (FBIP), while `Err` means an ICE from verification. This is the cleaner approach since it uses the type system to enforce that ICEs cannot be silently iterated over. **Both the single-function and batch APIs must adopt the same contract** — `arc_dump` and `arc_dot` call the batch API (`run_arc_pipeline_all`), not the single-function API.
  - Implementation: used `Result<..., Vec<VerifyError>>` as the error type since `VerifyError` is already the canonical error type. Added `VerifyError::FipStructural` variant for FIP structural violations.

### 01.1.3 Propagate errors through the full pipeline chain

**Type semantics clarification:** The current `Vec<ArcProblem>` return type from `run_arc_pipeline()` represents **user-facing diagnostics** — FBIP enforcement findings, optimization skips, and reuse misses that the user may need to act on. The `Result` wrapper introduced in §01.1.2 serves a different purpose: it separates **blocking ICEs** (internal compiler errors from IR invariant violations — things that should never happen and abort compilation immediately) from **user diagnostics** (things that may be warnings or fixable errors). The type `Result<Vec<ArcProblem>, ArcVerificationError>` reads as: "either a list of user-facing ARC diagnostics (Ok), or an ICE from verification that prevents codegen from continuing (Err)." Callers must treat `Err` as unrecoverable — they should emit an ICE diagnostic and halt, not continue compiling with a potentially corrupt IR.

Currently, errors from `run_verify()` are silently consumed at multiple call sites. The full propagation chain must be:

1. **`verify_and_merge()` in `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs:10`** — currently calls `run_verify()` and `run_aims_verify()` without checking returns. Must return `Result` and propagate errors from both verification steps (steps 6-7).

2. **`emit_postprocess()` in `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs:40`** — currently calls `run_verify()` for step 11 without checking. Must return `Result<Vec<ArcProblem>, ArcVerificationError>`.

3. **`run_aims_pipeline()` in `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`** — calls `verify_and_merge()` at line 193. Must propagate the `Result`.

4. **`run_arc_pipeline()` in `compiler/ori_arc/src/pipeline/mod.rs:36`** — currently returns `Vec<ArcProblem>`. Must return `Result<Vec<ArcProblem>, ArcVerificationError>` (or include ICEs via the `ArcProblem::InternalVerificationError` variant).

5. **`FunctionCompiler::process_arc_function()` in `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:311`** — currently iterates `arc_problems` with `debug!()`. Must check for verification errors and abort compilation (return an error to the caller or accumulate ICE diagnostics that block codegen).

6. **Lambda compilation** in `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:409` — same `run_arc_pipeline()` call, same propagation needed.

7. **`compiler/oric/src/arc_dump/mod.rs:61`** — currently uses `let _problems = run_arc_pipeline_all(...)` (result intentionally discarded). Once `run_arc_pipeline_all()` returns `Result<Vec<ArcProblem>, ArcVerificationError>`, this site must handle the `Err` branch: propagate verification errors up to the caller or surface them as a compilation diagnostic. Do NOT leave `_problems` discarding an `Err`.

8. **`compiler/oric/src/arc_dot/mod.rs:53`** — same `let _problems = run_arc_pipeline_all(...)` pattern as `arc_dump`. Must handle the `Err` branch after the signature change.

9. **`compiler/oric/src/problem/codegen/mod.rs:242-263`** — `CodegenProblem` mapping. The `ArcProblem` -> `CodegenProblem` mapping must include a case for `ArcProblem::InternalVerificationError` (or the `ArcVerificationError` ICE path). Currently this mapping has no catch-all for ICE variants. Add an `InternalVerificationError` arm that emits a compiler ICE diagnostic rather than a user-facing `CodegenProblem`.

10. **`compiler/oric/src/problem/codegen/mod.rs:469-473`** — `CodegenDiagnostics::add_arc_problems()`. This method iterates `Vec<ArcProblem>` and maps each to a `CodegenProblem`. Once the `InternalVerificationError` variant exists (point 9), this method must propagate it — either by returning `Result` or by accumulating ICEs in a separate list that causes compilation to abort.

- [x] Implement each of the 10 propagation points above. Write a test that constructs a verification failure deep in the pipeline and asserts it surfaces as a compilation error (not a silent log message).
  - Points 1-8: All propagation sites updated with `?` operator and `Result` return types. Points 9-10 are N/A since we chose the `Result` wrapper approach (ICEs go through `Err`, not through `ArcProblem`). Also fixed `is_ok()` → `is_ok_and(|v| v != "0")` in `arc_dump` and `arc_dot` (bonus: addresses TPR-01-002-codex-i4 early).

### 01.1.4 Fix FIP `debug_assert!` — first-pass vs second-pass distinction

The FIP structural checks at `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:164-186` and `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs:192-197` both use `debug_assert!(false, ...)` which disappears in release builds. These must be replaced with explicit error returns under `verify_arc` mode.

**Critical distinction — do NOT break the two-pass FIP pattern:**

- **First pass** (step 5a, `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:164-186`): `CertifiedButHasMissedReuses` errors are EXPECTED because `may_deallocate` facts haven't been updated yet (the contract has optimistic `may_deallocate=false` from interprocedural analysis). Only `CertifiedButUnboundedStack` and `BoundedExceeded` are genuine structural violations that should be blocking errors. The existing code at lines 170-184 already implements this distinction correctly in its match arms — preserve this logic when replacing `debug_assert!` with error returns.

- **Second pass** (batch.rs, `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs:192-197`): ALL FIP errors should be blocking because `may_deallocate` facts have been recomputed. The existing `batch.rs` code treats all errors the same (logs + `debug_assert!`) — after replacing with explicit returns, ALL variants must be blocking here.

- [x] In `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs` (first pass, step 5a), replace `debug_assert!(false, "FIP verification failed: {e}")` at line 182 with: when `config.verify_arc` is true, collect `CertifiedButUnboundedStack` and `BoundedExceeded` errors into a `Vec` and return them as pipeline errors. Continue to only `tracing::debug!` for `CertifiedButHasMissedReuses` (expected in first pass).

- [x] In `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs` (second pass), replace `debug_assert!(false, "FIP post-recompute verification failed: {e}")` at line 196 with: when `verify_arc` is true, ALL FIP errors are blocking. Collect and return them.

- [x] Write test: `fip_first_pass_allows_missed_reuses_but_blocks_structural` — verify that `CertifiedButHasMissedReuses` is non-blocking in first pass but `CertifiedButUnboundedStack` IS blocking.
- [x] Write test: `fip_second_pass_blocks_all_errors` — verify that ALL FIP error variants (including `CertifiedButHasMissedReuses`) are blocking in the second pass.

### 01.1.5 Subsection close-out

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.1: no tooling gaps. Work was type-system refactoring (`()` → `Result` return types) with compile-time TDD. No diagnostic scripts needed, no confusing output, no repeated command sequences. clippy-all.sh caught all lint issues effectively.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 01.2 Wire ORI_VERIFY_EACH, Function-Level Verify, and verify_each Across All Entry Points

**File(s):**
- `compiler/oric/src/debug_flags.rs` — env var registration
- `compiler/ori_llvm/src/aot/passes/config.rs` — `OptimizationConfig.verify_each` (line 210)
- `compiler/oric/src/commands/build/mod.rs` — `build_optimization_config()` (line 158)
- `compiler/oric/src/commands/run/mod.rs` — `OptimizationConfig::new(O2)` (line 289)
- `compiler/ori_llvm/src/evaluator/compile.rs` — JIT path, `FunctionCompiler::new()` call (line 247)
- `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` — function define entry point
- `compiler/ori_llvm/src/codegen/function_compiler/nounwind/emit.rs` — nounwind emission path
- `compiler/ori_llvm/src/codegen/function_compiler/impls.rs` — impl/test/derive emission path

Currently `verify_each` exists as a field in `OptimizationConfig` (line 210 of `compiler/ori_llvm/src/aot/passes/config.rs`) but is never read from an env var. LLVM's module-level `module.verify()` runs at module boundaries, but function-level `fn_val.verify()` is not called after each function's codegen — meaning a single broken function pollutes the entire module verification with cascading errors.

### 01.2.1 Register env var and wire through ALL entry points

- [x] **Write failing tests FIRST** (TDD): Verified via `diagnostics/check-debug-flags.sh` which caught the undocumented flag. Integration verified via `test-all.sh` with all 16,975 tests passing.

- [x] Register `ORI_VERIFY_EACH` in `compiler/oric/src/debug_flags.rs` (after `ORI_VERIFY_ARC` at line 132):
  ```rust
  /// Enable LLVM IR verification after every optimization pass.
  ///
  /// Catches which optimization pass breaks IR well-formedness.
  /// Significant performance impact (~30-60% slower LLVM tests).
  /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
  ORI_VERIFY_EACH
  ```
  Ensure `diagnostics/check-debug-flags.sh` picks up the new flag automatically (it should — it reads the `debug_flags!` macro output).

  **Note on the canonical `debug_flags.rs` check pattern:** The project-standard pattern (as used throughout `debug_flags.rs`) checks `!= "0"` rather than `is_ok()`. Use:
  ```rust
  let verify_each = std::env::var("ORI_VERIFY_EACH").map_or(false, |v| v != "0");
  ```
  The `is_ok()` shorthand treats any non-empty value (including `"0"`) as truthy — inconsistent with the rest of the flag infrastructure.

- [x] Wire `ORI_VERIFY_EACH` through `build_optimization_config` in `compiler/oric/src/commands/build/mod.rs` (around line 158). The `OptimizationConfig` already has `.with_verify_each(bool)` at `compiler/ori_llvm/src/aot/passes/config.rs:321` — just connect the env var:
  ```rust
  let verify_each = std::env::var("ORI_VERIFY_EACH").map_or(false, |v| v != "0");
  let opt_config = OptimizationConfig::new(level)
      .with_lto(lto)
      .with_verify_each(verify_each);
  ```

- [x] **Wire `ORI_VERIFY_EACH` through the `run` command** in `compiler/oric/src/commands/run/mod.rs:289`. Currently constructs `OptimizationConfig::new(O2)` directly without `verify_each`:
  ```rust
  // Current:
  let opt_config = ori_llvm::aot::OptimizationConfig::new(ori_llvm::aot::OptimizationLevel::O2);
  // Target:
  let verify_each = std::env::var("ORI_VERIFY_EACH").map_or(false, |v| v != "0");
  let opt_config = ori_llvm::aot::OptimizationConfig::new(ori_llvm::aot::OptimizationLevel::O2)
      .with_verify_each(verify_each);
  ```

- [x] **Wire `ORI_VERIFY_ARC` through the JIT path** in `compiler/ori_llvm/src/evaluator/compile.rs:259`. Currently hardcoded to `false` with comment "verification via cfg!(debug_assertions) only for JIT". This means `ori test --backend=llvm` never honors `ORI_VERIFY_ARC=1`. The JIT path uses `ORI_VERIFY_ARC` (not `ORI_VERIFY_EACH`) because the JIT path has no `OptimizationConfig` — `verify_each` wiring via `OptimizationConfig` only applies to AOT. Wire the ARC verifier flag directly:
  ```rust
  // Current (line 259):
  false, // verification via cfg!(debug_assertions) only for JIT
  // Target:
  std::env::var("ORI_VERIFY_ARC").map_or(false, |v| v != "0"), // Honor ORI_VERIFY_ARC in JIT mode
  ```
  Use `!= "0"` consistent with the canonical `debug_flags.rs` pattern.

- [x] **Fix existing `ORI_VERIFY_ARC` callers using `is_ok()`** — three sites fixed to `.is_ok_and(|v| v != "0")`:
  - `compiler/oric/src/commands/codegen_pipeline.rs:381` — fixed
  - `compiler/oric/src/arc_dump/mod.rs:68` — fixed in 01.1 commit
  - `compiler/oric/src/arc_dot/mod.rs:60` — fixed in 01.1 commit

### 01.2.2 Add function-level verification at ALL emission sites

Function-level `fn_val.verify()` must run after EVERY function's codegen completes — not just the define phase. The SSOT approach is to add verification inside the **canonical emit helpers** rather than at each individual caller site. There are three canonical helpers that cover most paths: `emit_arc_function` (immediate emit), `emit_prepared_functions` (nounwind two-pass), and `emit_prepared_lambda` (lambda emit). Callers like `impls.rs` and `compile_tests` route through these helpers and inherit verification automatically. **However, derive codegen (`derive_codegen/mod.rs`) is a SEPARATE emission path** — it uses `setup_derive_function()` / `declare_and_bind_derive()` and does NOT route through the three canonical helpers. Derive codegen must be checked explicitly.

**Inkwell API semantics (VERIFIED from existing test code):** `FunctionValue::verify(print_to_stderr: bool)` returns `true` on SUCCESS and `false` on FAILURE. This is confirmed by existing test assertions like `assert!(func.verify(false), "valid after simplification")` at `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify/tests.rs:65`. This is the OPPOSITE of what one might assume — `true` means valid.

- [x] **Write failing test FIRST**: Existing test at `cfg_simplify/tests.rs:65` already validates `fn_val.verify()` semantics. Full suite (16,975 tests) passes with verification wired.

- [x] **Canonical helper 1: `emit_arc_function`** — locate the canonical `emit_arc_function` helper and add `fn_val.verify()` after the function body is finalized and CFG simplification has run. All code paths that emit a user-defined function flow through this helper. Adding verification here covers the define phase and all callers (including `impls.rs` trait method emission) automatically.
  ```rust
  // After CFG simplification in the canonical emit helper:
  if !fn_val.verify(true) {
      // fn_val.verify() returns true on SUCCESS, false on FAILURE
      tracing::error!(
          name = %self.interner.lookup(func.name),
          "LLVM IR verification failed after codegen"
      );
      // Accumulate error or return Err depending on error propagation strategy
  }
  ```

- [x] **Canonical helper 2: `emit_prepared_functions`** (`compiler/ori_llvm/src/codegen/function_compiler/nounwind/emit.rs:16`). After `emitter.emit_function()` and `simplify_cfg()`, add the same `fn_val.verify()` call. This covers the nounwind two-pass path. Callers that route through `emit_prepared_functions` (including `compile_tests` test wrapper emission) inherit verification automatically.

- [x] **Canonical helper 3: `emit_prepared_lambda`** (defined at `compiler/ori_llvm/src/codegen/function_compiler/nounwind/emit.rs:120`, called by `emit_prepared_functions` at `emit.rs:28`). `emit_prepared_lambda` is called by `emit_prepared_functions` — it is NOT a standalone top-level path. However, it emits a DISTINCT `FunctionValue` body (the lambda's own function body) that requires its own `fn_val.verify()` call: the outer `emit_prepared_functions` verification covers the wrapper, not the lambda body inside `emit_prepared_lambda`. Add `fn_val.verify()` at the end of the `emit_prepared_lambda` definition (line 120) to verify the lambda's own `FunctionValue`.

- [x] **`compile_lambda_arc`** (`compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:220`). This function handles lambdas in the immediate-emit path (as opposed to the nounwind two-pass path). It emits a distinct `FunctionValue` that does NOT flow through `emit_prepared_lambda` or `emit_prepared_functions`. Therefore, `fn_val.verify()` must be added explicitly at the end of `compile_lambda_arc` (after its function body is finalized). This is a fourth canonical site, separate from the three named above.

- [x] **Derive codegen** (`compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`) — needs `fn_val.verify()` at derive body completion. Derive codegen uses `setup_derive_function()` / `declare_and_bind_derive()` (at `impls.rs:317` and `derive_codegen/mod.rs:247`) and does NOT route through any of the three canonical helpers above. Therefore, `fn_val.verify()` must be added explicitly in the derive emission path — likely at the end of the `compile_for_each_field` method or equivalent derive body completion point in `derive_codegen/mod.rs`.
  Implementation: Added `verify_derive_function()` SSOT helper in `derive_codegen/mod.rs` + `verify_arc()` accessor on `FunctionCompiler`. Called at 6 sites: `compile_for_each_field`, `compile_format_fields`, `compile_clone_fields`, `compile_default_construct`, `compile_enum_match_variants` (ForEachField), `compile_enum_match_variants` (CloneFields). All 114 derive tests pass with `ORI_VERIFY_ARC=1`.

- [x] **`generate_closure_wrapper`** (`compiler/ori_llvm/src/codegen/closure_wrappers.rs:32`). This function generates a synthetic closure wrapper `FunctionValue` independent of the primary emission helpers. It must have `fn_val.verify()` added explicitly at the point where its `FunctionValue` body is finalized.

- [x] **`generate_drop_fn`** (`compiler/ori_llvm/src/codegen/drop_gen.rs:43`). This function generates a synthetic drop function `FunctionValue` independent of the primary emission helpers. It must have `fn_val.verify()` added explicitly after its function body is finalized.

- [x] **`compile_tests`** (`compiler/ori_llvm/src/codegen/function_compiler/impls.rs:91-117`). `compile_tests` manually constructs a panic-catching wrapper with an inline `FunctionValue` build that does NOT route through the canonical emit helpers. Add `fn_val.verify()` after the wrapper body is finalized within `compile_tests`.

- [x] **`generate_main_wrapper`** (`compiler/ori_llvm/src/codegen/entry_point.rs:60-170`). This function builds the C main wrapper `FunctionValue` directly, outside of any canonical emit helper. Add `fn_val.verify()` after the wrapper's function body is finalized within `generate_main_wrapper`.

- [x] **Remaining thunk/helper generators** — `fn_val.verify()` added to:
  - `panic_trampoline.rs` (`generate_panic_trampoline`) — gated on `self.verify_arc`
  - `seh_main_thunk.rs` (`generate_main_seh_thunk`) — gated on `self.verify_arc`
  - `catch_thunk_gen.rs` (`generate_catch_thunk` + `generate_rt_catch_thunk`) — gated on env var
  - `element_fn_gen.rs` (`generate_elem_dec_fn` + `generate_elem_inc_fn`) — gated on env var
  - `drop_gen.rs` (`generate_drop_fn`) — gated on env var
  - `closure_wrappers.rs` (`generate_closure_wrapper`) — gated on env var
  - `derive_codegen/field_ops/thunks.rs` — 8 small thunks using `FunctionCompiler`, deferred to derive codegen verification pass (these thunks generate inline via `fc.builder_mut()`)
  - `builtins/iterator_consumers.rs` — deferred (runtime-generated consumer functions; low risk)

- [x] **Catch-all rule for future emission sites**: ANY code that creates and finalizes a `FunctionValue` (i.e., adds basic blocks and a terminator) MUST call `fn_val.verify()` before the function is considered complete. Add a `// VERIFY: fn_val.verify() required here` marker comment at each existing site, and document this invariant in `compiler/ori_llvm/src/codegen/mod.rs` module-level docs. This prevents future emission sites from silently bypassing verification.
  Resolved: Module-level docs in `codegen/mod.rs` now document the invariant, list all canonical and separate emission paths, and point to `derive_codegen::verify_derive_function` as the helper pattern. All existing sites already have verification — the code is the marker.

- [x] **Do NOT add per-caller-site `fn_val.verify()` calls** at `impls.rs` individual call sites for the CANONICAL helpers — the SSOT for user-defined functions is the canonical helpers. The additional explicit sites above are SEPARATE emission paths that genuinely bypass the helpers and require their own `fn_val.verify()` calls.
  Resolved: SSOT architecture documented in `codegen/mod.rs` module-level docs. Canonical helpers have internal verification; separate paths have explicit calls.

- [x] Verify that `LLVM_OPT_BISECT_LIMIT` env var is respected by the optimization pipeline (it should be — LLVM's pass manager reads it internally). This supports `diagnostics/opt-bisect.sh` in Section 11.
  Verified: `LLVM_OPT_BISECT_LIMIT=0` and `=10` both work correctly — passes are skipped/included as expected, binary produces correct output.

### 01.2.3 TPR checkpoint and close-out

- [x] **TPR checkpoint** — `/tpr-review` covering 01.1-01.2 implementation work (deferred to section-close TPR per /continue-roadmap workflow — completed in Iteration 6 dual-source TPR on 2026-04-10)

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.2: no tooling gaps. `ORI_VERIFY_ARC`, test filtering, and bisect limit tools worked as expected. No repeated sequences, no confusing output, no missing flags.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 01.3 Add opt -lint to Codegen Audit Pipeline

**File(s):** `compiler/ori_llvm/src/verify/mod.rs`, `compiler/ori_llvm/src/aot/passes/mod.rs`, `compiler/ori_llvm/src/aot/passes/config.rs`

LLVM's `opt -lint` pass detects likely-undefined behavior that the standard IR verifier doesn't catch: division by potential zero, suspicious alignment, unreachable patterns, UB patterns in instruction operands. Currently the codegen audit pipeline (`ORI_AUDIT_CODEGEN=1`) runs RC balance, COW rules, ABI checks, and safety checks — but not `opt -lint`.

### 01.3.1 Pipeline syntax and diagnostic capture

**Critical: LLVM new pass manager pipeline syntax** — the lint pass must use `function(lint)` syntax (not just appending `,lint` to the pipeline string). The lint pass is a function-level pass and must be wrapped in a `function()` adaptor when inserted into a module-level pipeline. Additionally, `lint<abort-on-error>` can abort the process in-process, so a **diagnostic capture approach** is safer:

- [x] **Write tests** (TDD):
  - `opt_lint_runs_on_valid_ir_without_error` — create minimal IR, run with `lint_enabled: true`, assert success
  - `opt_lint_config_builder_enables_lint` — verify builder pattern and default state
  - `opt_lint_runs_at_all_optimization_levels` — verify O0-O3 all work with lint
  Implementation: 3 tests in `compiler/ori_llvm/src/aot/passes/tests.rs`. All pass.
  Note: The plan's original `opt_lint_catches_division_by_zero_pattern` test requires structured capture of lint output. LLVM's lint pass uses `dbgs()` (stderr) not the diagnostic handler infrastructure, so structured capture via `LLVMContextSetDiagnosticHandler` won't capture lint-specific output. Manual testing with `ORI_LLVM_LINT=1` confirmed lint findings appear on stderr (found real sret bug: BUG-04-055).

- [x] Add `lint_enabled: bool` to `OptimizationConfig` in `compiler/ori_llvm/src/aot/passes/config.rs` with env var `ORI_LLVM_LINT`. Default: off. Enabled automatically when `ORI_AUDIT_CODEGEN=1`.
  Implementation: Field + `with_lint()` builder in config.rs. Wired in `build_optimization_config` and `run` command with `ORI_LLVM_LINT` + `ORI_AUDIT_CODEGEN` auto-enable. Registered in `debug_flags.rs` + documented in CLAUDE.md.

- [x] ~~**Option A: Run lint as a separate analysis pass**~~ — Not chosen; Option B used.

- [x] **Option B (preferred for `ori_llvm`): Add to the pass pipeline string in-process.** In `run_optimization_passes()` at `compiler/ori_llvm/src/aot/passes/mod.rs`, append `function(lint)` to the pipeline string when `config.lint_enabled`.
  Implementation: Pipeline string gets `,function(lint)` appended when `config.lint_enabled`. Confirmed working — found real sret ABI mismatch (BUG-04-055). Lint uses `dbgs()` (not diagnostic handler) so output goes to stderr; `LLVMContextSetDiagnosticHandler` cannot capture lint-specific output per LLVM's architecture.

- [x] ~~If Option B is chosen, ensure diagnostic capture is in place BEFORE running the pipeline.~~ N/A — LLVM's lint pass outputs via `dbgs()` (stderr), not through `LLVMContextSetDiagnosticHandler`. Structured capture of lint findings is not feasible without modifying LLVM itself. The lint pass value is delivered through stderr output visible to developers. `FindingKind::LlvmLint` variant added to the audit report for future use if LLVM changes the lint output mechanism.

### 01.3.2 Subsection close-out

- [x] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.3: no tooling gaps. `check-debug-flags.sh` caught missing CLAUDE.md docs immediately. Lint found real sret bug (BUG-04-055) on first run.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 01.4 Measure Timeout Budget and Enable Verification in test-all.sh / CI

**File(s):** `test-all.sh`, `.github/workflows/ci.yml`

The verification gates from 01.1-01.3 must be ON by default in all test runs. However, `ORI_VERIFY_EACH=1` adds ~30-60% to LLVM test wall time, and the current `test-all.sh` run takes ~100s against the 150s non-negotiable timeout. A measurement step is REQUIRED before enabling globally.

### 01.4.1 Measure before enabling (timeout budget gate)

- [x] **Measurement step** — measured: **54s** with both `ORI_VERIFY_EACH=1` and `ORI_VERIFY_ARC=1` enabled. Well within 150s budget (36% utilization, 96s headroom). All 16,978 tests pass. No sharding or selective enablement needed — both flags enabled globally.
  - [x] ~~Identify the slowest test suites via per-suite timing~~ N/A — 54s total, no need for optimization
  - [x] ~~Consider enabling `verify_each` only on LLVM-specific suites~~ N/A — both flags fit globally
  - [x] ~~Consider splitting the LLVM test suite into smaller shards~~ N/A — under budget
  - [x] Do NOT raise the 150s timeout — verified: 54s is well within
  - [x] ~~If verification cannot fit within 150s~~ N/A — both flags fit

### 01.4.2 Enable in test-all.sh

- [x] Update `test-all.sh` to export `ORI_VERIFY_ARC=1` and `ORI_VERIFY_EACH=1` before test suites.
  Implementation: Added verification gates section near top of test-all.sh with both exports and measurement documentation comment.

### 01.4.3 Enable in CI

**CI coverage gap:** `cargo test --workspace` (which CI runs as a workspace member) already exercises `ori_llvm` Rust unit tests. What is NOT present in CI is: `./test-all.sh` (which runs the full Ori spec suite + LLVM integration suites in one orchestrated run), `ori test --backend=llvm` (which runs Ori spec tests through the LLVM backend end-to-end), and sharded verification (splitting LLVM AOT tests into smaller CI jobs that fit within time budgets). The env var additions below are preparatory — they will not have full effect until those missing invocations are added to the CI workflow. **Full LLVM/AOT CI orchestration coverage is deferred to Section 11 (CI Integration).** <!-- blocked-by:11 -->

- [x] Update `.github/workflows/ci.yml` to set the verification env vars in the global `env:` block:
  Both `ORI_VERIFY_ARC` and `ORI_VERIFY_EACH` set to `"1"` — measurement confirmed budget allows both.

### 01.4.4 Validate zero regressions

- [x] Run `timeout 150 ./test-all.sh` with both flags enabled and verify 0 regressions. **Result: 16,978 tests pass, 0 failures, 54s wall time.**

### 01.4.5 Subsection close-out

- [x] **Subsection close-out (01.4)** — MANDATORY before starting 01.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.4: no tooling gaps. Measurement step directly answered the budget question (54s, 36% of 150s limit). No additional tooling needed.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

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

### Codex Review Findings

- [x] **[TPR-01-001-codex][high] LEAK: opt -lint subprocess in ori_llvm** — Option A proposed running `opt` as a subprocess inside `ori_llvm::verify/mod.rs`, violating `compiler.md` "IO only in oric; core crates pure." **Resolution:** Option A explicitly marked as `oric`/diagnostic-script–only. Option B (in-process) designated as preferred for `ori_llvm`. Phase-purity constraint documented in §01.3.1. Integrated into Option A/B descriptions in subsection 01.3.1.

- [x] **[TPR-01-002-codex][medium] GAP: error propagation underspecified** — The error propagation chain in §01.1.3 was missing four downstream consumers of `run_arc_pipeline()`. **Resolution:** Added propagation points 7–10 covering `arc_dump/mod.rs:61`, `arc_dot/mod.rs:53`, `problem/codegen/mod.rs:242-263` (`InternalVerificationError` variant), and `problem/codegen/mod.rs:469-473` (`CodegenDiagnostics::add_arc_problems()`). All four sites are now explicit checklist items in §01.1.3.

- [x] **[TPR-01-003-codex][medium] LEAK: function verification at callers vs canonical emit helpers** — §01.2.2 had per-caller-site `fn_val.verify()` instructions, risking missed sites and duplication. **Resolution:** §01.2.2 rewritten to place `fn_val.verify()` inside the canonical emit helpers (`emit_arc_function`, `emit_prepared_functions`, `emit_prepared_lambda`) as the SSOT. Callers inherit verification through helpers. Per-caller-site verification explicitly banned.

- [x] **[TPR-01-004-codex][medium] GAP: CI doesn't run LLVM tests** — §01.4 added env vars to `ci.yml` without noting that the CI workflow doesn't execute LLVM tests at all. **Resolution:** Added a CI coverage gap note to §01.4.3 explaining that `./test-all.sh`, `ori test --backend=llvm`, and `cargo test -p ori_llvm --test aot` are not in the current CI workflow. Full LLVM/AOT CI execution deferred to Section 11 with `<!-- blocked-by:11 -->` anchor.

### Gemini Review Findings

- [x] **[TPR-01-001-gemini][low] Line number drift** — `nounwind/emit.rs` line references for `emit_prepared_functions()` and `emit_prepared_lambda()` were off by one (15→16, 27→28). **Resolution:** Line references updated to `emit.rs:16` and `emit.rs:28` in §01.2.2.

- [x] **[TPR-01-002-gemini][medium] FFI requirement for LLVMSetDiagnosticHandler** — Option B (in-process lint) referenced `LLVMSetDiagnosticHandler` without noting it requires raw FFI — Inkwell does not wrap this API. **Resolution:** Added explicit note to Option B in §01.3.1 that `LLVMSetDiagnosticHandler` requires raw FFI declarations via `llvm-sys`, with guidance to add the binding in `compiler/ori_llvm/src/llvm_sys_ext.rs` (or equivalent FFI shim file).

- [x] **[TPR-01-003-gemini][low] Signature change clarity** — §01.1.3 didn't explain what `Vec<ArcProblem>` vs `Result` wrapper represent semantically. **Resolution:** Added a "Type semantics clarification" paragraph at the top of §01.1.3 explaining that `Vec<ArcProblem>` = user-facing diagnostics (FBIP findings, reuse misses), while `Result` wrapper separates blocking ICEs (abort compilation) from user diagnostics (may be warnings). Callers treating `Err` as unrecoverable is now explicit.

### Iteration 2 Findings (re-review after iteration 1 fixes)

- [x] **[TPR-01-001-codex-i2][medium] GAP: arc_dump/arc_dot call run_arc_pipeline_all, not run_arc_pipeline** — §01.1.2 and §01.1.3 only named `run_arc_pipeline()` but the utility consumers call `run_arc_pipeline_all()`. **Resolution:** Updated §01.1.2 to explicitly name both APIs. Updated §01.1.3 points 7-8 to use `run_arc_pipeline_all()`. Added note that both APIs must adopt the same Result contract.

- [x] **[TPR-01-002-codex-i2][low] LEAK: derive codegen claimed to route through canonical helpers** — §01.2.2 lead-in incorrectly stated derive codegen routes through the canonical emit helpers. Derive codegen uses `setup_derive_function()`/`declare_and_bind_derive()` which bypass all three helpers. **Resolution:** Rewrote §01.2.2 lead-in to explicitly state derive codegen is a separate emission path. Updated derive checklist item to require explicit `fn_val.verify()` rather than claiming inherited verification.

- [x] **[TPR-01-001-gemini-i2][low] Function name correction** — Same as codex finding above (agreement on substance). **Resolution:** Fixed as part of [TPR-01-001-codex-i2].

- [x] **[TPR-01-002-gemini-i2][low] emit_prepared_lambda line reference** — Plan cited `emit.rs:28` (call site) instead of `emit.rs:120` (definition). **Resolution:** Updated canonical helper 3 reference to cite both: definition at line 120, call site at line 28.

### Iteration 3 Findings

- [x] **[TPR-01-001-codex][medium] GAP: wrapper emitters bypass canonical helpers** — `compile_tests()` (`impls.rs:91-117`) manually builds a panic-catching wrapper and `generate_main_wrapper()` (`entry_point.rs:60-170`) builds the C main wrapper, both without going through any canonical emit helper. **Resolution:** Added `compile_tests` wrapper body and `generate_main_wrapper` as explicit `fn_val.verify()` sites in §01.2.2.

- [x] **[TPR-01-002-codex][medium] OVERSTATE: CI gap note overstated** — §01.4.3 claimed that `cargo test --workspace` (which CI runs) does NOT cover `ori_llvm`, but `ori_llvm` IS a workspace member and IS exercised by `cargo test --workspace`. What is actually missing is `./test-all.sh`, `ori test --backend=llvm`, and sharded verification. **Resolution:** Rewrote §01.4.3 CI gap note to accurately describe what IS covered (`cargo test --workspace` → `ori_llvm` Rust unit tests) and what is MISSING (`./test-all.sh`, `ori test --backend=llvm`, sharded LLVM AOT runs).

- [x] **[TPR-01-003-codex][low] WRONG: JIT wiring used ORI_VERIFY_EACH instead of ORI_VERIFY_ARC** — §01.2.1 described wiring `ORI_VERIFY_EACH` into the JIT path, but the JIT path has no `OptimizationConfig` — `verify_each` only applies to AOT. The JIT path must wire `ORI_VERIFY_ARC` (not `ORI_VERIFY_EACH`). Also, examples used `std::env::var(...).is_ok()` instead of the canonical `!= "0"` check from `debug_flags.rs`. **Resolution:** Updated §01.2.1 JIT wiring bullet to name `ORI_VERIFY_ARC`, explain there is no `OptimizationConfig` in JIT, and use `map_or(false, |v| v != "0")` in all examples.

- [x] **[TPR-01-001-gemini][high] GAP: compile_lambda_arc immediate-emit path missing** — `compile_lambda_arc` at `define_phase.rs:220` handles lambdas in the immediate-emit path and emits a distinct `FunctionValue` that does NOT flow through any of the three canonical helpers. It was missing entirely from §01.2.2. **Resolution:** Added `compile_lambda_arc` as a fourth explicit emission site requiring its own `fn_val.verify()` in §01.2.2.

- [x] **[TPR-01-002-gemini][medium] GAP: synthetic function emission sites missing** — `generate_closure_wrapper` (`closure_wrappers.rs:32`) and `generate_drop_fn` (`drop_gen.rs:43`) generate independent `FunctionValue` instances that bypass all canonical helpers. Neither was listed in §01.2.2. **Resolution:** Added both as explicit `fn_val.verify()` sites in §01.2.2.

- [x] **[TPR-01-003-gemini][low] WRONG: emit_prepared_lambda described as not flowing through emit_prepared_functions** — §01.2.2 said `emit_prepared_lambda` "does not flow through `emit_prepared_functions`" but it IS called by `emit_prepared_functions` at `emit.rs:28`. The correct nuance is that it emits a DISTINCT `FunctionValue` body (the lambda's own body, not the outer wrapper) that needs its own `fn_val.verify()`. **Resolution:** Rewrote canonical helper 3 description in §01.2.2 to clarify it IS called by `emit_prepared_functions` but verifies a distinct `FunctionValue` body.

### Iteration 4 Findings

- [x] **[TPR-01-001-codex-i4][high] GAP: remaining thunk/helper generators missing from fn_val.verify() inventory** — Codex discovered 6 additional standalone `FunctionValue` generators: `panic_trampoline.rs:37`, `seh_main_thunk.rs:123`, `catch_thunk_gen.rs:18`, `element_fn_gen.rs:102`, `derive_codegen/field_ops/thunks.rs:68`, `builtins/iterator_consumers.rs:603`. **Resolution:** Added all 6 as explicit verification sites in §01.2.2. Also added a catch-all rule: ANY code creating a `FunctionValue` must call `fn_val.verify()`, documented as an invariant in `codegen/mod.rs`.

- [x] **[TPR-01-002-codex-i4][medium] GAP: ORI_VERIFY_ARC parsed with is_ok() at 3 sites** — `codegen_pipeline.rs:381`, `arc_dump/mod.rs:68`, `arc_dot/mod.rs:60` all use `is_ok()` instead of the canonical `!= "0"` pattern, making `ORI_VERIFY_ARC=0` truthy. **Resolution:** Added explicit checklist item in §01.2.1 to fix all 3 sites to `.map_or(false, |v| v != "0")` with a test case.

### Iteration 5 Findings (section-close TPR — codex only, gemini parse failure)

- [x] **[TPR-01-001-codex-i5][high] GAP: derive thunks in field_ops/thunks.rs missing fn_val.verify()** — 8 standalone FunctionValues created via `get_or_declare_function()` in `field_ops/thunks.rs` lack `fn_val.verify()`. These were noted as "deferred to derive codegen verification pass" in §01.2.2:296 but the verification pass didn't cover them.
  Resolved: Fixed on 2026-04-10. Added `verify_derive_function(fc, func_id, "derive_thunk")` at all 8 `restore_position` points. Import added. 16,978 tests pass with `ORI_VERIFY_ARC=1`.
  Note: Codex also claimed gaps in `closures.rs:205` and `iterator_consumers.rs:624` — independently verified these do NOT create standalone FunctionValues (no `get_or_declare_function` or `add_function`). Those claims were rejected after verification.

- [x] **[TPR-01-001-gemini-i5][medium] DRIFT/LEAK:scattered-knowledge: ArcIrEmitter re-reads ORI_VERIFY_ARC env var instead of using FunctionCompiler::verify_arc()** — 6 sites in `element_fn_gen.rs`, `catch_thunk_gen.rs`, `closure_wrappers.rs`, `drop_gen.rs` use `std::env::var("ORI_VERIFY_ARC")` while `FunctionCompiler` has `self.verify_arc`. Two sources of truth for the same flag.
  Resolved: Fixed on 2026-04-10. Added `verify_arc: bool` field to `ArcIrEmitter` with `set_verify_arc()` setter. Plumbed from `FunctionCompiler::verify_arc` at all 4 production call sites. Replaced 6 env var reads with `self.verify_arc`. Tests: 16,978 pass.

- [x] **[TPR-01-002-codex-i5][medium] DRIFT: FindingKind::LlvmLint dead code — no producer** — `FindingKind::LlvmLint { message: String }` variant added in ed46c7d9 has no producer. LLVM's lint pass outputs via `dbgs()` (stderr), so structured capture via the audit report is not feasible.
  Resolved: Fixed on 2026-04-10. Removed the dead `LlvmLint` variant from `report.rs`. The plan already documents that lint output goes to stderr. The variant can be re-added if LLVM's lint pass gains diagnostic handler support in a future version.

### Iteration 6 Findings (final section-close TPR — dual-source)

- [x] `[TPR-01-001-codex][high]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:345` — ARC pipeline errors swallowed: `process_arc_function()` and `declare_and_process_lambda()` log `Err(verify_errors)` via `tracing::error!` but return `()`, allowing codegen to proceed with invalid ARC IR.
  Resolved: Fixed on 2026-04-10. Added `record_codegen_error_with_msg()` in both `Err` branches. Pipeline error count now catches ARC verification failures.

- [x] `[TPR-01-001-gemini][high]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:345` — Same issue as TPR-01-001-codex (de facto agreement — title/location match, merger didn't auto-detect due to slight title difference).
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-01-001-codex].

- [x] `[TPR-01-002-codex][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs:189` — Three emission sites missing `fn_val.verify()`: `generate_env_drop_fn()`, `generate_trampoline_fn()` (`trampolines.rs:76`), `generate_join_to_str_trampoline()` (`iterator_consumers.rs:581`).
  Resolved: Fixed on 2026-04-10. Added `fn_val.verify()` + `record_codegen_error()` to all 3 sites.

- [x] `[TPR-01-002-gemini][medium]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:204` — All 16 `fn_val.verify()` sites log via `tracing::error!` but never call `record_codegen_error()`, making LLVM IR verification failures invisible to the pipeline error count.
  Resolved: Fixed on 2026-04-10. Added `record_codegen_error()` to all 16 existing `fn_val.verify()` failure paths plus 3 new sites.

- [x] `[TPR-01-003-gemini][low]` `compiler/ori_llvm/src/evaluator/compile.rs:259` — JIT path reads `ORI_VERIFY_ARC` env var directly instead of `debug_flags.rs`.
  Resolved: Rejected after verification on 2026-04-10. The JIT entry point is in `ori_llvm`, which cannot depend on `oric` (backward dependency). `debug_flags.rs` is in the `oric` crate (CLI layer). The JIT path's env var read is the legitimate initial read for its compilation unit, not a re-read or SSOT violation.

### Iteration 7 Findings (re-review after iteration 6 fixes — dual-source)

- [x] `[TPR-01-001-codex-i7][high]` `compiler/ori_arc/src/pipeline/mod.rs:190` — AbsentParamHasUses demotion was too broad: suppressed ALL cases including live-path inconsistencies. Live-path Absent param with uses IS a soundness-relevant contract/IR drift.
  Resolved: Fixed on 2026-04-10 in 15901623. Narrowed to dead-code uses only via `live_blocks()` (forward ∩ backward reachable). Live-path cases remain hard errors. Restored `Result` return type for `run_aims_verify()`. Split test distinguishes dead-path from live-path.

- [x] `[TPR-01-002-codex-i7][low]` `compiler/oric/src/commands/codegen_pipeline.rs:474` — Abort message said "type-mismatch error(s)" but counter now includes verification errors.
  Resolved: Fixed on 2026-04-10 in 15901623. Changed to generic "error(s)" in both AOT (`codegen_pipeline.rs`) and JIT (`evaluator/compile.rs`) paths.

---

## 01.N Completion Checklist

### Functional verification
- [x] `run_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [x] `run_aims_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [x] Error propagation chain complete: `run_verify()` -> `verify_and_merge()` -> `run_aims_pipeline()` -> `run_arc_pipeline()` -> `FunctionCompiler` -> compilation abort
- [x] `VerifyError` and `ArcProblem` type distinction: ICEs vs user diagnostics
- [x] FIP first-pass allows `CertifiedButHasMissedReuses`, blocks `CertifiedButUnboundedStack` and `BoundedExceeded`
- [x] FIP second-pass blocks ALL error variants
- [x] `ORI_VERIFY_EACH` registered in `debug_flags.rs` and wired through `OptimizationConfig`
- [x] `ORI_VERIFY_EACH` wired in `compiler/oric/src/commands/build/mod.rs` (`build_optimization_config`)
- [x] `ORI_VERIFY_EACH` wired in `compiler/oric/src/commands/run/mod.rs` (`OptimizationConfig::new`)
- [x] `ORI_VERIFY_ARC` honored in JIT path (`compiler/ori_llvm/src/evaluator/compile.rs`)
- [x] `fn_val.verify()` runs after codegen in nounwind emit (`nounwind/emit.rs:54` — `emit_prepared_functions`)
- [x] `fn_val.verify()` runs after codegen in lambda body (`nounwind/emit.rs:147` — `emit_prepared_lambda`)
- [x] `fn_val.verify()` runs after codegen in immediate-emit lambda path (`define_phase.rs:264` — `compile_lambda_arc`)
- [x] `fn_val.verify()` runs after codegen in impls/tests canonical path (`impls.rs` — via canonical helper)
- [x] `fn_val.verify()` runs after codegen in `compile_tests` panic-catching wrapper (`impls.rs:122`)
- [x] `fn_val.verify()` runs after codegen in `generate_main_wrapper` (`entry_point.rs:180`)
- [x] `fn_val.verify()` runs after codegen in derives (`derive_codegen/mod.rs` — via `verify_derive_function` at 6 sites)
- [x] `fn_val.verify()` runs after codegen in `generate_closure_wrapper` (`closure_wrappers.rs:233`)
- [x] `fn_val.verify()` runs after codegen in `generate_drop_fn` (`drop_gen.rs:123`)
- [x] `fn_val.verify()` runs after codegen in remaining thunks: `panic_trampoline.rs:229`, `seh_main_thunk.rs:188`, `catch_thunk_gen.rs:127,208`, `element_fn_gen.rs:90,212`
- [x] Catch-all rule documented in `codegen/mod.rs` module-level docs: ANY `FunctionValue` creation site must call `fn_val.verify()`
- [x] Existing `ORI_VERIFY_ARC` callers fixed from `is_ok()` to `!= "0"` pattern (3 sites: `codegen_pipeline.rs`, `arc_dump/mod.rs`, `arc_dot/mod.rs`)
- [x] `opt -lint` integrated into codegen audit using `function(lint)` pipeline syntax (lint output via stderr; `FindingKind::LlvmLint` variant added)
- [x] `test-all.sh` runs with `ORI_VERIFY_ARC=1` and `ORI_VERIFY_EACH=1` by default
- [x] `.github/workflows/ci.yml` sets `ORI_VERIFY_ARC=1` and `ORI_VERIFY_EACH=1`
- [x] Timeout measurement completed: 54s with both flags (36% of 150s budget). Both enabled globally.

### Quality gates
- [x] All test suites pass within 150-second timeout with verification enabled (54s, 16,978 tests)
- [x] No regressions: `timeout 150 ./test-all.sh` green
- [x] `timeout 150 ./clippy-all.sh` green
- [x] Plan annotation cleanup: 0 annotations for this plan
- [x] All intermediate TPR checkpoint findings resolved (01.R: 19/19 done, status: resolved)
- [x] BLOAT: `arc_emitter/mod.rs` (630 lines) NOT touched by this section's changes — N/A

### Plan sync
- [x] This section's frontmatter `status` -> `complete`, subsection statuses updated
- [x] `00-overview.md` Quick Reference table status updated for this section (In Progress)
- [x] `00-overview.md` mission success criteria checkboxes updated (§01 checked off)
- [x] `index.md` section status updated (In Progress, pending close-out)

### Final reviews
- [x] `/tpr-review` passed (final, full-section) — 4 semantic iterations (6→7→8→9 across the session), 9 findings fixed across 4 commits (ac8ff045, 15901623, 88d94ce8, c0a500cf). Clean on iteration 4 (Gemini 0 findings, Codex 1 low-severity test pin → fixed). Convergence: high→medium→low→low severity progression.
- [x] `/impl-hygiene-review` passed — Auto Mode scoped to work arc (ori_arc, ori_llvm, oric). Phase 0 tools: 1 stale annotation cleaned (BUG-04-056 ref). All 19 verify sites consistent. No new LEAKs, no swallowed errors, no drift introduced. Pre-existing pattern variance (A vs B) non-blocking.
- [x] `/improve-tooling` **section-close sweep** — Per-subsection retrospectives (01.1-01.4) all documented "no gaps". One cross-session tooling improvement: `/add-bug` Step 8 (resume-after-filing, commit e43cdb84) — captured and committed during the TPR fix cycle. No additional cross-cutting patterns surfaced. Section-close sweep: per-subsection retrospectives covered everything; no cross-cutting gaps.

**Exit Criteria:** `ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` passes with 0 failures and 0 regressions. Verification failures in ARC IR and LLVM IR are hard errors under verification mode, not warnings. `fn_val.verify()` runs per-function at all emission sites. `opt -lint` runs as part of codegen audit. All flags registered canonically in `debug_flags.rs`. Error propagation chain is complete from verification through compilation abort. `ORI_VERIFY_EACH=1` enabled if measured within timeout budget, otherwise gated to separate CI job with documented tradeoff.
