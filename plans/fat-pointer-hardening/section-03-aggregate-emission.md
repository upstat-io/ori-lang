---
section: "03"
title: "Aggregate Value Emission"
status: not-started
goal: "Fat pointer values (str, [T], closures) are copied with aggregate load/store instead of field-by-field GEP+load+insertvalue, eliminating 3-6x instruction bloat"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Aggregate Load/Store for Fat Pointers"
    status: not-started
  - id: "03.2"
    title: "Deduplicate ptrtoint in SSO Guard"
    status: not-started
  - id: "03.3"
    title: "Single-Predecessor Block Merging for SSO Paths"
    status: not-started
  - id: "03.4"
    title: "Dead Unwind Elimination for nounwind Callees"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
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

- [ ] Identify all callsites in `value_emission.rs` and `apply_helpers.rs` that emit field-by-field copy sequences for fat pointer types
- [ ] Replace with aggregate `load` + `store` for AOT mode (keep field-by-field as fallback for JIT mode if needed)
- [ ] Apply to all fat pointer types: `str` (`{i64, i64, ptr}`), `[T]` (`{i64, i64, ptr}`), closures (`{ptr, ptr}`), maps/sets (`{i64, i64, ptr}`)
- [ ] Verify the fix applies when passing fat pointers as function arguments, returning them, binding them in `let`, and storing them in struct fields
- [ ] Measure instruction count reduction on J14 and J16
- [ ] Implement **direct pointer forwarding** for borrowed parameters: when a function receives `ptr readonly dereferenceable(24)` and calls a runtime function that also takes `ptr` (e.g., `ori_str_len`), forward the parameter pointer directly instead of copying to a local alloca. J16's `@get_len` shows 11 extra instructions from this unnecessary copy. This is a separate optimization from aggregate load/store
- [ ] Implement **sret forwarding**: when `ori_str_from_raw` writes to an sret alloca and the result is immediately stored to another sret ptr (e.g., `@make_string`), pass the final destination directly to `ori_str_from_raw`. J16 shows +10 instructions from this intermediate copy
- [ ] Gate the JIT vs AOT mode check: use `self.builder.is_jit_mode()` or equivalent flag from `CodegenContext`. If no such flag exists, add one to `CodegenContext` or `ArcIrEmitter`

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

- [ ] Modify `emit_sso_check` in `rc_buffer_ops.rs` to reuse the `ptrtoint` result from the SSO bit-test for the null check, instead of calling `is_null_ptr` which emits a second independent `ptrtoint`
- [ ] Verify the fix applies to all fat pointer RC operations, not just strings

---

## 03.3 Single-Predecessor Block Merging for SSO Paths

**File(s):** `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify/mod.rs`

J14 found redundant unconditional branches (`br label %bb1` at end of `bb0`) in `@sso_len` and `@heap_len`. Block `bb1` has a single predecessor (`bb0`), so the two blocks should be merged into one. This is a block merging issue (single-predecessor successor), not an empty block issue. The existing `cfg_simplify` pass performs entry block merging (added in commit d2c9a929) but may not handle the general single-predecessor case.

- [ ] Verify the CFG simplification pass runs after SSO guard emission
- [ ] Check whether `merge_entry_blocks()` in `cfg_simplify/mod.rs` handles the `bb0 -> bb1` pattern where `bb1` has a single predecessor — if not, extend it to merge any single-predecessor blocks, not just entry blocks
- [ ] If the pass already runs, debug why it misses these blocks in `@sso_len` and `@heap_len` — likely because `bb1` is not an entry block
- [ ] Implement general single-predecessor successor merging in the CFG simplification pass
- [ ] Verify no redundant unconditional branches remain in any function that operates on strings

### Cleanup

- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify/mod.rs:34` — Replace `use std::collections::HashMap` with `rustc_hash::FxHashMap` (deterministic hashing, less allocation overhead in a per-function pass)

---

## 03.4 Dead Unwind Elimination for nounwind Callees

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/dead_unwind.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`

J16 found that `@check_pass` invokes `@_ori_get_len` (which is `nounwind`) via `invoke` instead of `call`, generating ~12 instructions of dead landing pad code. The same pattern appears in `@check_multi`'s invoke to `@_ori_longer`.

**Codebase note:** `terminators.rs:230` already implements `InvokeMode::Call` when `is_nounwind` is true. The issue is likely that the callee is not in `ctx.nounwind_functions` — the nounwind analysis may not detect user-defined Ori functions as nounwind (it may only cover runtime functions). Check `ctx.nounwind_functions` population in `codegen/function_compiler/` or the ARC pipeline.

- [ ] Verify `dead_unwind.rs` runs after nounwind analysis is applied
- [ ] Determine why user-defined Ori functions that cannot unwind are not in `ctx.nounwind_functions` — check how the set is populated and whether it only includes runtime functions
- [ ] Fix the invoke emission path in `terminators.rs` to use `call` instead of `invoke` when the callee is known `nounwind`
- [ ] Test: `@check_pass` should use `call` (not `invoke`) to call `@_ori_get_len`
- [ ] Verify no dead landing pads remain for nounwind callees

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `str` passing uses aggregate load/store (2 instructions, not 10)
- [ ] `[T]` passing uses aggregate load/store
- [ ] Closure passing uses aggregate load/store
- [ ] Borrowed parameter forwarding: `@get_len(ptr readonly)` forwards ptr directly to `ori_str_len(ptr)` without copying to local alloca (0 extra instructions, not 11)
- [ ] Sret forwarding: `@make_string` passes sret ptr directly to `ori_str_from_raw` without intermediate alloca (3 instructions, not 13)
- [ ] SSO guard emits a single `ptrtoint` per guard (not duplicate)
- [ ] No redundant unconditional branches between single-predecessor blocks in string function CFGs
- [ ] JIT mode still works (field-by-field fallback if needed)
- [ ] No dead landing pads for nounwind callees (J16 LOW-2)
- [ ] `./test-all.sh` green (debug AND release)
- [ ] `./clippy-all.sh` green
- [ ] J14 re-run: score improves from 9.4 (control_flow: 8/10 from redundant `br` and ir_quality: 8/10 from duplicate ptrtoint -- both eliminated)
- [ ] J16 re-run: score improves from 9.4 (other_findings: 7/10 from HIGH-1 aggregate pattern + attributes_safety: 9/10 from LOW-2 invoke-to-nounwind -- both eliminated)

**Exit Criteria:** `python3 .claude/skills/code-journey/extract-metrics.py` on J14 and J16 IR reports 0 unjustified instructions AND 0 CF defects AND `./test-all.sh` passes in both debug and release.
