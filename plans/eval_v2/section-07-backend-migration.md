---
section: "07"
title: Backend Migration
status: not-started
goal: Rewrite ori_eval and ori_arc/ori_llvm to dispatch on CanExpr instead of ExprKind, then delete all ExprKind dispatch from backends
sections:
  - id: "07.1"
    title: Evaluator Migration
    status: not-started
  - id: "07.2"
    title: LLVM/ARC Migration
    status: not-started
  - id: "07.3"
    title: Dead Code Removal
    status: not-started
  - id: "07.4"
    title: Sync Verification
    status: not-started
  - id: "07.5"
    title: Completion Checklist
    status: not-started
---

# Section 07: Backend Migration

**Status:** Not Started
**Goal:** The payoff. Rewrite both backends to consume `CanExpr` exclusively. Delete all `ExprKind` dispatch from backends. New language features only need one implementation: in `ori_canon`.

**Depends on:** Sections 01-04 (canonical IR types, lowering, patterns, constants) must be complete and validated. Sections 05-06 (eval modes, diagnostics) are independent and can proceed in parallel.

**Prior art:**
- **Roc** — Both dev backend and gen_llvm consume `mono::Stmt`/`mono::Expr` exclusively. Zero parse-AST dispatch in either backend.
- **Elm** — JS codegen consumes `Opt.Expr` exclusively. Zero `Can.Expr_` dispatch in codegen.

---

## 07.1 Evaluator Migration

Rewrite `ori_eval` interpreter dispatch from `ExprKind` to `CanExpr`.

**Strategy:** Shadow approach — build new `eval_canon(can_id: CanId)` alongside existing `eval_inner(expr_id: ExprId)`. Validate both produce identical results on the spec test suite. Then delete `eval_inner`.

- [ ] Add `CanonResult` as a field on `Interpreter` (alongside existing `ExprArena`)
- [ ] Implement `eval_canon(can_id: CanId) -> EvalResult` dispatch on `CanExpr`
  - [ ] All `CanExpr` variants handled exhaustively (no `_ =>` catch-all)
  - [ ] Reuse existing evaluation logic from `exec/` modules — adapt to `CanId` references
  - [ ] `CanExpr::Constant(id)` → return value from `ConstantPool` directly
  - [ ] `CanExpr::Match { decision_tree, .. }` → call `eval_decision_tree()` (Section 03)
  - [ ] No sugar variants to handle (type-level guarantee)
- [ ] Validate: run `cargo st tests/` through both old and new paths, compare results
- [ ] Cut over: replace `eval_inner` calls with `eval_canon`
- [ ] Delete `eval_inner` and all `ExprKind` dispatch from `ori_eval`

**Migration scope for eval:** The main dispatch (`interpreter/mod.rs`) and exec modules (`exec/expr.rs`, `exec/call.rs`, `exec/control.rs`, `exec/pattern.rs`) need to be migrated. Method dispatch, environment, value types, and error handling remain unchanged.

---

## 07.2 LLVM/ARC Migration

Update `ori_arc` to lower from `CanExpr` instead of `ExprKind`.

- [ ] Update `ori_arc/src/lower/` to consume `CanArena` + `CanExpr`
  - [ ] All `CanExpr` variants handled exhaustively
  - [ ] `CanExpr::Constant(id)` → emit LLVM constant directly
  - [ ] `CanExpr::Match { decision_tree, .. }` → read pre-compiled tree from `DecisionTreePool`, emit ARC IR blocks (existing `emit_decision_tree()` from LLVM V2 Section 10.4)
  - [ ] No sugar handling (type-level guarantee)
- [ ] Update `ori_llvm/src/codegen/expr_lowerer.rs` if it directly accesses `ExprKind`
  - [ ] Should only access `CanExpr` through `ori_arc`'s ARC IR
