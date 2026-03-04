---
section: "03"
title: "Exception Handling"
status: complete
goal: "Clean exception handling: no empty landing pads, no orphaned blocks, consistent nounwind propagation"
inspired_by:
  - "Rust rustc_codegen_llvm — nounwind propagation through call graph"
  - "Swift lib/SILOptimizer/ — exception handling cleanup passes"
depends_on: []
sections:
  - id: "03.1"
    title: "Fix H1 — Empty landing pads for recursive functions"
    status: complete
  - id: "03.2"
    title: "Fix M10 — Inconsistent nounwind on _ori_main"
    status: complete
  - id: "03.3"
    title: "Fix M11 — Orphaned landing pads"
    status: complete
  - id: "03.4"
    title: "Completion Checklist"
    status: complete
---

# Section 03: Exception Handling

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

- [x] Identify the condition that triggers invoke vs call in the codegen
- [x] Change: use `invoke` only when caller has live ARC values at the call site
- [x] Functions with no ARC values should use `call` even for recursive/panicking callees
- [x] If a function has no ARC values AND all callees are nounwind → mark it `nounwind`
- [x] Verify: `fib` uses `call` (no ARC values) instead of `invoke` — zero landing pads
- [x] Verify: string programs still work correctly (abort-on-panic means no unwind cleanup currently needed; when unwinding is added, RC insertion will populate unwind blocks and `invoke` will be used)

---

## 03.2 Fix M10 — Inconsistent nounwind on _ori_main

**Journey:** J8 (confirmed J9) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (nounwind analysis)

`_ori_main` sometimes has `nounwind` (J1, J2, J4) and sometimes doesn't (J8, J9), even when all callees are `nounwind`. The nounwind analysis doesn't propagate through monomorphized call sites.

- [x] Find the nounwind analysis code (`nounwind.rs` — two-pass prepare/analyze/emit)
- [x] Verify it considers monomorphized function names when checking callees (mono propagation existed but was OUTSIDE the fixed-point loop)
- [x] Fix: moved mono propagation INSIDE fixed-point loop so callers of original generic names are re-analyzed after propagation
- [x] Verify: `_ori_main` consistently has `nounwind` when all callees are nounwind (including through generics)

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

- [x] Identify where ARC cleanup landing pads are generated (mod.rs unwind_blocks → landingpad emission)
- [x] Fix: `callee_will_be_intercepted()` helper detects callees handled by builtin/format/prelude handlers; dead_unwind detection skips creating LLVM blocks for their unwind targets
- [x] Verify: string programs have 0 orphaned landing pads (No predecessors!)
- [x] Verify: `catch(expr:)` landing pads still work correctly (declared user functions excluded from interception check)

---

## 03.4 Completion Checklist

- [x] Pure-integer functions use `call` (not `invoke`) — no empty landing pads
- [x] `_ori_main` consistently has `nounwind` when all callees are nounwind
- [x] 0 orphaned landing pad blocks (no blocks with "No predecessors!")
- [x] ARC cleanup paths still work correctly (string/list programs don't leak or crash)
- [x] `./test-all.sh` green
- [x] `./scripts/valgrind-aot.sh` — 0 errors

**Exit Criteria:** Generated IR for all 12 journeys has 0 orphaned blocks, 0 empty landing pads for non-ARC functions, and consistent nounwind attributes. `grep "No predecessors" *.ll` returns 0 matches.
