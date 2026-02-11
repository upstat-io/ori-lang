---
section: "07"
title: Hygiene Pass
status: complete
goal: Fix remaining hygiene issues in the now-sole canonical evaluation path
sections:
  - id: "07.1"
    title: RAII Scope Guards
    status: complete
  - id: "07.2"
    title: Performance Issues
    status: complete
  - id: "07.3"
    title: Consistency Fixes
    status: complete
  - id: "07.4"
    title: Visibility Audit
    status: complete
  - id: "07.5"
    title: Completion Checklist
    status: complete
---

# Section 07: Hygiene Pass

**Status:** Complete
**Goal:** Fix remaining hygiene issues in the canonical evaluation path, now that it's the sole evaluator.

**Prerequisite:** Section 06 complete (legacy code deleted, canonical path is the only path).

**Context:** These issues were identified during the eval_v2 code review but deferred because they didn't block migration. Now that legacy code is removed, they should be cleaned up.

---

## 07.1 RAII Scope Guards

Replaced all manual `push_scope()` / `pop_scope()` pairs with RAII guards:

- [x] `eval_can_block` — Uses `self.scoped()` guard, `?` operator works naturally
- [x] `eval_can_for` — Uses `self.scoped()` per iteration; `continue`/`return`/`break` all auto-pop
- [x] `eval_can_match` — Guard evaluation uses block-scoped `self.scoped()`; arm body uses `with_match_bindings()`
- [x] `WithCapability` arm — Uses `self.with_binding()` for single-binding RAII
- [x] `eval_can_recurse` — Uses `self.with_binding()` for memoized self scope

**Impact:** Eliminated 12 manual `pop_scope()` calls across 5 functions. Early returns, continues, and error propagation no longer need explicit scope cleanup.

---

## 07.2 Performance Issues

- [x] `eval_can_for` — Replaced eager `.collect::<Vec<_>>()` with `Box<dyn Iterator<Item = Value>>`. Range and Str iterate lazily (no intermediate Vec). List and Map clone elements on demand.
- [x] `eval_can_range` — Eliminated `CanId→ExprId→CanId` roundtrip by inlining range bound evaluation directly via `eval_can()`. Deleted the now-dead `exec::expr::eval_range()` function.

**Impact:** `for x in 0..1_000_000` no longer allocates a million-element Vec upfront. Range evaluation has zero type conversion overhead.

---

## 07.3 Consistency Fixes

- [x] `BufferPrintHandler` — Switched from `std::sync::Mutex` to `parking_lot::Mutex`. Removed 4 `unwrap_or_else(PoisonError::into_inner)` calls, replaced with simple `.lock()`.

**Impact:** Consistent with rest of codebase (`SharedMutableRegistry` already uses `parking_lot`). Simpler API, no poisoning overhead.

---

## 07.4 Visibility Audit

- [x] `ModeState::call_count` — Downgraded from `pub` to private (only used in `check_budget()`)
- [x] `Interpreter::imported_arena` — Downgraded from `pub` to `pub(crate)` (only accessed within ori_eval, set via builder)
- [x] `Interpreter::print_handler` — Downgraded from `pub` to `pub(crate)` (external access via `get_print_output()`/`clear_print_output()`)
- [x] `TypeNames` struct and `TypeNames::new()` — Downgraded from `pub` to `pub(crate)` (internal type)
- [x] `exec/` modules — Kept `pub` (re-exported from `oric::eval` for test access)
- [x] Module doc updated: removed "legacy `eval(ExprId)` path is retained" wording
- [x] Clippy clean on all crates

---

## 07.5 Completion Checklist

- [x] All manual scope push/pop in `can_eval.rs` replaced with RAII guards
- [x] No eager `.collect()` before iteration in `eval_can_for`
- [x] `CanId→ExprId→CanId` roundtrip eliminated (inlined range evaluation)
- [x] `BufferPrintHandler` uses `parking_lot::Mutex`
- [x] `ModeState::call_count` is private
- [x] Visibility audit complete — tightened 4 items
- [x] `./test-all.sh` passes (8419 tests, 0 failures)
- [x] `./clippy-all.sh` passes (0 warnings)

**Exit Criteria:** Met. The canonical evaluation path is clean: RAII scope guards, lazy iteration, consistent mutex usage, minimal visibility. The evaluator codebase is ready for future development without legacy baggage.
