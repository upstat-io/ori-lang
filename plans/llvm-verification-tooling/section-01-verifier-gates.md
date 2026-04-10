---
section: "01"
title: "Verifier Gates & Quick Wins"
status: not-started
reviewed: false
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
    status: not-started
  - id: "01.2"
    title: "Wire ORI_VERIFY_EACH, Function-Level Verify, and verify_each Across All Entry Points"
    status: not-started
  - id: "01.3"
    title: "Add opt -lint to Codegen Audit Pipeline"
    status: not-started
  - id: "01.4"
    title: "Measure Timeout Budget and Enable Verification in test-all.sh / CI"
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

- [ ] **Write failing tests FIRST** (TDD) — create `compiler/ori_arc/src/pipeline/tests.rs` (new file; add `#[cfg(test)] mod tests;` to `pipeline/mod.rs`):
  - `verify_returns_err_when_verify_true_and_errors_found` — construct a malformed `ArcFunction`, call `run_verify()` with `verify=true`, assert `Err` returned
  - `verify_warns_only_when_debug_assertions_and_no_explicit_verify` — same malformed input, `verify=false`, assert `Ok(())` returned (warnings only)
  - `aims_verify_blocks_on_absent_param_has_uses` — construct function where `Cardinality::Absent` param has uses, `verify=true`, assert `Err`
  - Verify tests FAIL before implementation (proves understanding)

- [ ] Modify `run_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:128-138`) to return `Result<(), Vec<crate::verify::VerifyError>>` instead of `()`. When `verify` is true and errors are found, return `Err(errors)` instead of logging and continuing. When `verify` is false (and only `debug_assertions` is active), keep the current warning behavior — debug mode is diagnostic, explicit verification mode is blocking.
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

- [ ] Apply the same pattern to `run_aims_verify()` (`compiler/ori_arc/src/pipeline/mod.rs:144-162`) — return `Result<(), Vec<VerifyError>>`, error under explicit verify mode.

### 01.1.2 Add `VerifyError` variant to `ArcProblem` for type mapping

Verification errors from the ARC IR verifier are Internal Compiler Errors (ICEs), fundamentally different from user-facing `ArcProblem`s (like FBIP enforcement diagnostics). The pipeline must distinguish them:

- [ ] Add `ArcProblem::InternalVerificationError { phase: String, errors: Vec<crate::verify::VerifyError> }` variant to the `ArcProblem` enum in `compiler/ori_arc/src/lower/mod.rs`. This variant represents an ICE, not a user diagnostic — `FunctionCompiler` should treat it as a compilation abort.

- [ ] Alternatively, change `run_arc_pipeline()` (`compiler/ori_arc/src/pipeline/mod.rs:36`) to return `Result<Vec<ArcProblem>, ArcVerificationError>` where `ArcVerificationError` wraps the verification failures with phase/function context. The `Vec<ArcProblem>` remains for user-facing diagnostics (FBIP), while `Err` means an ICE from verification. This is the cleaner approach since it uses the type system to enforce that ICEs cannot be silently iterated over.

### 01.1.3 Propagate errors through the full pipeline chain

**Type semantics clarification:** The current `Vec<ArcProblem>` return type from `run_arc_pipeline()` represents **user-facing diagnostics** — FBIP enforcement findings, optimization skips, and reuse misses that the user may need to act on. The `Result` wrapper introduced in §01.1.2 serves a different purpose: it separates **blocking ICEs** (internal compiler errors from IR invariant violations — things that should never happen and abort compilation immediately) from **user diagnostics** (things that may be warnings or fixable errors). The type `Result<Vec<ArcProblem>, ArcVerificationError>` reads as: "either a list of user-facing ARC diagnostics (Ok), or an ICE from verification that prevents codegen from continuing (Err)." Callers must treat `Err` as unrecoverable — they should emit an ICE diagnostic and halt, not continue compiling with a potentially corrupt IR.

Currently, errors from `run_verify()` are silently consumed at multiple call sites. The full propagation chain must be:

1. **`verify_and_merge()` in `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs:10`** — currently calls `run_verify()` and `run_aims_verify()` without checking returns. Must return `Result` and propagate errors from both verification steps (steps 6-7).

2. **`emit_postprocess()` in `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs:40`** — currently calls `run_verify()` for step 11 without checking. Must return `Result<Vec<ArcProblem>, ArcVerificationError>`.

3. **`run_aims_pipeline()` in `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs`** — calls `verify_and_merge()` at line 193. Must propagate the `Result`.

4. **`run_arc_pipeline()` in `compiler/ori_arc/src/pipeline/mod.rs:36`** — currently returns `Vec<ArcProblem>`. Must return `Result<Vec<ArcProblem>, ArcVerificationError>` (or include ICEs via the `ArcProblem::InternalVerificationError` variant).

5. **`FunctionCompiler::process_arc_function()` in `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:311`** — currently iterates `arc_problems` with `debug!()`. Must check for verification errors and abort compilation (return an error to the caller or accumulate ICE diagnostics that block codegen).

