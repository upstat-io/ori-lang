---
section: "02"
title: "Dead Aggregate Load Elimination"
status: not-started
reviewed: false
goal: "Eliminate unnecessary aggregate loads of borrowed parameters that are only forwarded by pointer"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Skip aggregate load when parameter is pointer-forwarded only"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Dead Aggregate Load Elimination

**Status:** Not Started
**Goal:** When a borrowed parameter (passed by pointer) is only forwarded to runtime functions that also take the parameter by pointer, skip the aggregate load entirely. The loaded SSA value is never used — only the original pointer is.

**Context:** Journeys J16 and J17 emit `%param.load = load { i64, i64, ptr }, ptr %param_ptr` instructions that are never referenced. The field usage pre-scan (`scan_used_fields()` at `emit_function.rs:144`) and `load_struct_selective()` calls (emit_function.rs:190-224) already perform selective loading based on field usage. The `borrowed_param_ptrs` map (emit_function.rs:215) stores the original pointer for forwarding. The mechanism already handles unused params (lines 200-208 pass an empty field set), but the loaded zero-initialized value is still bound via `def_var_repr()` at line 210 even when the parameter is only forwarded by pointer to runtime calls (e.g., `ori_str_len` takes a `ptr` parameter, not individual fields).

**Depends on:** None.

---

## 02.1 Skip aggregate load when parameter is pointer-forwarded only

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs`

The `scan_used_fields()` pre-scan (field_scan/mod.rs) identifies which struct fields are actually used per variable. **However**, the scan is conservative: `Apply { args }` (field_scan/mod.rs:105-110) marks all argument variables as needing all fields (`None`), even when the LLVM emitter will forward the pointer directly via `borrowed_param_ptrs` without using the loaded value. This means the `used_fields` map will report "all fields needed" for parameters that are only pointer-forwarded, and `load_struct_selective()` loads all fields.

Two approaches:
1. **Smarter scan**: Teach `scan_used_fields` to detect when a variable is only used as an `Apply` arg to a callee that takes it by pointer (runtime functions with `Ty::Ptr` params). This requires knowing the callee ABI at scan time.
2. **Emission-level optimization**: At the LLVM emission level, detect when a param's loaded value is never referenced after binding (the `borrowed_param_ptrs` map handles all uses). This avoids changing the scan but requires tracking whether the loaded SSA value is actually used.

- [ ] Choose approach and implement. Approach 2 is simpler: in the Indirect/Reference arm (lines 184-216), after the load+bind, check if the loaded value would ever be referenced. If the variable is only used via `borrowed_param_ptrs` forwarding, the loaded value is dead
  - The `borrowed_param_ptrs` map (line 215) already stores the original pointer, so pointer-forwarding continues to work
  - Currently lines 193-198 load with `selective.as_ref()` which for `Some(&None)` becomes `None` (load all fields) — this is the dead load
  - The `def_var_repr()` call at line 210 binds the loaded value, but it is never referenced by any LLVM instruction
  - **Root cause in field_scan**: `scan_used_fields()` at `field_scan/mod.rs:105-109` marks ALL Apply/PartialApply/Construct/Reuse args via `mark_all_slice`, which sets `None` (all fields needed). This means `used_fields.get(&param.var)` returns `Some(None)` for any param used in any Apply, even if the LLVM emission only needs the pointer. The scan doesn't distinguish "used as Apply arg" from "used as value" — it treats both as "all fields needed."
  - **For approach 1**: The scan would need access to callee ABI info (which callee takes the param by pointer vs by value). This info is available in `codegen_ctx.functions` but not passed to `scan_used_fields`.
  - **For approach 2**: Skip the load entirely when the param variable is ONLY used in Apply/Invoke instructions where the emitter will forward via `borrowed_param_ptrs`. This can be checked after emission: if the loaded SSA value was never referenced by any LLVM instruction, the load is dead. However, LLVM's dead code elimination at -O1+ removes these anyway — the value is only in -O0 debug builds. Consider whether the optimization is worth the complexity.

- [ ] Verify that removing the `def_var_repr()` call does not break downstream code that assumes the var is defined. In the ARC IR, these variables appear in `Apply` args, which the emitter resolves via `self.var(arg)`. If the var is not defined, `self.var()` would return `None`/poison. The emitter must use `borrowed_param_ptrs` for pointer-forwarded args instead.
  - **Specific check**: Trace all `self.var(param.var)` call sites to confirm they all go through the `apply_param_passing_with_forwarding` path (which checks `borrowed_param_ptrs` first) rather than raw `self.var()`. If ANY path resolves the var without checking `borrowed_param_ptrs`, removing the `def_var_repr` would produce poison/undef values.
  - **RcInc/RcDec**: If the param variable has RC operations emitted by the ARC pipeline, those use `self.var()` directly. Borrowed params should not have RcInc/RcDec (they're borrowed), but verify this with a `debug_assert!`.

- [ ] Add test in `compiler/ori_llvm/tests/aot/ir_quality_codegen.rs`: function taking `str` parameter that only calls `.length()` (forwarded by pointer) should emit 0 aggregate loads (no `%param.load` in the IR)
- [ ] Add test: function taking `str` parameter that uses it BOTH by pointer (runtime call) AND by value (e.g., string concatenation) — this param MUST still be loaded
- [ ] Add test: function taking `[int]` parameter forwarded to `ori_list_len` — same pattern as str
- [ ] **Semantic pin**: function with ONLY pointer-forwarded params produces correct runtime output (not just correct IR shape)

- [ ] Verify J16's `@get_len` and `@longer` no longer emit dead `%param.load` instructions

---

## Cleanup

- [ ] **[WASTE]** `field_scan/mod.rs:23-26` — The `#[expect(clippy::too_many_lines)]` on `scan_used_fields` is justified by the function's instruction-walking scope, but if approach 1 (smarter scan) is chosen and the function grows, consider extracting per-instruction-type handlers into helper functions. Currently 143 lines — monitor during implementation.

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Parameters forwarded only by pointer emit zero aggregate load instructions
- [ ] Parameters used both by pointer AND by value still emit correct loads
- [ ] `timeout 150 cargo t -p ori_llvm` passes (debug)
- [ ] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] J16's `@get_len` is 2 instructions (call + ret), not 3
- [ ] `debug_assert!` that borrowed params have no RcInc/RcDec emitted (validates the assumption that borrowed params can skip loading)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/16-fat-ownership-transfer.ori` shows no `%param.load` in `@get_len` or `@longer`.
