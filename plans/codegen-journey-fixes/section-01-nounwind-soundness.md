---
section: "01"
title: "Nounwind Soundness"
status: complete
goal: "All functions are correctly classified as nounwind or may-unwind — no false nounwind claims"
inspired_by:
  - "Rust rustc_codegen_llvm unwind handling (compiler/rustc_codegen_llvm/src/builder.rs)"
  - "LLVM LangRef nounwind semantics (if a nounwind function unwinds, behavior is undefined)"
depends_on: []
sections:
  - id: "01.1"
    title: "Indirect Call Conservatism"
    status: complete
  - id: "01.2"
    title: "Monomorphized Callee Ordering"
    status: complete
  - id: "01.3"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Nounwind Soundness

**Status:** Complete
**Goal:** Every function's nounwind classification is sound — no function marked `nounwind` can ever unwind. Indirect calls (closures, function pointers) are conservatively treated as may-unwind. Monomorphized callees are analyzed before their callers so the nounwind set is complete.

**Context:** Journey 4 discovered that `_ori_apply` is marked `nounwind` despite calling through a function pointer. If the closure target panics, unwinding crosses a `call` instruction (not `invoke`) inside a `nounwind` function — this is undefined behavior per LLVM semantics. Journey 3 found that monomorphized generic functions (e.g., `identity<int>`) are compiled AFTER their callers, so callers can't know they're nounwind and must use `invoke` with unnecessary landing pads.

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/builder.rs`: Uses `fn_abi.can_unwind` which is computed from the function signature's `conv` and `can_unwind` fields. Indirect calls always set `can_unwind = true`.
- **Zig** `src/Compilation.zig`: Functions with `noreturn` or known-pure bodies get nounwind. Indirect calls (function pointers, `@call`) are always treated as potentially unwinding.

**Depends on:** None (this is Phase 0 — must land first).

---

## 01.1 Indirect Call Conservatism

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

**Finding #2** (HIGH): `is_arc_function_nounwind()` only checks `ArcTerminator::Invoke` callees against `nounwind_functions` and `ArcInstr::Apply` names against the `ori_panic` prefix. Indirect calls through closure function pointers appear as `Apply` instructions whose names don't match `ori_panic*`, so they're silently treated as nounwind. Any higher-order function receiving a panicking closure produces UB.

**Root cause:** The analysis has no concept of "indirect call" vs "direct call." It only pattern-matches on function name strings. A closure invocation (`_ori_apply` or inline call through `%closure.fn_ptr`) uses an `Apply` instruction with a non-panic name, passing the nounwind check.

**Fix approach:**

A function is NOT nounwind if it contains ANY of:
1. An `Invoke` terminator calling a function not in `nounwind_functions`
2. An `Apply` instruction calling a panic function (`ori_panic*`)
3. An `Apply` instruction calling through an indirect target (closure/fn-ptr)

The key addition is (3): detect indirect calls and conservatively mark the function as may-unwind.

- [x] Identify how indirect calls appear in ARC IR (2026-02-26)
  - ARC IR has distinct `ArcInstr::Apply` (direct, target=Name) vs `ArcInstr::ApplyIndirect` (indirect, target=ArcVarId/closure)
  - The old code's `else { true }` catch-all in `is_arc_function_nounwind()` silently treated `ApplyIndirect` as nounwind

- [x] Update `is_arc_function_nounwind()` to reject functions with indirect calls (2026-02-26)
  - Changed `if let Apply ... else { true }` to explicit `match` with `ApplyIndirect { .. } => false` arm
  - Added doc comment explaining the conservatism: indirect calls cannot be statically resolved

- [x] Add test: function with direct nounwind calls → still nounwind (2026-02-26)
- [x] Add test: function with closure invocation → NOT nounwind (2026-02-26)
- [x] Add test: `_ori_apply` specifically → NOT nounwind (2026-02-26)
  - Tested via `nounwind_indirect_call_is_not_nounwind` and `nounwind_mixed_safe_and_indirect_is_not_nounwind`
- [x] Verify Journey 4 program compiles with `invoke` for closure calls (not `call`) (2026-02-26)
  - `_ori_apply` no longer has `nounwind` attribute
  - `_ori_main` uses `invoke fastcc i64 @_ori_apply(...)` with landing pad
  - Program output: 16 (correct)

---

## 01.2 Monomorphized Callee Ordering

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

**Finding #3** (HIGH): `_ori_main` calls `identity<int>` and `first<int,int>`, both trivially nounwind. But monomorphized functions are compiled AFTER `_ori_main`, so at analysis time their names aren't in `nounwind_functions`. Result: `_ori_main` uses `invoke` with landing pads for calls that can never unwind.

**Impact:** All generic function calls pay unnecessary `invoke` + landing pad overhead. This affects every program using generics.

**Fix approach — 2 options:**

**(a) Two-pass analysis** (recommended — simple, correct):
1. First pass: compile all functions to ARC IR (but don't emit LLVM IR yet)
2. Analyze nounwind on all ARC functions (now the full set is available)
3. Second pass: emit LLVM IR using the complete nounwind set

This separates "ARC compilation" from "LLVM emission" — a clean architectural boundary.

**Why this is best:** The nounwind set is complete and correct before any LLVM IR is emitted. No ordering tricks, no fixups, no re-analysis.

**Trade-off:** Requires buffering all ARC functions before emission. Memory cost is proportional to program size, but ARC IR is compact.

**(b) Topological ordering** (alternative):
Compile callees before callers so nounwind info propagates forward. Requires building a call graph and topological sorting.

**Downside:** Indirect calls and recursion break topological ordering. More complex to implement and still incomplete for mutual recursion.

**(c) Post-hoc fixup** (not recommended):
Compile everything, then revisit callers to downgrade `invoke` → `call` where callees turned out nounwind.

**Downside:** Requires mutating already-emitted LLVM IR, which is fragile and LLVM's builder API doesn't support well.

**Recommended path:** Option (a) — two-pass with ARC IR buffering.

### Implementation steps

1. Refactor `compile_function()` to return `ArcFunction` without emitting LLVM IR
2. Collect all `ArcFunction` results (including monomorphized variants)
3. Run `is_arc_function_nounwind()` on the full set, building complete `nounwind_functions`
4. Emit LLVM IR for all functions using the complete nounwind set
5. Verify Journey 3 program: `_ori_main` uses `call` (not `invoke`) for `identity<int>`

- [x] Refactor to separate ARC compilation from LLVM emission (2026-02-26)
  - Split `compile_function()` into `prepare_function()` (returns `PreparedFunction`) and `emit_prepared_function()`
  - New `PreparedFunction` struct holds ARC IR + metadata without emitting LLVM IR
  - `PreparedLambda` struct for closure bodies (same separation)
- [x] Build complete nounwind set from all ARC functions (2026-02-26)
  - `compute_nounwind_set()` runs fixed-point iteration over all `PreparedFunction`s
  - Populates `CodegenContext.nounwind_functions` before any LLVM emission
- [x] Emit LLVM IR using complete nounwind set (2026-02-26)
  - `emit_prepared_function()` and `emit_prepared_lambda()` use the pre-computed nounwind set
  - `emit_invoke` in `arc_emitter` checks `ctx.nounwind_functions` to choose `call` vs `invoke`
- [x] Test: monomorphized nounwind callee → caller uses `call` not `invoke` (2026-02-26)
  - AOT test `test_mono_nounwind_callee_uses_call_not_invoke` in `generics.rs`
  - Unit tests `compute_nounwind_set_propagates_to_generic_original_name` and `compute_nounwind_set_does_not_propagate_if_any_mono_may_unwind`
- [x] Test: monomorphized may-unwind callee → caller still uses `invoke` (2026-02-26)
  - AOT test `test_mono_may_unwind_callee_uses_invoke` in `generics.rs`
- [x] Verify no regressions: `./test-all.sh` and `./llvm-test.sh` (2026-02-26)
  - 10,175 tests passed, 0 failures
  - Additional fix: mono dispatch propagation bridges name domains (ARC IR uses original generic names, nounwind set uses mangled names)

---

## 01.3 Completion Checklist

- [x] No function marked `nounwind` contains an indirect call (closure/fn-ptr invocation) (2026-02-26)
- [x] `_ori_apply` is NOT marked nounwind (2026-02-26)
  - Verified: `_ori_apply` has no `nounwind` attribute, uses `call i64 %closure.fn_ptr(...)` (indirect)
- [x] Monomorphized nounwind callees are in `nounwind_functions` before their callers are emitted (2026-02-26)
  - Two-pass: prepare → compute_nounwind_set → emit
  - Mono dispatch propagation bridges original→mangled name domains
- [x] Journey 3 program: `_ori_main` uses `call` for `identity<int>` and `first<int,int>` (2026-02-26)
  - `%call = call fastcc i64 @"_ori_identity$24m$24int"(i64 7)`
  - `%call1 = call fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)`
- [x] Journey 4 program: `_ori_apply` uses `invoke` (not `call`) for closure target (2026-02-26)
  - `%call = invoke fastcc i64 @_ori_apply(...) to label %bb1 unwind label %bb2`
  - Landing pad present with `cleanup` clause
- [x] `./test-all.sh` green (2026-02-26)
  - 10,175 tests passed, 0 failures
- [x] `./llvm-test.sh` green (2026-02-26)
  - 379 unit + 1012 AOT tests passed
- [x] No new clippy warnings: `./llvm-clippy.sh` (2026-02-26)

**Exit Criteria:** `is_arc_function_nounwind()` returns `false` for any function containing an indirect call. All monomorphized callees are analyzed before their callers. Journey 3 and Journey 4 programs produce correct results with optimal invoke/call usage. Zero regressions in test suite.
