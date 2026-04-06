---
section: "06"
title: "LCFail Resolution — BUG-04-030/031/032/033"
status: in-progress
reviewed: true
goal: "Reduce LLVM spec test LCFails from 2656 toward zero by fixing all known codegen root causes"
inspired_by:
  - "Rust rustc monomorphize collector — closures always concrete by codegen"
  - "Lean 4 LCNF ToMono — type erasure ensures no unresolved vars at codegen"
  - "Swift SIL ARC verification — validates RC balance before codegen"
depends_on: ["04B"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Missing JIT Runtime Functions (Root Cause D)"
    status: complete
  - id: "06.2"
    title: "Generalized Var Resolution (Root Cause A)"
    status: in-progress  # multi-inst fix done; 2 deferred items (var_reprs, LCFail count)
  - id: "06.3"
    title: "ARC IR Index Bounds Safety (Root Cause B)"
    status: not-started
  - id: "06.4"
    title: "Polymorphic Type Selection Fix (Root Cause E)"
    status: not-started
  - id: "06.5"
    title: "List Concat Calling Convention (Root Cause F)"
    status: not-started
  - id: "06.6"
    title: "Short-Circuit Codegen Fixes (BUG-04-031/032)"
    status: not-started
  - id: "06.7"
    title: "Multi-Clause Function Lowering (BUG-04-033)"
    status: not-started
  - id: "06.8"
    title: "ABI Type Resolution Audit (Root Cause C)"
    status: not-started
  - id: "06.9"
    title: "Verification & LCFail Measurement"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: LCFail Resolution — BUG-04-030/031/032/033

**Status:** In Progress — 06.1 complete. 06.2–06.8 not started.
**Goal:** Systematically fix all known LLVM codegen root causes that produce LCFails (LLVM Compile Failures) in the spec test suite. Current baseline: 2656 LCFails. Target: <500 LCFails (stretch: <100).

**Depends on:** Section 04B (lambda monomorphization foundations)

**Root Causes Addressed:**

| ID | Root Cause | Bug | Est. LCFails | Subsystem |
|----|-----------|-----|-------------|-----------|
| D | Missing JIT runtime functions | BUG-04-030 | 2 | `runtime_decl/` |
| A | Generalized Vars leak to codegen | BUG-04-030 | ~279 | `ori_types`, `ori_arc` |
| B | u32::MAX index out of bounds | BUG-04-030 | ~50+ (cascading from A) | `arc_emitter/` |
| E | Wrong concrete type selection | BUG-04-030 | multi-function files | `lambda_mono/` |
| F | List concat in mono lambda crash | BUG-04-030 | segfault | `list_cow.rs` |
| — | PHINode with Option methods in `&&` | BUG-04-031 | entire file LCFail | `short_circuit.rs` |
| — | Side-effect propagation in `&&`/`||` | BUG-04-032 | wrong output | `short_circuit.rs` |
| — | Multi-clause function PHINode | BUG-04-033 | clause-dispatch files | `arc_emitter/` |
| C | StructValue vs IntValue ABI mismatch | BUG-04-030 | 4+ files | `abi/`, `arc_emitter/` |

**Implementation Order:** D → A → B → E → F → 031/032 → 033 → C → Verify

---

## 06.1 Missing JIT Runtime Functions (Root Cause D)

**Complexity:** Trivial | **Impact:** 7 LCFails fixed (audit found 7 missing, not 2)

Full audit of `runtime_fn("...")` calls vs RT_FUNCTIONS table found 7 undeclared functions. All added to RT_FUNCTIONS + JIT symbol lookup + module re-exports.

- [x] Add `ori_iter_flatten` entry to `runtime_functions.rs` with correct signature and `jit_allowed: true` (2026-04-04)
- [x] Add `ori_iter_join` entry to `runtime_functions.rs` with correct signature and `jit_allowed: true` (2026-04-04)
- [x] Add `ori_iter_cycle` entry (adapter: `(Ptr, I64) -> Ptr`) — discovered during audit (2026-04-04)
- [x] Add `ori_iter_rev` entry (adapter: `(Ptr, I64) -> Ptr`) — discovered during audit (2026-04-04)
- [x] Add `ori_iter_last` entry (consumer: `(Ptr, I64, Ptr) -> void`) — discovered during audit (2026-04-04)
- [x] Add `ori_iter_rfind` entry (consumer: `(Ptr, Ptr, Ptr, I64, Ptr) -> void`) — discovered during audit (2026-04-04)
- [x] Add `ori_iter_rfold` entry (consumer: `(Ptr, Ptr, Ptr, Ptr, I64, I64, Ptr) -> void`) — discovered during audit (2026-04-04)
- [x] Add JIT symbol mappings in `evaluator/runtime_mappings.rs` for all 7 functions (2026-04-04)
- [x] Add module re-exports in `ori_rt/src/iterator/mod.rs` for all 7 functions (2026-04-04)
- [x] Verify: `cargo test -p ori_llvm -- jit_symbol` passes (both enforcement tests green) (2026-04-04)
- [x] Verify: `cargo test -p ori_llvm --test aot` passes (2098 passed, 0 failed) (2026-04-04)
- [x] Verify: `./test-all.sh` — 14,760 passed, 0 failed. LLVM backend spec tests crash on pre-existing Root Cause B (u32::MAX index, §06.3) (2026-04-04)

---

## 06.2 Generalized Var Resolution (Root Cause A)

**Complexity:** Moderate-Complex | **Impact:** ~279 LCFails (foundational — fixing this may cascade-reduce Root Cause B)

Type checker stores Unbound Vars that get `VarState::Generalized` during let-polymorphism. `Pool::resolve_fully()` (`ori_types/src/pool/accessors.rs:428-431`) doesn't handle `Generalized`, so these vars leak unresolved to ARC lowering and codegen.

### Investigation & Fix

- [x] Write failing test matrix BEFORE implementation (2026-04-05): `tests/spec/inference/generalized_var_resolution.ori` — 6 tests covering polymorphic lambda patterns (list indexing, Option/List wrapping, identity with collections, len, const with collections). All pass interpreter, all LCFail through LLVM.
- [x] Read and trace `VarState::Generalized` lifecycle (2026-04-05): Traced through generalization.rs → pool/accessors.rs → monomorphization.rs → lambda.rs. Root cause: `resolve_fully()` returns Generalized vars unchanged; for lambdas in non-generic functions, no `MonoInstance`/`body_type_map` exists. The LLVM lambda mono pipeline's `is_polymorphic_lambda`, `build_bound_var_map`, and `find_all_instantiation_types` all missed Generalized vars in container types.
- [x] Implement fix — LLVM lambda mono pipeline (2026-04-05): Extended lambda mono to handle Generalized vars via four changes:
  1. `is_polymorphic_lambda`: added `contains_var(pool, p.ty)` for params and `contains_var` for return type — detects nested vars in containers like `List<Var>`
  2. `build_bound_var_map`: added `map_types_structural` for container params when `contains_var` (parallel walk of schema+concrete types)
  3. New `apply_concrete_param_types`: directly substitutes container params from concrete function type's param Idx values (avoids need for mutable pool)
  4. New `find_concrete_types_from_calls` + `apply_call_site_types`: extracts concrete types from `ApplyIndirect` call sites by following `PartialApply → Let copy → ApplyIndirect` chain — handles let-polymorphic lambdas where type narrowing happens at call sites
- [x] Verify: `timeout 150 ./test-all.sh` — 14,809 passed, 0 failed, 0 regressions from single-inst fix (2026-04-05)

### Multi-Instantiation Fix (lambdas used at 2+ concrete types)

**Problem**: Let-polymorphic lambdas called at 2+ types (e.g., `let head = xs -> xs[0]; head([1,2]); head(["a","b"])`) produce ARC IR where Let copies of the PartialApply result have **concrete params but Scheme return types** (e.g., `([int]) -> forall t16`). `find_all_instantiation_types` rejects these because `is_concrete_function` requires ALL types (including return) to be concrete. Additionally, cloning the lambda requires rewriting the **parent function's ARC IR** — specifically `var_types`, `var_reprs`, and RC ops — to reflect each clone's concrete return type.

**Architecture**: Option A (modify parent var_types + update/remove RC ops). The parent function stays as a single IR object with consistent type information. After cloning, we walk the parent's IR to fix up types and RC operations. This matches the existing `rewrite_parent_for_multi_inst` pattern but extends it to handle Scheme return types.

**Prior art**: Rust `rustc_monomorphize` creates per-instance copies with fully-concrete types; Lean 4 `ToMono` erases types before codegen. Ori's approach is closer to Rust — concrete clones with parent IR fixup.

#### Phase A: Detection — relax `find_all_instantiation_types`

- [x] Add `has_concrete_params(pool, resolved) -> bool` to `type_predicates.rs` — checks that a Function type's params are all concrete, return type may be anything. (2026-04-05)
- [x] In `find_all_instantiation_types`: accept Let copies matching `is_concrete_function(pool, resolved) || has_concrete_params(pool, resolved)`. Dedup key uses params only for the `has_concrete_params` branch. (2026-04-05)
- [x] Write failing test BEFORE implementation: verified baseline 6 LCFails in `generalized_var_resolution.ori` and simple multi-inst test through `--backend=llvm`. (2026-04-05)

#### Phase B: Clone resolution — concrete return types from call sites

- [x] In `clone_multi_inst_lambda`: resolve concrete return type from call site when `pool.function_return(concrete_fn_ty)` is Scheme/Var. Implemented `find_call_site_return_type` + `resolve_call_result_type` that follows: Let copy → ApplyIndirect/InvokeIndirect → result var → downstream narrowing Let. (2026-04-05)
- [x] Apply the resolved concrete return type via `resolve_lambda_return_types(&mut clone, schema_ret, concrete_ret)` — updates clone's `return_type`, matching `var_types`, and `Construct` instruction types. (2026-04-05)
- [x] Apply `apply_concrete_param_types` for container param types with nested vars. (2026-04-05)
- [x] Run `fallback_bound_vars_to_int` as final safety net. (2026-04-05)

#### Phase C: Parent IR fixup — var_types, instruction ty, and matching

- [x] `fixup_call_result_types`: resolve concrete return types for `ApplyIndirect`/`InvokeIndirect` result vars via downstream narrowing Let copies. Updates both `parent.var_types` and instruction `ty` fields. (2026-04-05)
- [x] `rewrite_parent_for_multi_inst`: accept `has_concrete_params` in addition to `is_concrete_function` for Let copy matching. (2026-04-05)
- [x] `find_matching_instantiation`: params-only fallback matching for Scheme return types. (2026-04-05)
- [x] Fixed mangling issue: `$` in lambda names was hex-encoded by the mangler (`$0` → `$240`). Changed separator from `$` to `__mono` (e.g., `lambda__mono0`, `lambda__mono1`). (2026-04-05)
- [ ] Recompute `parent.var_reprs` from concrete types — RC strategy may need updating for result vars whose classification changed from generic to Scalar/DefiniteRef. Not yet needed for passing tests but could cause RC imbalance in edge cases.
- [ ] **Debug validation**: `debug_assert!` verifying var_types/var_reprs consistency for RC ops — deferred to when var_reprs fixup is implemented.

#### Phase D: Verification

- [x] `timeout 150 ./test-all.sh` green — 14,809 passed, 0 failures, 0 regressions (LLVM spec crash is pre-existing BUG-04-030 Root Cause B) (2026-04-05)
- [x] Debug AND release builds pass (`cargo b --release`) (2026-04-05)
- [x] Multi-inst test passes both interpreter and LLVM: `let head = xs -> xs[0]; head([1,2,3]); head(["a","b","c"])` — dual-exec parity verified (2026-04-05)
- [ ] Multi-inst tests in `tests/spec/inference/generalized_var_resolution.ori` pass through LLVM — 6 still LCFail from pre-existing Root Causes (unresolved `len` dispatch, `assert_eq` invoke). Multi-inst detection and cloning works but downstream codegen issues block these specific tests.
- [x] Existing `test_multi_inst_tuple_lambda` and `test_multi_inst_map_lambda` AOT tests still pass — all 5 multi-inst AOT tests pass (2026-04-05)
- [x] `ORI_CHECK_LEAKS=1` clean on multi-inst test programs (2026-04-05)
- [x] `./clippy-all.sh` passes (2026-04-05)
- [ ] Count LCFails after fix: LLVM spec tests crash on Root Cause B (§06.3), preventing accurate count. Multi-inst patterns compile correctly when not blocked by other root causes.

### Matrix Testing

- Types: int, float, str, bool, [int], Option<int>, (int, str), {str: int}
- Patterns: simple let-poly, nested let-poly, let-poly in lambda capture, let-poly across function boundaries, **multi-inst (2+ types for same lambda)**
- Semantic pin: `let id = x -> x; id(42) + id("hello".len())` — must produce correct results via LLVM
- Negative pin: multi-inst lambda with wrong types should produce type error (not codegen crash)

### Matrix Testing

- Types: int, float, str, bool, [int], Option<int>, (int, str), {str: int}
- Patterns: simple let-poly, nested let-poly, let-poly in lambda capture, let-poly across function boundaries
- Semantic pin: `let id = x -> x; id(42) + id("hello".len())` — must produce correct results via LLVM

---

## 06.3 ARC IR Index Bounds Safety (Root Cause B)

**Complexity:** Moderate | **Impact:** ~50+ LCFails (many may be resolved by 06.2)

Pattern: `index out of bounds: the len is N but the index is 4294967295`. Missing var/block definitions in ARC lowering + unsafe direct array indexing in codegen at `emitter_utils.rs:219`.

### Investigation & Fix

- [ ] After 06.2 is complete, re-count u32::MAX errors — if significantly reduced, remaining cases are the real Root Cause B
- [ ] Add bounds checks to all `.index()` uses in `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs`:
  - Line 219: `block()` — direct array index, no bounds check → add `.get()` with error
  - Review all 35+ `.index()` uses in arc_emitter for similar patterns
- [ ] Add sentinel constants to `ori_arc/src/ir/mod.rs` for `ArcVarId` and `ArcBlockId`:
  - `ArcVarId::INVALID` = u32::MAX sentinel
  - Guard against sentinel in `def_var()`, `get_var()`, `block()`
- [ ] Trace remaining u32::MAX errors to their lowering source:
  - Which expressions/patterns cause lowering to skip `fresh_var()` calls?
  - Add `debug_assert!(var.raw() != u32::MAX)` at key lowering points
- [ ] Verify: `timeout 150 ./test-all.sh` green, LCFail count further reduced

---

## 06.4 Polymorphic Type Selection Fix (Root Cause E)

**Complexity:** Moderate | **Impact:** Multi-function file LCFails (files with 2+ polymorphic lambdas)

`find_concrete_copy_of()` (`lambda_mono/type_resolve.rs:359-383`) returns the FIRST concrete Function type without checking arity or parameter types. In multi-function files with different polymorphic instantiations, the wrong type is selected.

### Fix

- [ ] Write failing test: multi-function file with 2+ polymorphic lambdas at different types, compiled via `--backend=llvm`
- [ ] Fix `find_concrete_copy_of()` in `type_resolve.rs`:
  - Accept the target lambda's expected arity as parameter
  - Before returning a match, verify: `pool.function_params(resolved).len() == expected_arity`
  - If arity doesn't match, continue searching
  - Add type structure matching if arity alone is insufficient
- [ ] Follow the pattern from `find_partial_apply_concrete_type()` (same file, lines 62-119) which already does multi-step search with fallbacks
- [ ] Verify: multi-function files that previously LCFailed now compile
- [ ] `timeout 150 ./test-all.sh` green

---

## 06.5 List Concat Calling Convention (Root Cause F)

**Complexity:** Moderate | **Impact:** Segfault fix for `app([1,2,3])([4,5,6])`

Monomorphized lambda with list `+` dispatch produces invalid calling convention for `ori_list_concat_cow`. Depends on 06.4 (correct type selection).

### Fix

- [ ] Write failing test: `let $app = a -> b -> a + b; app([1, 2, 3])([4, 5, 6])` via `--backend=llvm` (both debug and release)
- [ ] After 06.4, verify if the test passes — the segfault may be caused by wrong type selection (E), not by F itself
- [ ] If still failing: audit `emit_list_concat_cow()` in `list_cow.rs:235-274`:
  - Check `elem_ty` is correctly resolved for the monomorphized lambda
  - Check sret calling convention matches between caller and callee
  - Check argument count and types match `ori_list_concat_cow` runtime function signature
- [ ] Verify: no SIGSEGV in debug or release
- [ ] `ORI_CHECK_LEAKS=1` clean on list concat lambda tests

---

## 06.6 Short-Circuit Codegen Fixes (BUG-04-031/032)

**Complexity:** Moderate | **Impact:** Unblocks `operators_logical.ori` (39 tests) and dual-exec parity

Two related bugs in short-circuit `&&`/`||` LLVM codegen:
- **BUG-04-031**: PHINode with Option method calls — `is_some(opt:) && opt.unwrap_or(default: 0) > 0` causes "PHINode should have one entry for each predecessor"
- **BUG-04-032**: Side-effect propagation — `{order = order + [1]; true} && {order = order + [2]; true}` produces `[1]` instead of `[1, 2]`

### Investigation

- [ ] Read `compiler/ori_arc/src/lower/expr/short_circuit.rs` — understand the lowering of `&&`/`||` to branch-based ARC IR
- [ ] Read `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` — understand how branches emit PHI nodes
- [ ] Trace the PHINode error: which function's CFG has missing predecessor entries?
- [ ] Trace the side-effect bug: where is the variable store/load across basic blocks lost?

### BUG-04-031 Fix (PHINode)

- [ ] Write failing test: `let opt = Some(42); is_some(opt:) && opt.unwrap_or(default: 0) > 0` via `--backend=llvm`
- [ ] Fix: ensure all basic blocks that branch into the merge point have corresponding PHI entries
  - The issue is likely that method calls on `Option` in the RHS branch create additional basic blocks (for invoke/landingpad) that aren't accounted for in the merge PHI
- [ ] Verify: `operators_logical.ori` compiles via `--backend=llvm` (no longer all-LCFail)

### BUG-04-032 Fix (Side-effects)

- [ ] Write failing test: block expressions with variable mutations in `&&`/`||` branches
- [ ] Fix: ensure variable stores in the evaluated branch are visible after the merge point
  - The issue is likely that short-circuit codegen creates a separate scope for the RHS block, and mutations don't propagate back to the outer scope
  - May need to use the same variable slots across both branches (not copies)
- [ ] Verify: `assert_eq(actual: order, expected: [1, 2])` passes via `--backend=llvm`
- [ ] Dual-execution parity: `dual-exec-verify.sh tests/spec/expressions/operators_logical.ori` — 0 mismatches

### Matrix Testing

- Short-circuit with: panic (already works), constants (already works), Option methods (031), block expressions (032), nested `&&`/`||`, closures in branches
- Semantic pin: `operators_logical.ori` passes all 39 tests via `--backend=llvm`

---

## 06.7 Multi-Clause Function Lowering (BUG-04-033)

**Complexity:** Moderate | **Impact:** Files with multi-clause functions (Ackermann pattern)

Two errors in multi-clause function LLVM emission:
1. `build_struct called with non-struct LLVM type (i64)` — clause dispatch treats int return as struct
2. PHINode predecessor mismatch from clause branches

### Fix

- [ ] Write failing test: Ackermann function with 3 clauses + literal patterns, compiled via `--backend=llvm`
- [ ] Read `compiler/ori_canon/` — verify multi-clause lowering to match tree is correct (it is per BUG-04-033 description)
- [ ] Trace the LLVM emission path for the lowered match tree:
  - Where does `build_struct` get called with an i64 type?
  - Where do the clause branches fail to generate PHI entries?
- [ ] Fix the emission: ensure clause dispatch correctly handles scalar return types (don't wrap i64 in struct)
- [ ] Fix PHI generation: ensure all clause branches have entries in the join PHI node
- [ ] Verify: Ackermann and similar multi-clause functions compile and run correctly via `--backend=llvm`
- [ ] Debug AND release produce identical results

---

## 06.8 ABI Type Resolution Audit (Root Cause C)

**Complexity:** Complex | **Impact:** 4+ files with StructValue/IntValue confusion

Systemic issue: LLVM emitter produces struct value where int value is expected (or vice versa). Requires auditing the ABI computation and type resolution pipeline.

### Audit

- [ ] Read `compiler/ori_llvm/src/codegen/abi/mod.rs` — understand `ParamPassing::Direct` vs `ParamPassing::Indirect` classification
- [ ] Identify all call sites where `build_struct` is called and verify the LLVM type is actually a struct type
- [ ] Identify all call sites where `build_int_to_ptr` or scalar loads are performed and verify the source is actually scalar
- [ ] Add validation at function declaration time: verify LLVM type matches ABI classification
  - `ParamPassing::Direct` → must be scalar or small struct (<= 16 bytes)
  - `ParamPassing::Indirect` → must be struct > 16 bytes
- [ ] Fix any misclassifications found

### Testing

- [ ] Write AOT tests for types that trigger ABI edge cases: empty struct, single-field struct, 16-byte struct (boundary), 17-byte struct, nested structs
- [ ] Verify: all ABI-sensitive tests pass in debug AND release
- [ ] `timeout 150 ./test-all.sh` green

---

## 06.9 Verification & LCFail Measurement

- [ ] Run `ori test --backend=llvm tests/` — record final LCFail count
- [ ] Compare against baseline (2656): calculate reduction percentage
- [ ] Run `timeout 150 ./test-all.sh` — full suite green
- [ ] Run `./clippy-all.sh` — clean
- [ ] `cargo build --release` — succeeds
- [ ] `diagnostics/dual-exec-verify.sh tests/spec/expressions/operators_logical.ori` — verified (unblocked by 06.6)
- [ ] `diagnostics/dual-exec-verify.sh tests/spec/patterns/catch.ori` — verified (unblocked by 06.2)
- [ ] Update BUG-04-030 in bug tracker with resolution status
- [ ] Update BUG-04-031, BUG-04-032, BUG-04-033 in bug tracker

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [x] Root Cause D fixed: 7 missing iterator functions declared in RT_FUNCTIONS + JIT mappings + re-exports (2026-04-04)
- [ ] Root Cause A fixed: Generalized vars no longer leak to codegen
- [ ] Root Cause B fixed: no u32::MAX index panics in ARC IR emission
- [ ] Root Cause E fixed: `find_concrete_copy_of()` validates arity before returning
- [ ] Root Cause F fixed: list concat in monomorphized lambda no longer crashes
- [ ] BUG-04-031 fixed: PHINode with Option method calls in short-circuit
- [ ] BUG-04-032 fixed: side-effect propagation in short-circuit blocks
- [ ] BUG-04-033 fixed: multi-clause function lowering PHINode
- [ ] Root Cause C fixed: ABI type classification validated
- [ ] LCFail count reduced from 2656 to target (<500, stretch <100)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Debug AND release builds pass
- [ ] `ORI_CHECK_LEAKS=1` clean on all new test programs
- [ ] Bug tracker entries updated (BUG-04-030, 031, 032, 033)
- [ ] `/tpr-review` passed — independent Codex review clean
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** LCFail count < 500. All 4 bug tracker entries resolved or significantly reduced. `operators_logical.ori` passes all 39 tests via `--backend=llvm`. No SIGSEGV in any test. Full test suite green.
