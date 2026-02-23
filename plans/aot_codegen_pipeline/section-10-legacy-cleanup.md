---
section: "10"
title: "Legacy Cleanup & Unification"
status: complete
goal: "Remove Tier 1 monolithic codegen, wire JIT to ARC pipeline, remove feature flags"
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "10.1"
    title: "Audit Tier 1 usage"
    status: complete
  - id: "10.2"
    title: "Wire JIT evaluator to ARC pipeline"
    status: complete
  - id: "10.3"
    title: "Delete Tier 1 codegen"
    status: complete
  - id: "10.4"
    title: "Remove feature flags and conditionals"
    status: complete
  - id: "10.5"
    title: "Final verification"
    status: complete
---

# Section 10: Legacy Cleanup & Unification

**Status:** Complete (2026-02-23)
**Goal:** One codegen path. The ARC IR pipeline is the only path from typed AST to LLVM IR. All legacy code removed.

**Context:** The old monolithic codegen (Tier 1) was completely removed during Sections 01-09. The `use_arc_codegen` feature flag, `ExprLowerer`, and all `lower_*.rs` files were deleted as part of the V2 architecture rollout. This section confirmed the cleanup was complete and removed the last vestige: the `Option` wrapper on `ArcIrEmitter::classifier` (which allowed `None` for the defunct Tier 1 path).

**Depends on:** Sections 01-05 — all complete.

---

## 10.1 Audit Tier 1 Usage

- [x] Grep the entire codebase for Tier 1 references:
  - `use_arc_codegen`: zero references
  - `ExprLowerer`: zero references
  - `Tier 1` / `tier_1` / `tier1`: one stale comment in `arc_emitter/mod.rs` (fixed)
  - `old codegen` / `legacy codegen` / `monolithic codegen`: zero references

- [x] List every file that still references the old codegen system
  - Only finding: `arc_emitter/mod.rs:132` had a comment "Tier 1 path" on the `Option<&dyn ArcClassification>` field — removed

- [x] Check `Cargo.toml` files for feature flags related to `arc_codegen`
  - Zero references in any `Cargo.toml`

- [x] Check `compiler/ori_llvm/src/codegen/mod.rs` for conditional dispatch
  - Clean V2 structure only — no conditional codegen paths

- [x] Document which deleted files were part of Tier 1
  - All ~25 files already deleted; zero `Glob` matches for `expr_lowerer*`, `lower_*`, `scope/`

---

## 10.2 Wire JIT Evaluator to ARC Pipeline

**File:** `compiler/ori_llvm/src/evaluator.rs`

Already complete — the evaluator was wired to the ARC pipeline during Sections 01-05:

- [x] Modify `evaluator.rs` to use `ArcClassifier` and the ARC pipeline:
  - `evaluator.rs:286` creates `ArcClassifier::new(self.pool)`
  - `evaluator.rs:291-374` lowers all functions through `lower_function_can`
  - `evaluator.rs:376` runs `infer_borrows(&arc_functions, &classifier)`
  - `evaluator.rs:389` passes `&classifier` to `FunctionCompiler::new`

- [x] Handle JIT-specific concerns:
  - JIT uses full `run_arc_pipeline` (no JitMode flag needed — the pipeline already handles JIT correctly)
  - RC insertion is always enabled

- [x] Verify JIT tests still pass after the switch
  - 428 LLVM tests pass, 0 failed

---

## 10.3 Delete Tier 1 Codegen

Already complete — all files deleted during V2 migration:

- [x] Confirm all deleted files are accounted for
  - `Glob` for `expr_lowerer*`, `lower_*`, `scope/` returns zero results
- [x] Grep for any `mod` declarations or `use` statements still referencing deleted modules
  - Zero references found
- [x] Remove any dead imports or conditional compilation blocks
  - None found
- [x] Clean up `compiler/ori_llvm/src/codegen/mod.rs` — remove all Tier 1 `mod` declarations
  - Already clean — only V2 modules declared

---

## 10.4 Remove Feature Flags and Conditionals

Already complete — `use_arc_codegen` was never a feature flag in the current codebase:

- [x] Remove `use_arc_codegen` from all `Cargo.toml` feature lists — zero references found
- [x] Remove `#[cfg(feature = "arc_codegen")]` from all source files — zero references found
- [x] Remove any `if use_arc_codegen { ... } else { ... }` runtime conditionals — zero references found
- [x] Update build scripts — no feature flag arguments to remove
- [x] Update `compile_common.rs` — no codegen path selection exists

**Additional cleanup performed:** Removed the `Option` wrapper on `ArcIrEmitter::classifier` field:
- Changed `classifier: Option<&'a dyn ArcClassification>` → `classifier: &'a dyn ArcClassification`
- Simplified ~12 `self.classifier.is_some_and(|c| c.needs_rc(...))` → `self.classifier.needs_rc(...)`
- Removed 2 `let Some(classifier) = self.classifier else { return ... }` early-return guards
- Updated 3 `FunctionCompiler` call sites from `Some(classifier as &dyn ...)` → `classifier as &dyn ...`
- Updated 1 test that passed `None` → now uses `TestClassifier` for scalar type verification

---

## 10.5 Final Verification

- [x] `cargo build` — compiles without Tier 1 code
- [x] `./build-all.sh` — LLVM build succeeds (`cargo bl` clean)
- [x] `./test-all.sh` — 9408 passed, 7 pre-existing failures (same as baseline)
- [x] `./llvm-test.sh` — 428 passed, 0 failed, 3 ignored
- [x] `./clippy-all.sh` — no new warnings (pre-existing `doc_markdown`, `declare_builtins!` issues unrelated)
- [x] `cargo bl` and `cargo blr` — both compile clean
- [x] Non-LLVM build (`cargo b`) — compiles clean

---

## 10.6 Completion Checklist

- [x] Zero references to `ExprLowerer` in codebase
- [x] Zero references to `use_arc_codegen` feature flag
- [x] Zero conditional codegen dispatch (no if/else for old vs new path)
- [x] JIT evaluator uses ARC pipeline
- [x] All ~25 deleted files confirmed gone with no dangling references
- [x] ~11,000 lines of legacy code removed
- [x] All build scripts updated (no changes needed)
- [x] Full test suite green (9408 passed, 7 pre-existing failures)
- [x] `ArcIrEmitter::classifier` made non-optional (removed last Tier 1 vestige)

**Exit Criteria:** `grep -r "ExprLowerer\|use_arc_codegen\|lower_builtin_methods\|lower_calls\|lower_collections\|lower_constructs\|lower_control_flow" compiler/` returns zero results. ✅ Verified.
