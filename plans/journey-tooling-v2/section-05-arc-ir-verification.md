---
section: "05"
title: "ARC IR-Level Verification"
status: not-started
goal: "Add a structural verification pass at the ARC IR level (before LLVM lowering) that catches RC bugs where all information is still visible"
inspired_by:
  - "Lean 4 Checker.lean (Compiler/IR/Checker.lean) — structural IR verification: variable scope, type consistency, jump validity"
  - "Rust rustc_mir_transform/src/check_*.rs — modular per-issue verification passes"
  - "Koka CheckFBIP.hs (Core/CheckFBIP.hs) — property verification on top of working RC system"
depends_on: []
sections:
  - id: "05.1"
    title: "Structural Invariants"
    status: not-started
  - id: "05.2"
    title: "RC Balance by Construction"
    status: not-started
  - id: "05.3"
    title: "Pipeline Integration"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: ARC IR-Level Verification

**Status:** Not Started
**Goal:** Add a verification pass to `ori_arc` that checks structural invariants of the ARC IR before LLVM lowering. This catches bugs at the level where all information is available (Construct = +1, RcInc = +1, RcDec = -1, type info, ownership annotations) — eliminating the need for heroic recovery at the LLVM IR level.

**Context:** The current verification happens at two levels: (1) the Rust `verify/` module walks live inkwell objects after LLVM lowering, and (2) the Python tools parse textual LLVM IR. Both suffer from information loss — by the time we're looking at LLVM IR, `Construct` has become `ori_rc_alloc` + GEP + store sequences, and ownership annotations are gone. Lean 4's `Checker.lean` validates their LCNF IR *before* lowering, where everything is explicit. We should do the same.

**ARC IR structure** (from `compiler/ori_arc/src/ir/`): `ArcFunction` contains `params: Vec<ArcParam>`, `blocks: Vec<ArcBlock>`, `var_types: Vec<Idx>`. Each `ArcBlock` has `id: ArcBlockId`, `params: Vec<(ArcVarId, Idx)>`, `body: Vec<ArcInstr>` (note: `body`, not `instructions`), and `terminator: ArcTerminator`.

**Note:** This section is **Rust code in `ori_arc`**, not Python. It's included in this plan because it's the architecturally correct solution to the false positive problem, even though it's independent of the Python tooling.

**Reference implementations:**
- **Lean 4** `Compiler/IR/Checker.lean`: Validates variable scope/uniqueness, type consistency, jump target validity, external function existence. ~200 lines, runs in O(n).
- **Rust** `rustc_mir_transform/src/check_alignment.rs`: Modular pass, `run_pass()` + `is_enabled()` pattern.
- **Koka** `Core/CheckFBIP.hs`: Property verification (FBIP compliance) on top of a working RC system. Produces warnings, not errors.

**Depends on:** Nothing (independent of Python tooling).

---

## 05.1 Structural Invariants

**File(s):** `compiler/ori_arc/src/verify/mod.rs` (new module) + `compiler/ori_arc/src/verify/tests.rs`

The module structure:
- `verify/mod.rs` — public `check_function()` entry point + structural checks
- `verify/tests.rs` — test cases (sibling tests pattern per CLAUDE.md)

Must also add `pub(crate) mod verify;` to `compiler/ori_arc/src/lib.rs` (between `mod pipeline;` and `pub mod rc_elim;`, alphabetically). Use `pub(crate)` since verification is internal -- external consumers should not call it directly. If needed for `cargo test -p ori_arc`, make `check_function` `pub`.

Verify structural properties of `ArcFunction` that must hold after lowering from CanExpr:

- [ ] **Variable scope**: Every `ArcVarId` used in an instruction must be defined (by a preceding instruction, as a function parameter, or as a block parameter) before use. No use-before-def.
  ```rust
  fn check_variable_scope(func: &ArcFunction) -> Vec<VerifyError> {
      // func.params is Vec<ArcParam>, each has a .var: ArcVarId field
      let mut defined: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
      let mut errors = Vec::new();
      for block in &func.blocks {
          // Block params are also definitions (phi-like values from Jump args)
          for (var, _ty) in &block.params {
              defined.insert(*var);
          }
          // ArcBlock uses .body, not .instructions
          for instr in &block.body {
              for used in instr.used_vars() {
                  if !defined.contains(&used) {
                      errors.push(VerifyError::UseBeforeDef { var: used, block: block.id });
                  }
              }
              if let Some(dst) = instr.defined_var() {
                  defined.insert(dst);
              }
          }
          // Terminators also use variables (e.g., Branch condition, Return value).
          for used in block.terminator.used_vars() {
              if !defined.contains(&used) {
                  errors.push(VerifyError::UseBeforeDef { var: used, block: block.id });
              }
          }
          // Invoke defines a dst in the normal successor — handled via
          // that block's param definitions (Jump args carry it).
      }
      errors
  }

  // NOTE: This flat-set approach is SOUND for SSA-form IR because each
  // ArcVarId is defined exactly once across the entire function. A use
  // in block B that references a definition in block A is valid if A
  // dominates B. Since we're checking ALL definitions globally (not
  // per-block), a non-dominating definition would still pass. For a
  // stricter check, use the dominator tree — but that's Phase 2 work.
  ```

