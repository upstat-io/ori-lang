---
section: "08"
title: "Loop & Range"
status: complete
goal: "Tail calls optimized; range materialization eliminated; no dead phis or duplicate computations in loops"
depends_on: []
sections:
  - id: "08.1"
    title: "Fix M4 — Tail call optimization"
    status: complete
  - id: "08.2"
    title: "Fix L5 — Range struct materialization"
    status: complete
  - id: "08.3"
    title: "Fix L6 — Duplicate computation in loops"
    status: complete
  - id: "08.4"
    title: "Fix L7 — Dead phi values at loop exit"
    status: complete
  - id: "08.5"
    title: "Completion Checklist"
    status: complete
---

# Section 08: Loop & Range

**Context:** J3 showed tail-recursive `gcd` compiles to `invoke` instead of a loop — stack overflow risk for large inputs. J7 showed range `1..=n` creates a 4-field struct then immediately destructures it — the struct serves no purpose. J7 also showed duplicate `i + 1` computation and dead phi nodes at loop exit.

---

## 08.1 Fix M4 — Tail Call Optimization

**Journey:** J3 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (function call emission)

`gcd(b, a%b)` is in tail position but compiles to `invoke` — no tail call annotation, no loop transformation.

**Fix options:**
- **(a) `musttail` annotation** — LLVM will optimize the call to a jump. Simple but only works for self-recursion.
- **(b) Loop transformation** — Detect tail-recursive calls and compile them as loops directly. More robust.
- **(c) Defer to LLVM** — LLVM's tail call optimization pass handles simple cases with `-O2`. But `-O0` (debug) doesn't optimize.

**Note:** This is an optimization, not a correctness fix. Defer if higher-priority work remains.

- [x] Assess: does LLVM's `-O2` handle gcd's tail recursion? — **YES: O2 converts `call fastcc @_ori_gcd` to loop with phi nodes, adds `norecurse nosync nounwind`**
- [x] No codegen change needed — LLVM handles it
- [x] Verify: `gcd(48, 18)` returns 6 ✓

---

## 08.2 Fix L5 — Range Struct Materialization

**Journey:** J7 | **Severity:** LOW
**File(s):** `compiler/ori_llvm/src/codegen/` (range/for..in codegen)

Range `1..=n` creates `{ i64, i64, i64, i64 }` (start, end, step, current) via 3 `insertvalue`, then immediately extracts all fields via 3 `extractvalue`. The struct is dead after extraction.

- [x] Assess: **LLVM O2 completely eliminates the range struct** — insertvalue/extractvalue round-trip optimized away, constants propagated directly into phi nodes
- [x] No codegen change needed

---

## 08.3 Fix L6 — Duplicate Computation in Loops

**Journey:** J7 | **Severity:** LOW

`sum_loop` computes `i + 1` twice — once for `total += i + 1` and once for `i += 1`. LLVM CSE eliminates this.

- [x] Assess: **LLVM CSE eliminates duplicate computation at O2** — no codegen change needed

---

## 08.4 Fix L7 — Dead Phi Values at Loop Exit

**Journey:** J7 (confirmed J10) | **Severity:** LOW

Loop exit blocks have phi nodes for variables that are never used after the loop. `sum_loop`'s exit has 3 phis but only 1 is used (total).

- [x] Assess: **LLVM DCE eliminates dead phis at O2** — no codegen change needed

---

## 08.5 Completion Checklist

- [x] Tail call optimization — **LLVM O2 converts to loop** (no codegen change needed)
- [x] Range materialization — **LLVM SROA eliminates intermediate struct** (no codegen change needed)
- [x] Duplicate computation — **LLVM CSE handles it** (no codegen change needed)
- [x] Dead phis — **LLVM DCE handles it** (no codegen change needed)
- [x] No test-all.sh needed (no codegen changes made)

**Exit Criteria:** All 4 items confirmed handled by LLVM O2 via `opt-21 -O2 -S` analysis. Journey 3 and 7 produce correct results.
