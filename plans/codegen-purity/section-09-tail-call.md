---
section: "09"
title: "Tail Call Optimization"
status: not-started
goal: "Tail-recursive functions are compiled to loops — no stack growth on tail calls"
inspired_by:
  - "Lean4 src/Lean/Compiler/IR/RC.lean — tail call detection and loop conversion"
  - "Koka src/Core/CheckFBIP.hs — FBIP tail call optimization"
  - "LLVM LangRef — musttail calling convention guarantees"
depends_on: ["04"]
sections:
  - id: "09.1"
    title: "Tail Call Detection"
    status: not-started
  - id: "09.2"
    title: "musttail Emission or Loop Lowering"
    status: not-started
---

# Section 09: Tail Call Optimization

**Status:** Not Started
**Goal:** Functions whose last action is a self-recursive call are compiled using `musttail` (or equivalent loop conversion) so they execute in constant stack space.

> **WARNING — VERY HIGH COMPLEXITY / VERY HIGH RISK:** This is the most complex section in the plan. It requires coordinated changes across TWO crates (`ori_arc` for detection/rewriting, `ori_llvm` for emission), touches the ARC pipeline's RC insertion pass (the most correctness-critical pass), and requires proving that RcDec hoisting is safe per-variable. The ARC+TCO interaction is research-grade — Lean 4's solution (`ownParamsUsingArgs`) required multiple iterations. Budget 2-3x the estimated effort.
>
> **Decision (2026-03-05): ATTEMPT.** The ARC+TCO interaction will be attempted, not deferred. Use the rollback plan (§09.2) if correctness cannot be verified.

> **Spec consultation:** Check `docs/ori_lang/v2026/spec/` for whether the Ori language promises tail call optimization. If the spec does NOT guarantee TCO, this section is a quality improvement only (not a correctness fix) and can be safely deferred. Document the spec's position either way.

> **Crate dependency order:** `ori_arc` does NOT depend on `ori_llvm`. Changes flow downstream only: (1) tail call detection/rewriting in `ori_arc` FIRST, (2) emission changes in `ori_llvm` SECOND. These must be in separate commits or a single coordinated commit. Never modify `ori_llvm` to work around a missing `ori_arc` feature.

**Context:** J3's `gcd` function is tail-recursive in the source (`else gcd(a: b, b: a % b)` is the last expression), but the generated code does not apply TCO. The recursive call is followed by stack cleanup rather than being converted to a loop. Despite `fastcc` being applied (which enables LLVM's TCO machinery), the IR structure prevents the optimization — likely because the ARC pipeline inserts cleanup code between the call and the return.

For deeply recursive inputs, this can cause stack growth and eventual overflow. While Ori does not currently promise TCO in the language spec, this remains a codegen quality issue: equivalent hand-written C would be a loop.

**Journey affected:** J3.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: Detects tail calls and either converts to loops or emits `musttail`.
- **Koka** `src/Core/CheckFBIP.hs`: FBIP (functional-but-in-place) optimization includes tail call conversion.
- **LLVM** `musttail`: Guarantees tail call optimization — callee reuses caller's stack frame.

---

## 09.1 Tail Call Detection

**File(s):** `compiler/ori_arc/src/rc_insert/` (RC insertion — inserts RcDec between call and return), `compiler/ori_arc/src/lib.rs` (`run_arc_pipeline` — pipeline ordering)

> **Phase boundary:** Tail call detection MUST be in `ori_arc`, not `ori_llvm`. The detection operates on ARC IR (inspecting `Apply` instructions, `Return` terminators, and intervening `RcDec` operations). The `ori_llvm` crate only consumes the annotation. File reference `compiler/ori_llvm/src/codegen/function_compiler/` removed — it is downstream only.

> **TDD requirement:** Write ARC IR unit tests in `ori_arc` that construct tail-recursive and non-tail-recursive ARC functions, run the detection pass, and assert correct annotation. Write LLVM AOT tests that verify the current non-optimized behavior (recursive `call` present in IR) BEFORE implementing. Then implement and verify the `call` is replaced.

Detect when a function's return value is exactly the result of a self-recursive call with no intervening operations (no drops, no cleanup, no transformations).

**ARC pipeline interaction (critical):** The ARC pipeline's RC insertion pass (`rc_insert/`) runs AFTER lowering and inserts `RcDec` operations for variables whose last use is before the return. For tail-recursive functions, this means:
- Parameters that are not passed to the recursive call get `RcDec` before the return
- Local variables get `RcDec` before the return
- These `RcDec` operations appear BETWEEN the recursive call and the return terminator
- This breaks both `musttail` (which requires `ret` immediately after `call`) and simple tail-call detection

**Two options for handling this:**
1. **Pre-RC tail call detection:** Detect tail calls in the ARC IR BEFORE `rc_insert` runs, then mark them. `rc_insert` can then hoist drops before the call instead of between call and return.
2. **Post-RC tail call rewrite:** After RC insertion, detect the pattern `Apply(self, args) → RcDec* → Return(call_result)` and rewrite to `RcDec* → Apply(self, args) → Return(call_result)` (hoist drops before the call). This is simpler but must verify safety: dropping a value before the recursive call only works if the dropped value is not used by the call.

Criteria for tail call eligibility:
1. The call is to the same function (self-recursion)
2. The call result is the function's return value (no transformation)
3. No ARC operations (rc_dec/rc_inc) between the call and the return, OR all intervening RcDec operations can be safely hoisted before the call
4. The function uses `fastcc` calling convention
5. All RcDec targets between call and return are NOT arguments to the recursive call (safe to drop before call)

