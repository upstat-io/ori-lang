---
section: "03"
title: "Aggregate Value Emission"
status: in-progress
goal: "Fat pointer values (str, [T], closures) are copied with aggregate load/store instead of field-by-field GEP+load+insertvalue, eliminating 3-6x instruction bloat"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Aggregate Load/Store for Fat Pointers"
    status: in-progress
  - id: "03.2"
    title: "Deduplicate ptrtoint in SSO Guard"
    status: complete
  - id: "03.3"
    title: "Single-Predecessor Block Merging for SSO Paths"
    status: in-progress
  - id: "03.4"
    title: "Dead Unwind Elimination for nounwind Callees"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 03: Aggregate Value Emission

**Status:** Not Started
**Goal:** All fat pointer value copies use aggregate operations (2 instructions: load + store) instead of field-by-field decomposition (10 instructions: 3 GEP + 3 load + 3 insertvalue + 1 store). This applies to ALL fat pointer operations across the entire compiler, not just the journey scenarios.

**Context:** J16 discovered that every `str` operation (passing to functions, returning, binding) emits a 10-instruction field-by-field copy sequence. The ideal is 2 instructions: one aggregate load and one aggregate store. This bloat affects every program that uses strings, lists, maps, closures, or any other fat pointer type. J14 also found duplicate `ptrtoint` operations in the SSO guard sequence, and redundant unconditional branches in string function CFGs.

**Reference implementations:**
- **LLVM** `docs/LangRef.rst`: `load {i64, i64, ptr}, ptr %src` is a single instruction that loads the entire aggregate
- **Rust** `compiler/rustc_codegen_llvm/src/abi.rs`: Uses `OperandValue::Immediate` for small aggregates, `Ref` for large ones — aggregate loads for by-value struct passing

---

## 03.1 Aggregate Load/Store for Fat Pointers

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs`

Currently, passing a `str` value emits:
```llvm
; ACTUAL: 10 instructions
%p0 = getelementptr inbounds {i64, i64, ptr}, ptr %src, i32 0, i32 0
%f0 = load i64, ptr %p0
%p1 = getelementptr inbounds {i64, i64, ptr}, ptr %src, i32 0, i32 1
%f1 = load i64, ptr %p1
%p2 = getelementptr inbounds {i64, i64, ptr}, ptr %src, i32 0, i32 2
%f2 = load ptr, ptr %p2
%v0 = insertvalue {i64, i64, ptr} undef, i64 %f0, 0
%v1 = insertvalue {i64, i64, ptr} %v0, i64 %f1, 1
%v2 = insertvalue {i64, i64, ptr} %v1, ptr %f2, 2
store {i64, i64, ptr} %v2, ptr %dst
```

The ideal:
```llvm
; IDEAL: 2 instructions
%v = load {i64, i64, ptr}, ptr %src
store {i64, i64, ptr} %v, ptr %dst
```

**Note on JIT safety:** The CLAUDE.md key rule says "never `load %BigStruct, ptr` for >16B in JIT — use per-field GEP+load+insert_value." This applies to JIT (FastISel) mode only. For AOT compilation (which uses the full LLVM backend), aggregate loads are safe and preferred. The fix should gate on JIT vs AOT mode.

- [x] Identify all callsites that emit field-by-field copy sequences — found in `load_struct_selective()` in `memory.rs` (NOT in `value_emission.rs` or `apply_helpers.rs` as originally suspected) (2026-03-18)
- [x] Replace with aggregate `load` + `store` for AOT mode — `load_struct_selective()` now delegates to `self.load()` which handles JIT/AOT mode correctly (2026-03-18)
- [x] Apply to all fat pointer types: `str`, `[T]`, closures, maps/sets — the fix is in `load_struct_selective()` which is the shared path for all param loading (2026-03-18)
- [x] Verify the fix applies when passing fat pointers as function arguments — confirmed via J16 IR: `@_ori_get_len` and `@_ori_longer` use aggregate loads (2026-03-18)
- [x] Measure instruction count reduction on J14 and J16 — J16 `@_ori_get_len`: 13→5 instructions, `@_ori_longer`: 27→11 instructions (2026-03-18)
- [ ] Implement **direct pointer forwarding** for borrowed parameters: when a function receives `ptr readonly dereferenceable(24)` and calls a runtime function that also takes `ptr` (e.g., `ori_str_len`), forward the parameter pointer directly instead of copying to a local alloca. J16's `@get_len` shows 5 instructions (load+store+call) where 2 would suffice (just call with param ptr)
- [ ] Implement **sret forwarding**: when `ori_str_from_raw` writes to an sret alloca and the result is immediately stored to another sret ptr (e.g., `@make_string`), pass the final destination directly to `ori_str_from_raw`. J16 shows +3 instructions from this intermediate copy
- [x] Gate the JIT vs AOT mode check — already existed: `CompilationMode::Jit/Aot` in `IrBuilder`, `load()` already gates. Fix uses this via `self.load()` delegation (2026-03-18)

---

## 03.2 Deduplicate ptrtoint in SSO Guard

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs`

