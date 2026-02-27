---
section: "04"
title: "Alignment"
status: not-started
goal: "All load/store instructions use natural alignment for their types"
depends_on: []
sections:
  - id: "04.1"
    title: "Fix M5 — align 4 → align 8 for i64"
    status: not-started
  - id: "04.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Alignment

**Status:** Not Started
**Goal:** All `load` and `store` instructions use the natural alignment for their type. `i64` uses `align 8`, `i32` uses `align 4`, `ptr` uses `align 8`.

**Context:** Every journey from J4 onward shows `load i64, ptr %..., align 4` where `align 8` is correct. This is a conservative understatement that prevents LLVM from emitting efficient aligned loads and may cause faults on strict-alignment architectures (ARM, RISC-V).

Confirmed locations: struct fields (J4), variant tag stores (J6, J11, J12), list element stores (J10), derived eq stores (J11), Option variant stores (J12).

---

## 04.1 Fix M5 — align 4 → align 8 for i64

**Journey:** J4 (confirmed J4, J6, J9, J10, J11, J12) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (load/store emission helpers)

- [ ] Find where `align 4` is hardcoded for struct/variant field loads
- [ ] Change to use `std::mem::align_of::<T>()` or LLVM's `DataLayout` for the correct alignment
- [ ] For i64: `align 8`. For i32: `align 4`. For ptr: `align 8`.
- [ ] Verify: Journey 4 `@_ori_area` loads use `align 8`
- [ ] Verify: Journey 12 variant tag stores use `align 8`
- [ ] Grep generated IR for all 12 journeys: no `load i64, ptr %..., align 4` remaining

---

## 04.2 Completion Checklist

- [ ] All `load i64` instructions use `align 8`
- [ ] All `store i64` instructions use `align 8`
- [ ] All `load ptr` instructions use `align 8`
- [ ] `./test-all.sh` green
- [ ] No alignment-related warnings from LLVM verifier

**Exit Criteria:** `grep -c "align 4" *.ll` returns 0 for i64 load/store instructions across all 12 journey programs.
