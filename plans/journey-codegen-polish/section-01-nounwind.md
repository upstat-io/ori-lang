---
section: "01"
title: "Nounwind Propagation"
status: complete
reviewed: true
goal: "All user functions and the C main() wrapper carry nounwind when provably non-unwinding"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-03-19
sections:
  - id: "01.1"
    title: "Noreturn-aware nounwind analysis"
    status: complete
  - id: "01.2"
    title: "Main wrapper nounwind propagation"
    status: complete
  - id: "01.3"
    title: "Known limitation: impl methods"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Nounwind Propagation

**Status:** Complete
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

- [x] **Investigate the actual nounwind failure** for J15/J16/J17. Root causes found (2026-03-19):
  - **Bug 1 — Invoke terminator gap**: `is_arc_function_nounwind()` checked runtime functions and intercepted builtins for `Apply` instructions but NOT for `Invoke` terminators. Builtins like `@length` (→ `ori_str_len`) are nounwind, but `Invoke @length` failed because only `nounwind_functions.contains()` was checked.
  - **Bug 2 — Protocol builtin gap**: `is_callee_intercepted()` returned `false` for `__iter_next`, `__collect_set`, `__index` (protocol builtins) because the `starts_with("__")` guard excluded them. But these are intercepted by `try_emit_protocol` and always emit `call`.
  - **Bug 3 — Post-hoc single-pass**: `apply_posthoc_nounwind()` iterated functions once. HashMap iteration order meant `main` could be checked before `check_capture` was marked nounwind.
  - All three hypotheses in the plan were wrong — the actual blockers were asymmetric handling of Apply vs Invoke, missing protocol recognition, and non-deterministic iteration.
- [x] **Verify ori_panic_cstr runtime behavior**: Confirmed `ori_panic_cstr` genuinely unwinds (`C-unwind` ABI + `_Unwind_RaiseException`). It cannot be marked `Nounwind`. The fix did not require changing `ori_panic_cstr` — the overflow check's `ori_panic_cstr` call is emitted at the LLVM level (not ARC level) so the ARC analysis never sees it.
- [x] Post-hoc nounwind pass upgraded to fixed-point iteration (2026-03-19). Both two-pass and post-hoc paths verified working together.

- [x] Add test in `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs` (2026-03-19):
  - `test_function_calling_builtin_method_gets_nounwind` — J15-like program with `for w in words do w.length()` verifies Invoke @length is recognized as nounwind
  - `test_closure_call_gets_nounwind_via_posthoc` — J17-like program with closure capture verifies post-hoc fixed-point catches call chains
  - **Semantic pin**: `test_panicking_main_wrapper_lacks_nounwind` (pre-existing) — function with `panic()` does NOT get nounwind

---

## 01.2 Main wrapper nounwind propagation

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`

The C `main()` wrapper (entry_point.rs:70-72) is marked nounwind if the Ori `@main` is in the nounwind set. The Ori `@main` function may not be getting into the nounwind set if its callees were added to the set in a later iteration of the fixed-point analysis.

- [x] Verify that `generate_main_wrapper()` is called AFTER `compute_nounwind_set()` completes. Confirmed: AOT path in `codegen_pipeline.rs` runs `apply_posthoc_nounwind()` (line 401) before `generate_main_wrapper()` (line 419). (2026-03-19)
- [x] Traced J15/J17 — `@_ori_main` was not in the nounwind set because its callees (`@count_chars`, `@check_capture`) failed the Invoke/ApplyIndirect checks. Fix 01.1 resolved all cascading failures. (2026-03-19)
- [x] Pre-existing test `test_trivial_main_wrapper_has_nounwind` (ir_quality_attributes.rs:183) already covers `@main () -> int = 42` verifying both `_ori_main` and `main` carry `nounwind`. New test `test_function_calling_builtin_method_gets_nounwind` covers non-trivial cascading case. (2026-03-19)

---

## 01.3 Known Limitation: Impl Methods

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` (lines 14-19)

Impl methods are compiled via the immediate-emit path (`emit_arc_function`) BEFORE the two-pass nounwind analysis runs. This means impl methods calling monomorphized generic functions use `invoke` instead of `call`, even if the callee is nounwind. The post-hoc pass (`apply_posthoc_nounwind`, nounwind.rs:492-511) partially compensates: it marks functions with no `invoke` instructions as nounwind after all emission is complete.

