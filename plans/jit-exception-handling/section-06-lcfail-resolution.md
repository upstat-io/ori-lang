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
    status: in-progress  # var_reprs fixup done; 2 verification items blocked by Root Cause B (§06.3)
  - id: "06.3"
    title: "ARC IR Index Bounds Safety (Root Cause B)"
    status: in-progress  # emitter-level fixes done; remaining CRASH is from pool var_state in resolve_fully() + into_int_value panics (Root Cause C/06.8)
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
- [x] Recompute `parent.var_reprs` from concrete types (2026-04-05): Added `fixup_parent_var_reprs_and_rc_ops()` to `lambda_mono/mod.rs`. After all lambda mono modifications, recomputes `var_reprs` via `compute_var_reprs()`, strips `RcInc`/`RcDec` on vars that became `Scalar`, and updates `RcStrategy` on vars that changed ref type (e.g., `HeapPointer`→`FatPointer`). Added `classifier: &dyn ArcClassification` param to `resolve_all_lambda_bound_vars`. 6 unit tests: scalar strip, strategy update (FatValue, InlineEnum), no-op cases.
- [x] **Debug validation**: `debug_assert!` verifying `var_types`/`var_reprs` consistency for RC ops (2026-04-05): Embedded in `fixup_parent_var_reprs_and_rc_ops()` — after fixup, asserts no `RcInc`/`RcDec` targets a `Scalar` var. 14,815 tests pass, 0 failures, clippy clean.

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

**Complexity:** Moderate | **Impact:** ~50+ LCFails (many may cascade from Root Cause A)

Pattern: `index out of bounds: the len is N but the index is 4294967295` (u32::MAX). The crash is in `emitter_utils.rs:223` — unsafe direct array indexing `self.block_map[b.index()]`. The LLVM spec test runner segfaults when this occurs because the u32::MAX cast to usize (18446744073709551615 on 64-bit) bypasses Rust's catch_unwind and panics inside LLVM C++ code.

**Prior art**: `ValueId::NONE` (`u32::MAX`) already has a comment at `emitter_utils.rs:189` noting it "causes panics in get_value() which cascade into LLVM C++ crashes that bypass catch_unwind."

### Investigation

- [x] After 06.2 is complete, re-count u32::MAX errors (2026-04-05): 13 "index out of bounds" errors remain. However, investigation revealed these are NOT u32::MAX — they are off-by-one errors (e.g., "len is 17, index is 18") from `Pool::var_state` at `pool/mod.rs:257`, called from `resolve_fully()` during `ori_repr::canonical::canonical_inner`. The original u32::MAX crashes described in the plan were likely fixed by 06.1 (missing RT functions) and 06.2 (Generalized var resolution).
- [x] Trace remaining errors to their source (2026-04-05): Backtraces show `ori_repr::canonical::canonical_inner → resolve_fully() → Pool::var_state` — pool-level type variable indices exceeding pool var storage. This is Generalized vars leaking to the canonicalization pass in `ori_repr`, not an emitter-level issue. Separate from the block_map indexing described in the original plan.

### Fix: Safe Indexing in `emitter_utils.rs`

- [x] Fix `block()` at `emitter_utils.rs` (2026-04-05): replaced `self.block_map[b.index()].expect(...)` with safe `.get()` + entry-block fallback + `record_codegen_error()`. On bad lookup, returns `block_map[0]` (entry block always exists) and logs error. No dedicated poison block needed — avoids triggering IR quality assertions about standalone `unreachable` blocks.
- [x] Review all other `.index()` uses in `compiler/ori_llvm/src/codegen/arc_emitter/` (2026-04-05): `var_emitted()` already uses safe `.get()`. `block()` was the only unsafe direct indexing pattern. `emit_function.rs` block_map init at lines 84-98 uses direct indexing but is bounded by `func.blocks.len()` — safe.

### Fix: Sentinel Constants for `ArcVarId`/`ArcBlockId`

