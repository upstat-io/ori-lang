---
section: "03"
title: "Exception Handling"
status: not-started
goal: "Clean exception handling: no empty landing pads, no orphaned blocks, consistent nounwind propagation"
inspired_by:
  - "Rust rustc_codegen_llvm — nounwind propagation through call graph"
  - "Swift lib/SILOptimizer/ — exception handling cleanup passes"
depends_on: []
sections:
  - id: "03.1"
    title: "Fix H1 — Empty landing pads for recursive functions"
    status: not-started
  - id: "03.2"
    title: "Fix M10 — Inconsistent nounwind on _ori_main"
    status: not-started
  - id: "03.3"
    title: "Fix M11 — Orphaned landing pads"
    status: not-started
  - id: "03.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Exception Handling

**Status:** Not Started
**Goal:** Exception handling infrastructure is minimal and correct. No empty landing pads when no cleanup is needed. No orphaned blocks with no predecessors. `nounwind` consistently applied based on call graph analysis.

**Context:** The EH infrastructure was added for ARC cleanup (strings, lists need `ori_rc_dec` on unwind). But it's applied too broadly — recursive pure-integer functions get `invoke` + empty landing pads, and ARC functions get orphaned pads that are never reached. This adds IR bloat and confuses optimization passes.

**Depends on:** Coordinate with Section 02 (H2) since nounwind analysis code is shared.

---

## 03.1 Fix H1 — Empty Landing Pads for Recursive Functions

**Journey:** J3 (confirmed J3, J5, J9, J10) | **Severity:** HIGH
**File(s):** `compiler/ori_llvm/src/codegen/` (call emission, nounwind analysis)

When any function in a module is recursive, ALL functions switch from `call` to `invoke` with empty landing pads. For `fib(10)`: ~354 useless landing pad blocks. The landing pads just `resume` — no cleanup.

```llvm
; Pure integer function — needs NO landing pad:
define fastcc i64 @_ori_fib(i64 %0) personality ptr @rust_eh_personality {
  %call = invoke fastcc i64 @_ori_fib(i64 %sub)
          to label %bb4 unwind label %bb5
bb5:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp    ; does NOTHING — no cleanup needed
}
```

**Fix:** Use `invoke` only when the caller has ARC-managed values that need cleanup on unwind. If the caller holds no ARC values at the call site, use `call` even for potentially-unwinding functions.

- [ ] Identify the condition that triggers invoke vs call in the codegen
- [ ] Change: use `invoke` only when caller has live ARC values at the call site
- [ ] Functions with no ARC values should use `call` even for recursive/panicking callees
- [ ] If a function has no ARC values AND all callees are nounwind → mark it `nounwind`
- [ ] Verify: Journey 3's `fib` uses `call` (no ARC values) instead of `invoke`
- [ ] Verify: Journey 9's `check_strings` still uses `invoke` where needed (has ARC values)

---

## 03.2 Fix M10 — Inconsistent nounwind on _ori_main

**Journey:** J8 (confirmed J9) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (nounwind analysis)

`_ori_main` sometimes has `nounwind` (J1, J2, J4) and sometimes doesn't (J8, J9), even when all callees are `nounwind`. The nounwind analysis doesn't propagate through monomorphized call sites.

- [ ] Find the nounwind analysis code
- [ ] Verify it considers monomorphized function names when checking callees
- [ ] Fix: `_ori_main` should be `nounwind` when all its callees are `nounwind`
- [ ] Verify: Journey 8 `_ori_main` has `nounwind`

---

## 03.3 Fix M11 — Orphaned Landing Pads

**Journey:** J9 (confirmed J10) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (ARC emitter)

The ARC emitter generates landing pad blocks for cleanup, but the actual runtime calls (`ori_str_from_raw`, `ori_rc_dec`) use `call` not `invoke`. The landing pads have no predecessors — pure dead code.

```llvm
bb2:                                  ; No predecessors!  ← DEAD
  %lp = landingpad { ptr, i32 } cleanup
  call void @ori_rc_dec(...)          ; cleanup that's never reached
  resume { ptr, i32 } %lp
```

**Fix:** Either:
- **(a)** Don't emit landing pads for calls that use `call` instead of `invoke`
- **(b)** Use `invoke` for runtime calls that may panic, connecting to the landing pads

Recommendation: Fix the emission to match — if a call uses `call`, don't emit a landing pad for it.

- [ ] Identify where ARC cleanup landing pads are generated
- [ ] Fix: only emit landing pad blocks when the corresponding call uses `invoke`
- [ ] Verify: Journey 9 `check_strings` has 0 orphaned landing pads
- [ ] Verify: Landing pads that ARE needed (for `invoke` calls) still work correctly

---

## 03.4 Completion Checklist

- [ ] Pure-integer functions use `call` (not `invoke`) — no empty landing pads
- [ ] `_ori_main` consistently has `nounwind` when all callees are nounwind
- [ ] 0 orphaned landing pad blocks (no blocks with "No predecessors!")
- [ ] ARC cleanup paths still work correctly (string/list programs don't leak or crash)
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` — 0 errors

**Exit Criteria:** Generated IR for all 12 journeys has 0 orphaned blocks, 0 empty landing pads for non-ARC functions, and consistent nounwind attributes. `grep "No predecessors" *.ll` returns 0 matches.
