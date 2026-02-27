---
section: "07"
title: "ARC Pipeline"
status: not-started
goal: "One drop function per unique type layout; no duplicate identical functions"
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
**Goal:** Each unique type layout generates exactly one drop function. No duplicate identical drop functions in generated IR.

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

**Fix:** Maintain a map from `(alloc_size, alloc_align)` → drop function. Before creating a new drop function, check if one already exists for this layout. If so, reuse it.

- [ ] Find where drop functions are generated in the ARC emitter
- [ ] Add a cache: `HashMap<(u64, u64), FunctionValue>` mapping (size, align) → drop function
- [ ] Before generating a new drop function, check the cache
- [ ] Verify: Journey 10 generates exactly 1 drop function for `[int]`
- [ ] Verify: Programs with multiple ARC types (strings + lists) generate separate drop functions for each unique layout

---

## 07.2 Completion Checklist

- [ ] Each unique (size, align) pair produces exactly one drop function
- [ ] Multi-type programs (strings + lists) still have correct separate drop functions
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` — 0 errors

**Exit Criteria:** Journey 10 IR contains exactly 1 `_ori_drop` function for list type (not 3).
