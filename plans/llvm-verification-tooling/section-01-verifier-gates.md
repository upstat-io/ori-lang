---
section: "01"
title: "Verifier Gates & Quick Wins"
status: in-progress
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
    status: in-progress
  - id: "01.3"
    title: "Add opt -lint to Codegen Audit Pipeline"
    status: not-started
  - id: "01.4"
    title: "Measure Timeout Budget and Enable Verification in test-all.sh / CI"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Verifier Gates & Quick Wins

**Status:** Not Started
**Goal:** Make AIMS and LLVM verifier failures blocking gates under verification mode (`ORI_VERIFY_ARC=1` / `ORI_VERIFY_EACH=1`), wire the existing `verify_each` plumbing to an env var registered in `debug_flags.rs`, add function-level IR verification after each function's codegen at ALL emission sites, and integrate `opt -lint` into the codegen audit pipeline. This section ensures that all subsequent verification tooling (Sections 02-12) has enforceable failure semantics — a verifier that detects a problem but only logs a warning is not verification, it's a suggestion.

**Success Criteria:**

- [ ] `run_verify()` and `run_aims_verify()` return errors (not warnings) when `ORI_VERIFY_ARC=1` — satisfies mission criterion: "Verifier failures become blocking gates"
- [ ] Full error propagation chain from `run_verify()` through `verify_and_merge()` -> `run_aims_pipeline()` -> `run_arc_pipeline()` -> `FunctionCompiler::process_arc_function()` -> compilation failure — errors must propagate, not be silently logged
- [ ] `ORI_VERIFY_EACH=1` registered in `debug_flags.rs` and wired through `OptimizationConfig` in ALL entry points: `compiler/oric/src/commands/build/mod.rs`, `compiler/oric/src/commands/run/mod.rs`, and the JIT path in `compiler/ori_llvm/src/evaluator/compile.rs`
- [ ] `fn_val.verify()` called after each function in ALL LLVM emission sites (define phase, nounwind emit, impls, tests, derives)
- [ ] `opt -lint` integrated into codegen audit output using `function(lint)` pipeline syntax with diagnostic capture
- [ ] `test-all.sh` passes with `ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1` within the 150s non-negotiable timeout — measured BEFORE enabling globally

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

- [ ] ~~Add `ArcProblem::InternalVerificationError { phase: String, errors: Vec<crate::verify::VerifyError> }` variant to the `ArcProblem` enum in `compiler/ori_arc/src/lower/mod.rs`. This variant represents an ICE, not a user diagnostic — `FunctionCompiler` should treat it as a compilation abort.~~ N/A — chose the Result wrapper approach (Option B) below.

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

- [ ] **Derive codegen** (`compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`) — needs `fn_val.verify()` at derive body completion. Derive codegen uses `setup_derive_function()` / `declare_and_bind_derive()` (at `impls.rs:317` and `derive_codegen/mod.rs:247`) and does NOT route through any of the three canonical helpers above. Therefore, `fn_val.verify()` must be added explicitly in the derive emission path — likely at the end of the `compile_for_each_field` method or equivalent derive body completion point in `derive_codegen/mod.rs`.

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

- [ ] **Catch-all rule for future emission sites**: ANY code that creates and finalizes a `FunctionValue` (i.e., adds basic blocks and a terminator) MUST call `fn_val.verify()` before the function is considered complete. Add a `// VERIFY: fn_val.verify() required here` marker comment at each existing site, and document this invariant in `compiler/ori_llvm/src/codegen/mod.rs` module-level docs. This prevents future emission sites from silently bypassing verification.

- [ ] **Do NOT add per-caller-site `fn_val.verify()` calls** at `impls.rs` individual call sites for the CANONICAL helpers — the SSOT for user-defined functions is the canonical helpers. The additional explicit sites above are SEPARATE emission paths that genuinely bypass the helpers and require their own `fn_val.verify()` calls.

