---
section: "09"
title: "Tail Call Optimization"
status: in-progress
goal: "Tail-recursive functions are compiled to loops — no stack growth on tail calls"
inspired_by:
  - "Lean4 src/Lean/Compiler/IR/RC.lean — tail call detection and loop lowering"
  - "Koka src/Core/CheckFBIP.hs — FBIP tail call optimization"
  - "LLVM LangRef — musttail calling convention guarantees"
depends_on: ["04"]
sections:
  - id: "09.1"
    title: "Tail Call Detection"
    status: complete
  - id: "09.2"
    title: "Loop Lowering (Primary) or musttail Emission (Fallback)"
    status: not-started
---

# Section 09: Tail Call Optimization

**Status:** Not Started
**Goal:** Functions whose last action is a self-recursive call are compiled as loops via ARC-level loop lowering (or `musttail` as fallback) so they execute in constant stack space.

> **WARNING — VERY HIGH COMPLEXITY / VERY HIGH RISK:** This is the most complex section in the plan. It requires coordinated changes across TWO crates (`ori_arc` for detection/rewriting, `ori_llvm` for emission), touches the ARC pipeline's RC insertion pass (the most correctness-critical pass), and requires proving that RcDec hoisting is safe per-variable. The ARC+TCO interaction is research-grade — Lean 4's solution (`ownParamsUsingArgs`) required multiple iterations. Budget 2-3x the estimated effort.
>
> **Decision (2026-03-05): ATTEMPT.** The ARC+TCO interaction will be attempted, not deferred. Use the rollback plan (§09.2) if correctness cannot be verified.

> **Spec position (verified):** The spec **guarantees** TCO. Annex E (System Considerations, §Recursion) states: "Tail calls are guaranteed to be optimized. A tail call does not consume stack space." Clause 15 (§15.3, Tail Call Optimization) guarantees `recurse` pattern's `self(...)` in tail position compiles to a loop with O(1) stack space. This makes TCO a **correctness requirement**, not merely a quality improvement.

> **Crate dependency order:** `ori_arc` does NOT depend on `ori_llvm`. Changes flow downstream only: (1) tail call detection/rewriting in `ori_arc` FIRST, (2) emission changes in `ori_llvm` SECOND. These must be in separate commits or a single coordinated commit. Never modify `ori_llvm` to work around a missing `ori_arc` feature.

**Context:** J3's `gcd` function is tail-recursive in the source (`else gcd(a: b, b: a % b)` is the last expression), but the generated code does not apply TCO. Despite `fastcc` being applied (which enables LLVM's TCO machinery), the IR structure prevents the optimization. The actual ARC IR for `gcd` is:

```
bb2:
  %10: int = Apply @gcd(%6, %9)    // tail call
  Jump bb3(%10)                      // jump to merge block
bb3: (%11: int)
  Return %11                         // return from merge
```

The call is in `bb2`, but `Return` is in the merge block `bb3`. LLVM requires `ret` immediately after `tail call` *in the same basic block* — the `br` to the merge block and the `phi` node break this. For `gcd` specifically (all-scalar parameters), there are no `RcDec` operations; the issue is purely the CFG merge structure. For functions with RC-managed parameters (strings, lists, etc.), the ARC pipeline also inserts `RcDec` between the call and the return, which would independently block TCO.

For deeply recursive inputs, this causes stack growth and eventual overflow. The Ori spec guarantees TCO (Annex E), making this a **conformance issue**, not just a quality improvement.

**Journey affected:** J3.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: Detects tail calls and either converts to loops or emits `musttail`.
- **Koka** `src/Core/CheckFBIP.hs`: FBIP (functional-but-in-place) optimization includes tail call loop lowering.
- **LLVM** `musttail`: Guarantees tail call optimization — callee reuses caller's stack frame.

---

## 09.1 Tail Call Detection

**File(s):** `compiler/ori_arc/src/rc_insert/` (RC insertion — inserts RcDec between call and return), `compiler/ori_arc/src/lib.rs` (`run_arc_pipeline` — pipeline ordering)

> **New module `compiler/ori_arc/src/tail_call/` hygiene requirements:**
> - `mod.rs` with `//!` module docs describing pass purpose, inputs, outputs, and pipeline placement
> - `pub(crate)` visibility on pass entry point (only called from `lib.rs` pipeline)
> - `#[cfg(test)] mod tests;` declaration at bottom of `mod.rs`
> - Sibling `tests.rs` with ARC IR construction tests (using `test_helpers.rs`)
> - `///` doc comments on all `pub`/`pub(crate)` functions
> - `#[tracing::instrument]` on pass entry point
> - If module exceeds ~200 lines, split detection and rewrite into `detect.rs` and `rewrite.rs` submodules
> - Add `mod tail_call;` declaration in `lib.rs` (alongside existing `mod block_merge;`)
> - **File size budget:** detection (~100-150 lines) + rewrite (~150-250 lines) + mod.rs dispatch (~50-80 lines) = ~300-480 lines max. If approaching 500, split immediately.

