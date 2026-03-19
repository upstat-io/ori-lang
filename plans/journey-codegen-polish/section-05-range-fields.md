---
section: "05"
title: "Range Unused Field Extraction"
status: complete
reviewed: true
goal: "Skip extraction of range fields that are compile-time constants and unused in the optimized bounds check"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Conditional field extraction in for_range.rs"
    status: complete
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Range Unused Field Extraction

**Status:** Complete
**Goal:** When a for-loop range has compile-time constant `step` and `inclusive` values, skip extracting the `inclusive` field since the bounds check comparison is already specialized (`<` vs `<=`).

**Context:** J07's `@sum_for` uses `1..=n` (inclusive range with step=1). The codegen at `for_range.rs:85-88` unconditionally extracts all 4 range fields (start, end, step, inclusive). When step=1 and inclusive=1, the bounds check is specialized to `sle` (signed less-or-equal) at lines 116-155, and the extracted `inclusive` value (`%proj.3`) is never referenced. This costs 1 unnecessary `extractvalue` instruction.

**Depends on:** None.

---

## 05.1 Conditional field extraction in for_range.rs

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_loops/for_range.rs` (lines 85-88)

The four `emit_project()` calls extract all fields unconditionally:
```rust
let start = self.builder.emit_project(Idx::INT, iter_val, 0, None);
let end = self.builder.emit_project(Idx::INT, iter_val, 1, None);
let step = self.builder.emit_project(Idx::INT, iter_val, 2, None);
let inclusive = self.builder.emit_project(Idx::INT, iter_val, 3, None);  // Sometimes unused
```

When the step and inclusive values are compile-time constants, the specialized bounds check (lines 116-155) doesn't use the `inclusive` variable. The extraction can be deferred to the fallback path (lines 153-154) that calls `emit_general_range_condition()`.

- [x] Defer the `inclusive` ARC IR `Project` instruction to the general-case arm. **Implemented Approach A** (2026-03-19): Added `pub fn get_field_literal_int()` to `ArcIrBuilder` (builder/mod.rs), replaced `get_literal_int(inclusive)` with `get_field_literal_int(iter_val, 3)` in `for_range.rs`, deferred `emit_project` for field 3 to the `_ =>` general-case arm. **IMPORTANT**: The `get_literal_int(inclusive)` call at line 95 requires the `inclusive` ArcVarId to exist — it traces the `Project → Construct` chain to determine if the field is a constant. Two approaches:
  - **Approach A** (recommended): Add a public `get_field_literal_int(base_var, field_index) -> Option<i64>` method to `ArcIrBuilder` (builder/mod.rs). It would call the existing private `get_construct_arg(base_var, field_index)` (builder/mod.rs:201-213), which traces `base_var → Construct → args[field] → get_literal_int(arg)`. This enables querying whether a field is a compile-time literal WITHOUT emitting a Project instruction. Then at `for_range.rs:95`, replace `self.builder.get_literal_int(inclusive)` with `self.builder.get_field_literal_int(iter_val, 3)`, and remove the `emit_project` at line 88 entirely. The `inclusive` variable is only created inside the `_ =>` fallback arm where `emit_general_range_condition` needs it.
  - **Approach B**: Keep the `emit_project` but mark it as a "query-only" extraction that the ARC→LLVM emitter can skip when the result is unused. However, this leaks optimization concerns into the ARC IR.
  - **Approach C (RULED OUT)**: Keep the `emit_project` at line 88, hoping the LLVM emitter skips unused Project results. **Verified**: The LLVM emitter's `emit_project()` (instr_dispatch.rs:53) unconditionally emits `extractvalue` or GEP+load — it does NOT check if the result is used. So unused Projects DO produce instructions. Approach C does not work.
  - Before: extract all 4 fields unconditionally
  - After: extract start, end, step unconditionally; extract inclusive only in the general case (or rely on dead code elimination)
  - This saves 1 `extractvalue` instruction for the common case (constant step + inclusive)
  - **Step extraction cannot be deferred**: The latch block at `for_range.rs:205-209` unconditionally uses the `step` variable for `i + step`. Even when step is a compile-time constant (1 or -1), the latch uses the extracted ARC variable, not a literal. The LLVM optimizer constant-folds `add i64 %i, 1` at -O1+, but the ARC IR extraction is needed. Only `inclusive` (field 3) can be deferred.
  - **Implementation note**: `for_range.rs:85-88` is in `ori_arc`, not `ori_llvm`. The `emit_project` here generates ARC IR instructions, not LLVM IR. The LLVM emitter then translates `Project` to `extractvalue`. The fix is in the ARC IR lowering, not LLVM codegen. The plan header correctly lists `ori_arc/src/lower/control_flow/for_loops/for_range.rs` but the overview places Section 05 under "ARC lowering" which is correct.
  - **Sync points for Approach A**: (1) Add `pub fn get_field_literal_int(&self, var: ArcVarId, field: u32) -> Option<i64>` to `compiler/ori_arc/src/lower/builder/mod.rs`, (2) Update `for_range.rs:85-95` to use the new method and defer inclusive extraction. Two files, one crate (`ori_arc`).

- [x] Verify that the `inclusive` variable is not used elsewhere in the function after the bounds check specialization (2026-03-19). Confirmed: `inclusive` only appears in `emit_general_range_condition()`. Not in entry_args, body_args, skip_args, exit_prep_args, or latch block. All 13,312 tests pass.

- [x] Add test: `for x in 1..=5 do total += x` should emit 3 `extractvalue` instructions — `test_range_constant_inclusive_skips_proj3` in `ir_quality_loops.rs` (2026-03-19)
- [x] Add test: `for x in 0..10 do total += x` (exclusive, step=1) — covered by existing `test_range_ascending_exclusive_single_icmp` (specialization works, no proj.3 emitted) (2026-03-19)
- [x] Add test: `for x in 10..0 by -1 do total += x` (descending) — covered by existing `test_range_descending_exclusive_single_icmp` (2026-03-19)
- [x] Add test: `for x in 0..n do total += x` (runtime end) — covered by existing `test_range_ascending_exclusive_single_icmp` (uses runtime `n`) (2026-03-19)
- [x] Add negative test: `for x in start..end by s do total += x` (all runtime) — covered by existing `test_range_variable_step_general_condition` (all fields extracted, general condition emitted) (2026-03-19)
- [x] **Semantic pin**: J07 produces exit code 30 in both eval and AOT (2026-03-19). All range spec tests pass.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [x] Constant-step, constant-inclusive ranges extract only 3 fields (2026-03-19)
- [x] General (runtime) ranges still extract all 4 fields (2026-03-19)
- [x] All 4 specialized arms (step=1/excl, step=1/incl, step=-1/excl, step=-1/incl) verified — all pass in debug + release (2026-03-19)
- [x] `timeout 150 cargo t -p ori_arc` passes (debug) — 994 tests (2026-03-19)
- [x] `timeout 150 cargo t -p ori_llvm` passes (debug) — 1719 tests (2026-03-19)
- [x] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release) (2026-03-19)
- [x] `timeout 150 ./test-all.sh` green — 13,312 pass, 0 fail (2026-03-19)
- [x] `timeout 150 cargo st tests/spec/` green (range-related spec tests) (2026-03-19)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/07-loops.ori` shows `@sum_for` with 0 unused `extractvalue` instructions. J07 scores 10.0/10.