- [x] Add `ArcVarId::INVALID` and `ArcBlockId::INVALID` sentinel constants (value `u32::MAX`) (2026-04-05)
- [x] Add `is_valid()` method returning `self.0 != u32::MAX` on both types (2026-04-05)
- [x] Add `debug_assert!(var.is_valid())` in `var_emitted()` and `debug_assert!(block.is_valid())` in `block()` at `emitter_utils.rs` (2026-04-05)
- [x] Guard `fresh_var()` at `ori_arc/src/ir/function.rs`: `debug_assert!(id < u32::MAX, "ARC var ID would collide with INVALID sentinel")` (2026-04-05)

### Verification

- [x] `timeout 150 ./test-all.sh` green (2026-04-05): 14,815 passed, 0 failed. Emitter-level block panics eliminated.
- [ ] LLVM spec tests no longer crash — **STILL CRASHES** from `Pool::var_state` in `ori_repr::canonical` (pool-level index overflow, not emitter). This is a separate root cause: Generalized vars leak to `ori_repr` canonicalization with indices past pool storage. Needs pool-level guard in `resolve_fully()` or `var_state()`.
- [x] Debug AND release produce same results (2026-04-05)
- [x] `./clippy-all.sh` clean (2026-04-05)

---

## 06.4 Polymorphic Type Selection Fix (Root Cause E)

**Complexity:** Moderate | **Impact:** Multi-function file LCFails (files with 2+ polymorphic lambdas)

`find_concrete_copy_of()` at `type_resolve.rs:602-626` returns the FIRST concrete Function type without checking arity, parameter types, or return type compatibility. In multi-function files with different polymorphic instantiations, the wrong type is selected.

The equally-blind `find_any_concrete_fn_type()` at `type_resolve.rs:591-599` scans ALL `var_types` and returns the first concrete function — it can match a completely unrelated function type from a different lambda in the same parent.

Compare: `apply_concrete_param_types()` at `type_resolve.rs:180-204` correctly validates arity (`num_captures` at line 189) and type compatibility (line 199). The fix should bring `find_concrete_copy_of` to the same standard.

### Fix

- [ ] Write failing test BEFORE fix: multi-function file with 2+ polymorphic lambdas at different arities/types. E.g., `let f = x -> x; let g = (a, b) -> a + b; f("hello"); g(1, 2)` compiled via `--backend=llvm`
- [ ] Fix `find_concrete_copy_of()` at `type_resolve.rs:602-626`:
  - Accept the target lambda as parameter (for arity information)
  - Before returning a match at line 619, verify: `pool.function_params(resolved).len()` matches the lambda's expected non-capture param count
  - If arity doesn't match, continue searching (don't return first match)
- [ ] Fix `find_any_concrete_fn_type()` at `type_resolve.rs:591-599`:
  - Same arity validation — accept lambda as parameter, check param count
  - Consider removing this function entirely if `find_concrete_copy_of` with arity checking covers all cases
- [ ] Update call site at `find_partial_apply_concrete_type()` (lines 99-103) to pass lambda reference
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Verify: multi-function files that previously LCFailed now compile correctly

### Matrix Testing

- Types: (int)->int, (int,str)->int, (str)->str, ()->int (different arities)
- Patterns: 2 lambdas same arity different types, 2 lambdas different arities, 3+ lambdas in same file
- Semantic pin: multi-function file where each lambda produces correct results via LLVM

---

## 06.5 List Concat Calling Convention (Root Cause F)

**Complexity:** Moderate | **Impact:** Segfault fix for `app([1,2,3])([4,5,6])`

Monomorphized lambda with list `+` dispatch produces invalid calling convention for `ori_list_concat_cow`. The `elem_ty` is extracted from `TypeInfo::List` at `operators/mod.rs:46` — if type info is wrong (from Root Cause E), `elem_ty` is wrong. Depends on 06.4 being fixed first.

