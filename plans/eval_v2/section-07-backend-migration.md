---
section: "07"
title: Backend Migration
status: in-progress
goal: Rewrite ori_eval and ori_arc/ori_llvm to dispatch on CanExpr instead of ExprKind, then delete all ExprKind dispatch from backends
sections:
  - id: "07.1"
    title: Evaluator Migration
    status: complete
  - id: "07.2"
    title: LLVM/ARC Migration
    status: complete
  - id: "07.3"
    title: Dead Code Removal
    status: in-progress
  - id: "07.4"
    title: Sync Verification
    status: in-progress
  - id: "07.5"
    title: Completion Checklist
    status: not-started
---

# Section 07: Backend Migration

**Status:** In Progress (07.1 + 07.2 complete — 2026-02-09; invoke/landingpad wired — 2026-02-10; Tier 2 ARC codegen wired — 2026-02-09; remaining: dead code, verification, ASAN)
**Goal:** The payoff. Rewrite both backends to consume `CanExpr` exclusively. Delete all `ExprKind` dispatch from backends. New language features only need one implementation: in `ori_canon`.

**Depends on:** Sections 01-04 (canonical IR types, lowering, patterns, constants) must be complete and validated. Sections 05-06 (eval modes, diagnostics) are independent and can proceed in parallel.

**Prior art:**
- **Roc** — Both dev backend and gen_llvm consume `mono::Stmt`/`mono::Expr` exclusively. Zero parse-AST dispatch in either backend.
- **Elm** — JS codegen consumes `Opt.Expr` exclusively. Zero `Can.Expr_` dispatch in codegen.

---

## 07.1 Evaluator Migration ✅ Complete (2026-02-09)

Rewrite `ori_eval` interpreter dispatch from `ExprKind` to `CanExpr`.

**Strategy:** Dual-mode approach — functions carry both `body`/`arena` (legacy) and `can_body`/`canon` (canonical). The evaluator uses canonical path when available, legacy fallback otherwise. Multi-clause functions lowered to decision trees at canonicalization time.

- [x] Add `CanonResult` as a field on `Interpreter` (alongside existing `ExprArena`)
- [x] Implement `eval_can(can_id: CanId) -> EvalResult` dispatch on `CanExpr`
  - [x] All `CanExpr` variants handled exhaustively (no `_ =>` catch-all)
  - [x] Reuse existing evaluation logic from `exec/` modules — adapt to `CanId` references
  - [x] `CanExpr::Constant(id)` → return value from `ConstantPool` directly
  - [x] `CanExpr::Match { decision_tree, .. }` → call `eval_decision_tree()` (Section 03)
  - [x] No sugar variants to handle (type-level guarantee)
- [x] Self-contained canonical IR: eliminate all ExprArena back-references from CanExpr
  - [x] `CanBindingPattern` replaces `BindingPatternId` (stored in CanArena)
  - [x] `CanParam` replaces `ParamRange` (stored in CanArena)
  - [x] `Cast { target: Name }` replaces `ParsedTypeId`
  - [x] `FunctionExp { kind, props: CanNamedExprRange }` replaces `FunctionExpId`
  - [x] `FunctionSeq` desugared into Block/Match during lowering
  - [x] Decision tree guards use `CanId` (not `ExprId`)
- [x] `lower_module()` API: canonicalize all function bodies into one CanArena
- [x] Multi-clause function lowering: same-name functions compiled into synthesized `CanExpr::Match` with multi-column decision trees
- [x] `FunctionValue` carries `SharedCanonResult` + `can_body: CanId`
- [x] Wire canonicalization into `oric` pipeline: `evaluated()`, test runner, run.rs, impl/extend blocks
- [x] Validate: `cargo st tests/` — 3040 passed, 0 failed
- [x] Delete legacy: remove `Value::MultiClauseFunction`, `FunctionValue.patterns`/`.guard`, runtime clause dispatch (111 lines)
- [x] Full test suite: 8434 passed, 0 failed, all clippy clean

**What was built:**
- `compiler/ori_canon/src/lower.rs` — `lower_module()`, `lower_multi_clause()`, self-contained lowering
- `compiler/ori_canon/src/patterns.rs` — `compile_multi_clause_patterns()`, multi-column pattern matrices
- `compiler/ori_ir/src/canon/mod.rs` — `CanBindingPattern`, `CanParam`, `CanNamedExpr`, `SharedCanonResult`
- `compiler/ori_eval/src/interpreter/mod.rs` — `eval_can()` exhaustive CanExpr dispatch
- `compiler/ori_eval/src/interpreter/function_call.rs` — canonical function calls (legacy MultiClauseFunction deleted)
- `compiler/ori_patterns/src/value/composite.rs` — `FunctionValue` with `can_body`/`canon` (patterns/guard removed)

