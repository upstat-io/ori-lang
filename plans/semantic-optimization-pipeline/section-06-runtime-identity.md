---
section: "06"
title: "Runtime Identity Fixes"
status: not-started
reviewed: false
goal: "Fix runtime string and list concat to be true algebraic identities so identity elimination in Section 09 is sound"
success_criteria:
  - "ori_str_concat(x, '') returns x with rc_inc (not a fresh copy) for heap strings"
  - "ori_str_concat('', x) returns x with rc_inc (not a fresh copy) for heap strings"
  - "ori_str_concat SSO case returns SSO by value (no RC needed)"
  - "ori_list_concat_cow([], x) transfers ownership of x (no copy, no rc_inc)"
  - "ORI_TRACE_RC=1 confirms zero extra allocations for identity operations"
  - "Satisfies mission criterion: identity operations are true identities"
inspired_by:
  - "Codex round 3 analysis: strings borrow (need rc_inc), lists consume (need ownership transfer)"
  - "ori_rt/src/string/ops.rs line 194-199 (current copy behavior)"
  - "ori_rt/src/list/cow_sort/mod.rs line 143-161 (current list identity behavior)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Fix ori_str_concat Identity"
    status: not-started
  - id: "06.2"
    title: "Fix ori_list_concat_cow Identity"
    status: not-started
  - id: "06.3"
    title: "Identity Operation Test Matrix"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Runtime Identity Fixes

**Status:** Not Started
**Goal:** Fix runtime string concat and list concat so that identity operations (concatenating with an empty value) return the original value instead of allocating a copy. This is a prerequisite for Section 08 (algebra law adoption) and Section 09 (algebraic normalization) — the compiler cannot safely eliminate `x + ""` if the runtime's `ori_str_concat` has different observable behavior (different allocation count) than just returning `x`.

**Critical distinction (from Codex consensus round 3):**
- **Strings borrow their inputs** — `ori_str_concat` takes `*const OriStr` (borrowed). To return the original, we need `rc_inc` on the retained string's heap data so it survives the caller's `rc_dec`.
- **Lists consume their inputs** — `ori_list_concat_cow` takes ownership. To return the original, we just transfer ownership (no `rc_inc`, no copy).

**Success Criteria:**

- [ ] `ORI_TRACE_RC=1` shows zero allocations for `"hello" + ""` and `"" + "hello"` (heap strings)
- [ ] `ORI_TRACE_RC=1` shows zero allocations for `[] + [1, 2, 3]` and `[1, 2, 3] + []`
- [ ] SSO strings (`"hi" + ""`) return SSO value directly (no RC operations needed)
- [ ] Slice-backed strings (`s.split("x")[0] + ""`) correctly retain via `ori_str_rc_inc` (slice-aware)
- [ ] Slice-backed lists (`list[1..3] + []`) correctly transfer with slice cap preservation
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks for all identity operation test programs
- [ ] Existing `ori_rt` tests updated to reflect new behavior (any tests that assert "owned copy" for empty operand)

**Context:** Currently, `ori_str_concat` (ops.rs:194-199) handles empty operands with `return OriStr::from_bytes(a_bytes)` which creates a *new* SSO or heap string — it does not return the original. This means `x + ""` and `x` produce different heap allocations with different reference counts. For the algebraic normalization pass (Section 09) to safely rewrite `x + "" → x`, the runtime must guarantee that `x + ""` and `x` are observationally equivalent (same backing storage, same RC topology).

**Depends on:** Nothing (can start immediately).

---

## 06.1 Fix ori_str_concat Identity

**File(s):** `compiler/ori_rt/src/string/ops.rs` (lines 193-199)

Replace the `OriStr::from_bytes()` copy path with an alias-preserving return for empty operands.

- [ ] Write failing tests first in `compiler/ori_rt/src/string/ops/tests.rs`:
  - `test_concat_empty_rhs_heap_preserves_pointer`: heap string `x + ""` → result data pointer == original data pointer
  - `test_concat_empty_lhs_heap_preserves_pointer`: `"" + x` → result data pointer == x's data pointer
  - `test_concat_empty_rhs_sso_returns_sso`: SSO string `"hi" + ""` → result is SSO (no heap alloc)
  - `test_concat_empty_rhs_slice_preserves_slice`: slice string `+ ""` → result is slice-backed
- [ ] Replace lines 194-199 in `ops.rs`:
  ```rust
  // OLD: return OriStr::from_bytes(a_bytes);
  // NEW:
  if b_len == 0 {
      // Return `a` as-is. Since ori_str_concat borrows inputs,
      // we must rc_inc the retained string's heap data so it
      // survives the caller's rc_dec of the original `a`.
      if a_ref.is_sso() {
          // SSO: return by value, no RC needed
          return *a_ref;
      } else {
          // Heap or slice: rc_inc the data pointer (slice-aware via cap)
          let heap = unsafe { &a_ref.heap };
          if !heap.data.is_null() {
              ori_str_rc_inc(heap.data, heap.cap); // 2 params: data_ptr + cap
          }
          return *a_ref;
      }
  }
  if a_len == 0 {
      // Same logic for empty `a`, retain `b`
      if b_ref.is_sso() {
          return *b_ref;
      } else {
          let heap = unsafe { &b_ref.heap };
          if !heap.data.is_null() {
              ori_str_rc_inc(heap.data, heap.cap); // 2 params: data_ptr + cap
          }
          return *b_ref;
      }
  }
  ```