J14 found that each SSO guard (the bit 63 check for inline strings) converts the same pointer to integer twice:

```llvm
; ACTUAL: 2 conversions for the same pointer
%rc_dec.p2i = ptrtoint ptr %data to i64        ; first conversion
%rc_dec.sso = and i64 %rc_dec.p2i, -9223372036854775808
%rc_dec.is_sso = icmp ne i64 %rc_dec.sso, 0
...
%rc_dec.null.p2i = ptrtoint ptr %data to i64   ; DUPLICATE
%rc_dec.is_null = icmp eq i64 %rc_dec.null.p2i, 0
```

The ideal: one `ptrtoint`, reuse the result for both SSO check and null check.

**Root cause:** `emit_sso_check` calls `ptr_to_int` at line ~267, then calls `is_null_ptr` at line ~279 which internally calls `ptr_to_int` again via `comparisons.rs:102`. The fix is to reuse the first `ptr_int` value for the null check via `icmp eq i64 %ptr_int, 0`.

- [x] Modify `emit_sso_check` in `rc_buffer_ops.rs` to reuse the `ptrtoint` result — already implemented: single `ptr_to_int` reused for both SSO flag and null check. Comment: "Reuse ptr_int for null check (avoids duplicate ptrtoint)" (pre-existing fix, verified 2026-03-18)
- [x] Verify the fix applies to all fat pointer RC operations — confirmed via J14 IR: 0 duplicate `ptrtoint` across all SSO guard sites (2026-03-18)

---

## 03.3 Single-Predecessor Block Merging for SSO Paths

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify/mod.rs`

J14 found redundant unconditional branches (`br label %bb1` at end of `bb0`) in `@sso_len` and `@heap_len`. Block `bb1` has a single predecessor (`bb0`), so the two blocks should be merged into one. This is a block merging issue (single-predecessor successor), not an empty block issue. The existing `cfg_simplify` pass performs entry block merging (added in commit d2c9a929) but may not handle the general single-predecessor case.

- [x] Verify the CFG simplification pass runs after SSO guard emission — confirmed: `simplify_cfg()` runs at function verification time, after all emission (2026-03-18)
- [x] Check whether `merge_entry_blocks()` handles the general single-predecessor case — confirmed: it only handled entry blocks. General case was missing (2026-03-18)
- [x] Confirmed `bb1` was not an entry block — `merge_entry_block()` only checked `blocks[0]` (2026-03-18)
- [x] Implement general single-predecessor successor merging — added `merge_single_predecessor_blocks()` with fixed-point iteration in `cfg_simplify/mod.rs`. Uses `LLVMInstructionRemoveFromParent` + `LLVMInsertIntoBuilder` to move instructions, then updates phi incoming blocks (2026-03-18)
- [x] Verify no redundant unconditional branches remain — confirmed via J16 IR: `@_ori_check_pass` and `@_ori_check_return` have bb0→bb1 merged (2026-03-18)

### Cleanup

- [x] **[WASTE]** `cfg_simplify/mod.rs` — Replaced `std::collections::HashMap` with `rustc_hash::FxHashMap` (2026-03-18)

---

## 03.4 Dead Unwind Elimination for nounwind Callees

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/dead_unwind.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`

