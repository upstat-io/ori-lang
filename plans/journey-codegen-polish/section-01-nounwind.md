---
section: "01"
title: "Nounwind Propagation"
status: not-started
reviewed: true
goal: "All user functions and the C main() wrapper carry nounwind when provably non-unwinding"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Noreturn-aware nounwind analysis"
    status: not-started
  - id: "01.2"
    title: "Main wrapper nounwind propagation"
    status: not-started
  - id: "01.3"
    title: "Known limitation: impl methods"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Nounwind Propagation

**Status:** Not Started
**Goal:** Every user function and the C `main()` wrapper is marked `nounwind` when all call paths are provably non-unwinding. Functions whose only non-nounwind calls are in provably-unreachable blocks (e.g., overflow panic paths terminated by `unreachable`) must also be classified as `nounwind`, OR the blocking callee (`ori_panic_cstr`) must be proven to actually be nounwind via runtime inspection.

**Context:** Journeys J15, J16, and J17 lose points because certain functions lack the `nounwind` attribute despite being effectively non-unwinding on their normal code paths. Two specific gaps exist: (1) `@_ori_main` is sometimes not marked `nounwind` because the fixed-point analysis misses it when all callees are nounwind, and (2) certain functions are conservatively marked as may-unwind due to an unidentified ARC-level callee that is not in the nounwind set. **Note**: the original hypothesis that overflow checks with `ori_panic_cstr` were the blocker is likely wrong -- those calls are emitted at the LLVM IR level, not the ARC IR level that `is_arc_function_nounwind()` analyzes. The actual blocker must be identified empirically (see 01.1 investigation task).

**Depends on:** None.

---

## 01.1 Noreturn-aware nounwind analysis

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`, `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

The fixed-point nounwind analysis in `compute_nounwind_set()` (nounwind.rs:290-411, doc comment from 280) determines which functions are nounwind by checking all call instructions. `is_arc_function_nounwind()` (define_phase.rs:421-464) checks each `Invoke` and `Apply` instruction against the known nounwind set.

The gap: when a function calls a `noreturn` function (e.g., `ori_panic_cstr`), the call is not classified as nounwind because the function isn't in the nounwind set — it's in the "noreturn" set. However, this is nuanced.

**CRITICAL**: `ori_panic_cstr` is intentionally NOT marked `Nounwind` (`runtime_decl/runtime_functions.rs:113-116`, attrs: `[Cold, Noreturn]`). The runtime comment (`runtime_decl/runtime_functions.rs:104`) and test (`runtime_decl/tests.rs:238-248`) explicitly state: "ori_panic_cstr must NOT be nounwind (must unwind for RC cleanup)". The panic path uses stack unwinding to run RC cleanup (landing pads) before the process terminates. A `noreturn` function that unwinds (like `ori_panic_cstr`) CAN unwind to the caller — unwinding is a different control-flow edge than a normal return.

**The correct analysis**: A `noreturn` function is NOT automatically nounwind-safe. The correct fix depends on what the function actually does:
- `noreturn` + `nounwind` (e.g., `abort()`) → safe, cannot unwind
- `noreturn` without `nounwind` (e.g., `ori_panic_cstr`) → MAY unwind for cleanup, NOT safe to treat as nounwind

The real question is: why does `_ori_check_multi` or `@_ori_main` fail to get `nounwind` when all their OTHER callees are nounwind?

**CRITICAL CORRECTION**: The overflow check path (`checked_ops.rs:198-204`) emits `ori_panic_cstr` at the **LLVM IR level** (via `IrBuilder::checked_add/sub/mul`), NOT at the ARC IR level. The `is_arc_function_nounwind()` analysis operates on **ARC IR** (`ArcInstr::Apply`, `ArcTerminator::Invoke`) and therefore **cannot see** overflow panic calls. This means Hypothesis A below is almost certainly wrong. The actual nounwind blocker must be an ARC-level callee — e.g., a non-nounwind runtime function called via `ArcInstr::Apply`, or a user function called via `ArcTerminator::Invoke` that isn't in the nounwind set yet.