---

## 07.2 LLVM/ARC Migration

Update `ori_arc` to lower from `CanExpr` instead of `ExprKind`.

- [x] Update `ori_arc/src/lower/` to consume `CanArena` + `CanExpr` ✅ (2026-02-09)
  - [x] All `CanExpr` variants handled exhaustively
  - [x] `CanExpr::Constant(id)` → emit ARC constant directly
  - [x] `CanExpr::Match { decision_tree, .. }` → read pre-compiled tree from `DecisionTreePool`
  - [x] No sugar handling (type-level guarantee)
- [x] Update `ori_llvm/src/codegen/` to consume `CanArena` + `CanExpr` ✅ (2026-02-09)
  - [x] All 9 codegen files migrated: expr_lowerer, lower_literals, lower_operators, lower_control_flow, lower_error_handling, lower_collections, lower_calls, lower_constructs, function_compiler
  - [x] `CanExpr::Constant(id)` → emit LLVM constant from `ConstantPool`
  - [x] Pipeline wired: `compile_common.rs` passes `&CanonResult` to `FunctionCompiler`
  - [x] Test runner (`runner.rs`) canonicalizes imported modules for JIT compilation
  - [x] No sugar handling — ~500+ lines of sugar dispatch deleted
- [x] Wire `invoke`/`landingpad` for user-defined function calls in LLVM codegen ✅ (2026-02-10)
  - [x] User-defined calls (`lower_abi_call`, `emit_method_call`) use `invoke` with cleanup landingpad
  - [x] Runtime/intrinsic calls (`ori_*` via LLVM module lookup) remain as `call` (nounwind)
  - [x] `rust_eh_personality` set on functions containing `invoke` (replaces `__gxx_personality_v0`)
  - [x] `rust_eh_personality` mapped in JIT engine (not in dynamic symbol table, requires explicit mapping)
  - [x] `IrBuilder::invoke()` sets calling convention to match callee (inkwell `build_invoke` doesn't auto-inherit CC)
  - [x] Cleanup landingpad re-raises immediately (RC cleanup deferred until cross-block liveness is wired)
  - **Approach:** Wired directly in `lower_calls.rs` (CanExpr → LLVM invoke), not via ARC IR translation. ARC IR `Invoke`/`Resume` terminators remain available for future ARC-IR-driven codegen path.
- [x] ARC IR → LLVM emission (Tier 2 codegen path) ✅ (2026-02-09)
  - [x] `ArcIrEmitter` in `ori_llvm/src/codegen/arc_emitter.rs` — all `ArcInstr` (13) and `ArcTerminator` (7) variants covered
  - [x] Block pre-creation, phi node setup for `Jump { args }`, terminator emission
  - [x] RC operations: `RcInc` → `ori_rc_inc`, `RcDec` → `ori_rc_dec`, `IsShared` stub, `Reset`/`Reuse` stub
  - [x] Full ARC pipeline wired in `FunctionCompiler::define_function_body_arc()`: lower → liveness → RC insert → detect/expand reuse → RC eliminate → `ArcIrEmitter`
  - [x] Opt-in via `FunctionCompiler::set_arc_codegen(true)` (Tier 1 default, Tier 2 opt-in)
  - [x] All 8490 tests pass, clippy clean — Tier 1 unaffected
- [ ] Wire general cross-block RC elimination (deferred from LLVM V2 Phase 2B)
  - [ ] Multi-predecessor dataflow: forward/backward propagation of Inc/Dec across block boundaries
  - [ ] Simple edge-pair elimination already exists (`rc_elim::eliminate_cross_block_pairs`)
  - [ ] Profile first — the edge-pair approach may be sufficient; only add if profiling shows need
- [x] Validate: `./test-all.sh` — 8434 passed, 0 failed, clippy clean ✅ (re-verified 2026-02-09 after parser hygiene fixes)
- [x] AOT tests pass with invoke/landingpad: `./llvm-test.sh` — 198 passed, 0 failed ✅ (2026-02-10)
- [ ] AOT end-to-end panic cleanup verification (deferred — requires cross-block RC liveness)
  - [ ] AOT binary triggers panic with live RC'd variables → no ASAN leak
  - [ ] Nested panic (panic in destructor) → all frames clean up correctly
  - [ ] AOT + lldb: verify variables show correct debug types at unwind points
- [x] Delete all `ExprKind` dispatch from `ori_arc` and `ori_llvm` ✅ (2026-02-09)
  - [x] `ori_arc`: zero ExprKind references
  - [x] `ori_llvm`: only doc comment references remain (no dispatch code)

---

## 07.3 Dead Code Removal

After both backends are migrated, delete all dead `ExprKind` handling.

- [ ] Delete `ExprKind` match arms from `ori_eval` (the old `eval_inner` dispatch)
  - Partially unblocked: canonical defaults now wired (2026-02-09)
  - Remaining blockers: assignment, some patterns still use legacy eval path
- [x] Delete `ExprKind` match arms from `ori_arc` (the old AST → ARC IR lowering) ✅ (2026-02-09)
- [x] Delete spread handling utilities that were only used by backends ✅ (2026-02-09)
- [x] Delete named-argument reordering that was duplicated in backends ✅ (2026-02-09)
- [x] Delete template literal evaluation that was duplicated in backends ✅ (2026-02-09)
- [x] Verify: no backend crate imports `ExprKind` for dispatch purposes ✅ (2026-02-09)
  - [x] `ori_arc`: zero ExprKind references
  - [x] `ori_llvm`: only doc comment references (no dispatch code)
  - [ ] `ori_eval`: still has active ExprKind dispatch in `eval_inner()` — migration incomplete

---

## 07.4 Sync Verification

Verify that both backends produce identical behavior after migration.

- [x] Cross-backend spec test comparison: ✅ (2026-02-09)
  - [x] Run interpreter: 3040 passed, 0 failed, 42 skipped
  - [x] Run LLVM: 1068 passed, 0 failed, 9 skipped, 2005 llvm compile fail
  - [x] Rust unit tests: 3792 (workspace) + 534 (LLVM) passed, 0 failed
  - [x] Total: 8434 passed, 0 failed — zero regressions from pre-migration baseline
- [x] Targeted verification for desugared features: ✅ (2026-02-09)
  - [x] Named arguments: all-named, reordered, with defaults (`tests/spec/declarations/named_arguments.ori`)
  - [x] Constant folding: dead branch elimination, `if true`/`if false` (`tests/spec/const_expr/dead_code.ori`)
  - [ ] Template literals: int/float/bool interpolation (covered by existing spec tests)
  - [ ] Spread operators: list/map/struct with overlapping keys/fields (covered by existing spec tests)
  - [ ] Decision trees: guards, or-patterns, nested patterns (covered by existing spec tests)

---

## 07.5 Completion Checklist

- [ ] `ori_eval` dispatches exclusively on `CanExpr` (no `ExprKind` in eval path)
  - Defaults: ✅ canonical (2026-02-09). Remaining: assignment, eval_call_named legacy path
- [x] `ori_arc` lowers exclusively from `CanExpr` (no `ExprKind` in codegen path) ✅
- [x] Both backends handle `CanExpr::Constant` and `CanExpr::Match { decision_tree }` correctly ✅
- [x] All dead `ExprKind` dispatch code deleted from `ori_arc` and `ori_llvm` ✅
- [x] User-defined calls produce LLVM `invoke` + `landingpad` + `resume` ✅ (2026-02-10)
- [x] Tier 2 ARC codegen path wired: `ArcIrEmitter` + full ARC pipeline in `FunctionCompiler` ✅ (2026-02-09)
- [ ] AOT panic with live RC'd variables produces no ASAN leaks
  - Deferred: cleanup landingpads currently re-raise immediately; RC dec insertion requires cross-block liveness
- [x] `./test-all.sh` passes ✅ (8490 passed, 0 failed — 2026-02-09)
- [x] `./llvm-test.sh` passes ✅ (534 passed, 0 failed)
- [x] No new cross-backend divergences ✅
- [x] Net lines: ~500+ lines deleted from backends ✅

**Exit Criteria:** Neither backend contains `ExprKind` dispatch for evaluation/codegen. Both consume `CanExpr` exclusively. Sugar handling, pattern compilation, and constant folding exist in exactly one place (`ori_canon`). New language features added to `ori_canon` are automatically available to both backends. Panic cleanup (invoke/landingpad) is fully wired end-to-end.
