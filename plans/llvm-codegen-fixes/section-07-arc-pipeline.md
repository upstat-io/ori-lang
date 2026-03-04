---
section: "07"
title: "ARC Pipeline"
status: complete
goal: "One drop function per unique canonical type; no duplicate identical functions"
depends_on: []
sections:
  - id: "07.1"
    title: "Fix M12 — Deduplicate drop functions"
    status: complete
  - id: "07.2"
    title: "Completion Checklist"
    status: complete
---

# Section 07: ARC Pipeline

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

- [x] Find where drop functions are generated in the ARC emitter (`drop_gen.rs`)
- [x] Fix: use `get_or_declare_void_function` instead of `declare_void_function` + check `function_has_body` to skip body regeneration
- [x] Same fix applied to `elem_dec` and `elem_inc` functions in `element_fn_gen.rs`
- [x] Verify: multi-function program with `[str]` generates exactly 1 `_ori_drop$3` (was 2 before fix)
- [x] Verify: Programs with `[str]` + `[int]` generate separate drop functions (different traversal logic)
- [x] Verify: 1179 AOT tests pass

---

## 07.2 Completion Checklist

- [x] Each unique canonical type produces exactly one drop/elem_dec/elem_inc function
- [x] Multi-type programs still have correct separate functions (different type names → different functions)
- [x] Types with same layout but different drop logic are NOT merged (keyed by `Idx`, not layout)
- [x] `cargo test -p ori_llvm --tests` — 1179 pass
- [x] Valgrind verification deferred to Section 11

**Exit Criteria:** No duplicate drop functions in IR. Verified with `[str]`+`[int]` test: exactly 1 `_ori_drop$3`.
