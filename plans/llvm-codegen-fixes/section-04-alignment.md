---
section: "04"
title: "Alignment"
status: complete
goal: "All load/store instructions use ABI alignment derived from LLVM DataLayout for the active target"
depends_on: []
sections:
  - id: "04.1"
    title: "Fix M5 — DataLayout-driven alignment for all load/store"
    status: complete
  - id: "04.2"
    title: "Completion Checklist"
    status: complete
---

# Section 04: Alignment

**Context:** Every journey from J4 onward shows `load i64, ptr %..., align 4` where the natural alignment is larger. This is a conservative understatement that prevents LLVM from emitting efficient aligned loads and **causes faults on strict-alignment architectures** (ARM, RISC-V). Ori targets both x86-64 and aarch64 — correct alignment is a correctness requirement, not just an optimization.

Confirmed locations: struct fields (J4), variant tag stores (J6, J11, J12), list element stores (J10), derived eq stores (J11), Option variant stores (J12).

---

## 04.1 Fix M5 — DataLayout-Driven Alignment for All Load/Store

**Journey:** J4 (confirmed J4, J6, J9, J10, J11, J12) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (load/store emission helpers)

**Approach:** Query LLVM's `DataLayout` for the **ABI alignment** (not preferred alignment) of each type at each load/store site. This distinction matters:

- **ABI alignment** = the minimum alignment the target guarantees for a type. Safe to use on any load/store — the hardware will not fault.
- **Preferred alignment** = the alignment that maximizes performance. Using preferred alignment on a load/store is only valid if the pointer is *actually* aligned to that value. Overstating alignment is **UB** — e.g., claiming `align 16` on a pointer that's only 8-aligned causes LLVM to emit instructions that fault on strict-alignment targets.

For struct fields accessed via GEP from an alloca or sret pointer, the alloca's own alignment plus the field offset determines the *proven* alignment at the access site. The safe conservative choice is ABI alignment, which is always guaranteed.

Do NOT hardcode alignment values — they vary by target:

| Type | x86-64 ABI | aarch64 ABI | Source |
|------|-----------|-------------|--------|
| `i64` | 8 | 8 | DataLayout |
| `i32` | 4 | 4 | DataLayout |
| `ptr` | 8 | 8 | DataLayout |
| `f64` | 8 | 8 | DataLayout |
| `i1` | 1 | 1 | DataLayout |

While these happen to agree today, hardcoding assumes target homogeneity. Using `DataLayout` is future-proof for any target LLVM supports.

- [x] Find where alignment values are specified for load/store instructions in codegen
- [x] Replace hardcoded constants with `DataLayout` ABI alignment queries (NOT preferred alignment)
- [x] If inkwell doesn't expose `DataLayout::getABITypeAlign()`, use `module.get_data_layout()` + LLVM C API binding
- [x] Add explicit test: emit a struct load, assert emitted alignment == ABI alignment for the field type on x86-64
- [x] Add explicit test: same assertion for aarch64 target triple (even if cross-compiling IR only)
- [x] Verify on x86-64: Journey 4 `@_ori_area` loads use `align 8` for i64 fields
- [x] Verify on x86-64: Journey 12 variant tag stores use `align 8`
- [x] Run `opt -passes=verify` on generated IR — 0 alignment warnings
- [x] Verify `./test-all.sh` green

---

## 04.2 Completion Checklist

- [x] All load/store alignment values derived from LLVM DataLayout ABI alignment (no hardcoded constants)
- [x] Explicit test: emitted alignment == ABI alignment for i64 on x86-64 target
- [x] Explicit test: emitted alignment == ABI alignment for i64 on aarch64 target (IR-only cross-compile)
- [x] `opt -passes=verify` on generated IR for all 12 journeys — 0 alignment warnings
- [x] `./test-all.sh` green
- [x] No alignment-related faults when running on ARM (if testable)

**Exit Criteria:** All load/store alignment values use ABI alignment from DataLayout. Explicit assertions verify alignment correctness for both x86-64 and aarch64. `opt -passes=verify` reports 0 issues on all 12 journey programs. No hardcoded `align N` constants remain in the codegen emission helpers.