> **Phase boundary:** Tail call detection MUST be in `ori_arc`, not `ori_llvm`. The detection operates on ARC IR (inspecting `ArcInstr::Apply` instructions, `ArcTerminator::Return`/`ArcTerminator::Jump` terminators, and intervening `ArcInstr::RcDec` operations). The `ori_llvm` crate only consumes the annotation.

> **TDD requirement:** Write ARC IR unit tests in `ori_arc` that construct tail-recursive and non-tail-recursive ARC functions, run the detection pass, and assert correct annotation. Write LLVM AOT tests that verify the current non-optimized behavior (recursive `call` present in IR) BEFORE implementing. Then implement and verify the `call` is replaced.

Detect when a function's return value is exactly the result of a self-recursive call with no intervening operations (no drops, no cleanup, no transformations).

**Two obstacles to TCO (both must be addressed):**

1. **CFG merge block (all functions):** ARC lowering places the tail call in one branch of an if/match, with a `Jump` to a shared merge block that does `Return`. The actual pattern is `Apply(self, args)` in block N → `Jump(merge, [result])` → `Return(result)` in merge block. Even with zero RC operations, LLVM cannot TCO because the call and ret are in different basic blocks. Loop lowering at the ARC level (replacing `Apply` + `Jump` with a direct `Jump` back to the entry) handles this naturally.

2. **ARC cleanup (RC-managed functions only):** The RC insertion pass (`rc_insert/`) runs AFTER lowering and inserts `RcDec` operations for variables whose last use is before the return. For tail-recursive functions with RC-managed parameters (strings, lists, etc.):
   - Parameters not passed to the recursive call get `RcDec` before the jump
   - Local RC variables get `RcDec` before the jump
   - These `RcDec` operations appear BETWEEN the `Apply` and the `Jump` terminator
   - This independently blocks TCO even if the merge block issue were resolved

**Two options for handling RC cleanup (Recommendation: Option 2):**
1. **Pre-RC tail call detection:** Detect tail calls in the ARC IR BEFORE `rc_insert` runs, then mark them. `rc_insert` can then hoist drops before the call instead of between call and jump.
2. **Post-RC tail call rewrite (recommended):** After RC insertion, detect the pattern `Apply(self, args) → RcDec* → Jump(merge, [result])` and rewrite to `RcDec* → Apply(self, args) → Jump(merge, [result])` (hoist drops before the call). This is simpler but must verify safety: dropping a value before the recursive call only works if the dropped value is not used by the call. This is the approach used by the pipeline placement in §09.2 (after `rc_identity` + `rc_elim`, before `block_merge`).

Criteria for tail call eligibility (at ARC IR level):
1. The call is to the same function (self-recursion) — the `ArcInstr::Apply { func, .. }` where `func == arc_func.name`
2. The call result flows to the function's return value with no transformation — typically `Apply(result)` → `Jump(merge, [result])` → `Return(result)` across 2-3 blocks
3. No ARC operations (`RcDec`/`RcInc`) between the call and the return path, OR all intervening `RcDec` operations can be safely hoisted before the call
4. All `RcDec` targets between call and return are NOT arguments to the recursive call (safe to drop before call)

Note: `fastcc` calling convention is an LLVM-level concern handled by `select_call_conv()` in `ori_llvm/src/codegen/abi/mod.rs`. All non-extern, non-main Ori functions already get `fastcc`. This is relevant for the `musttail` fallback but not for ARC-level loop lowering.

**Existing infrastructure (must build on, not duplicate):**
- **`ori_arc/src/borrow/mod.rs`** — `check_tail_call()` (line ~425) already detects tail calls during borrow inference. It finds `Apply` instructions whose `dst` matches the `Return` value and promotes borrowed params to owned to preserve tail call eligibility. This is the existing tail call detection — new detection should reuse or extend this logic, not duplicate it.
- **`ori_llvm/src/codegen/ir_builder/calls.rs`** — `call_tail()` (line ~50) already exists and sets `set_tail_call(true)` on LLVM call instructions. This is the LLVM-level `tail` attribute (weaker than `musttail`). Currently unused by the ARC emitter.
- **`recurse` pattern sentinel** — The `recurse(...)` pattern lowers to `Apply @__recurse(...)` in ARC IR (`ori_arc/src/lower/constructs.rs`, line ~171), using a sentinel name. Direct self-recursion (e.g., `gcd(a: b, b: a % b)`) lowers to `Apply @gcd(...)` with the actual function name. TCO detection must handle both patterns.
- **Borrow inference tail call preservation** — `ori_arc/src/borrow/mod.rs` module doc (§Tail Call Preservation) describes the ownership promotion strategy. When a tail call passes a borrowed param to an owned callee position, the param is promoted to owned — this avoids needing `RcDec` after the tail call (Lean 4 `ownParamsUsingArgs` pattern).