6. **Lambda compilation** in `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:409` — same `run_arc_pipeline()` call, same propagation needed.

7. **`compiler/oric/src/arc_dump/mod.rs:61`** — currently uses `let _problems = run_arc_pipeline(...)` (result intentionally discarded). Once `run_arc_pipeline()` returns `Result<Vec<ArcProblem>, ArcVerificationError>`, this site must handle the `Err` branch: propagate verification errors up to the caller or surface them as a compilation diagnostic. Do NOT leave `_problems` discarding an `Err`.

8. **`compiler/oric/src/arc_dot/mod.rs:53`** — same `let _problems =` pattern as `arc_dump`. Must handle the `Err` branch after the signature change.

9. **`compiler/oric/src/problem/codegen/mod.rs:242-263`** — `CodegenProblem` mapping. The `ArcProblem` -> `CodegenProblem` mapping must include a case for `ArcProblem::InternalVerificationError` (or the `ArcVerificationError` ICE path). Currently this mapping has no catch-all for ICE variants. Add an `InternalVerificationError` arm that emits a compiler ICE diagnostic rather than a user-facing `CodegenProblem`.

10. **`compiler/oric/src/problem/codegen/mod.rs:469-473`** — `CodegenDiagnostics::add_arc_problems()`. This method iterates `Vec<ArcProblem>` and maps each to a `CodegenProblem`. Once the `InternalVerificationError` variant exists (point 9), this method must propagate it — either by returning `Result` or by accumulating ICEs in a separate list that causes compilation to abort.

- [ ] Implement each of the 10 propagation points above. Write a test that constructs a verification failure deep in the pipeline and asserts it surfaces as a compilation error (not a silent log message).

### 01.1.4 Fix FIP `debug_assert!` — first-pass vs second-pass distinction

The FIP structural checks at `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:164-186` and `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs:192-197` both use `debug_assert!(false, ...)` which disappears in release builds. These must be replaced with explicit error returns under `verify_arc` mode.

**Critical distinction — do NOT break the two-pass FIP pattern:**

- **First pass** (step 5a, `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs:164-186`): `CertifiedButHasMissedReuses` errors are EXPECTED because `may_deallocate` facts haven't been updated yet (the contract has optimistic `may_deallocate=false` from interprocedural analysis). Only `CertifiedButUnboundedStack` and `BoundedExceeded` are genuine structural violations that should be blocking errors. The existing code at lines 170-184 already implements this distinction correctly in its match arms — preserve this logic when replacing `debug_assert!` with error returns.

- **Second pass** (batch.rs, `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs:192-197`): ALL FIP errors should be blocking because `may_deallocate` facts have been recomputed. The existing `batch.rs` code treats all errors the same (logs + `debug_assert!`) — after replacing with explicit returns, ALL variants must be blocking here.

- [ ] In `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs` (first pass, step 5a), replace `debug_assert!(false, "FIP verification failed: {e}")` at line 182 with: when `config.verify_arc` is true, collect `CertifiedButUnboundedStack` and `BoundedExceeded` errors into a `Vec` and return them as pipeline errors. Continue to only `tracing::debug!` for `CertifiedButHasMissedReuses` (expected in first pass).

- [ ] In `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs` (second pass), replace `debug_assert!(false, "FIP post-recompute verification failed: {e}")` at line 196 with: when `verify_arc` is true, ALL FIP errors are blocking. Collect and return them.

- [ ] Write test: `fip_first_pass_allows_missed_reuses_but_blocks_structural` — verify that `CertifiedButHasMissedReuses` is non-blocking in first pass but `CertifiedButUnboundedStack` IS blocking.
- [ ] Write test: `fip_second_pass_blocks_all_errors` — verify that ALL FIP error variants (including `CertifiedButHasMissedReuses`) are blocking in the second pass.