- [ ] Verify: `ori_str_rc_inc(data_ptr, cap)` signature — takes 2 params (confirmed: `rc/mod.rs:343` takes `*mut u8` + `i64`). The `cap` parameter enables slice-aware RC: negative cap values (SLICE_FLAG set) are handled correctly to find the original buffer's header.
- [ ] Verify: `timeout 150 cargo t -p ori_rt` passes
- [ ] Update any existing tests that assert `from_bytes` behavior for empty operands

---

## 06.2 Fix ori_list_concat_cow Identity

**File(s):** `compiler/ori_rt/src/list/cow_sort/mod.rs` (lines 128-161)

The `[] + x` path (lines 143-161) already handles the unique case correctly (takeover, O(1)). Fix the shared/slice case to also be a true identity — transfer ownership rather than copying.

- [ ] Write failing tests first in `compiler/ori_rt/src/list/cow_sort/tests.rs` (or `compiler/ori_rt/src/tests.rs`):
  - `test_concat_empty_lhs_shared_transfers`: `[] + shared_list` → result is the shared list (rc_inc, not copy)
  - `test_concat_empty_lhs_slice_transfers`: `[] + slice_list` → result preserves slice encoding
  - `test_concat_empty_rhs_unique_noop`: `unique_list + []` → result is the list unchanged (already works)
- [ ] Fix the shared/slice path for `list1` empty (cow_sort/mod.rs ~line 143-161):
  ```rust
  // For [] + x: transfer ownership of x regardless of sharing status.
  // Since list concat CONSUMES both inputs, we just write x's {len, cap, data}
  // to out_ptr and do NOT dec x (the caller already transferred ownership).
  // We DO dec the empty list1 if it has storage.
  if len1 == 0 {
      // Write list2 fields to output
      write_list_fields(out_ptr, data2, len2, cap2);
      // Dec empty list1 if it owns storage (cap != 0 and not null)
      if cap1 != 0 && !data1.is_null() {
          ori_buffer_rc_dec(data1, std::ptr::null(), 0);
      }
      return;
  }
  ```
- [ ] The `x + []` path (lines 136-141) already returns list1 unchanged — verify it still works
- [ ] Verify: `timeout 150 cargo t -p ori_rt` passes
- [ ] Verify: `ORI_CHECK_LEAKS=1` reports zero leaks for identity operations

---

## 06.3 Identity Operation Test Matrix

**File(s):** `tests/spec/expressions/identity_operations.ori` (NEW), `compiler/ori_llvm/tests/aot/identity_ops.rs` (NEW)

Comprehensive test matrix verifying identity operations produce correct results and correct RC behavior.

- [ ] Create Ori spec test `tests/spec/expressions/identity_operations.ori` (**NOTE: MUST include `use std.testing { assert_eq }` — not in prelude**):
  - `"hello" + ""` == `"hello"` (str right identity)
  - `"" + "hello"` == `"hello"` (str left identity)
  - `[1, 2, 3] + []` == `[1, 2, 3]` (list right identity)
  - `[] + [1, 2, 3]` == `[1, 2, 3]` (list left identity)
  - `"" + ""` == `""` (both empty)
  - `[] + []` == `[]` (both empty)
- [ ] Create AOT test `compiler/ori_llvm/tests/aot/identity_ops.rs`:
  - Test RC balance: `ORI_TRACE_RC=1` shows balanced inc/dec for identity ops
  - Test with different element types: `[str]`, `[Option<int>]`, `[[int]]` (nested)
  - Test with SSO vs heap strings
  - Run in both debug and release (`cargo t -p ori_llvm` + `cargo t -p ori_llvm --release`)
- [ ] Create Valgrind test `tests/valgrind/identity_ops.ori`:
  - Exercise all identity patterns with Valgrind to catch use-after-free or leaks

**Matrix dimensions:**
- Types: `str` (SSO), `str` (heap), `str` (slice), `[int]`, `[str]`, `[Option<int>]`, `[[int]]` (nested list)
- Patterns: `x + empty`, `empty + x`, `empty + empty`, `x + empty` where x is shared, `x + empty` where x is unique
- Semantic pin: `ORI_TRACE_RC=1` output for `"hello" + ""` shows zero allocations (only passes with the fix)
- Negative pin: NOT testing float identity (excluded by design)

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] `ori_str_concat` returns alias for empty operands (heap: rc_inc, SSO: value copy, slice: slice-aware rc_inc)
- [ ] `ori_list_concat_cow` transfers ownership for `[] + x` (no copy for shared/slice)
- [ ] Identity test matrix covers all type × pattern combinations
- [ ] `ORI_TRACE_RC=1` confirms zero extra allocations
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks
- [ ] Valgrind test passes
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — runtime identity affects AOT binaries, release must be tested)
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
  - [ ] Section 08 `depends_on` verified (runtime identity must land before stdlib law adoption)
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ORI_TRACE_RC=1 ./binary` shows zero allocations for all identity operations. Valgrind clean. All existing tests pass (updated where behavior changed). The runtime now implements algebraic identity correctly.