- [ ] **Investigate the actual nounwind failure** for J15/J16/J17. Use `ORI_DUMP_AFTER_ARC=1 ori build <journey_file>` to inspect the ARC IR of the affected functions. Identify which specific Apply/Invoke callee is blocking nounwind classification.
  - **Hypothesis A (LIKELY WRONG — see CRITICAL CORRECTION above)**: ~~The overflow check's `ori_panic_cstr` call blocks classification.~~ Overflow `ori_panic_cstr` calls are emitted at the LLVM IR level by `checked_ops.rs`, not at the ARC IR level. `is_arc_function_nounwind()` (define_phase.rs:421-464, Apply arm at 432-448) does not see them. Confirm/eliminate this hypothesis first by running the investigation step.
  - **Hypothesis B (MORE LIKELY)**: A different non-nounwind runtime call or user function at the ARC IR level is the blocker. Possible candidates: `ori_iter_drop`, `ori_list_push`, `ori_str_concat`, or any runtime function without `Nounwind` in its `RT_FUNCTIONS` attrs. Identify by inspecting the ARC IR dump.
  - **Hypothesis C**: The fixed-point iteration converges but the Ori `@main` function is not in the prepared batch (it may be compiled via the immediate-emit `emit_arc_function` path instead of the two-pass path). Check whether `@_ori_main` goes through `prepare_all_cached` or `emit_arc_function`.
  - **If the blocker is an ARC-level Apply to a non-nounwind runtime function**: Either (a) add `Nounwind` to that runtime function's attrs if it truly cannot unwind, or (b) teach `is_arc_function_nounwind()` to exclude calls in blocks terminated by `Unreachable` (cold-block exclusion approach described below).
  - **Cold-block exclusion approach**: In `is_arc_function_nounwind()` (define_phase.rs:421-464), when an `Apply` calls a runtime function that is `noreturn` (but not `nounwind`), check if the ARC block terminates with `ArcTerminator::Unreachable`. If so, the unwind from this call cannot propagate to the function's normal return path. This is safe because: (a) the call never returns (noreturn), and (b) any unwind from it will only clean up frames that are already being torn down by the panic.
  - **Alternative — mark `ori_panic_cstr` as nounwind**: If Ori's panic mechanism does NOT use C++ exceptions / LLVM personality-based unwinding, and instead uses `longjmp` or `abort()`, then `ori_panic_cstr` can be safely marked nounwind. This would be the simplest fix but requires verifying the runtime implementation.
- [ ] **Verify ori_panic_cstr runtime behavior**: The implementation is at `compiler/ori_rt/src/io/mod.rs:129-157`. It is `extern "C-unwind"` and calls `aot_raise_exception()` which calls `_Unwind_RaiseException` (Itanium) or `RaiseException` (MSVC SEH). **This confirms `ori_panic_cstr` genuinely unwinds** — it MUST NOT be marked `Nounwind`. The cold-block exclusion approach (or identifying a different blocker) is the correct path.
  - **Confirmed**: `ori_panic_cstr` uses C exception unwinding (`C-unwind` ABI + `_Unwind_RaiseException`). It cannot be marked nounwind.
- [ ] Note: there is also a post-hoc nounwind pass (`apply_posthoc_nounwind()` at nounwind.rs:492-511) that marks functions with no `invoke` instructions. If the fix prevents `invoke` emission for the blocking callee, the post-hoc pass may also catch functions. Both paths should be verified.
  - This affects `_ori_check_multi` in J16 which calls `ori_panic_cstr` (marked `noreturn cold`) on the overflow path

- [ ] Add test in `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs`:
  - If fix is "mark ori_panic_cstr nounwind": test that a function calling only nounwind + noreturn-nounwind functions gets `nounwind`
  - If fix is "cold-block exclusion": test that a function with a conditional branch to a `ori_panic_cstr`+`unreachable` block still gets `nounwind`
  - **Semantic pin**: test that a function with a real (non-noreturn) non-nounwind callee does NOT get `nounwind` — ensures the fix doesn't over-classify

