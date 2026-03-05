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

**Context:** J3's `gcd` function is tail-recursive in the source (`else gcd(a: b, b: a % b)` is the last expression), but the generated code does not apply TCO. The recursive call is followed by stack cleanup rather than being converted to a loop. Despite `fastcc` being applied (which enables LLVM's TCO machinery), the IR structure prevents the optimization — likely because the ARC pipeline inserts cleanup code between the call and the return.

For deeply recursive inputs, this can cause stack growth and eventual overflow. While Ori does not currently promise TCO in the language spec, this remains a codegen quality issue: equivalent hand-written C would be a loop.

**Journey affected:** J3.

**Reference implementations:**
- **Lean4** `src/Lean/Compiler/IR/RC.lean`: Detects tail calls and either converts to loops or emits `musttail`.
- **Koka** `src/Core/CheckFBIP.hs`: FBIP (functional-but-in-place) optimization includes tail call conversion.
- **LLVM** `musttail`: Guarantees tail call optimization — callee reuses caller's stack frame.

---

## 09.1 Tail Call Detection

**File(s):** `compiler/ori_arc/src/` (ARC pipeline), `compiler/ori_llvm/src/codegen/function_compiler/`

Detect when a function's return value is exactly the result of a self-recursive call with no intervening operations (no drops, no cleanup, no transformations).

Criteria for tail call eligibility:
1. The call is to the same function (self-recursion)
2. The call result is the function's return value (no transformation)
3. No ARC operations (rc_dec/rc_inc) between the call and the return
4. The function uses `fastcc` calling convention

- [ ] Add tail call detection pass to the ARC pipeline or codegen
- [ ] Handle: the ARC pipeline may insert cleanup between the tail call and return — these must be hoisted before the call for TCO to work
- [ ] Annotate eligible calls in the ARC IR

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

**File(s):** `compiler/ori_arc/src/` (ARC-level loop conversion), `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`

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
- [ ] If ARC-safe loop lowering is not defensible for this cycle, mark L-10 as deferred in `§10.9` with concrete rationale and follow-up scope (no silent partial landing)

### 09.2 Completion Checklist

- [ ] Self-recursive tail calls compiled as loops (no `call` to self in IR) OR `musttail` for zero-ARC cases
- [ ] J3 `gcd` function emits a loop, not a recursive call
- [ ] Stress test: tail recursion at depth >= 100,000 runs without stack overflow in AOT
- [ ] ARC cleanup (RcDec) hoisted correctly before loop back-edge — no leaks, no double-free
- [ ] Non-tail-recursive functions unchanged (no false positives)
- [ ] AOT test in `compiler/ori_llvm/tests/aot/` for deep tail recursion
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 09 Exit Criteria

J3's `gcd` function is stack-safe (via `musttail` or loop lowering). A recursion-depth stress test (>100,000) runs without stack overflow in AOT mode. Non-tail-recursive functions show no behavioral change. If deferred, `§10.9` has concrete rationale.