**CRITICAL: Existing `check_tail_call` limitation.** The current `check_tail_call()` in `borrow/mod.rs` only examines blocks whose terminator is `ArcTerminator::Return`. It looks for an `Apply` whose `dst` matches the `Return` value in THE SAME BLOCK. But the actual ARC IR pattern for tail calls is cross-block: `Apply` in block N → `Jump(merge, [result])` → `Return(result)` in merge block. The `Apply` and `Return` are in DIFFERENT blocks connected by a `Jump`. Therefore:
1. `check_tail_call()` as-is will NOT detect the `gcd` pattern (or any if/else tail call).
2. The new detection pass must trace through `Jump` terminators: start from a `Return` block, find the single predecessor's `Jump`, and check if the jump arg originated from an `Apply` in that predecessor.
3. Alternatively, detect during ARC lowering before the merge block is created (if pattern matching on the CanExpr is feasible).
4. The borrow inference `check_tail_call()` should still be kept for ownership promotion — it runs at a different phase and serves a different purpose.

**`__recurse` sentinel resolution — MUST be addressed before detection:**
The `__recurse` sentinel is created in `ori_arc/src/lower/constructs.rs:171` during `recurse()` pattern lowering. It is NEVER resolved elsewhere — the LLVM emitter's `emit_apply` will fail to find it in `ctx.functions` and log an "unresolved function" error. This is a latent bug for any program using `recurse()` compiled via AOT.

For TCO detection, `__recurse` must be resolved to the current function name. Two options:
1. **Resolve at lowering time** — The `ArcLowerer` has access to the function name via `self.builder` context. Store the function name and emit `Apply @func_name(...)` instead of `Apply @__recurse(...)`. This is the cleanest fix.
2. **Resolve at detection time** — The TCO detection pass compares `Apply.func` against both `arc_func.name` AND the interned `__recurse` sentinel.

**Recommendation: Option 1** — resolve at lowering time. This fixes the latent AOT bug AND simplifies TCO detection (only compare against `arc_func.name`). The sentinel was introduced because "ARC IR doesn't track the current function name in the lowerer." **This is currently true:** `ArcIrBuilder` has no `func_name` field, and `ArcLowerer` has no `func_name` field either. However, `lower_function_can()` receives `name: Name` (line 110) and constructs the `ArcLowerer` at line 153. **Implementation: add a `func_name: Name` field to `ArcLowerer`**, set it from the `name` parameter in `lower_function_can()`. Then `lower_exp_recurse()` in `constructs.rs` can use `self.func_name` instead of interning `"__recurse"`. Verify that `self.func_name` matches the `name` passed to `builder.finish(name, ...)` (same `Name` value, trivially true).

**If Option 1 is not feasible** (e.g., unexpected issue with adding the field), use Option 2 and add a `resolve_recurse_sentinel(func: &mut ArcFunction, interner: &StringInterner)` helper that rewrites all `Apply { func: __recurse, .. }` to `Apply { func: func.name, .. }` as the first step of the detection pass.

