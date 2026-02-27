---
section: "08"
title: "Loop & Range"
status: not-started
goal: "Tail calls optimized; range materialization eliminated; no dead phis or duplicate computations in loops"
depends_on: []
sections:
  - id: "08.1"
    title: "Fix M4 — Tail call optimization"
    status: not-started
  - id: "08.2"
    title: "Fix L5 — Range struct materialization"
    status: not-started
  - id: "08.3"
    title: "Fix L6 — Duplicate computation in loops"
    status: not-started
  - id: "08.4"
    title: "Fix L7 — Dead phi values at loop exit"
    status: not-started
  - id: "08.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Loop & Range

**Status:** Not Started
**Goal:** Tail-recursive functions compile as loops (no stack growth). Range iteration doesn't materialize an intermediate struct. Loop codegen produces minimal, clean SSA.

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

- [ ] Assess: does LLVM's `-O2` handle gcd's tail recursion? If so, mark as low priority.
- [ ] If implementing: detect self-recursive calls in tail position
- [ ] Emit `musttail` annotation or compile as loop
- [ ] Verify: `gcd(48, 18)` still returns 6

---

## 08.2 Fix L5 — Range Struct Materialization

**Journey:** J7 | **Severity:** LOW
**File(s):** `compiler/ori_llvm/src/codegen/` (range/for..in codegen)

Range `1..=n` creates `{ i64, i64, i64, i64 }` (start, end, step, current) via 3 `insertvalue`, then immediately extracts all fields via 3 `extractvalue`. The struct is dead after extraction.

- [ ] Assess: does this actually affect performance after LLVM optimization? (SROA should eliminate it)
- [ ] If implementing: emit range loop directly from start/end/step values without intermediate struct
- [ ] If LLVM handles it: mark as deferred

---

## 08.3 Fix L6 — Duplicate Computation in Loops

**Journey:** J7 | **Severity:** LOW

`sum_loop` computes `i + 1` twice — once for `total += i + 1` and once for `i += 1`. LLVM CSE eliminates this.

- [ ] Assess: does this affect -O0 performance meaningfully?
- [ ] If implementing: share the result of `i + 1` between both uses
- [ ] If LLVM handles it: mark as deferred

---

## 08.4 Fix L7 — Dead Phi Values at Loop Exit

**Journey:** J7 (confirmed J10) | **Severity:** LOW

Loop exit blocks have phi nodes for variables that are never used after the loop. `sum_loop`'s exit has 3 phis but only 1 is used (total).

- [ ] Assess: does LLVM's dead code elimination handle this?
- [ ] If implementing: only emit phis for variables used after the loop
- [ ] If LLVM handles it: mark as deferred

---

## 08.5 Completion Checklist

- [ ] Tail call optimization assessed (implemented or deferred to LLVM -O2)
- [ ] Range materialization assessed (implemented or deferred to LLVM SROA)
- [ ] Duplicate computation assessed (implemented or deferred to LLVM CSE)
- [ ] Dead phis assessed (implemented or deferred to LLVM DCE)
- [ ] `./test-all.sh` green

**Exit Criteria:** All items assessed. Those not handled by LLVM optimization passes are implemented. Journey 3 and 7 produce correct results.