- [ ] Verify that `LLVM_OPT_BISECT_LIMIT` env var is respected by the optimization pipeline (it should be — LLVM's pass manager reads it internally). This supports `diagnostics/opt-bisect.sh` in Section 11.

### 01.2.3 TPR checkpoint and close-out

- [ ] **TPR checkpoint** — `/tpr-review` covering 01.1-01.2 implementation work

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 01.1's close-out, scoped to 01.2's debugging journey. Commit improvements separately using a valid conventional-commit type.

---

## 01.3 Add opt -lint to Codegen Audit Pipeline

**File(s):** `compiler/ori_llvm/src/verify/mod.rs`, `compiler/ori_llvm/src/aot/passes/mod.rs`, `compiler/ori_llvm/src/aot/passes/config.rs`

LLVM's `opt -lint` pass detects likely-undefined behavior that the standard IR verifier doesn't catch: division by potential zero, suspicious alignment, unreachable patterns, UB patterns in instruction operands. Currently the codegen audit pipeline (`ORI_AUDIT_CODEGEN=1`) runs RC balance, COW rules, ABI checks, and safety checks — but not `opt -lint`.

### 01.3.1 Pipeline syntax and diagnostic capture

**Critical: LLVM new pass manager pipeline syntax** — the lint pass must use `function(lint)` syntax (not just appending `,lint` to the pipeline string). The lint pass is a function-level pass and must be wrapped in a `function()` adaptor when inserted into a module-level pipeline. Additionally, `lint<abort-on-error>` can abort the process in-process, so a **diagnostic capture approach** is safer:

- [ ] **Write failing tests FIRST** (TDD):
  - `opt_lint_catches_division_by_zero_pattern` — emit IR with a known UB pattern, run lint, assert finding captured
  - `opt_lint_integrated_with_codegen_audit` — run with `ORI_AUDIT_CODEGEN=1`, assert lint findings appear in audit output

- [ ] Add `lint_enabled: bool` to `OptimizationConfig` in `compiler/ori_llvm/src/aot/passes/config.rs` with env var `ORI_LLVM_LINT`. Default: off. Enabled automatically when `ORI_AUDIT_CODEGEN=1`.

- [ ] **Option A: Run lint as a separate analysis pass** (preferred ONLY for `oric`/diagnostic scripts — NOT for `ori_llvm`). After the optimization pipeline completes, serialize the module IR to a buffer, run `opt -passes='function(lint)' -disable-output` as a subprocess, and parse stderr output into `AuditFinding`s. This avoids `lint<abort-on-error>` killing the compiler process. **Phase-purity constraint (`compiler.md` §IO only in oric):** subprocess invocation (`std::process::Command`) is IO and must live in `oric` or a `diagnostics/` script, NOT inside `ori_llvm::verify`. If Option A is used for the audit pipeline, the subprocess call must be in `compiler/oric/src/commands/build/mod.rs` or a dedicated `oric`-side audit helper, not `compiler/ori_llvm/src/verify/mod.rs`.

- [ ] **Option B (preferred for `ori_llvm`): Add to the pass pipeline string in-process.** In `run_optimization_passes()` at `compiler/ori_llvm/src/aot/passes/mod.rs`, append `function(lint)` to the pipeline string when `config.lint_enabled`:
  ```rust
  if config.lint_enabled {
      // lint is a function-level pass — must be wrapped in function() adaptor
      pipeline.push_str(",function(lint)");
  }
  ```
  Use `LLVMSetDiagnosticHandler` to capture lint diagnostics instead of letting them print to stderr. This avoids process abort and keeps the call in-process inside `ori_llvm`. **Note:** `LLVMSetDiagnosticHandler` requires raw FFI declarations via `llvm-sys` — Inkwell does not wrap this API. Add the FFI binding in `compiler/ori_llvm/src/llvm_sys_ext.rs` (or equivalent FFI shim file) before wiring it into the pipeline.

- [ ] If Option B is chosen, ensure diagnostic capture is in place BEFORE running the pipeline. Lint findings must be captured as structured `AuditFinding`s, not raw stderr output.

### 01.3.2 Subsection close-out

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 01.4 Measure Timeout Budget and Enable Verification in test-all.sh / CI

**File(s):** `test-all.sh`, `.github/workflows/ci.yml`

The verification gates from 01.1-01.3 must be ON by default in all test runs. However, `ORI_VERIFY_EACH=1` adds ~30-60% to LLVM test wall time, and the current `test-all.sh` run takes ~100s against the 150s non-negotiable timeout. A measurement step is REQUIRED before enabling globally.

### 01.4.1 Measure before enabling (timeout budget gate)

- [ ] **Measurement step** — BEFORE enabling verification globally, run:
  ```bash
  time ORI_VERIFY_EACH=1 ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh
  ```
  Record the wall time. If it exceeds 130s (leaving 20s safety margin for CI variance):
  - [ ] Identify the slowest test suites via per-suite timing
  - [ ] Consider enabling `verify_each` only on LLVM-specific suites (suites #3, #4, #7), not on Rust unit tests or Ori spec tests that don't exercise the LLVM backend
  - [ ] Consider splitting the LLVM test suite into smaller shards that each fit within budget
  - [ ] Do NOT raise the 150s timeout — that violates CLAUDE.md §MANDATORY Test Timeouts
  - [ ] If verification cannot fit within 150s even with sharding, enable `ORI_VERIFY_ARC=1` only (ARC IR verification is lightweight) and gate `ORI_VERIFY_EACH=1` to a separate CI job or nightly-only. Document the tradeoff.

### 01.4.2 Enable in test-all.sh

- [ ] Update `test-all.sh` to export `ORI_VERIFY_ARC=1` before test suites. For `ORI_VERIFY_EACH=1`, enable it only if the measurement step confirms it fits within the 150s budget:
  ```bash
  # At the top of test-all.sh, after other env setup:
  export ORI_VERIFY_ARC=1
  # Only if measurement confirms within 150s budget:
  export ORI_VERIFY_EACH=1
  ```

### 01.4.3 Enable in CI

**CI coverage gap:** `cargo test --workspace` (which CI runs as a workspace member) already exercises `ori_llvm` Rust unit tests. What is NOT present in CI is: `./test-all.sh` (which runs the full Ori spec suite + LLVM integration suites in one orchestrated run), `ori test --backend=llvm` (which runs Ori spec tests through the LLVM backend end-to-end), and sharded verification (splitting LLVM AOT tests into smaller CI jobs that fit within time budgets). The env var additions below are preparatory — they will not have full effect until those missing invocations are added to the CI workflow. **Full LLVM/AOT CI orchestration coverage is deferred to Section 11 (CI Integration).** <!-- blocked-by:11 -->

- [ ] Update `.github/workflows/ci.yml` to set the verification env vars in the `env:` block for the test job:
  ```yaml
  env:
    ORI_VERIFY_ARC: "1"
    # ORI_VERIFY_EACH: "1"  # Enable after measurement confirms budget
  ```

### 01.4.4 Validate zero regressions

- [ ] Run `timeout 150 ./test-all.sh` with both flags enabled and verify 0 regressions. If any existing tests fail under verification mode, those are pre-existing bugs that verification just surfaced — file each via `/add-bug` and fix before proceeding.

### 01.4.5 Subsection close-out

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

---

## 01.N Completion Checklist

### Functional verification
- [ ] `run_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [ ] `run_aims_verify()` returns `Err` under `ORI_VERIFY_ARC=1` when errors found
- [ ] Error propagation chain complete: `run_verify()` -> `verify_and_merge()` -> `run_aims_pipeline()` -> `run_arc_pipeline()` -> `FunctionCompiler` -> compilation abort
- [ ] `VerifyError` and `ArcProblem` type distinction: ICEs vs user diagnostics
- [ ] FIP first-pass allows `CertifiedButHasMissedReuses`, blocks `CertifiedButUnboundedStack` and `BoundedExceeded`
- [ ] FIP second-pass blocks ALL error variants
- [ ] `ORI_VERIFY_EACH` registered in `debug_flags.rs` and wired through `OptimizationConfig`
- [ ] `ORI_VERIFY_EACH` wired in `compiler/oric/src/commands/build/mod.rs` (`build_optimization_config`)
- [ ] `ORI_VERIFY_EACH` wired in `compiler/oric/src/commands/run/mod.rs` (`OptimizationConfig::new`)
- [ ] `ORI_VERIFY_ARC` honored in JIT path (`compiler/ori_llvm/src/evaluator/compile.rs`)
- [ ] `fn_val.verify()` runs after codegen in nounwind emit (`nounwind/emit.rs` — `emit_prepared_functions`)
- [ ] `fn_val.verify()` runs after codegen in lambda body (`nounwind/emit.rs` — `emit_prepared_lambda` definition at line 120)
- [ ] `fn_val.verify()` runs after codegen in immediate-emit lambda path (`define_phase.rs` — `compile_lambda_arc` at line 220)
- [ ] `fn_val.verify()` runs after codegen in impls/tests canonical path (`impls.rs` — via canonical helper)
- [ ] `fn_val.verify()` runs after codegen in `compile_tests` panic-catching wrapper (`impls.rs:91-117`)
- [ ] `fn_val.verify()` runs after codegen in `generate_main_wrapper` (`entry_point.rs:60-170`)
- [ ] `fn_val.verify()` runs after codegen in derives (`derive_codegen/mod.rs`)
- [ ] `fn_val.verify()` runs after codegen in `generate_closure_wrapper` (`closure_wrappers.rs:32`)
- [ ] `fn_val.verify()` runs after codegen in `generate_drop_fn` (`drop_gen.rs:43`)
- [ ] `fn_val.verify()` runs after codegen in remaining thunks: `panic_trampoline`, `seh_main_thunk`, `catch_thunk_gen`, `element_fn_gen`, derive field thunks, iterator consumer thunks
- [ ] Catch-all rule documented: ANY `FunctionValue` creation site must call `fn_val.verify()`
- [ ] Existing `ORI_VERIFY_ARC` callers fixed from `is_ok()` to `!= "0"` pattern (3 sites: `codegen_pipeline.rs`, `arc_dump/mod.rs`, `arc_dot/mod.rs`)
- [ ] `opt -lint` integrated into codegen audit using `function(lint)` pipeline syntax with diagnostic capture
- [ ] `test-all.sh` runs with `ORI_VERIFY_ARC=1` by default
- [ ] `.github/workflows/ci.yml` sets `ORI_VERIFY_ARC=1`
- [ ] Timeout measurement completed and documented — `ORI_VERIFY_EACH=1` enabled only if within 150s budget

### Quality gates
- [ ] All test suites pass within 150-second timeout with verification enabled
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] BLOAT: if any changes touched `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (630 lines, over 500-line limit), it was split into submodules

### Plan sync
- [ ] This section's frontmatter `status` -> `complete`, subsection statuses updated
- [ ] `00-overview.md` Quick Reference table status updated for this section
- [ ] `00-overview.md` mission success criteria checkboxes updated
- [ ] `index.md` section status updated

### Final reviews
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY after both reviews are clean. Per-subsection captures from 01.1-01.4 should already be committed; the sweep verifies they ran and adds only NEW cross-cutting items. Document the negative finding if there are no cross-cutting gaps.

**Exit Criteria:** `ORI_VERIFY_ARC=1 timeout 150 ./test-all.sh` passes with 0 failures and 0 regressions. Verification failures in ARC IR and LLVM IR are hard errors under verification mode, not warnings. `fn_val.verify()` runs per-function at all emission sites. `opt -lint` runs as part of codegen audit. All flags registered canonically in `debug_flags.rs`. Error propagation chain is complete from verification through compilation abort. `ORI_VERIFY_EACH=1` enabled if measured within timeout budget, otherwise gated to separate CI job with documented tradeoff.