- [ ] Wire ARC IR `Invoke` terminators to LLVM `invoke` instructions (deferred from LLVM V2 Phase 3F)
  - [ ] Translate `ArcTerminator::Invoke { dst, ty, func, args, normal, unwind }` → LLVM `invoke` (IrBuilder methods already exist: `invoke`, `landingpad`, `resume`, `set_personality`)
  - [ ] Translate `ArcTerminator::Resume` → LLVM `landingpad` (cleanup) + `resume`
  - [ ] Set `__gxx_personality_v0` on functions containing `invoke`
  - [ ] ARC IR already emits Invoke for user-defined calls, Apply for runtime/intrinsics (`ori_*`, `__*`)
  - [ ] ARC IR RC insertion already generates `RcDec` cleanup in unwind blocks
  - [ ] ARC IR liveness already scopes Invoke `dst` to normal successor only (not unwind)
  - **Context:** All ARC IR infrastructure was built in LLVM V2 Phase 3A-3E. Only the LLVM codegen translation was deferred because the old AST-based codegen doesn't consume ARC IR. Once this migration switches codegen to ARC IR, the translation is mechanical.
- [ ] Wire general cross-block RC elimination (deferred from LLVM V2 Phase 2B)
  - [ ] Multi-predecessor dataflow: forward/backward propagation of Inc/Dec across block boundaries
  - [ ] Simple edge-pair elimination already exists (`rc_elim::eliminate_cross_block_pairs`)
  - [ ] Profile first — the edge-pair approach may be sufficient; only add if profiling shows need
- [ ] Validate: run `./llvm-test.sh`, compare pass/fail with pre-migration baseline
- [ ] AOT end-to-end panic cleanup verification
  - [ ] AOT binary triggers panic with live RC'd variables → no ASAN leak
  - [ ] Nested panic (panic in destructor) → all frames clean up correctly
  - [ ] AOT + lldb: verify variables show correct debug types at unwind points
- [ ] Delete all `ExprKind` dispatch from `ori_arc` and `ori_llvm`

---

## 07.3 Dead Code Removal

After both backends are migrated, delete all dead `ExprKind` handling.

- [ ] Delete `ExprKind` match arms from `ori_eval` (the old `eval_inner` dispatch)
- [ ] Delete `ExprKind` match arms from `ori_arc` (the old AST → ARC IR lowering)
- [ ] Delete spread handling utilities that were only used by backends
- [ ] Delete named-argument reordering that was duplicated in backends
- [ ] Delete template literal evaluation that was duplicated in backends
- [ ] Verify: no backend crate imports `ExprKind` for dispatch purposes
  - [ ] Backends may still import `ExprKind` for diagnostics or error messages — that's fine
  - [ ] The key invariant: no `match expr.kind { ExprKind::... }` in backend evaluation/codegen paths

---

## 07.4 Sync Verification

Verify that both backends produce identical behavior after migration.

- [ ] Cross-backend spec test comparison:
  - [ ] Run interpreter: `cargo st tests/spec/`
  - [ ] Run LLVM: `./llvm-test.sh`
  - [ ] Compare: no new divergences (same tests pass/fail as before migration)
- [ ] Targeted verification for desugared features:
  - [ ] Named arguments: all-named, mixed positional+named
  - [ ] Template literals: int/float/bool interpolation
  - [ ] Spread operators: list/map/struct with overlapping keys/fields
  - [ ] Decision trees: guards, or-patterns, nested patterns
  - [ ] Constant folding: `1 + 2`, `if true`, dead branches

---

## 07.5 Completion Checklist

- [ ] `ori_eval` dispatches exclusively on `CanExpr` (no `ExprKind` in eval path)
- [ ] `ori_arc` lowers exclusively from `CanExpr` (no `ExprKind` in codegen path)
- [ ] Both backends handle `CanExpr::Constant` and `CanExpr::Match { decision_tree }` correctly
- [ ] All dead `ExprKind` dispatch code deleted from both backends
- [ ] ARC IR `Invoke` terminators produce LLVM `invoke` + `landingpad` + `resume`
- [ ] AOT panic with live RC'd variables produces no ASAN leaks
- [ ] `./test-all.sh` passes
- [ ] `./llvm-test.sh` passes
- [ ] No new cross-backend divergences
- [ ] Net lines: more deleted from backends than added in `ori_canon`

**Exit Criteria:** Neither backend contains `ExprKind` dispatch for evaluation/codegen. Both consume `CanExpr` exclusively. Sugar handling, pattern compilation, and constant folding exist in exactly one place (`ori_canon`). New language features added to `ori_canon` are automatically available to both backends. Panic cleanup (invoke/landingpad) is fully wired end-to-end.