> **NOTE (known limitation):** The flat-set approach is an over-approximation.
> It will not catch use-before-def where the definition exists but does not
> dominate the use (e.g., variable defined in one branch, used in the other
> branch's successor). Since `ori_arc` already has a `DominatorTree` in
> `graph/`, a future Phase 2 enhancement can use it for precise checking.
> Document this limitation in the code with a `// TODO(verify): ...` comment.

- [ ] **Block connectivity**: Every block referenced in a terminator (Jump, Branch, Switch, **Invoke**) must exist in the function. No dangling block references. The `ArcTerminator` enum has these block-referencing variants:
  - `Jump { target }` — 1 target
  - `Branch { then_block, else_block }` — 2 targets
  - `Switch { cases, default }` — N+1 targets
  - `Invoke { normal, unwind }` — 2 targets
  - `Return`, `Resume`, `Unreachable` — 0 targets

- [ ] **Terminator presence**: Every block must end with exactly one terminator. No blocks without terminators, no terminators in the middle of a block.

- [ ] **Type consistency**: If `var_reprs` maps a variable to a `ValueRepr`, RC operations on that variable must be type-compatible (e.g., `RcInc`/`RcDec` should never operate on variables with `ValueRepr::Scalar` -- those have no RC header). Note: use `ValueRepr` (per-variable representation from `var_reprs`), not `ArcClass` (higher-level type classification from `classify/`).

---

## 05.2 RC Balance by Construction

**File(s):** `compiler/ori_arc/src/verify/mod.rs`

Verify RC balance properties after the RC insertion pass:

- [ ] **No RC on scalars**: After `compute_var_reprs()`, `RcInc`/`RcDec` must never operate on variables with `ValueRepr::Scalar` in `var_reprs`. This would indicate a misclassification bug described in `.claude/rules/arc.md`.

- [ ] **Ownership consistency**: If a parameter has `ownership == Ownership::Borrowed` (available directly on `ArcParam`), no `RcDec` should target its `.var` within the function body (borrowed values are not owned, so must not be freed). Implementation:
  ```rust
  let borrowed_params: FxHashSet<ArcVarId> = func.params.iter()
      .filter(|p| p.ownership == Ownership::Borrowed)
      .map(|p| p.var)
      .collect();
  // Then for each RcDec { var, .. } in all blocks:
  //   if borrowed_params.contains(&var) → VerifyError::DecOnBorrowed
  ```

- [ ] **Invoke dst scoping**: `ArcTerminator::Invoke { dst, .. }` defines a variable that should be visible in the `normal` successor block but NOT in the `unwind` block. The scope checker must add `dst` to `defined` only for the normal successor path, not globally. This is a subtlety that the flat-set approach (see 05.1) does NOT handle correctly -- it would add `dst` globally. For correctness, either:
  1. Accept the flat-set approach with a documented over-approximation (no false positives, possible false negatives), OR
  2. Use per-block defined sets with dominator-tree propagation (more complex, fully correct)

- [ ] **Return value ownership**: If a function returns an `Owned` value, there must be a path from allocation/inc to the return without a matching dec.

- [ ] **No double-dec**: After RC elimination, no variable should have more `RcDec` operations than `RcInc + allocation` operations on any execution path.

---

## 05.3 Pipeline Integration

**File(s):** `compiler/ori_arc/src/pipeline.rs` (where `run_arc_pipeline()` lives; `lib.rs` just re-exports it)

Integrate the verifier into `run_arc_pipeline()` as a debug-only check:

- [ ] Add `verify_arc_function()` call after RC insertion and after RC elimination:
  ```rust
  // In run_arc_pipeline() (pipeline.rs):
  // ... existing passes ...
  crate::rc_insert::insert_rc_ops_with_ownership(&mut func, ...);

  #[cfg(debug_assertions)]
  {
      let errors = verify::check_function(&func);
      if !errors.is_empty() {
          // Log errors but don't panic — this is diagnostic, not blocking
          for e in &errors {
              tracing::warn!("ARC IR verification: {e:?}");
          }
      }
  }

  crate::rc_elim::eliminate_rc_ops_dataflow(&mut func, ...);
  // ... verify again after elimination ...
  ```

- [ ] Gate behind `debug_assertions` (zero cost in release builds)
- [ ] Add `ORI_VERIFY_ARC=1` environment variable for explicit opt-in in release builds (follows existing `ORI_AUDIT_CODEGEN` pattern -- acceptable for diagnostic tooling, not production passes)
- [ ] Add tests in `compiler/ori_arc/src/verify/tests.rs`

---

## 05.4 Completion Checklist

- [ ] `compiler/ori_arc/src/verify/mod.rs` exists with structural checks
- [ ] `pub(crate) mod verify;` added to `compiler/ori_arc/src/lib.rs`
- [ ] Variable scope, block connectivity, terminator presence verified
- [ ] RC-on-scalar detection works (catches `Scalar` misclassification)
- [ ] Ownership consistency checked (no dec on borrowed params)
- [ ] Integrated into `run_arc_pipeline()` under `debug_assertions`
- [ ] All existing tests pass: `cargo test -p ori_arc`
- [ ] Verifier produces zero warnings on all 12 journey programs
- [ ] `./test-all.sh` green

**Exit Criteria:** `ORI_VERIFY_ARC=1 cargo test -p ori_llvm` runs the ARC IR verifier on all AOT test programs with zero warnings. Any genuine RC bugs are caught at the ARC IR level before they become mysterious imbalances at the LLVM IR level.
