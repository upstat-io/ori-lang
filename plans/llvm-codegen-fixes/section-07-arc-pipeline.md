---
section: "07"
title: "ARC Pipeline"
status: not-started
goal: "One drop function per unique canonical type; no duplicate identical functions"
depends_on: []
sections:
  - id: "07.1"
    title: "Fix M12 — Deduplicate drop functions"
    status: not-started
  - id: "07.2"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: ARC Pipeline

**Status:** Not Started
**Goal:** Each unique canonical type generates exactly one drop function. No duplicate identical drop functions in generated IR.

**Context:** J10 showed 3 identical drop functions (`_ori_drop$202`, `_ori_drop$202.1`, `_ori_drop$202.2`) for `[int]` — all calling `ori_rc_free(ptr, i64 24, i64 8)`. Each list literal gets its own copy despite having the same layout.

---

## 07.1 Fix M12 — Deduplicate Drop Functions

**Journey:** J10 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/` (drop function generation)

```llvm
; Current — 3 identical functions:
define void @"_ori_drop$202"(ptr %0) { call @ori_rc_free(ptr, i64 24, i64 8); ret void }
define void @"_ori_drop$202.1"(ptr %0) { ... identical ... }
define void @"_ori_drop$202.2"(ptr %0) { ... identical ... }
```

**Fix:** Deduplicate by canonical drop-function identity, NOT by `(alloc_size, alloc_align)`. Two types can share layout (size + align) but require different drop behavior — e.g., `[str]` needs recursive ARC traversal of elements while `[int]` does not, even if both have the same allocation size. Keying by layout would alias incompatible drop logic and cause use-after-free or leaked memory.

The existing `drop_fn_cache` already keys by type index (`Idx`), which is correct for type identity. The bug is that multiple pool entries for the *same concrete type* (e.g., three `[int]` list literals) get different `Idx` values, generating duplicate drop functions.

**Correct key:** Mangled type name (the same string used for `_ori_drop$<mangled>` naming). Types with identical mangled names have identical drop behavior by construction. This deduplicates within a type while keeping different types separate.

- [ ] Find where drop functions are generated in the ARC emitter (`drop_gen.rs`)
- [ ] Change cache key from `Idx` to mangled type name (`String` or interned `Name`)
- [ ] Before generating a new drop function, check if a function with that mangled name already exists
- [ ] Verify: Journey 10 generates exactly 1 drop function for `[int]`
- [ ] Verify: Programs with `[str]` + `[int]` generate separate drop functions (different traversal logic)
- [ ] Verify: Struct types with nested ARC fields get distinct drop functions per struct type

---

## 07.2 Completion Checklist

- [ ] Each unique canonical type (by mangled name) produces exactly one drop function
- [ ] Multi-type programs (strings + lists) still have correct separate drop functions
- [ ] Types with same layout but different drop logic (e.g., `[str]` vs `[int]`) are NOT merged
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` — 0 errors

**Exit Criteria:** Journey 10 IR contains exactly 1 `_ori_drop` function for list type (not 3).