J16 found that `@check_pass` invokes `@_ori_get_len` (which is `nounwind`) via `invoke` instead of `call`, generating ~12 instructions of dead landing pad code. The same pattern appears in `@check_multi`'s invoke to `@_ori_longer`.

**Codebase note:** `terminators.rs:230` already implements `InvokeMode::Call` when `is_nounwind` is true. The issue is likely that the callee is not in `ctx.nounwind_functions` — the nounwind analysis may not detect user-defined Ori functions as nounwind (it may only cover runtime functions). Check `ctx.nounwind_functions` population in `codegen/function_compiler/` or the ARC pipeline.

- [x] Verify `dead_unwind.rs` runs after nounwind analysis — confirmed: `detect_dead_unwind_blocks` checks `ctx.nounwind_functions` which is populated by `compute_nounwind_set` before emission (2026-03-18)
- [x] Determine why user-defined Ori functions not in nounwind set — ROOT CAUSE: `is_arc_function_nounwind()` in `define_phase.rs` didn't recognize builtin method calls (e.g. `Apply @length`) as nounwind. `is_rt_fn_nounwind("length")` returns `None`, and `length` is not in `nounwind_functions`. Fixed by adding `is_callee_intercepted()` check that mirrors `callee_will_be_intercepted` logic: format calls, prelude functions, and builtin methods on builtin types are all intercepted → always emit `call` → effectively nounwind (2026-03-18)
- [x] Fix the invoke emission path — the `emit_invoke` in `terminators.rs` was already correct; the fix was in the pre-analysis `is_arc_function_nounwind()`. Now `get_len` is correctly identified as nounwind during the two-pass analysis, so `check_pass`'s invoke to `get_len` is downgraded to `call` (2026-03-18)
- [x] Test: `@check_pass` uses `call` (not `invoke`) to call `@_ori_get_len` — confirmed via J16 IR + regression test `test_nounwind_callee_uses_call` (2026-03-18)
- [x] Verify no dead landing pads for nounwind callees — confirmed: `@_ori_check_pass` has no `personality`, no `landingpad`, no `resume` (2026-03-18)

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] `str` passing uses aggregate load/store (1 load instruction, not 9 GEP+load+insertvalue) — verified via J16 `@_ori_get_len` IR (2026-03-18)
- [x] `[T]` passing uses aggregate load/store — same codepath as str (load_struct_selective → load) (2026-03-18)
- [x] Closure passing uses aggregate load/store — same codepath (2026-03-18)
- [ ] Borrowed parameter forwarding: `@get_len(ptr readonly)` forwards ptr directly to `ori_str_len(ptr)` without copying to local alloca
- [ ] Sret forwarding: `@make_string` passes sret ptr directly to `ori_str_from_raw` without intermediate alloca
- [x] SSO guard emits a single `ptrtoint` per guard (not duplicate) — pre-existing fix, verified via regression test `test_sso_guard_single_ptrtoint` (2026-03-18)
- [x] No redundant unconditional branches between single-predecessor blocks — verified via J16 `@_ori_check_pass` and `@_ori_check_return` IR + regression test `test_single_predecessor_block_merged` (2026-03-18)
- [x] JIT mode still works (field-by-field fallback) — JIT guard in `IrBuilder::load()` at line 74 unchanged; `load_struct_selective` delegates to `load()` which checks mode (2026-03-18)
- [x] No dead landing pads for nounwind callees (J16 LOW-2) — fixed via `is_callee_intercepted()` in nounwind pre-analysis + regression test `test_nounwind_callee_uses_call` (2026-03-18)
- [x] `./test-all.sh` green (debug) — 12,972 tests pass, 0 failures (2026-03-18)
- [x] `./clippy-all.sh` green (2026-03-18)
- [ ] J14 re-run: rescore with extract-metrics.py
- [ ] J16 re-run: rescore with extract-metrics.py

**Exit Criteria:** `python3 .claude/skills/code-journey/extract-metrics.py` on J14 and J16 IR reports 0 unjustified instructions AND 0 CF defects AND `./test-all.sh` passes in both debug and release.