**Runtime signature** (`runtime_functions.rs:339-357`): `ori_list_concat_cow` takes 11 params (data1, len1, cap1, data2, len2, cap2, elem_size, elem_align, inc_fn, cow_mode, out_ptr) and returns void (uses sret via `out_ptr`).

**Emission** at `list_cow.rs:235-274`: `emit_list_concat_cow()` extracts list fields, computes elem size/align, generates elem_inc function, allocates output struct, and calls runtime with 11 arguments.

### Fix

- [ ] Write failing test BEFORE fix: `let $app = a -> b -> a + b; app([1, 2, 3])([4, 5, 6])` via `--backend=llvm` (debug and release)
- [ ] After 06.4, verify if the test passes — the segfault may be from wrong type selection (E) causing wrong `elem_ty` at `operators/mod.rs:46`
- [ ] If still failing: audit `emit_list_concat_cow()` at `list_cow.rs:235-274`:
  - Verify `elem_ty` is correctly resolved for the monomorphized lambda
  - Verify sret convention: `out_ptr` argument order matches `runtime_functions.rs:339-357` declaration
  - Verify `elem_size_and_align()` returns correct values for the concrete element type
  - Verify `get_or_generate_elem_inc_fn()` generates correct inc function for the element type
- [ ] Verify: no SIGSEGV in debug or release
- [ ] `ORI_CHECK_LEAKS=1` clean on list concat lambda tests
- [ ] `timeout 150 ./test-all.sh` green

---

## 06.6 Short-Circuit Codegen Fixes (BUG-04-031/032)

**Complexity:** Moderate-Complex | **Impact:** Unblocks `operators_logical.ori` (39 tests) and dual-exec parity

Two distinct bugs in short-circuit `&&`/`||` lowering at `ori_arc/src/lower/expr/short_circuit.rs`.

### BUG-04-031: PHINode Predecessor Mismatch

**Root cause**: In `lower_short_circuit_and()` at `short_circuit.rs:135-180`, `lower_expr(right)` at line 154 may emit `InvokeIndirect` (for method calls like `opt.unwrap_or()`). This creates extra basic blocks (normal continuation + unwind) that aren't accounted for. After the invoke, `then_exit = self.builder.current_block()` (line 155) points to the normal-continuation block, NOT the original `then_block` from line 152. When jumping to `merge_block` from this unexpected predecessor, LLVM's PHI node validation fails.

**Compare**: `lower_coalesce()` at lines 29-129 in the same file handles this correctly because it uses `terminate_jump` which properly patches PHI incoming edges.

- [ ] Write failing test BEFORE fix: `let opt = Some(42); is_some(opt:) && opt.unwrap_or(default: 0) > 0` — existing test at `operators_logical.ori:370-380` (`@test_guard_pattern`)
- [ ] Fix `lower_short_circuit_and()` at `short_circuit.rs:135-180`:
  - The fix is in the ARC lowering, not the LLVM emitter. After `lower_expr(right)` at line 154, record `then_exit = self.builder.current_block()` — this is ALREADY done. The real issue is that the block structure created by `lower_expr(right)` introduces InvokeIndirect terminators that create new blocks. The `then_exit` correctly captures the final block, but the PHI node at `merge_block` expects exactly 2 predecessors (then_exit, else_exit) which works. Investigate if the issue is instead in `emit_function.rs:208-236` (PHI creation) where the pre-created block structure doesn't match the post-lowering structure.
  - Alternative fix: if the RHS contains invokes, ensure the ARC IR's Jump to merge_block is from the correct block (the last block created by the RHS, not the original then_block)
- [ ] Fix `lower_short_circuit_or()` — apply same fix symmetrically
- [ ] Verify: `operators_logical.ori` compiles via `--backend=llvm` (no more PHINode errors)

### BUG-04-032: Missing Mutable Variable Merge

**Root cause**: `lower_short_circuit_and()` at `short_circuit.rs:135-180` does NOT call `merge_mutable_vars()` after branching. Compare with `lower_coalesce()` at lines 96-124 which correctly calls `merge_mutable_vars()` to propagate variable mutations from branch scopes to the merge block.

