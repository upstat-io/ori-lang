---
section: "04"
title: "IR Readability"
status: complete
goal: "LLVM struct types use human-readable names (%ori.Point, not %ori.3)"
inspired_by:
  - "Rust rustc_codegen_llvm type naming (compiler/rustc_codegen_llvm/src/type_.rs)"
depends_on: []
sections:
  - id: "04.1"
    title: "Named Struct Types"
    status: complete
  - id: "04.2"
    title: "Completion Checklist"
    status: complete
---

# Section 04: IR Readability

**Status:** Not Started
**Goal:** LLVM struct types use human-readable names like `%ori.Point` instead of auto-generated names like `%ori.3`. This is purely cosmetic — it doesn't affect codegen or runtime behavior, but significantly improves IR readability during debugging.

**Context:** Journey 5 found that struct types use opaque names (`%ori.3` instead of `%Point`). The naming infrastructure already exists in `type_info/mod.rs` — `type_name()` tries `pool.struct_name(idx)` first and falls back to `"ori.{Fallback}.{idx}"`. The fallback path is being hit too often.

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/type_.rs`: Named struct types always use the Rust type name (e.g., `%"std::option::Option<i32>"`).

**Depends on:** None (fully independent).

---

## 04.1 Named Struct Types

**File(s):** `compiler/ori_llvm/src/codegen/type_info/mod.rs`

**Finding #8** (LOW): `type_name()` (lines 1015-1031) falls back to index-based names when `pool.struct_name(idx)` returns `None`.

**Root cause investigation needed:**
- Why does `pool.struct_name(idx)` return `None` for user-defined structs?
- Is the struct name not being stored in the Pool during type checking?
- Or is the `Idx` being used from a different pool (cross-pool issue)?

- [x] Investigate why `pool.struct_name(idx)` returns `None` for Journey 5's `Point` struct
  - **Root cause**: `pool.struct_name(idx)` actually returns `Name` correctly, but
    `type_name()` called `name.raw()` which returns the raw `u32` packed interned ID
    (shard + local index), not the human-readable string
  - Fix: thread `StringInterner` through `TypeLayoutResolver` and use `try_lookup()`

- [x] Fix the root cause:
  - Added optional `interner: Option<&'a StringInterner>` field to `TypeLayoutResolver`
  - Updated `new()` to accept the interner parameter
  - Added `resolve_name()` method using `interner.try_lookup()` with numeric fallback
  - Fixed `type_name()` to use `resolve_name()` instead of `name.raw()`
  - Updated 54 call sites (1 production + 53 tests)

- [x] Test: compile `struct Point { x: int, y: int }` — IR shows `%ori.Point` not `%ori.3`
- [x] Test: compile enum — IR shows `%ori.Color` not `%ori.Enum.5`
- [x] Verify no regressions: `./llvm-test.sh` — all 1460 tests pass

---

## 04.2 Completion Checklist

- [x] All user-defined struct types use `%ori.{Name}` in generated IR
- [x] All user-defined enum types use `%ori.{Name}` in generated IR
- [x] Fallback to index-based names only for truly anonymous types (if any exist)
- [x] `./test-all.sh` green (10184 passed, 0 failed)
- [x] `./llvm-test.sh` green (1460 passed, 0 failed)
- [x] `./llvm-clippy.sh` green

**Exit Criteria:** Journey 5 program IR contains `%ori.Point` (or equivalent human-readable name) instead of `%ori.3`. All named types in the test suite use readable names.
