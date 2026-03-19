---
section: "02"
title: "Dead Aggregate Load Elimination"
status: complete
reviewed: true
goal: "Eliminate unnecessary aggregate loads of borrowed parameters that are only forwarded by pointer"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Skip aggregate load when parameter is pointer-forwarded only"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Dead Aggregate Load Elimination

**Status:** Complete
**Goal:** When a borrowed parameter (passed by pointer) is only forwarded to runtime functions that also take the parameter by pointer, skip the aggregate load entirely. The loaded SSA value is never used — only the original pointer is.

**Context:** Journeys J16 and J17 emit `%param.load = load { i64, i64, ptr }, ptr %param_ptr` instructions that are never referenced. The field usage pre-scan (`scan_used_fields()` at `emit_function.rs:104`) and `load_struct_selective()` calls (emit_function.rs:153-169) already perform selective loading based on field usage. The `borrowed_param_ptrs` map (emit_function.rs:175) stores the original pointer for forwarding. The mechanism already handles unused params (lines 160-168 pass an empty field set), but the loaded zero-initialized value is still bound via `def_var_repr()` at line 170 even when the parameter is only forwarded by pointer to runtime calls (e.g., `ori_str_len` takes a `ptr` parameter, not individual fields).

**Depends on:** None.

---

## 02.1 Skip aggregate load when parameter is pointer-forwarded only

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/field_scan/mod.rs`

**Approach chosen:** Hybrid — new `compute_pointer_only_params()` function in `field_scan/mod.rs` that walks the ARC IR to identify params whose loaded values are provably never needed. Uses a callback `is_forwarding_safe(callee, args) -> bool` to check whether each Apply/Invoke callee will use pointer forwarding. The callback combines `is_callee_intercepted()` (inverted) with a whitelist for `str.length`/`str.len` (which use `str_to_ptr_forwarded` — checks `borrowed_param_ptrs`).

For eligible params, `emit_function.rs` binds `const_zero_ty()` (no load instruction emitted) instead of `load_struct_selective()`. The `borrowed_param_ptrs` entry is still registered, so pointer forwarding works normally.

- [x] Choose approach and implement. Hybrid approach: `compute_pointer_only_params()` in field_scan + `is_forwarding_safe` callback in emit_function. (2026-03-19)
  - The `borrowed_param_ptrs` map (line 175) still stores the original pointer
  - Pointer-only params get `const_zero_ty()` binding — no load instruction
  - Mixed-use params (Project + Apply, or intercepted builtin) still load normally
  - **Root cause addressed**: Apply/Invoke args that go through ABI or forwarding intercepts don't need the loaded value

- [x] Verify that removing the `def_var_repr()` call does not break downstream code. (2026-03-19)
  - `def_var_repr` IS still called — it binds const_zero, so `self.var()` returns a valid value
  - `apply_param_passing_with_forwarding` checks `borrowed_param_ptrs` first → uses pointer, const_zero ignored
  - `str_to_ptr_forwarded` checks `borrowed_param_ptrs` first → same
  - Runtime call path (apply.rs:220-228) checks `borrowed_param_ptrs` first → same
  - **RcInc/RcDec**: `debug_assert!` added that pointer-only params have no RC operations — verified by 1704 AOT tests passing

- [x] Add test in `compiler/ori_llvm/tests/aot/ir_quality_codegen.rs`: `test_str_param_pointer_only_no_load` — function taking `str` parameter that only calls `.length()` should emit 0 aggregate loads (2026-03-19)
- [x] Add test: `test_str_param_mixed_use_still_loads` — function taking `str` parameter used BOTH by pointer (length) AND by value (concat) MUST still be loaded (2026-03-19)
- [x] Add test: `test_list_param_forwarded_to_length` — `[int]` param forwarded to `xs.length()` produces correct output (2026-03-19)
- [x] **Semantic pin**: `test_str_pointer_only_correct_output` and `test_multi_str_pointer_only_correct_output` — verify runtime output is correct (not just IR shape) (2026-03-19)

- [x] Verify J16's `@get_len` and `@longer` no longer emit dead `%param.load` instructions (2026-03-19)

---

## Cleanup

- [x] **[WASTE]** `field_scan/mod.rs:23-26` — `scan_used_fields` is 170 lines, within limit. `compute_pointer_only_params` is ~160 lines. Total file is ~340 lines — well under 500. No split needed. (2026-03-19)

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [x] Parameters forwarded only by pointer emit zero aggregate load instructions — `test_str_param_pointer_only_no_load`, `test_multi_str_param_pointer_only_no_loads`, `test_struct_whole_passthrough_no_load` (2026-03-19)
- [x] Parameters used both by pointer AND by value still emit correct loads — `test_str_param_mixed_use_still_loads` (2026-03-19)
- [x] `timeout 150 cargo t -p ori_llvm` passes (debug) — 453 lib + 1704 integration tests, 0 failures (2026-03-19)
- [x] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release) — 1704 tests, 0 failures (2026-03-19)
- [x] `timeout 150 ./test-all.sh` green — 13,321 tests, 0 failures (2026-03-19)
- [x] J16's `@get_len` is 2 instructions (call + ret), not 3 — verified via `ORI_DUMP_AFTER_LLVM=1` (2026-03-19)
- [x] `debug_assert!` that borrowed params have no RcInc/RcDec emitted — added in emit_function.rs, validated by all 1704 AOT tests passing in debug mode (2026-03-19)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/16-fat-ownership-transfer.ori` shows no `%param.load` in `@get_len` or `@longer`. ✅ Verified 2026-03-19.
