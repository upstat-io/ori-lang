---
section: "04"
title: "IR Readability"
status: not-started
goal: "LLVM struct types use human-readable names (%ori.Point, not %ori.3)"
inspired_by:
  - "Rust rustc_codegen_llvm type naming (compiler/rustc_codegen_llvm/src/type_.rs)"
depends_on: []
sections:
  - id: "04.1"
    title: "Named Struct Types"
    status: not-started
  - id: "04.2"
    title: "Completion Checklist"
    status: not-started
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

- [ ] Investigate why `pool.struct_name(idx)` returns `None` for Journey 5's `Point` struct
  - Add tracing: `tracing::debug!("type_name: idx={idx:?}, struct_name={:?}", pool.struct_name(idx))`
  - Run Journey 5 program with `ORI_LOG=ori_llvm=debug`
  - Check if the struct name is stored in the Pool at all

- [ ] Fix the root cause:
  - If struct name isn't stored: ensure type checker or canonicalizer stores it in Pool
  - If cross-pool idx: re-intern the idx into the correct pool (see MEMORY.md cross-pool section)
  - If naming logic bug: fix `type_name()` to resolve correctly

- [ ] Test: compile `struct Point { x: int, y: int }` — IR shows `%ori.Point` not `%ori.3`
- [ ] Test: compile enum — IR shows `%ori.Color` not `%ori.Enum.5`
- [ ] Verify no regressions: `./llvm-test.sh`

---

## 04.2 Completion Checklist

- [ ] All user-defined struct types use `%ori.{Name}` in generated IR
- [ ] All user-defined enum types use `%ori.{Name}` in generated IR
- [ ] Fallback to index-based names only for truly anonymous types (if any exist)
- [ ] `./llvm-test.sh` green
- [ ] `./llvm-clippy.sh` green

**Exit Criteria:** Journey 5 program IR contains `%ori.Point` (or equivalent human-readable name) instead of `%ori.3`. All named types in the test suite use readable names.