- [x] Verified: impl method limitation does NOT affect J07/J15/J16/J17 journey scores. All user functions in these journeys go through the two-pass pipeline. No impl methods are involved. (2026-03-19)
- [x] N/A — impl methods do not affect journey scores, so no plan for folding them into the two-pass batch is needed for this plan's goals. (2026-03-19)

---

## Cleanup

- [x] **[DRIFT]** `define_phase.rs` / `emit_function.rs` — Extracted shared `is_callee_intercepted()` free function into `context.rs`. `callee_will_be_intercepted()` delegates to it; `nounwind/analyze.rs` calls it directly. (2026-03-19)
- [x] **[BLOAT]** `nounwind.rs` (541 lines) → split into `nounwind/` submodule: `mod.rs` (41), `types.rs` (39), `prepare.rs` (216), `analyze.rs` (241), `emit.rs` (147). All under 500. (2026-03-19)
- [x] **[BLOAT]** `define_phase.rs` (518 lines) → 402 lines after extracting `is_callee_intercepted` and `is_arc_function_nounwind` to their proper homes. (2026-03-19)

## 01.R Third Party Review Findings

- [x] `[TPR-01-001][high]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:421` — The unstaged nounwind follow-up currently breaks `ori_llvm` compilation.
  Resolved: Fixed on 2026-03-19. Extracted duplicate `is_arc_function_nounwind` to `nounwind/analyze.rs`, extracted `is_callee_intercepted` to `context.rs` (now `pub(crate)`). Build succeeds, 453 lib + 1700 integration tests pass.
- [x] `[TPR-01-002][medium]` `plans/journey-codegen-polish/section-01-nounwind.md:110` — The completion checklist overstates verification for the current repository state.
  Resolved: Fixed on 2026-03-19. After TPR-01-001 fix, re-verified: `cargo test -p ori_llvm --lib` (453 pass), `cargo test -p ori_llvm --tests` (1700 pass), `./test-all.sh` (13,317 pass, 0 fail). Completion checklist is accurate.
- [x] `[TPR-01-003][high]` `compiler/ori_llvm/src/codegen/arc_emitter/context.rs:84` — `is_callee_intercepted()` ignores monomorphized generic dispatch, so nounwind/dead-unwind analysis can misclassify generic calls as intercepted builtin methods.
  Resolved: Fixed on 2026-03-19. Added `ctx.mono_dispatch.contains_key(&callee)` check before the builtin method heuristic in `is_callee_intercepted()` (context.rs:92-98). Regression test `test_generic_call_with_builtin_arg_not_treated_as_intercepted` in `ir_quality_attributes.rs` verifies a generic may-unwind function called with `str` arg causes the caller to lack `nounwind`. Semantic pin confirmed: test FAILS without the fix (`_ori_main` incorrectly gets `nounwind`). Full suite: 13,322 pass, 0 fail.

---

## 01.N Completion Checklist

- [x] Functions with only nounwind calls (including intercepted builtins, protocols, and runtime functions) are classified as `nounwind`. Fix: Invoke terminator now checks runtime fns + intercepted builtins (same as Apply). Protocol builtins recognized. Post-hoc uses fixed-point. (2026-03-19)
- [x] C `main()` wrapper carries `nounwind` when Ori `@main` is nounwind — verified for J15, J16, J17 (2026-03-19)
- [x] `@_ori_main` carries `nounwind` in J15, J16, J17 scenarios — verified via `ORI_DUMP_AFTER_LLVM=1` (2026-03-19)
- [x] Semantic pin test: `test_panicking_main_wrapper_lacks_nounwind` (pre-existing) + `test_non_nounwind_callee_blocks_nounwind` plan item covered by same test (2026-03-19)
- [x] `timeout 150 cargo t -p ori_llvm` passes (debug) — 550 passed (2026-03-19)
- [x] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release) — 550 passed (2026-03-19)
- [x] `timeout 150 ./test-all.sh` green — 13,317 tests, 0 failures (2026-03-19)
- [x] No regressions in J01-J14 nounwind attributes — full AOT test suite passes (2026-03-19)
- [x] No new invariant `debug_assert!` needed — the fix extends existing checks (Invoke terminator mirrors Apply instruction logic), no new precondition. Protocol builtin recognition uses exhaustive `ProtocolBuiltin::from_name` from `ori_ir` (2026-03-19)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/17-fat-closure-capture.ori` shows `nounwind` on all user functions and the C `main()` wrapper. J16's `@_ori_check_multi` carries `nounwind`.