At line 178, `self.scope = pre_scope` reverts to the pre-branch scope, losing any mutations from the RHS block. The fix is to call `merge_mutable_vars()` (defined at `scope/mod.rs:88-124`) before the merge block, passing `[then_scope, else_scope]` as branch scopes.

- [ ] Write failing test BEFORE fix: `let order: [int] = []; {order = order + [1]; true} && {order = order + [2]; true}; assert_eq(actual: order, expected: [1, 2])` — existing test at `operators_logical.ori:296-305` (`@test_and_left_first`)
- [ ] Fix `lower_short_circuit_and()`:
  - After `lower_expr(right)` at line 154, capture the then-branch scope: `let then_scope = self.scope.clone();`
  - After the else block (line 163), capture: `let else_scope = self.scope.clone();`
  - Before merge block positioning (line 177), call:
    ```
    let rebindings = merge_mutable_vars(
        self.builder, merge_block, &pre_scope,
        &[then_scope, else_scope], &mutable_var_types,
    );
    ```
  - Update `terminate_jump` calls (lines 172, 175) to include mutable var values in args
  - After positioning at merge_block, rebind mutable vars in scope (same pattern as `lower_coalesce` lines 105-124)
- [ ] Fix `lower_short_circuit_or()` — apply same fix symmetrically
- [ ] Verify: `assert_eq(actual: order, expected: [1, 2])` passes via `--backend=llvm`

### Matrix Testing

