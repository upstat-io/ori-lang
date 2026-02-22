---
section: "10"
title: "Legacy Cleanup & Unification"
status: not-started
goal: "Remove Tier 1 monolithic codegen, wire JIT to ARC pipeline, remove feature flags"
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "10.1"
    title: "Audit Tier 1 usage"
    status: not-started
  - id: "10.2"
    title: "Wire JIT evaluator to ARC pipeline"
    status: not-started
  - id: "10.3"
    title: "Delete Tier 1 codegen"
    status: not-started
  - id: "10.4"
    title: "Remove feature flags and conditionals"
    status: not-started
  - id: "10.5"
    title: "Final verification"
    status: not-started
---

# Section 10: Legacy Cleanup & Unification

**Status:** Not Started
**Goal:** One codegen path. The ARC IR pipeline is the only path from typed AST to LLVM IR. All legacy code removed.

**Context:** The old monolithic codegen (Tier 1) lived in `ori_llvm/src/codegen/` with 20+ `lower_*.rs` files that have now been deleted from git. However, the `use_arc_codegen` feature flag and conditional code paths may still exist. This section ensures complete removal and confirms the ARC pipeline (Tier 2) handles everything the old codegen did.

**Depends on:** Sections 01-05 — the ARC pipeline must be complete before legacy removal.

---

## 10.1 Audit Tier 1 Usage

- [ ] Grep the entire codebase for Tier 1 references:
  ```
  grep -r "use_arc_codegen" compiler/
  grep -r "ExprLowerer" compiler/
  grep -r "Tier 1\|tier_1\|tier1" compiler/
  grep -r "old codegen\|legacy codegen\|monolithic codegen" compiler/
  ```

- [ ] List every file that still references the old codegen system

- [ ] Check `Cargo.toml` files for feature flags related to `arc_codegen`

- [ ] Check `compiler/ori_llvm/src/codegen/mod.rs` for conditional dispatch:
  ```rust
  // Look for patterns like:
  if cfg!(feature = "arc_codegen") {
      // new path
  } else {
      // old path ← this needs to go
  }
  ```

- [ ] Document which deleted files (`git status` shows `D` files) were part of Tier 1

---

## 10.2 Wire JIT Evaluator to ARC Pipeline

**File:** `compiler/ori_llvm/src/evaluator.rs`

The JIT evaluator currently uses LLVM's JIT compilation. It should use the ARC pipeline for consistency.

- [ ] Modify `evaluator.rs` to use `ArcClassifier` and the ARC pipeline:
  - Initialize `ArcClassifier` from the type pool
  - Lower functions through `lower_function_can` → `run_arc_pipeline` → LLVM JIT emission
  - This gives JIT the same correctness guarantees as AOT

- [ ] Handle JIT-specific concerns:
  - JIT may skip optimization passes for speed (use `run_arc_pipeline` with a `JitMode` flag that skips reset/reuse and RC elimination)
  - JIT still needs correct RC insertion (no skipping that)

- [ ] Verify JIT tests still pass after the switch

---

## 10.3 Delete Tier 1 Codegen

**Files:** Already deleted from git working tree — this step confirms and cleans up any remnants.

The git status shows these files as deleted:
- `compiler/ori_llvm/src/codegen/expr_lowerer.rs`
- `compiler/ori_llvm/src/codegen/lower_builtin_methods/` (8 files)
- `compiler/ori_llvm/src/codegen/lower_calls.rs`
- `compiler/ori_llvm/src/codegen/lower_collection_methods/` (4 files)
- `compiler/ori_llvm/src/codegen/lower_collections.rs`
- `compiler/ori_llvm/src/codegen/lower_constructs.rs`
- `compiler/ori_llvm/src/codegen/lower_control_flow.rs`
- `compiler/ori_llvm/src/codegen/lower_conversion_builtins.rs`
- `compiler/ori_llvm/src/codegen/lower_error_handling.rs`
- `compiler/ori_llvm/src/codegen/lower_for_loop.rs`
- `compiler/ori_llvm/src/codegen/lower_iterator_trampolines.rs`
- `compiler/ori_llvm/src/codegen/lower_lambdas.rs`
- `compiler/ori_llvm/src/codegen/lower_literals.rs`
- `compiler/ori_llvm/src/codegen/lower_operators.rs`
- `compiler/ori_llvm/src/codegen/scope/` (2 files)

Total: ~25 files, ~11,000 lines

- [ ] Confirm all deleted files are accounted for
- [ ] Grep for any `mod` declarations or `use` statements still referencing deleted modules
- [ ] Remove any dead imports or conditional compilation blocks
- [ ] Clean up `compiler/ori_llvm/src/codegen/mod.rs` — remove all Tier 1 `mod` declarations

---

## 10.4 Remove Feature Flags and Conditionals

- [ ] Remove `use_arc_codegen` from all `Cargo.toml` feature lists
- [ ] Remove `#[cfg(feature = "arc_codegen")]` from all source files
- [ ] Remove any `if use_arc_codegen { ... } else { ... }` runtime conditionals
- [ ] Update `build-all.sh`, `llvm-build.sh`, `llvm-test.sh`, `llvm-clippy.sh` to remove feature flag arguments
- [ ] Update `compiler/oric/src/commands/compile_common.rs` to remove codegen path selection

---

## 10.5 Final Verification

- [ ] `cargo build` — compiles without Tier 1 code
- [ ] `./build-all.sh` — LLVM build succeeds
- [ ] `./test-all.sh` — all tests pass
- [ ] `./llvm-test.sh` — all AOT tests pass
- [ ] `./clippy-all.sh` — no warnings
- [ ] `cargo bl` and `cargo blr` — debug and release LLVM builds work
- [ ] Compile and run a non-trivial Ori program end-to-end

---

## 10.6 Completion Checklist

- [ ] Zero references to `ExprLowerer` in codebase
- [ ] Zero references to `use_arc_codegen` feature flag
- [ ] Zero conditional codegen dispatch (no if/else for old vs new path)
- [ ] JIT evaluator uses ARC pipeline
- [ ] All ~25 deleted files confirmed gone with no dangling references
- [ ] ~11,000 lines of legacy code removed
- [ ] All build scripts updated
- [ ] Full test suite green

**Exit Criteria:** `grep -r "ExprLowerer\|use_arc_codegen\|lower_builtin_methods\|lower_calls\|lower_collections\|lower_constructs\|lower_control_flow" compiler/` returns zero results (excluding plans/ and .md files).