- [x] **PREREQUISITE:** Resolve `__recurse` sentinel to actual function name. **Option 1 (preferred):** (a) Add `pub(crate) func_name: Name` field to `ArcLowerer` in `lower/expr/mod.rs`. (b) Set it from `name` parameter in `lower_function_can()` at `lower/mod.rs:153`. (c) In `lower/constructs.rs:lower_exp_recurse()`, replace `self.interner.intern("__recurse")` with `self.func_name`. If Option 2, add `resolve_recurse_sentinel()` helper as first step of detection pass. (2026-03-06: Also fixed `self(...)` calls — parser emits `Ident("self")` not `SelfRef`, resolved to `func_name` in `lower_ident` and `lower_call`. Also fixed `lower_exp_recurse` to emit conditional control flow instead of eager evaluation.)
- [x] **PREREQUISITE:** Verify that `recurse()` pattern programs compile and run correctly via AOT before and after the sentinel resolution fix (this may expose a latent AOT bug — see note above). (2026-03-06: Confirmed — before fix, AOT panicked with "unresolved function `self`". After fix, factorial(5)=120 and gcd(48,18)=6 both work correctly in AOT. All 4164 spec tests pass.)
- [x] Add tail call detection pass as a NEW function (not extending `check_tail_call()` — different purpose, different phase). Detection must trace cross-block patterns: `Return(v)` → predecessor `Jump(merge, [v])` → `Apply { dst: v, func: self_name }` in predecessor block. (2026-03-06: `tail_call::detect_tail_calls()` in `ori_arc/src/tail_call/mod.rs`)
- [x] Confirm pipeline placement in `run_arc_pipeline()`: AFTER `rc_identity` + `rc_elim` (all RC ops finalized) and BEFORE `block_merge` (as specified in §09.2's pipeline code sample). Document placement with comment in `run_arc_pipeline()`. Update `lib.rs` pipeline ordering documentation. (2026-03-06: placed at line ~205 with ordering comment, pipeline doc updated)
- [x] Handle: for RC-managed functions, the ARC pipeline inserts `RcDec` between the tail call `Apply` and the `Jump` terminator — these must be hoisted before the call for TCO to work (2026-03-06: detection verifies all post-Apply instructions are RcDec; actual hoisting deferred to §09.2 rewrite)
- [x] Safety check: all hoisted `RcDec` targets are NOT among the arguments to the recursive `Apply` (would be use-after-free). Formally: for each `RcDec { var: v }` between Apply and Jump, assert `v` is not in `Apply.args`. (2026-03-06: `find_tail_apply_in_block()` builds arg_set and rejects if any RcDec target overlaps)
- [x] **Annotation format:** Choose one of: (a) new field `tail_calls: Vec<(ArcBlockId, usize)>` on `ArcFunction` (block + instruction index), (b) new `ArcInstr::TailApply` variant (replaces `Apply` for eligible calls), or (c) bitflag on `ArcInstr::Apply`. **Recommendation: (a)** — sidecar annotation, no ARC IR variant changes, minimal disruption. If (b) is chosen, ALL passes that match on `ArcInstr::Apply` must also handle `TailApply` (grep for `ArcInstr::Apply` across all of `ori_arc` to enumerate). (2026-03-06: chose option (a) — `tail_calls: Vec<TailCallSite>` on ArcFunction with `TailCallSite { call_block, call_instr_idx }`)
- [x] **Multi-clause functions:** Ori functions with multiple clauses (e.g., `@f (0: int) -> int = 1` then `@f (n) = n * f(n-1)`) are lowered to a single ARC function with pattern-matching dispatch. The recursive `Apply @f(...)` uses the actual function name. Verify detection works for multi-clause functions (the Apply is inside a match arm block, which then Jumps to the merge). (2026-03-06: tests `multi_clause_tail_recursive_arm_detected` and `multi_clause_function_detected` verify both cases)
- [x] **Mutual recursion exclusion:** Only self-recursion is eligible: `Apply.func == arc_func.name`. Mutual recursion (A calls B calls A) requires the callee's stack frame to be compatible, which is not guaranteed. Verify that `Apply.func != arc_func.name` correctly excludes mutual recursion. Also exclude `ApplyIndirect` (closure calls — callee identity unknown at compile time). (2026-03-06: detection checks `func != func_name` and only matches `ArcInstr::Apply`, excluding ApplyIndirect)
- [x] Document in `tail_call/mod.rs` module doc: `ApplyIndirect` tail calls (closure-based mutual recursion) are out of scope — callee identity unknown at compile time, cannot verify stack frame compatibility (2026-03-06: documented in module doc)

### 09.1 Completion Checklist

- [x] `__recurse` sentinel resolved to actual function name (no unresolved `__recurse` in ARC IR post-lowering) (2026-03-06)
- [x] Tail call detection correctly identifies self-recursive tail calls across the cross-block pattern (Apply → Jump → Return) (2026-03-06)
- [x] Detection accounts for `RcDec` operations between Apply and Jump (verifies hoisting is safe; actual hoisting is in §09.2) (2026-03-06)
- [x] Non-tail calls (result transformed, intervening side effects) are correctly excluded (2026-03-06)
- [x] Mutual recursion is correctly excluded (self-recursion only for this section) (2026-03-06)
- [x] `ApplyIndirect` calls are excluded (callee identity unknown) (2026-03-06)
- [x] Multi-clause functions with self-recursive calls in match arms are detected (2026-03-06)
- [x] `recurse()` pattern calls (formerly `__recurse` sentinel) are detected (2026-03-06: resolved at lowering time — emits Apply @func_name(...) directly)
- [x] Annotation format chosen and implemented (sidecar on `ArcFunction` or IR variant) (2026-03-06: option (a) — `TailCallSite` struct with `call_block` + `call_instr_idx`)
- [x] `run_arc_pipeline()` updated with new pass placement and ordering comment (2026-03-06)
- [x] `//!` module doc on `tail_call/mod.rs` describing pass purpose, inputs, outputs (2026-03-06)
- [x] `///` doc comments on all `pub`/`pub(crate)` functions in the new module (2026-03-06)
- [x] `#[tracing::instrument(skip_all)]` on pass entry point (2026-03-06)
- [x] All new source files (excluding tests) under 500 lines (2026-03-06: mod.rs ~170 lines)
- [x] `tracing::debug!` at pass entry/exit (function name, whether tail call found, how many rewritten) (2026-03-06)
- [x] ARC IR unit test: self-recursive tail call detected (scalar args) (2026-03-06)
- [x] ARC IR unit test: self-recursive tail call detected (RC-managed args, RcDec hoisted) (2026-03-06)
- [x] ARC IR unit test: non-tail call (result used in addition) NOT detected (2026-03-06)
- [x] ARC IR unit test: mutual recursion NOT detected (Apply to different function name) (2026-03-06)
- [x] ARC IR unit test: `RcDec` target used as arg to recursive call — NOT eligible (safety check) (2026-03-06)
- [x] All tests in sibling `tests.rs` files (not inline `#[cfg(test)]` blocks with test bodies) (2026-03-06)
- [x] `./test-all.sh` green (2026-03-06: 12,125 tests passing)
- [x] `./clippy-all.sh` green (2026-03-06)
- [x] `./fmt-all.sh` green (2026-03-06)

---

## 09.2 Loop Lowering (Primary) or musttail Emission (Fallback)

**File(s):** `compiler/ori_arc/src/` (ARC-level loop lowering — primary), `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` (downstream: emission of the loop structure)

> **Phase boundary:** Loop lowering converts the ARC tail-call pattern (`Apply(self, new_args)` in branch block → `Jump(merge, [result])` → `Return(result)` in merge) into a `Jump(entry, new_args)` back-edge — this is an ARC IR → ARC IR transformation. It MUST live in `ori_arc`. The `terminators.rs` file in `ori_llvm` only needs to emit the resulting `Jump` terminator (which it already handles — see `ArcTerminator::Jump` in `terminators.rs`). Do NOT add tail-call-specific logic to `ori_llvm`.

> **Pipeline ordering:** If implemented as a new pass, it must be added to `run_arc_pipeline()` in `ori_arc/src/lib.rs` with explicit documentation of where it runs relative to `rc_insert`. Per ARC pipeline rules: do NOT add passes without updating `run_arc_pipeline()`. Do NOT call out of order.

**Recommended approach: Loop lowering (primary).** Two independent obstacles prevent `musttail`: (1) the ARC IR lowers tail calls through a merge block (`Apply` → `Jump(merge)` → `Return`), putting the call and ret in different basic blocks; (2) for RC-managed functions, `RcDec` operations appear between the call and the jump. Loop lowering at the ARC IR level (replacing the tail `Apply` + `Jump` with a back-edge `Jump` to the entry) avoids both issues entirely.

**`musttail` (fallback):** Would require both: (a) the `Apply` and `Return` in the same basic block (no merge block), AND (b) zero ARC cleanup between them. The current ARC IR structure never satisfies (a), making `musttail` impractical without a separate ARC IR restructuring pass. Loop lowering is strictly superior.

```
; CURRENT ARC IR (before TCO):
; bb0:
;   %4: bool = %1 == 0
;   Branch %4 ? bb1 : bb2
; bb1:                          ; base case
;   Jump bb3(%0)
; bb2:                          ; recursive case
;   %9: int = %0 % %1
;   %10: int = Apply @gcd(%1, %9)
;   Jump bb3(%10)
; bb3: (%11: int)
;   Return %11

; AFTER LOOP LOWERING (ARC IR rewrite):
; bb0:                          ; entry — jumps to loop header
;   Jump bb1(%0, %1)
; bb1: (%a: int, %b: int)      ; loop header (was merge point)
;   %done = %b == 0
;   Branch %done ? bb2 : bb3
; bb2:                          ; exit
;   Return %a
; bb3:                          ; loop body
;   %rem = %a % %b
;   Jump bb1(%b, %rem)          ; back-edge (was recursive Apply)

; RESULTING LLVM IR:
bb0:
  br label %loop
loop:
  %a = phi i64 [%a_init, %bb0], [%b, %loop.body]
  %b = phi i64 [%b_init, %bb0], [%rem, %loop.body]
  %done = icmp eq i64 %b, 0
  br i1 %done, label %exit, label %loop.body
loop.body:
  %rem = srem i64 %a, %b
  br label %loop
exit:
  ret i64 %a

; FALLBACK (musttail, only when no ARC cleanup AND same-block ret):
%result = musttail call fastcc i64 @_ori_gcd(i64 %b, i64 %rem)
ret i64 %result
```

Requirements for `musttail` (fallback only):
- Caller and callee must have the same number and type of parameters
- Caller and callee must use the same calling convention
- The `ret` must immediately follow the call (no intervening instructions — **no RcDec!**)
- Return ABI must match exactly (including sret/aggregate conventions)
- Varargs and ABI-mismatched signatures are ineligible

### Loop Lowering Algorithm

> **COMPLEXITY WARNING:** The loop lowering rewrite modifies the function's block structure (creates new blocks, removes old ones, rewrites terminators and block params). This is comparable in complexity to `block_merge/select.rs` (300 lines) or `block_merge/merge.rs` (173 lines). The ArcIrBuilder is designed for forward construction, not in-place mutation — the rewrite operates directly on `ArcFunction.blocks` (Vec mutation). Study `block_merge/compact.rs` (block renumbering after removal) and `block_merge/merge.rs` (terminator rewriting) for established patterns. Do NOT invent new mutation patterns — reuse existing infrastructure.

The rewrite operates on the ARC IR after tail call detection (09.1) has annotated eligible calls. For each annotated tail call:

1. **Identify the pattern:** Find the `Apply { dst: result, func: self_name, args: new_args }` in block B, followed by `Jump { target: merge, args: [result] }` as B's terminator, where merge block's terminator is `Return { value: merge_param }`.
2. **Create loop header:** Create a new block (or repurpose the merge block) with block params matching the function's parameters. The entry block gets a `Jump` to this header with the original parameter values.
3. **Rewrite base case:** The base-case branch (which previously jumped to merge with the base result) now jumps to an exit block that does `Return`.
4. **Rewrite recursive case:** Replace the `Apply` + `Jump(merge, [result])` with `RcDec*` (hoisted from between Apply and Jump) + `Jump(header, new_args)`. The new args are the recursive call's arguments, which become the next iteration's "parameters."
5. **Handle multiple tail call sites:** If the function has multiple tail-recursive paths (e.g., `match` arms that each recursively call), each path gets its own back-edge to the header.
6. **Remove dead merge block:** The original merge block is no longer needed (the exit block replaces it). Remove it during a compact step.

### Pipeline Placement (MUST be explicit)

The loop lowering pass MUST be placed in `run_arc_pipeline()` in `ori_arc/src/lib.rs`. Recommended placement:

```
...existing passes...
rc_identity::propagate_rc_identity(func, &identity_map, pool);  // existing — normalize RC targets
rc_elim::eliminate_rc_ops_dataflow(func, &ownership);            // existing — remove dead RC ops
tail_call::rewrite_tail_calls(func, interner);                   // NEW — after RC ops are final
block_merge::merge_blocks(func);                                 // existing — cleans up dead blocks from TCO rewrite
...remaining passes...
```

**Why after RC elimination:** The RcDec operations are in their final positions — identity propagation has normalized targets and elimination has removed dead pairs. We can safely determine which drops to hoist and which are already eliminated. Running before RC elimination would require the pass to reason about RC operations that may later be removed.

**Why before block_merge:** The loop lowering creates new blocks (header, exit) and removes the merge block. Block merge (Phase 1: compact) will clean up any dead blocks. Phase 4 (merge jump chains) may further simplify the entry → header chain. This is the same pattern as other ARC passes — create the structure, let block_merge clean it up.

**Update `run_arc_pipeline()` documentation:** Add a comment explaining the new pass's purpose and ordering rationale. Update the module doc in `ori_arc/src/lib.rs` (pipeline listing at line ~46) to include the tail call pass.

> **NOTE: `lib.rs` function body exception.** Per hygiene rules, `lib.rs` should be an index file (no function bodies). `ori_arc/src/lib.rs` is a **known exception** — it contains `run_arc_pipeline()`, `run_arc_pipeline_all()`, and `run_uniqueness_analysis()` (382 lines currently). Adding a single function call to the tail call pass within the existing `run_arc_pipeline()` body is acceptable. Do NOT add any new function definitions to `lib.rs` — the pass implementation must live in `tail_call/mod.rs`. If `lib.rs` approaches 500 lines, extract the pipeline functions into a `pipeline.rs` submodule.

### Interaction with `block_merge` Pass

The loop-lowered function creates a back-edge from the recursive-case block to the header. This creates a multi-predecessor block (the header), which is:
- **NOT merged by Phase 4** (merge only targets single-predecessor blocks). Correct — we want the header to remain.
- **Subject to Phase 1 (compact)** — the dead merge block will be removed. Correct.
- **Subject to Phase 5 (single-pred phi)** — the exit block may be single-predecessor; if its params are unnecessary, they'll be cleaned up. Correct.
- **Subject to Phase 6 (dead param)** — if any header block params are unused (e.g., a parameter that is never read in the loop body), they'll be removed. Correct.

The entry block's `Jump(header, original_params)` is a single-predecessor edge to the header (the header also has the back-edge from the body). Phase 4 will NOT merge entry into header because header has 2 predecessors. This is correct.

**Conclusion:** No special handling needed. Block merge naturally handles the TCO-generated loop structure.

- [ ] Create `compiler/ori_arc/src/tail_call/` module directory with `mod.rs` (dispatch + `//!` module docs), and `tests.rs` (sibling test file with `#[cfg(test)] mod tests;` in `mod.rs`). If detection + rewrite exceed ~200 lines each, split into `detect.rs` and `rewrite.rs` submodules.
- [ ] Pass entry point: `pub(crate) fn rewrite_tail_calls(func: &mut ArcFunction, interner: &StringInterner)` — `pub(crate)` (only called from `lib.rs`), with `#[tracing::instrument(skip_all)]` and `///` doc comment explaining purpose and pipeline placement.
- [ ] Implement the 6-step rewrite algorithm described above
- [ ] Add `mod tail_call;` to `lib.rs` module declarations (alongside `mod block_merge;`)
- [ ] Add single call `tail_call::rewrite_tail_calls(func, interner);` to `run_arc_pipeline()` AFTER `rc_elim` and BEFORE `block_merge` — with ordering comment
- [ ] Update `ori_arc/src/lib.rs` module doc (pipeline listing at line ~46) to include the tail call pass
- [ ] Detect tail position: call result flows through Jump args to Return (cross-block pattern)
- [ ] Handle ARC cleanup: hoist `RcDec` operations from between Apply and Jump to before the Apply, then place them before the back-edge Jump. They clean up values from the current iteration, not the next.
- [ ] Handle multiple tail call sites in a single function (multiple match arms with self-calls)
- [ ] Optionally implement `musttail` as fallback for zero-ARC-cleanup functions (low priority — loop lowering is strictly superior)
- [ ] Write stress test with linear-depth tail recursion (e.g., countdown to 0 at depth >= 100_000)
- [ ] Verify stress test runs without stack overflow in AOT
- [ ] Verify: non-tail-recursive functions are unaffected (no structural changes to non-tail-recursive functions)
- [ ] If ARC-safe loop lowering is not defensible for this cycle, mark L-10 as deferred in `§10.8` with concrete rationale and follow-up scope (no silent partial landing)

### Rollback Plan

If tail call optimization introduces correctness regressions (leaks, double-frees, wrong results):
1. **Revert the tail call detection/rewrite pass** — specific files to revert:
   - `compiler/ori_arc/src/tail_call/` (or `tail_call.rs`) — the new detection/rewrite module (delete entirely)
   - `compiler/ori_arc/src/lib.rs` — remove the pass invocation from `run_arc_pipeline()` and pipeline doc
   - `compiler/ori_arc/src/lower/constructs.rs` — revert `__recurse` sentinel resolution if changed (restore `__recurse` sentinel)
   - `compiler/ori_arc/src/ir/mod.rs` — remove any new `ArcInstr` variant or `ArcFunction` field
   - Keep all NEW tests — convert them to `#[ignore = "TCO reverted: ARC cleanup hoisting safety unverified"]`
2. Mark L-10 as deferred in §10.8 with rationale: "ARC cleanup hoisting safety could not be verified"
3. Do NOT leave a partially-working TCO pass — it must be all-or-nothing. If detection works but rewrite is unsound, revert BOTH.
4. If the `__recurse` resolution fix (prerequisite) is independently correct and does not regress, it may be kept even if the TCO pass is reverted — it fixes a latent AOT bug.

### 09.2 Completion Checklist

- [ ] Self-recursive tail calls compiled as loops (no `call` to self in IR) OR `musttail` for zero-ARC cases
- [ ] J3 `gcd` function emits a loop, not a recursive call
- [ ] Stress test: tail recursion at depth >= 100,000 runs without stack overflow in AOT
- [ ] ARC cleanup (RcDec) hoisted correctly before loop back-edge — no leaks, no double-free
- [ ] `ORI_CHECK_LEAKS=1` on tail-recursive programs reports 0 leaks
- [ ] `ORI_TRACE_RC=1` on tail-recursive programs shows balanced RC ops
- [ ] Non-tail-recursive functions unchanged (no false positives)
- [ ] Functions where tail call rewrite is UNSAFE (RcDec target used by recursive call) are correctly excluded
- [ ] `run_arc_pipeline()` in `ori_arc/src/lib.rs` updated with new pass and ordering comment
- [ ] `ori_arc/src/lib.rs` module doc updated to include tail call pass in pipeline listing
- [ ] `./fmt-all.sh` green
- [ ] All new source files under 500 lines (excluding tests)
- [ ] No `#[allow(clippy::...)]` without justification — use `#[expect(clippy::..., reason = "...")]`

**Test scenarios (all must be covered):**
- [ ] AOT test: scalar args only (gcd with int params) — loop in IR, no `call @gcd`
- [ ] AOT test: RC-managed args (e.g., tail-recursive string processing function) — verify no leak, balanced RC
- [ ] AOT test: mixed scalar and RC-managed args — RcDec hoisted for RC args, scalars unchanged
- [ ] AOT test: nested tail calls in match arms (match with multiple recursive arms each in tail position)
- [ ] AOT test: tail call in if/else branches (`if cond then f(a) else f(b)` where both branches are tail calls)
- [ ] AOT test: `recurse()` pattern (was `__recurse` sentinel) — verify TCO works for `recurse(...)` calls
- [ ] AOT test: multi-clause function with tail-recursive clause — verify loop lowering
- [ ] AOT test: function with both tail and non-tail recursive calls — only tail calls are optimized
- [ ] AOT test: deep tail recursion (>= 100,000 depth) — no stack overflow
- [ ] AOT test: deep non-tail recursion (e.g., 100 depth) — still works correctly (stack grows as expected)
- [ ] Negative test: non-tail call (result used in `result + 1`) — NOT optimized, call preserved
- [ ] Negative test: mutual recursion — NOT optimized
- [ ] Spec test in `tests/spec/`: tail recursion with depth > 10,000 runs successfully
- [ ] Spec test: `recurse()` pattern with depth > 10,000 runs successfully
- [ ] IR quality test in `compiler/ori_llvm/tests/aot/ir_quality.rs`: `gcd` function has no `call @_ori_gcd` in IR, has `br label %loop` instead
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Test Locations

All tests must follow sibling `tests.rs` conventions and be placed in the correct file for their scope:

| Test Type | File | Convention |
|-----------|------|------------|
| ARC IR unit tests (detection, rewrite) | `compiler/ori_arc/src/tail_call/tests.rs` (new) | Sibling `tests.rs` |
| ARC IR unit tests (cross-block detection) | `compiler/ori_arc/src/tail_call/tests.rs` | Construct ARC functions, run pass, assert annotations |
| LLVM IR quality tests (loop in IR) | `compiler/ori_llvm/tests/aot/ir_quality.rs` | Integration test — compile Ori, check IR patterns |
| AOT execution tests (deep recursion) | `compiler/ori_llvm/tests/aot/tail_call.rs` (new) or `compiler/ori_llvm/tests/aot/arc.rs` | Compile and execute, assert no crash |
| Spec tests (functional behavior) | `tests/spec/recursion/` or `tests/spec/functions/` | `.ori` spec tests with deep recursion |
| Leak/RC balance tests | `compiler/ori_llvm/tests/aot/arc.rs` | `ORI_CHECK_LEAKS=1` + `ORI_TRACE_RC=1` |

---

## Cross-Section Interactions

### §04 (ARC Closure Lifecycle) + §09 (Tail Calls)

**Coordination required (noted in 00-overview.md):** Tail-call lowering must not regress closure/environment lifetime correctness. Specifically:
- If the tail-recursive function captures variables in a closure environment, the `RcDec` for the environment must happen at the right time.
- Loop lowering replaces the recursive `Apply` with a `Jump` back-edge. Any `RcDec` for closure environments that was between Apply and Jump must be preserved as part of the loop body (before the back-edge Jump).
- §04's closure environment RC fixes are complete — verify that the loop-lowered code still calls `RcDec` on closure environments at the right time.

### §08 (Loop IR Quality) + §09 (Tail Calls)

As noted in §08's cross-section analysis: tail call optimization converts recursive calls to loops. The resulting loop has the same structure as a `lower_loop` loop (header block with params, body, back-edge). §08.2's invariant-param elimination and §08.1's CSE apply equally to TCO-generated loops.

**Implementation order:** §08 SHOULD be implemented before §09 so the loop optimization infrastructure is tested on simpler source-level loops before being applied to TCO-generated loops. TCO-generated loops can then benefit from §08's optimizations automatically via `block_merge`.

### §01 (Block Merging) + §09 (Tail Calls)

The block_merge pass runs AFTER the tail call rewrite in `run_arc_pipeline()`. It will:
- Remove the dead merge block (Phase 1: compact)
- Potentially merge the entry → header chain if entry is a single-predecessor pass-through (Phase 4: merge jump chains). However, the header has 2 predecessors (entry + back-edge), so Phase 4 will NOT merge entry into header.
- Clean up any dead block params on the exit block (Phase 6: dead params)

No special handling needed — block_merge operates correctly on TCO-generated loops.

---

## Section 09 Exit Criteria

J3's `gcd` function is stack-safe (via loop lowering). A recursion-depth stress test (>100,000) runs without stack overflow in AOT mode. Non-tail-recursive functions show no behavioral change. `run_arc_pipeline()` is updated with the new pass. `__recurse` sentinel is resolved. Both `recurse()` pattern and direct self-recursion are optimized. If deferred, `§10.8` has concrete rationale and follow-up plan.