- [ ] Add tail call detection pass to the ARC pipeline or codegen
- [ ] Determine placement: before `rc_insert` (option 1) or after (option 2)
- [ ] Handle: the ARC pipeline inserts `RcDec` between the tail call and return — these must be hoisted before the call for TCO to work
- [ ] Safety check: all hoisted RcDec targets are NOT used by the recursive call (would be use-after-free)
- [ ] Annotate eligible calls in the ARC IR
- [ ] Consider: what happens with `RcDec` for the closure environment of `ApplyIndirect` tail calls (mutual recursion via closures — currently out of scope)

### 09.1 Completion Checklist

- [ ] Tail call detection correctly identifies self-recursive tail calls
- [ ] Detection handles ARC cleanup between call and return (hoisting or deferral)
- [ ] Non-tail calls (result transformed, intervening side effects) are correctly excluded
- [ ] Mutual recursion is correctly excluded (self-recursion only for this section)
- [ ] Annotation in ARC IR marks eligible calls
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

---

## 09.2 Loop Lowering (Primary) or musttail Emission (Fallback)

**File(s):** `compiler/ori_arc/src/` (ARC-level loop conversion — primary), `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` (downstream: emission of the loop structure)

> **Phase boundary:** Loop lowering converts an ARC `Apply(self, args) → Return(result)` pattern into ARC `Jump(loop_header, args)` — this is an ARC IR → ARC IR transformation. It MUST live in `ori_arc`. The `terminators.rs` file in `ori_llvm` only needs to emit the resulting `Jump` terminator (which it already handles). Do NOT add tail-call-specific logic to `ori_llvm`.

> **Pipeline ordering:** If implemented as a new pass, it must be added to `run_arc_pipeline()` in `ori_arc/src/lib.rs` with explicit documentation of where it runs relative to `rc_insert`. Per ARC pipeline rules: do NOT add passes without updating `run_arc_pipeline()`. Do NOT call out of order.

**Recommended approach: Loop lowering (primary).** The ARC pipeline inserts `RcDec` operations between tail calls and returns for RC cleanup. This means `musttail` constraints (nothing between `call` and `ret`) are almost never satisfiable for ARC-managed functions. Converting self-tail recursion to an explicit loop at the ARC IR level avoids this conflict entirely.

**`musttail` (fallback):** Only viable for functions with zero ARC cleanup between the tail call and return. This is rare in practice — most non-trivial functions have at least one RC-managed variable.

```
; PRIMARY TARGET (loop lowering at ARC level):
; gcd(a, b) becomes:
loop:
  %a = phi i64 [%a_init, %entry], [%b_next, %loop]
  %b = phi i64 [%b_init, %entry], [%rem, %loop]
  %rem = srem i64 %a, %b
  %done = icmp eq i64 %b, 0
  br i1 %done, label %exit, label %loop

; FALLBACK (musttail, only when no ARC cleanup exists):
%result = musttail call fastcc i64 @_ori_gcd(i64 %b, i64 %rem)
ret i64 %result
```

Requirements for `musttail` (fallback only):
- Caller and callee must have the same number and type of parameters
- Caller and callee must use the same calling convention
- The `ret` must immediately follow the call (no intervening instructions — **no RcDec!**)
- Return ABI must match exactly (including sret/aggregate conventions)
- Varargs and ABI-mismatched signatures are ineligible

- [ ] Implement loop lowering for self-tail-recursive functions (prefer ARC IR level)
- [ ] Detect tail position: call result is the return value, with no intervening computation
- [ ] Handle ARC cleanup: hoist `RcDec` operations before the loop back-edge (they clean up values from the current iteration, not the next)
- [ ] Optionally implement `musttail` as fallback for zero-ARC-cleanup functions
- [ ] Write stress test with linear-depth tail recursion (e.g., countdown to 0 at depth >= 100_000)
- [ ] Verify stress test runs without stack overflow in AOT
- [ ] Verify: non-tail-recursive functions are unaffected
- [ ] If ARC-safe loop lowering is not defensible for this cycle, mark L-10 as deferred in `§10.8` with concrete rationale and follow-up scope (no silent partial landing)

### Rollback Plan

If tail call optimization introduces correctness regressions (leaks, double-frees, wrong results):
1. Revert the tail call detection/rewrite pass
2. Mark L-10 as deferred in §10.8 with rationale: "ARC cleanup hoisting safety could not be verified"
3. Add a `#[ignore]` test documenting the desired behavior for future implementation
4. Do NOT leave a partially-working TCO pass — it must be all-or-nothing

### 09.2 Completion Checklist

- [ ] Self-recursive tail calls compiled as loops (no `call` to self in IR) OR `musttail` for zero-ARC cases
- [ ] J3 `gcd` function emits a loop, not a recursive call
- [ ] Stress test: tail recursion at depth >= 100,000 runs without stack overflow in AOT
- [ ] ARC cleanup (RcDec) hoisted correctly before loop back-edge — no leaks, no double-free
- [ ] `ORI_CHECK_LEAKS=1` on tail-recursive programs reports 0 leaks
- [ ] `ORI_TRACE_RC=1` on tail-recursive programs shows balanced RC ops
- [ ] Non-tail-recursive functions unchanged (no false positives)
- [ ] Functions where tail call rewrite is UNSAFE (RcDec target used by recursive call) are correctly excluded
- [ ] AOT test in `compiler/ori_llvm/tests/aot/` for deep tail recursion
- [ ] AOT test: tail-recursive function with RC-managed parameters (e.g., `gcd` with string args) — verify no leak
- [ ] Spec test in `tests/spec/`: tail recursion with depth > 10,000 runs successfully
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 09 Exit Criteria

J3's `gcd` function is stack-safe (via `musttail` or loop lowering). A recursion-depth stress test (>100,000) runs without stack overflow in AOT mode. Non-tail-recursive functions show no behavioral change. If deferred, `§10.8` has concrete rationale.