---

## 01.2 Main wrapper nounwind propagation

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`

The C `main()` wrapper (entry_point.rs:70-72) is marked nounwind if the Ori `@main` is in the nounwind set. The Ori `@main` function may not be getting into the nounwind set if its callees were added to the set in a later iteration of the fixed-point analysis.

- [ ] Verify that `generate_main_wrapper()` is called AFTER `compute_nounwind_set()` completes. If it's called before, the Ori `@main` may not yet be classified
- [ ] If the ordering is correct, trace J15/J17 to determine why `@_ori_main` is not in the nounwind set. The likely cause is that `@_ori_main` calls user functions that call `noreturn` functions — fix 01.1 should cascade to fix this
- [ ] Add test: compile `@main () -> int = 42` and verify both `@_ori_main` and `@main` carry `nounwind`

---

## 01.3 Known Limitation: Impl Methods

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` (lines 14-19)

Impl methods are compiled via the immediate-emit path (`emit_arc_function`) BEFORE the two-pass nounwind analysis runs. This means impl methods calling monomorphized generic functions use `invoke` instead of `call`, even if the callee is nounwind. The post-hoc pass (`apply_posthoc_nounwind`, nounwind.rs:492-511) partially compensates: it marks functions with no `invoke` instructions as nounwind after all emission is complete.

- [ ] Verify that the impl method limitation does NOT affect J07/J15/J16/J17 journey scores. If any journey loses points due to impl method nounwind gaps, the fix is to fold impl methods into the two-pass batch (requires moving `compile_impls()` before the two-pass analysis, which is a larger refactor).
- [ ] If the limitation DOES affect journey scores, add a concrete plan for folding impl methods into the two-pass batch — this is NOT deferred, it is a concrete task required to reach 10.0/10.

---

## Cleanup

- [ ] **[DRIFT]** `define_phase.rs:471-517` / `emit_function.rs:30-88` — `is_callee_intercepted()` and `callee_will_be_intercepted()` are near-exact duplicates (same logic, same structure, same comments, different `self` types). This is a sync point that WILL drift. Extract shared logic into a free function or a trait method that both can call, taking the needed context (functions map, method_functions, type_idx_to_name, type_info) as parameters.
- [ ] **[BLOAT]** `nounwind.rs` — Currently 541 lines (exceeds 500-line limit). Section 01 will add code here. Split prepare/analyze/emit phases into submodules (e.g., `nounwind/prepare.rs`, `nounwind/analyze.rs`, `nounwind/emit.rs`) or extract `PreparedFunction`/`PreparedLambda` types to a separate file.
- [ ] **[BLOAT]** `define_phase.rs` — Currently 518 lines (exceeds 500-line limit). Extract `is_callee_intercepted()` (see DRIFT item above) and `declare_and_process_lambda()` (lines 318-401) to reduce below 500.

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] Functions whose only non-nounwind calls are in provably-cold/unreachable blocks are classified as `nounwind` (or `ori_panic_cstr` is correctly marked nounwind if that's the chosen fix)
- [ ] C `main()` wrapper carries `nounwind` when Ori `@main` is nounwind
- [ ] `@_ori_main` carries `nounwind` in J15, J16, J17 scenarios
- [ ] Semantic pin test: function with a real non-nounwind callee does NOT get `nounwind`
- [ ] `timeout 150 cargo t -p ori_llvm` passes (debug)
- [ ] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release — FastISel behavior differs)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] No regressions in J01-J14 nounwind attributes
- [ ] Any new invariant (e.g., "cold-block exclusion only applies when terminator is Unreachable") has a `debug_assert!` at the point where it is relied upon

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/17-fat-closure-capture.ori` shows `nounwind` on all user functions and the C `main()` wrapper. J16's `@_ori_check_multi` carries `nounwind`.
