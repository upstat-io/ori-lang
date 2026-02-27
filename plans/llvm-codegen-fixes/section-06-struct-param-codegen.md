---
section: "06"
title: "Struct & Param Codegen"
status: not-started
goal: "Partial field access loads only needed fields; iterator loop avoids unnecessary Option tuple"
depends_on: []
sections:
  - id: "06.1"
    title: "Fix M6 — Lazy struct load for partial field access"
    status: not-started
  - id: "06.2"
    title: "Fix M13 — Eliminate unnecessary Option tuple in iterator"
    status: not-started
  - id: "06.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Struct & Param Codegen

**Status:** Not Started
**Goal:** Functions that access only some struct fields load only those fields. Iterator loops check the `i8` result directly without building an intermediate `{ i64, i64 }` tuple.

**Context:** J4 showed that `area(r: Rect)` loads all 4 fields of Rect (including nested Point.x and Point.y) just to access `width` and `height` — 17 instructions instead of 4. J10 showed that the iterator loop builds a `{ tag, value }` tuple every iteration just to immediately destructure it.

---

## 06.1 Fix M6 — Lazy Struct Load for Partial Field Access

**Journey:** J4 (confirmed J4, J10) | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/` (load_indirect_param)

The `load_indirect_param` pattern always loads the entire struct into an SSA aggregate, then uses `extractvalue` to access fields. It doesn't know which fields will be accessed.

**Fix approach:** Load fields on demand. When a field access (`extractvalue`) is encountered, emit a GEP+load for just that field from the pointer, not the entire struct.

**Trade-off:** This requires changing from "load once, extract many" to "GEP+load per access." For functions that access ALL fields, this is slightly worse (more GEP instructions). For functions that access few fields of large structs, it's much better.

- [ ] Identify the `load_indirect_param` implementation
- [ ] Option A: Keep current approach but note it as acceptable (LLVM optimizes away unused loads)
- [ ] Option B: Load fields lazily — emit GEP+load at each `extractvalue` site
- [ ] Evaluate: does LLVM's dead load elimination already handle this?
- [ ] If LLVM handles it: mark as LOW priority

---

## 06.2 Fix M13 — Eliminate Unnecessary Option Tuple in Iterator

**Journey:** J10 | **Severity:** MEDIUM
**File(s):** `compiler/ori_llvm/src/codegen/` (for..in codegen)

The iterator loop builds an Option-like `{ i64, i64 }` tuple on every iteration:

```llvm
; Current — builds tuple, then immediately destructures:
%iter_next.has = call i8 @ori_iter_next(ptr %iter, ptr %scratch, i64 8)
%iter_next.tag = zext i8 %iter_next.has to i64
%iter_next.elem = load i64, ptr %scratch, align 4
%iter_next.0 = insertvalue { i64, i64 } undef, i64 %iter_next.tag, 0
%iter_next.1 = insertvalue { i64, i64 } %iter_next.0, i64 %iter_next.elem, 1
%proj.0 = extractvalue { i64, i64 } %iter_next.1, 0    ; check tag
%ne = icmp ne i64 %proj.0, 0
; ... later:
%proj.1 = extractvalue { i64, i64 } %iter_next.1, 1    ; get element
```

Target:
```llvm
; Direct — no intermediate tuple:
%has_next = call i8 @ori_iter_next(ptr %iter, ptr %scratch, i64 8)
%ne = icmp ne i8 %has_next, 0
; ... in loop body:
%elem = load i64, ptr %scratch, align 8    ; load only when needed
```

- [ ] Find where the iterator Option tuple is constructed in codegen
- [ ] Replace with direct `i8` check + deferred element load
- [ ] Verify: Journey 10 iterator loop has no `insertvalue`/`extractvalue` for the Option tuple
- [ ] Verify: Iterator still works correctly (same total, same iteration count)

---

## 06.3 Completion Checklist

- [ ] Struct field access approach decided (lazy load vs LLVM optimization)
- [ ] Iterator loop avoids unnecessary Option tuple construction
- [ ] `./test-all.sh` green
- [ ] Journey 4 and Journey 10 produce correct results

**Exit Criteria:** Journey 10 iterator loop body has 0 `insertvalue`/`extractvalue` instructions for the iteration Option.
