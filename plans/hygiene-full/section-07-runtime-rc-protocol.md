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
  status: resolved
  updated: 2026-04-01
sections:
  - id: "07.1"
    title: "RC Dec Protocol Extraction"
    status: complete
  - id: "07.2"
    title: "Immortal Object Check in Collection Decs"
    status: complete
  - id: "07.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "07.N"
    title: "Completion Checklist"
    status: in-progress
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

- [x] **LEAK:algorithmic-duplication** -- RC dec protocol (null check, fetch_sub, underflow detect, fence, trace) independently implemented in 6 dec functions across `rc/mod.rs`, `rc/list_rc.rs`, `rc/map_rc.rs`, `rc/set_rc.rs` (2026-04-01)
- [x] Extract a shared `rc_dec_to_zero(data_ptr) -> bool` in `rc/mod.rs` that handles immortal check, atomic dec, underflow detection, trace logging, and acquire fence — returns whether cleanup should proceed (2026-04-01)
- [x] Convert all 6 dec functions to use the shared core: `ori_rc_dec`, `ori_buffer_rc_dec`, `slice_buffer_rc_dec`, `ori_map_buffer_rc_dec`, `ori_set_buffer_rc_dec` — `ori_str_rc_dec` already delegated to `ori_rc_dec` (2026-04-01)
- [x] Both `single-threaded` and default (multi-threaded) paths covered — unified in `rc_dec_to_zero` with `#[cfg]` gates (2026-04-01) Also fixed 2 missing immortal checks: `slice_buffer_rc_dec` (single-threaded path) and `ori_set_buffer_rc_dec` (both paths)

---

## 07.2 Immortal Object Check in Collection Decs

**File(s):** `compiler/ori_rt/src/rc/list_rc.rs:72`, `compiler/ori_rt/src/rc/map_rc.rs`

The immortal sentinel check (`if current_rc == MAX_REFCOUNT { return; }`) is present in `ori_rc_dec()` (line 210-217) and `ori_rc_inc()` but may be missing from `ori_buffer_rc_dec()` and the map RC dec function. An immortal object (e.g., a compile-time constant list or string) whose buffer goes through `ori_buffer_rc_dec()` would have its refcount decremented despite being immortal.

- [x] **GAP** -- Immortal object check (`MAX_REFCOUNT` sentinel) was MISSING from `ori_buffer_rc_dec()`, `slice_buffer_rc_dec()`, and `ori_map_buffer_rc_dec()` (2026-04-01)
- [x] Verified: `ori_buffer_rc_dec` at `list_rc.rs:72` did NOT check `MAX_REFCOUNT`. Fixed. (2026-04-01)
- [x] Added immortal checks to all collection dec functions: `ori_buffer_rc_dec` (both paths), `slice_buffer_rc_dec`, `ori_map_buffer_rc_dec` (both paths) (2026-04-01)
- [x] Immortal buffer behavior verified via shared core: `rc_dec_to_zero` includes the immortal check for all paths (both MT and ST). The immortal check was previously tested via `ori_rc_dec` Rust unit tests. With the shared core, all 6 dec functions now inherit the same protection. (2026-04-01)

---

## 07.R Third Party Review Findings

- [x] `[TPR-07-001][medium]` `compiler/ori_rt/src/tests.rs:377` — Section 07 marks immortal-buffer coverage as verified, but the test suite only exercises `ori_rc_inc`/`ori_rc_dec` on an immortal allocation.
  Resolved: Fixed on 2026-04-01. Added 4 new tests: `buffer_rc_dec_skips_at_max_refcount`, `map_buffer_rc_dec_skips_at_max_refcount`, `set_buffer_rc_dec_skips_at_max_refcount`, `slice_buffer_rc_dec_skips_at_max_refcount`. Each test verifies: (1) refcount remains MAX_REFCOUNT after collection dec call, (2) cleanup functions are NOT called on immortal objects. All 6 immortal-path tests pass.
- [x] `[TPR-07-002][high]` `compiler/ori_rt/src/rc/elem_header.rs:127` — `store_elem_count` uses a plain `slot.write()` which creates a data race when two threads drop references to the same buffer concurrently.
  Resolved: Fixed on 2026-04-01. Made `store_elem_count`, `load_elem_count`, and `load_elem_count_const` use `AtomicI64` with `Relaxed` ordering on the multi-threaded path (`#[cfg(not(feature = "single-threaded"))]`). `Relaxed` is sufficient because the refcount's `Release`/`Acquire` pair in `rc_dec_to_zero` already provides the happens-before relationship. Single-threaded path remains plain read/write.
- [x] `[TPR-07-003][high]` `compiler/ori_rt/src/lib.rs:336` — `ori_args_from_argv()` defers the `[str]` buffer's `elem_dec_fn`, but list slice/COW helpers copy the current header destructor into derived buffers.
  Resolved: Fixed on 2026-04-01. Added `ori_str_elem_dec` runtime function (takes `*mut u8` → OriStr, SSO-aware, calls `ori_str_rc_dec` on heap strings). `ori_args_from_argv` now stores `Some(ori_str_elem_dec)` via `store_elem_dec_fn` at construction time. COW/slice propagation paths will now correctly copy the destructor into derived buffers.

---

## 07.N Completion Checklist

- [x] RC dec protocol (null check, immortal check, atomic op, underflow detect, fence, trace) implemented in exactly one shared function (2026-04-01) `rc_dec_to_zero()` in `rc/mod.rs`
- [x] All 6 dec functions (`ori_rc_dec`, `ori_buffer_rc_dec`, `slice_buffer_rc_dec`, `ori_str_rc_dec`, `ori_map_buffer_rc_dec`, `ori_set_buffer_rc_dec`) use the shared core (2026-04-01) `ori_str_rc_dec` delegates through `ori_rc_dec`; all others call `rc_dec_to_zero` directly
- [x] Immortal object check present in ALL dec paths (2026-04-01) Fixed 2 missing: `slice_buffer_rc_dec` (ST) and `ori_set_buffer_rc_dec` (both paths)
- [x] Both `single-threaded` and multi-threaded paths share the same protocol structure (2026-04-01) Unified in `rc_dec_to_zero` with `#[cfg]` gates
- [x] `timeout 150 ./test-all.sh` passes with zero regressions (2026-04-01) 14,933 passed, 0 failed
- [x] `./clippy-all.sh` passes (2026-04-01)
- [x] Plan annotation cleanup (2026-04-01) No hygiene-full section 07 annotations in source code
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `grep -c 'fetch_sub' compiler/ori_rt/src/rc/ --include="*.rs"` returns at most 2 (one for multi-threaded core, one for single-threaded core). All dec functions delegate to the shared core. `./test-all.sh` green.
