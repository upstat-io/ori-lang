---
section: "07"
title: "Runtime RC Protocol DRY + Correctness"
status: in-progress
reviewed: true
goal: "RC dec protocol defined once and reused by all dec functions; immortal object check present in all dec paths"
inspired_by:
  - "Swift runtime/HeapObject.cpp -- single reference counting protocol with per-type hooks"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "RC Dec Protocol Extraction"
    status: not-started
  - id: "07.2"
    title: "Immortal Object Check in Collection Decs"
    status: complete
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Runtime RC Protocol DRY + Correctness

**Status:** Not Started
**Goal:** The RC decrement protocol (null check, immortal check, atomic fetch_sub, underflow detection, acquire fence, drop call) is defined once as a shared function and reused by all dec paths. All collection buffer dec functions include immortal object checks.

**Context:** The RC dec protocol is duplicated across 5 functions in `compiler/ori_rt/src/rc/`:
- `ori_rc_dec()` in `mod.rs:196` -- generic RC dec with `Option<drop_fn>`
- `ori_buffer_rc_dec()` in `list_rc.rs:72` -- buffer dec with element cleanup
- `ori_str_rc_dec()` in `mod.rs:313` -- string-specific dec
- Map RC dec in `map_rc.rs` -- map-specific element cleanup
- Slice buffer RC dec in `list_rc.rs` -- slice-specific path

Each independently implements: null check, fetch_sub, underflow detection, acquire fence, drop/cleanup. The immortal sentinel check (`MAX_REFCOUNT`) is present in `ori_rc_dec()` (line 210-217) but may be missing from `ori_buffer_rc_dec()` and other collection-specific dec functions.

**Reference implementations:**
- **Swift** `stdlib/public/runtime/HeapObject.cpp`: Single `swift_release_n()` with per-type destructor hooks

**Depends on:** None.

**Test strategy:** This section touches safety-critical runtime code. Testing must be rigorous:
- **Matrix testing**: Run `ORI_CHECK_LEAKS=1` on all existing AOT tests after the refactor
- **Semantic pin**: Rust unit test that creates a buffer, calls `ori_buffer_rc_dec` with refcount=1, verifies the drop function is called
- **Immortal pin**: Rust unit test that creates an immortal buffer (refcount=MAX_REFCOUNT), calls each dec function, verifies refcount is unchanged
- **Release testing**: `cargo b --release && timeout 150 ./test-all.sh` -- release builds strip debug_assert, so correctness must not depend on them
- **Valgrind**: Run `diagnostics/valgrind-aot.sh` on key test programs after the refactor

---

## 07.1 RC Dec Protocol Extraction

**File(s):** `compiler/ori_rt/src/rc/mod.rs`, `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_rt/src/rc/map_rc.rs`

The core protocol (null check, immortal check, fetch_sub, underflow abort, trace, acquire fence, trigger cleanup) is repeated in every dec function with only the cleanup action differing.

- [ ] **LEAK:algorithmic-duplication** -- RC dec protocol (null check, fetch_sub, underflow detect, fence, trace) independently implemented in 5 dec functions across `rc/mod.rs`, `rc/list_rc.rs`, `rc/map_rc.rs`
- [ ] Extract a shared `rc_dec_core(data_ptr, cleanup_fn)` that handles the protocol and calls the type-specific cleanup only when refcount reaches zero
- [ ] Convert all 5 dec functions to use the shared core
- [ ] Verify both `single-threaded` and default (multi-threaded) paths are covered

---

## 07.2 Immortal Object Check in Collection Decs

**File(s):** `compiler/ori_rt/src/rc/list_rc.rs:72`, `compiler/ori_rt/src/rc/map_rc.rs`

The immortal sentinel check (`if current_rc == MAX_REFCOUNT { return; }`) is present in `ori_rc_dec()` (line 210-217) and `ori_rc_inc()` but may be missing from `ori_buffer_rc_dec()` and the map RC dec function. An immortal object (e.g., a compile-time constant list or string) whose buffer goes through `ori_buffer_rc_dec()` would have its refcount decremented despite being immortal.

- [x] **GAP** -- Immortal object check (`MAX_REFCOUNT` sentinel) was MISSING from `ori_buffer_rc_dec()`, `slice_buffer_rc_dec()`, and `ori_map_buffer_rc_dec()` (2026-04-01)
- [x] Verified: `ori_buffer_rc_dec` at `list_rc.rs:72` did NOT check `MAX_REFCOUNT`. Fixed. (2026-04-01)
- [x] Added immortal checks to all collection dec functions: `ori_buffer_rc_dec` (both paths), `slice_buffer_rc_dec`, `ori_map_buffer_rc_dec` (both paths) (2026-04-01)
- [ ] Add a test that creates an immortal buffer and verifies its refcount is unchanged after dec calls

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] RC dec protocol (null check, immortal check, atomic op, underflow detect, fence, trace) implemented in exactly one shared function
- [ ] All 5 dec functions (`ori_rc_dec`, `ori_buffer_rc_dec`, `ori_str_rc_dec`, map dec, slice dec) use the shared core
- [ ] Immortal object check present in ALL dec paths (verified by test)
- [ ] Both `single-threaded` and multi-threaded paths share the same protocol structure
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 07` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `grep -c 'fetch_sub' compiler/ori_rt/src/rc/ --include="*.rs"` returns at most 2 (one for multi-threaded core, one for single-threaded core). All dec functions delegate to the shared core. `./test-all.sh` green.