### 01.1.5 Subsection close-out

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 01.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ...`).

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

- [ ] **Write failing tests FIRST** (TDD):
  - `verify_each_env_var_registered_in_debug_flags` — set env var, assert it is read
  - `verify_each_wired_through_build_optimization_config` — assert `with_verify_each(true)` is called when env var set
  - `verify_each_wired_through_run_optimization_config` — same for the `run` command path
  - Verify tests FAIL before implementation

- [ ] Register `ORI_VERIFY_EACH` in `compiler/oric/src/debug_flags.rs` (after `ORI_VERIFY_ARC` at line 132):
  ```rust
  /// Enable LLVM IR verification after every optimization pass.
  ///
  /// Catches which optimization pass breaks IR well-formedness.
  /// Significant performance impact (~30-60% slower LLVM tests).
  /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
  ORI_VERIFY_EACH
  ```
  Ensure `diagnostics/check-debug-flags.sh` picks up the new flag automatically (it should — it reads the `debug_flags!` macro output).

- [ ] Wire `ORI_VERIFY_EACH` through `build_optimization_config` in `compiler/oric/src/commands/build/mod.rs` (around line 158). The `OptimizationConfig` already has `.with_verify_each(bool)` at `compiler/ori_llvm/src/aot/passes/config.rs:321` — just connect the env var:
  ```rust
  let verify_each = std::env::var("ORI_VERIFY_EACH").is_ok();
  let opt_config = OptimizationConfig::new(level)
      .with_lto(lto)
      .with_verify_each(verify_each);
  ```

- [ ] **Wire `ORI_VERIFY_EACH` through the `run` command** in `compiler/oric/src/commands/run/mod.rs:289`. Currently constructs `OptimizationConfig::new(O2)` directly without `verify_each`:
  ```rust
  // Current:
  let opt_config = ori_llvm::aot::OptimizationConfig::new(ori_llvm::aot::OptimizationLevel::O2);
  // Target:
  let verify_each = std::env::var("ORI_VERIFY_EACH").is_ok();
  let opt_config = ori_llvm::aot::OptimizationConfig::new(ori_llvm::aot::OptimizationLevel::O2)
      .with_verify_each(verify_each);
  ```

- [ ] **Wire `ORI_VERIFY_ARC` through the JIT path** in `compiler/ori_llvm/src/evaluator/compile.rs:259`. Currently hardcoded to `false` with comment "verification via cfg!(debug_assertions) only for JIT". This means `ori test --backend=llvm` never honors `ORI_VERIFY_ARC=1`:
  ```rust
  // Current (line 259):
  false, // verification via cfg!(debug_assertions) only for JIT
  // Target:
  std::env::var("ORI_VERIFY_ARC").is_ok(), // Honor ORI_VERIFY_ARC in JIT mode
  ```

### 01.2.2 Add function-level verification at ALL emission sites

Function-level `fn_val.verify()` must run after EVERY function's codegen completes — not just the define phase. The SSOT approach is to add verification inside the **canonical emit helpers** (`emit_arc_function`, `emit_prepared_functions`, `emit_prepared_lambda`) rather than at each individual caller site. Callers (`impls.rs`, `compile_tests`, derive codegen) route through these helpers and therefore inherit verification automatically without requiring per-call-site changes.

**Inkwell API semantics (VERIFIED from existing test code):** `FunctionValue::verify(print_to_stderr: bool)` returns `true` on SUCCESS and `false` on FAILURE. This is confirmed by existing test assertions like `assert!(func.verify(false), "valid after simplification")` at `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify/tests.rs:65`. This is the OPPOSITE of what one might assume — `true` means valid.

- [ ] **Write failing test FIRST**: `function_level_verify_catches_malformed_ir` — construct a function with an unterminated basic block (missing terminator instruction), call `fn_val.verify(false)`, assert it returns `false`.

- [ ] **Canonical helper 1: `emit_arc_function`** — locate the canonical `emit_arc_function` helper and add `fn_val.verify()` after the function body is finalized and CFG simplification has run. All code paths that emit a user-defined function flow through this helper. Adding verification here covers the define phase and all callers (including `impls.rs` trait method emission) automatically.
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

- [ ] **Canonical helper 2: `emit_prepared_functions`** (`compiler/ori_llvm/src/codegen/function_compiler/nounwind/emit.rs:16`). After `emitter.emit_function()` and `simplify_cfg()`, add the same `fn_val.verify()` call. This covers the nounwind two-pass path. Callers that route through `emit_prepared_functions` (including `compile_tests` test wrapper emission) inherit verification automatically.

- [ ] **Canonical helper 3: `emit_prepared_lambda`** (`compiler/ori_llvm/src/codegen/function_compiler/nounwind/emit.rs:28`). After lambda codegen, add `fn_val.verify()`. Lambda emission is a distinct path that does not flow through `emit_prepared_functions` — it needs its own verification call inside the helper.

- [ ] **Derive codegen** (`compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`). Check whether derived method emission routes through one of the three canonical helpers above. If yes, no additional change is needed — verification is inherited. If derived methods have a standalone emission path that bypasses all three helpers, add `fn_val.verify()` at that path. Document which case applies during implementation.

- [ ] **Do NOT add per-caller-site `fn_val.verify()` calls** at `impls.rs` individual call sites or `compile_tests` loop bodies — the SSOT is the canonical helpers, not individual callers.

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

**CI coverage gap:** The current `.github/workflows/ci.yml` workflow does not run `./test-all.sh`, `ori test --backend=llvm`, or `cargo test -p ori_llvm --test aot`. As a result, LLVM/AOT test results are not verified in CI at all. The env var additions below are preparatory — they will not have effect until the CI workflow is wired to actually execute the LLVM test suite. **Full LLVM/AOT CI execution coverage is deferred to Section 11 (CI Integration).** <!-- blocked-by:11 -->

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
- [ ] `fn_val.verify()` runs after codegen in nounwind emit (`nounwind/emit.rs`)
- [ ] `fn_val.verify()` runs after codegen in impls/tests (`impls.rs`)
- [ ] `fn_val.verify()` runs after codegen in derives (`derive_codegen/mod.rs`)
- [ ] `fn_val.verify()` runs after codegen in lambda emit (`nounwind/emit.rs`)
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