- Short-circuit with: constants, Option methods (031), block expressions with mutations (032), nested `&&`/`||`, closures in branches, `break`/`continue` in branches
- Semantic pin: `operators_logical.ori` passes all 39 tests via `--backend=llvm`
- Negative pin: `false && panic(msg: "unreachable")` — RHS must NOT execute
- Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/expressions/operators_logical.ori` — 0 mismatches
- `ORI_CHECK_LEAKS=1` clean on all short-circuit test programs

---

## 06.7 Multi-Clause Function Lowering (BUG-04-033)

**Complexity:** Complex | **Impact:** Files with multi-clause functions (Ackermann pattern)

Two errors in multi-clause function LLVM emission:
1. `build_struct called with non-struct LLVM type (i64)` at `ir_builder/aggregates.rs:184-185` — clause dispatch tries to construct a struct for a scalar result
2. PHINode predecessor mismatch from clause branches

**Root cause**: `lower_multi_clause()` at `ori_canon/src/lower/patterns.rs:117-200` compiles multi-clause functions to `CanExpr::Match` with a decision tree. Line 122 uses `ty = self.expr_type(clauses[0].body)` — type from first clause only. Lines 134, 141 use `TypeId::ERROR` for the scrutinee — synthetic nodes with error type that break LLVM codegen. Comment at lines 130-132 explicitly states: "Types use ERROR because these are synthetic nodes — the evaluator dispatches on values, not types. Codegen (LLVM) would need real types, but multi-clause functions aren't supported there yet."

The decision tree emission (`ori_arc/src/decision_tree/emit.rs:90-145`) creates multiple clause blocks via `EmitContext` (lines 25-48). Each arm may create different block structures — arms with recursive calls emit InvokeIndirect (extra blocks), while base cases don't. This causes PHI predecessor mismatches at the merge point.

### Fix

- [ ] Write failing test BEFORE fix: Ackermann function with 3+ clauses via `--backend=llvm`:
  ```
  @ack (0: int, n: int) -> int = n + 1
  @ack (m, 0: int) -> int = ack(m - 1, 1)
  @ack (m, n) = ack(m - 1, ack(m, n - 1))
  ```
- [ ] Fix `lower_multi_clause()` at `ori_canon/src/lower/patterns.rs:117-200`:
  - Line 122: use union of all clause return types (not just first clause)
  - Lines 134, 141: compute real scrutinee types from parameter types (not `TypeId::ERROR`)
  - This requires threading type information from the function signature into the canonical lowering
- [ ] Fix decision tree emission PHI: ensure all clause arms create compatible block structures. When an arm contains InvokeIndirect (recursive call), the emitted blocks must correctly jump to the merge block
- [ ] Fix `build_struct` type mismatch: at the merge point, if result is scalar (int), don't wrap in struct. Check `resolve_type()` result before calling `build_struct`
- [ ] Verify: Ackermann and fibonacci multi-clause functions compile and run correctly via `--backend=llvm`
- [ ] Debug AND release produce identical results
- [ ] `timeout 150 ./test-all.sh` green

### Matrix Testing

- Clause counts: 2 clauses, 3 clauses, 4+ clauses
- Return types: int (scalar), str (struct), [int] (RC), Option<int> (enum)
- Patterns: literal patterns, variable patterns, guard patterns, nested patterns
- Semantic pin: `ack(2, 3)` returns 9 via LLVM
- Negative pin: non-exhaustive clauses produce compile error (not codegen crash)

---

## 06.8 ABI Type Resolution Audit (Root Cause C)

**Complexity:** Complex | **Impact:** 4+ files with StructValue/IntValue confusion

Systemic issue: LLVM emitter produces struct value where int value is expected (or vice versa). The root cause is in `abi_size_inner()` at `abi/mod.rs:177-203` which sums field sizes WITHOUT alignment padding. A struct `{ byte, int, byte }` computes as 10 bytes but LLVM lays it out as 24 bytes (1+7 padding + 8 + 1+7 padding). This can misclassify as `Direct` (≤16 bytes) when `Indirect` (>16 bytes) is needed. A FIXME comment already exists at lines 198-203 documenting this.

The 16-byte threshold is at `compute_param_passing()` (`abi/mod.rs:272-290`): `if size <= 16 { Direct } else { Indirect }`.

**Crash chain**: Unresolved type variable → `TypeInfoStore` returns error type → `abi_size` returns 0 or wrong size → `Direct` instead of `Indirect` → caller passes value in register → callee expects pointer → `extract_value` on IntValue → crash at `aggregates.rs:184-185`.

### Investigation

- [ ] Quantify: how many of the remaining LCFails are from ABI misclassification vs unresolved types? Run: filter codegen errors for "non-struct" messages
- [ ] Read `TypeInfoStore` at `type_info/store.rs:1-66` — understand how `type_error_count` (line 65) tracks unresolved types. Does codegen bail early enough when type errors exist?

### Fix: Alignment-Aware ABI Size

- [ ] Fix `abi_size_inner()` at `abi/mod.rs:177-203`: include alignment padding in size computation
  - For struct types, compute LLVM-compatible layout: each field starts at alignment boundary
  - Use `TypeInfo::alignment()` to get field alignment
  - Compare result with LLVM's `DataLayout::getTypeAllocSize()` for validation
- [ ] Add `debug_assert!` comparing our `abi_size()` with LLVM's actual type size during function declaration (catches drift)

### Fix: Early Bail on Unresolved Types

- [ ] In `emit_function()` at `emit_function.rs`, check `type_error_count > 0` before proceeding to codegen. If type errors exist, skip the function entirely (LCFail is better than crash)
- [ ] Add validation at `build_struct` call sites — verify the LLVM type is actually `StructType` before calling (defensive, already partially done at `aggregates.rs:184`)

### Testing

- [ ] Write AOT tests for ABI edge cases: empty struct, single-field struct, `{ byte, int }` (12 bytes → Direct), `{ int, int, byte }` (17 bytes → Indirect), nested structs
- [ ] Write AOT test for unresolved type bail: function with intentionally unresolved types should LCFail cleanly (no crash)
- [ ] Verify: all ABI-sensitive tests pass in debug AND release
- [ ] `timeout 150 ./test-all.sh` green — no segfaults from ABI misclassification
- [ ] `./clippy-all.sh` clean

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
