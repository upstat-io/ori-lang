---
section: "04"
title: "Exposed Bug Fixes"
status: in-progress
reviewed: true
goal: "Fix LLVM codegen bugs exposed by the expanded JIT test coverage"
inspired_by:
  - "LLVM sadd.with.overflow pattern for checked arithmetic (already used for add/sub/mul)"
  - "Existing COW codegen in compiler/ori_llvm/src/codegen/arc_emitter/builtins/"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Division by zero checked codegen"
    status: complete
  - id: "04.2"
    title: "COW nested map/list double-free"
    status: in-progress
  - id: "04.3"
    title: "Tuple/struct for-yield element copy"
    status: complete
  - id: "04.4a"
    title: "Negative range iteration"
    status: complete
  - id: "04.4b"
    title: "Coalesce ARC leak"
    status: complete
  - id: "04.4c"
    title: "Coalesce None path"
    status: complete
  - id: "04.5"
    title: "AIMS invoke RC analysis for test body functions"
    status: complete
  - id: "04.6"
    title: "Panic handler exception propagation"
    status: complete
  - id: "04.H"
    title: "Hygiene Observations"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04: Exposed Bug Fixes

**Status:** Not Started
**Goal:** Fix LLVM codegen bugs exposed by the expanded JIT test coverage from Sections 01-03.

**Context:** Sections 01-03 (now complete) expanded LLVM spec test coverage significantly. Several tests fail due to pre-existing LLVM codegen bugs across 4 categories: division by zero, COW double-free, tuple/struct layout corruption, and coalesce/range issues. These bugs existed before the EH work but were never exercised because the tests couldn't run through LLVM.

**Depends on:** Section 03 (JIT test execution enabled) -- COMPLETE.

**Subsection independence:** The 4 subsections touch disjoint code paths and can be worked in any order. However, per stabilization discipline (narrow the front), complete one fully before starting the next. Recommended order: 04.1 (simplest, isolated sdiv/srem fix) -> 04.4 (three small independent fixes, no crashes) -> 04.3 (FATAL type confusion crash, needs investigation) -> 04.2 (FATAL double-free crash, most complex investigation).

**First step (before any fixes):** Run `timeout 150 cargo run -q -p oric --bin ori -- test --backend=llvm tests/` and record the exact failure count as a baseline. This baseline is used in 04.N and 05.N to verify all failures are resolved.

---

## 04.1 Division by zero checked codegen

**File(s):**
- `compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs` — `emit_int_binary_op()` (lines 106-108)
- `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs` — reference: `emit_checked_binop()` pattern (reuse for div checks)

**Tests:** `test_div_by_zero`, `test_mod_by_zero`, `test_zero_div_zero`, `test_div_overflow_min` in `tests/spec/types/integer_safety.ori`

**Root cause:** LLVM's `sdiv` and `srem` produce undefined behavior on division by zero -- they don't panic. The compiler emits checked arithmetic for `+`/`-`/`*` (via `llvm.sadd.with.overflow` etc. in `checked_ops.rs`) but not for `/`/`%`/`div`. The unchecked calls are:
- `strategy.rs` line 106: `BinaryOp::Div` -> `self.builder.sdiv(lhs, rhs, "div")`
- `strategy.rs` line 107: `BinaryOp::Mod` -> `self.builder.srem(lhs, rhs, "rem")`
- `strategy.rs` line 108: `BinaryOp::FloorDiv` -> `self.builder.sdiv(lhs, rhs, "floordiv")`

**Implementation approach:** Add a `checked_div` / `checked_rem` method to `IrBuilder` in `checked_ops.rs`, following the `emit_checked_binop` pattern:
1. Emit `icmp eq rhs, 0` -> `br` to panic_bb / check_overflow_bb
2. In check_overflow_bb (for `sdiv`/`floordiv` only): emit `icmp eq lhs, MIN` + `icmp eq rhs, -1` -> `and` -> `br` to panic_bb / continue_bb
3. In panic_bb: `ori_panic_cstr("division by zero")` or `ori_panic_cstr("integer overflow in division")` + `unreachable`
4. In continue_bb: emit `sdiv`/`srem` + return result
5. Replace the three unchecked calls in `emit_int_binary_op()` with the checked versions

- [x] Write failing test matrix BEFORE implementation — 4 tests in `integer_safety.ori` verified failing (26 pass, 4 fail baseline)
- [x] Add `checked_div` and `checked_rem` methods to `IrBuilder` in `checked_ops.rs` — zero check + MIN/-1 overflow check + `sdiv`/`srem`, with `emit_panic_block` helper
- [x] Replace unchecked calls in `emit_int_binary_op()` (`strategy.rs`): Div→`checked_div`, Mod→`checked_rem`, FloorDiv→`checked_div`
- [x] Verify: 30 passed, 0 failed (debug)
- [x] Verify debug AND release: 30 passed, 0 failed (release)
- [x] Verify interpreter parity: 30 passed (interpreter)

**Matrix test dimensions:**
- Operations: div (`/`), mod (`%`), floor_div (`div`)
- Values: zero divisor, MIN/-1 overflow, near-boundary valid, normal, negative numerator/denominator
- Types: int only (float division doesn't panic -- float produces `inf`/`NaN`)

**Semantic pin:** `catch(expr: 1 / 0)` returns `Err("division by zero")` in both interpreter and LLVM.
**Negative pin:** The 4 `assert_panics` tests in `integer_safety.ori` ARE the negative pins -- they reject the old behavior where division by zero silently produced UB.

---

## 04.2 COW nested map/list double-free

**File(s):**
- `compiler/ori_rt/src/list/cow.rs` — `ori_list_push_cow()` slow path: `inc_copied_elements` (line 151) increments elements byte-copied from old buffer, but for nested collections these elements are themselves RC pointers whose inner structure also needs incrementing
- `compiler/ori_rt/src/list/cow.rs` — `propagate_elem_header()` (line 21): propagates `elem_dec_fn` from old to new buffer header
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_cow.rs` — LLVM-side COW list emission (where `inc_fn` argument is constructed)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs` — map insert codegen
- `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` — drop function generation for nested types

**Tests:** `test_map_of_lists`, `test_map_loop_build` in `tests/spec/collections/cow/nested.ori`; `test_loop_sharing` in `tests/spec/collections/cow/sharing.ori`

**Root cause (CORRECTED 2026-04-03):** ~~Shared root cause with 04.5~~ **`ori_map_get` does a shallow byte-copy of the value without `RcInc` for RC-managed value types.** The runtime's `ori_map_get` (in `ori_rt/src/map/mod.rs:264`) uses `std::ptr::copy_nonoverlapping` to copy the value from the map's internal hash table to the output buffer. For RC-managed value types like `[int]`, this creates a second pointer to the same data without incrementing the reference count. When the map is subsequently freed (via `RcDec` → elem cleanup), the inner `[int]` buffer's RC goes to 0 and it's freed. The caller still holds the shallow-copied pointer → second `RcDec` on already-freed memory → double-free.

This bug affects BOTH `@main` and test body functions (not test-body-only as previously hypothesized). Minimal reproducer: `let m: {str: [int]} = {"a": [1,2,3]}; let v = m.get("a")` — crashes when `v` is used after `m` is freed.

**Fix (2026-04-03):** Added conditional `RcInc` in `emit_map_get` (`compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs`). After `ori_map_get` returns, if the value type is RC-managed and the tag is Some (0), load the value and call `inc_value_rc` to give the caller its own reference. Uses branch: `is_some → inc_bb (RcInc) → cont_bb`.

- [x] Reproduce: `{str: [int]}` insert + access crashes with double-free (confirmed 2026-04-03)
- [x] Investigate root cause: `ORI_TRACE_RC=1` shows list buffer freed twice — once by map cleanup, once by caller cleanup. `ori_map_get` shallow byte-copy is missing RcInc. (2026-04-03)
- [x] Fix: conditional `RcInc` in `emit_map_get` for RC-managed value types on the Some path (2026-04-03)
- [x] Verify: `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/collections/cow/nested.ori` — 7 pass, 0 fail (2026-04-03)
- [x] Verify: `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/collections/cow/sharing.ori` — 9 pass, 0 fail (2026-04-03)
- [x] Verify: `ORI_CHECK_LEAKS=1` reports 0 leaks on both files (2026-04-03)
- [ ] Verify debug AND release: both test files pass with `--release`
- [ ] Verify interpreter parity: `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/nested.ori` — 0 mismatches
- [ ] Verify interpreter parity: `diagnostics/dual-exec-verify.sh tests/spec/collections/cow/sharing.ori` — 0 mismatches

**Matrix test dimensions:**
- Collection types: `{str: [int]}`, `[[int]]`, `{str: {str: int}}`
- Operations: insert, push, fork (COW), read-after-mutation
- RC patterns: single-owner mutation, shared mutation (COW path), nested drop

**Semantic pin:** A test that pushes `[1, 2]` into a `[[int]]`, reads it back, and asserts deep equality -- only passes when the inner list's RC is correctly incremented during COW copy.
**Negative pin:** The existing FATAL crash (`ori_rc_dec called on already-freed allocation`) IS the negative pin -- it rejects the old behavior where inner RC was unmanaged.

---

## 04.3 Tuple/struct for-yield type confusion crash

**File(s):**
- `compiler/ori_llvm/src/codegen/arc_emitter/` — element copy / RC codegen for compound types in for-yield
- Key files to check: `mod.rs` (emission loop), `builtins/collections/list_builtins/` (list iteration element loading), `drop_gen.rs` (drop functions for compound types)
- `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs` — `aggregate_size_with_padding()` (does it agree with `pool_type_store_size()`?)

**Tests:** `test_for_yield_tuple_padding`, `test_for_yield_tuple_two_gaps`, `test_for_yield_padded_struct`, `test_for_yield_padded_enum` in `tests/spec/types/struct_layout.ori`

**Root cause:** Running `ori test --backend=llvm tests/spec/types/struct_layout.ori` crashes with `FATAL -- ori_rc_inc called with misaligned pointer 0x74736574` (note: `0x74736574` is `"test"` in ASCII -- the string data bytes themselves). This is a type confusion bug: a string data pointer is being treated as an RC-managed allocation pointer. The underlying `pool_type_store_size()` alignment bugs (TPR-06-007/008/013) were already fixed in the repr-opt plan (Rust + interpreter tests pass), but the LLVM codegen path for for-yield with compound types containing strings still has a codegen bug where field offsets or RC operations target the wrong memory.

**Note:** Tests earlier in the file that DO NOT use for-yield (lines 1-78: `test_struct_field_access`, `test_struct_construction`, `test_struct_passed_to_fn`, `test_struct_returned`, `test_two_field_reorder`, `test_three_types`) pass correctly. The crash is specific to for-yield over lists of padded compound types (lines 127+).

**Investigation approach:**
1. Create minimal reproducer: `let items = [(true, "hello", 1, false)]; for item in items yield item`
2. `ORI_TRACE_RC=1` on the reproducer to identify which RC operation receives `0x74736574`
3. `ORI_DUMP_AFTER_LLVM=1` to see the LLVM IR and check field offsets in the for-yield element copy path
4. Check `emitter_utils.rs` `aggregate_size_with_padding` -- compare its output for `(bool, str, int, bool)` with `pool_type_store_size()` output. Disagreement means the for-yield buffer is allocated with one size but elements are stored/loaded with another, causing field misalignment.
5. Check the `elem_inc_fn` / `elem_dec_fn` function pointers for the tuple type -- if the drop function walks fields at wrong offsets, it will read string data bytes as a pointer.
6. The fix is likely in either: (a) the element size calculation used for for-yield buffer allocation, or (b) the field offsets used in the generated `elem_inc_fn`/`elem_dec_fn` for compound types.

- [x] Verify FATAL crash baseline — confirmed `0x74736574` misaligned pointer crash
- [x] Investigate — root cause: ARC `pool_type_store_size` returns original layout size (48) but LLVM struct uses reordered layout (40). for-yield `ori_list_new`/`ori_list_push` use ARC size (48) but GEP indexing uses LLVM stride (40) → elements stored at wrong offsets
- [x] Fix: added `for_yield_elem_size_types` pre-scan in `emit_function.rs` that maps elem_size `ArcVarId` → element `Idx`. In `emit_instr` (`instr_dispatch.rs`), override the ARC-emitted literal with `element_store_size(elem_ty)` from the LLVM type resolver. Also added `scan_for_yield_elem_size_types()` function.
- [x] Verify: 16 passed, 0 failed (debug)
- [x] Verify: 16 passed, 0 failed (release)
- [x] Verify interpreter parity: 16 passed (interpreter)

**Matrix test dimensions:**
- Tuple sizes: 2-element (no padding), 4-element with padding gaps
- Field types: mixed (bool+str+int), with strings (RC fields)
- Compound types: tuples, structs, enums with payload
- Patterns: for-yield collect, for-yield with field projection, destructure after collect

**Semantic pin:** A for-yield over `[(true, "test", 42, false)]` that collects and accesses the string field -- only passes when field offsets in the element copy function match the actual layout.
**Negative pin:** The existing FATAL crash (`ori_rc_inc called with misaligned pointer 0x74736574`) IS the negative pin -- it rejects the old behavior where string data bytes were treated as RC pointers.

---

## 04.4 Coalesce ARC leak + negative range + coalesce None path

These are 3 independent bugs with disjoint root causes. Work them sequentially per narrow-front discipline.

### 04.4a Negative range iteration

**File(s):**
- `compiler/ori_arc/src/lower/collections/mod.rs` — `lower_range()` (line 217): uses `i64::MAX` as sentinel for unbounded `end`
- `compiler/ori_rt/src/iterator/next.rs` — `next_range()` (line 106): bounds check logic
- `compiler/ori_rt/src/iterator/sources.rs` — `ori_iter_from_range()` (line 45): range creation

**Test:** `test_neg_step_iter` in `tests/spec/traits/iterator/infinite_range.ori`

**Root cause (CONFIRMED):** The ARC lowering for infinite ranges (`0..`) emits `i64::MAX` as the sentinel `end` value (`lower/collections/mod.rs:218`). The runtime `next_range()` treats this as a regular endpoint. For positive step, `*current < i64::MAX` works fine. But for `(0.. by -1)`, the check becomes: `step > 0` is false, so it enters the else branch: `*current > end` = `0 > i64::MAX` = false. The range is immediately exhausted, producing `[]` instead of `[0, -1, -2, -3, -4]`.

**Fix approach:** Modify `next_range()` in `compiler/ori_rt/src/iterator/next.rs` to recognize the sentinel values. When `end == i64::MAX` and `step > 0`, or `end == i64::MIN` and `step < 0`, the range is always in bounds. Also change `lower_range()` in `lower/collections/mod.rs` to emit `i64::MIN` (not `i64::MAX`) as the sentinel when the step is known negative at lower time. Since step may not be known at lower time, the runtime must handle both sentinels:
- `end == i64::MAX && step != 0`: always in bounds (ascending unbounded)
- `end == i64::MIN && step != 0`: always in bounds (descending unbounded)

Alternative (cleaner): change the runtime `IterState::Range` to use a boolean `unbounded: bool` field instead of a sentinel. This requires updating `ori_iter_from_range` signature (add `unbounded: bool` param), the LLVM runtime declaration, and the ARC lowering to emit the flag. This is a larger change but eliminates the sentinel design smell.

- [x] Verify the bug: confirmed `test_neg_step_iter` produces `[]` instead of `[0, -1, -2, -3, -4]` (13 pass, 1 fail baseline)
- [x] Implement fix: sentinel-based approach in `next_range()` (`compiler/ori_rt/src/iterator/next.rs`). When `end == i64::MAX && step < 0`, treat as unbounded (always in bounds). No API change needed.
- [x] Verify: 14 passed, 0 failed (LLVM)
- [x] Verify interpreter parity: 14 passed (interpreter)

**Semantic pin:** `(0.. by -1).take(count: 5).collect()` produces `[0, -1, -2, -3, -4]` -- only passes when unbounded negative ranges are not prematurely exhausted.
**Negative pin:** The old behavior where `(0.. by -1).take(count: 5).collect()` produces `[]` is rejected.

### 04.4b Coalesce ARC leak

**File(s):**
- `compiler/ori_arc/src/lower/expr/mod.rs` — `lower_coalesce()` (line 448): branch-based coalesce lowering
- `compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs` — `emit_coalesce()` (line 134): LLVM-side coalesce (NOTE: this is a fallback path that may be dead code since coalesce is intercepted at ARC lowering)

**Test:** `test_list_coalesce` in `tests/spec/test_coalesce_copy.ori`

**Root cause hypothesis:** The `lower_coalesce()` function (line 448-516) evaluates LHS, extracts the tag, and branches. On the Some path, it extracts the payload (or passes through if chaining). The AIMS analysis should insert `RcDec` for the LHS Option wrapper after the payload is extracted, but this may not happen correctly on the Some path -- the LHS variable is still live at the branch point and the payload is a projection from it. If the AIMS analysis doesn't realize the Option wrapper needs cleanup after projection, the wrapper leaks.

**Investigation:**
1. `ORI_CHECK_LEAKS=1` on a minimal coalesce reproducer with `Option<[int]>` to confirm the leak
2. `ORI_TRACE_RC=1` to identify which object leaks (the Option wrapper? the list?)
3. `ORI_DUMP_AFTER_ARC=1` to check if `RcDec` is emitted for the LHS on both Some and None paths
4. Check if `emit_coalesce()` in `strategy.rs` (the select-based fallback) is ever reached -- if so, it doesn't do any RC management at all

- [x] Verify the bug: confirmed `test_list_coalesce` reports ARC leak (1 allocation not freed)
- [x] Investigate: root cause is `propagate_borrowed_closure` over-conservatively marking merge block params as borrowed. See 04.5 for full analysis. Not a coalesce-specific issue.
- [x] Fix: resolved by 04.5 (`propagate_borrowed_closure` unanimity for Jump param propagation) (2026-04-03)
- [x] Verify: `ORI_CHECK_LEAKS=1` 0 leaks on `test_coalesce_copy.ori` — all 17 pass (2026-04-03)
- [x] Verify debug AND release — both pass (2026-04-03)

**Semantic pin:** `let xs: Option<[int]> = Some([1, 2, 3]); xs ?? []` returns `[1, 2, 3]` with 0 leaks -- only passes when the Option wrapper's RC is decremented after payload extraction.
**Negative pin:** The ARC leak on `test_list_coalesce` (Option wrapper not freed) rejects the old behavior.

### 04.4c Coalesce None path

**File(s):** Same as 04.4b -- `lower_coalesce()` in `compiler/ori_arc/src/lower/expr/mod.rs`

**Test:** `test_none_evaluates_default` in `tests/spec/test_coalesce_copy.ori`

**Root cause hypothesis:** The None path of `lower_coalesce()` evaluates the RHS lazily. The test `test_none_evaluates_default` uses a block expression `{evaluated = true; 99}` as the default. The assertion failure may be in the side effect (`evaluated = true` not executing) or in the value (99 not returned). Since this test involves mutable variable capture across the branch boundary, the scope restoration (`self.scope = pre_scope.clone()` at line 496) may interfere with the mutable binding.

**Investigation:**
1. Run `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/test_coalesce_copy.ori` and capture the exact assertion failure message
2. Check if the simpler `test_none_returns_default` (line 18: `opt ?? 0`) passes -- this isolates whether the issue is with the None path in general or only with side-effecting blocks
3. `ORI_DUMP_AFTER_ARC=1` on the failing test to check if the None block correctly evaluates the RHS and joins with the merge block

- [x] Verify the bug: confirmed `test_none_evaluates_default` assertion fails (15 pass, 2 fail baseline)
- [x] Investigate: `lower_coalesce` didn't merge mutable variables across branch paths — `self.scope = pre_scope` at merge block restored old scope, losing mutations from the None branch
- [x] Fix: added `merge_mutable_vars` to `lower_coalesce` (same pattern as `lower_if`). Collects `mutable_var_types`, saves `some_scope`/`none_scope`, calls `merge_mutable_vars`, includes diverged vars in jump args, rebinds at merge block.
- [x] Verify: 16 pass, 1 fail (the remaining 1 fail is `test_list_coalesce` from 04.4b — blocked by 04.5)
- [x] Verify interpreter parity: all pass (interpreter)

**Semantic pin:** `let x: Option<int> = None; let result = x ?? { side_effect(); 99 }` returns `99` with the side effect executed -- only passes when the None path correctly evaluates the RHS block and preserves scope.
**Negative pin:** The assertion failure on `test_none_evaluates_default` rejects the old behavior where the RHS block was not evaluated or scope was corrupted.

### 04.4 Combined Verification

All three bugs (04.4a, 04.4b, 04.4c) are covered by two test files: `infinite_range.ori` covers 04.4a; `test_coalesce_copy.ori` covers both 04.4b and 04.4c.

- [x] Verify: `ORI_CHECK_LEAKS=1` reports 0 leaks on `infinite_range.ori` (04.4a) — ✓ 2026-04-03
- [x] Verify: `ORI_CHECK_LEAKS=1` reports 0 leaks on `test_coalesce_copy.ori` (04.4b + 04.4c) — all 17 pass, 0 leaks (2026-04-03)
- [x] Verify debug AND release for both test files — confirmed (2026-04-03)

---

## 04.5 AIMS invoke RC analysis for test body functions

**Root cause of 04.4b (coalesce leak) AND 04.2 (COW nested double-free).** Both bugs manifest only in test body functions (compiled via immediate-emit path), never in `@main` functions. The same code works correctly outside of tests.

**File(s):**
- `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs` — `emit_terminator_rc()` (line 17): inserts RcInc for variables live at block exit
- `compiler/ori_arc/src/aims/emit_rc/helpers.rs` — `is_live_at_exit()` (line 78): checks cardinality at block exit
- `compiler/ori_arc/src/aims/emit_rc/arg_ownership.rs` — `emit_arg_ownership()`: determines `[own]` vs `[borrow]` for call args

**Root cause (CONFIRMED):** When a merge block param (from coalesce `??` or branch) holds an RC-managed value and is passed to a function call at an `Invoke` terminator with `[own]` semantics, AIMS inserts `RcInc` for the merge param (to give the callee its own reference) but does NOT insert the matching `RcDec` for the caller's original reference on either the normal or unwind path. This causes:
- **Leak**: caller's reference is never freed (coalesce case, 04.4b)
- **Double-free**: callee frees its owned reference AND the caller's cleanup code frees the same allocation (COW nested case, 04.2)

**Why test-only:** Test body functions are compiled via the immediate-emit path (`emit_arc_function` in `impls.rs`), which does NOT run the two-pass nounwind analysis. Without nounwind analysis, function calls like `assert_eq` are emitted as `Invoke` terminators (may-unwind) instead of `Apply`/`call` (nounwind). Regular functions benefit from the two-pass pipeline which marks `assert_eq` as nounwind → emitted as `call` → cleanup placed after the call (single path, no normal/unwind split) → works correctly.

**ARC IR evidence (from `@check()` void function with coalesce + assert_eq):**
```
bb3: (%11: [int])         // merge block param from coalesce
  %13: [int] = %11        // alias
  %17: [int] = Construct   // expected [1,2,3]
  RcInc %13 [HeapPtr]     // gives callee ownership — but caller's ref never freed!
  Invoke @assert_eq(%13 [own], %17 [own]) normal bb4 unwind bb5
bb4:
  Return                   // NO RcDec for %11/%13 — leaked!
bb5:
  RcDec %17 [HeapPtr]     // only expected list cleaned up
  Resume                   // %11/%13 leaked on unwind too!
```

**Investigation approach:**
1. Check `is_live_at_exit(bb3, %13)` — hypothesis: returns `true` when it should return `false` (var is last-used at the invoke, not live in successors)
2. If `is_live_at_exit` is correct, the issue is in `emit_terminator_rc`: it inserts RcInc for live-at-exit vars but should ALSO insert RcDec for the original reference on the normal path
3. Check if the alias `%13 = %11` confuses backward demand propagation — `%11` is a block param, `%13` is a local alias; they share the same RC but may have separate AIMS states
4. Compare the AIMS state map for a function compiled via two-pass (nounwind, uses `Apply`) vs immediate-emit (uses `Invoke`) for the same code

**Fix approach (two options, choose the more correct one):**

**Option A: Fix AIMS RC emission for invoke terminators (correct fix).**
In `emit_terminator_rc`, when a variable gets `RcInc` because `is_live_at_exit` is true AND it's passed to an `[own]` invoke arg: the normal-path successor (bb4) must include a corresponding `RcDec` for that variable. Currently, the unwind path's edge_cleanup handles some dec insertions, but the normal path is missing the dec.

Implementation:
1. In `emit_terminator_rc` or the subsequent edge_cleanup pass, detect: variable V has RcInc at the invoke AND is passed to an `[own]` param AND is NOT used in the normal-path successor
2. Insert `RcDec V` at the beginning of the normal-path successor (or append to the invoke's normal-path jump args as "needs cleanup")
3. Similarly for the unwind path: insert `RcDec V` in the landingpad cleanup block

**Option B: Run nounwind analysis for test body functions (simpler but narrower).**
Move test body compilation to AFTER the two-pass nounwind analysis, or run a single-pass nounwind analysis for test bodies. This converts `assert_eq` to `call` (nounwind), avoiding the invoke split entirely. Simpler but only fixes the symptom for nounwind callees — an `invoke` to a genuinely may-unwind function would still have the same bug.

- [x] Investigate: root cause is `propagate_borrowed_closure` in `borrowed_defs.rs` — marks merge block params as borrowed when ANY predecessor passes a borrowed value (Project result), but should only mark borrowed when ALL predecessors pass borrowed values. This causes `all_borrowed_defs` to include the merge param, which prevents `collect_invoke_edge_decs` Cat 2 from emitting RcDec. (2026-04-03)
- [x] Determined fix: neither Option A nor B — the actual root cause was in `propagate_borrowed_closure`, not in `emit_terminator_rc` or nounwind analysis. Fix requires unanimity for borrowed propagation through Jump params to merge blocks. (2026-04-03)
- [x] Implemented fix: changed `propagate_borrowed_closure` to pre-collect all Jump predecessors per block param and only mark borrowed when ALL incoming args are borrowed. (2026-04-03)
- [x] Verify: `ORI_CHECK_LEAKS=1 timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/test_coalesce_copy.ori` — 0 leaks, 17 pass (04.4b resolved) (2026-04-03)
- [x] Verify: `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/collections/cow/nested.ori` — 7 pass (resolved by 04.2 fix, not 04.5) (2026-04-03)
- [x] Verify: `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/collections/cow/sharing.ori` — 9 pass (resolved by 04.2 fix) (2026-04-03)
- [x] Verify: full LLVM spec test run doesn't crash: 1776 pass, 5 fail (pre-existing shift overflow checks, filed as BUG-04-029), 0 FATAL (2026-04-03)
- [x] Verify debug AND release for all affected test files: both debug and release pass all 17 coalesce tests with 0 leaks; dual-exec parity confirmed (2026-04-03)

**Matrix test dimensions:**
- Value types: `[int]`, `str`, `Option<[int]>`, `{str: [int]}` (all RC-managed types through merge+invoke path)
- Branch constructs: coalesce (`??`), `if`-`then`-`else`, `match`
- Call patterns: `assert_eq`, `assert`, user function (all may-unwind calls after merge)
- Execution contexts: test body (immediate-emit) vs `@main` (two-pass nounwind)

**Semantic pin:** `let opt: Option<[int]> = None; let a = opt ?? [1,2,3]; assert_eq(actual: a, expected: [1,2,3])` in a test body function — passes with 0 leaks. Only passes when merge block param RC is correctly managed at invoke terminators.
**Negative pin:** The current 1-allocation leak on `test_list_coalesce` and the FATAL double-free on `cow/nested.ori` reject the broken behavior.

---

## 04.6 Panic handler exception propagation

**File(s):**
- `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs` — `generate_main_wrapper()`: the LLVM main wrapper that uses invoke/landingpad
- `compiler/ori_llvm/src/codegen/function_compiler/panic_trampoline.rs` — panic trampoline generation
- `compiler/ori_rt/src/io/mod.rs` — `ori_panic`/`ori_panic_cstr` dispatch

**Test:** `panic::test_panic_handler_receives_message` in `compiler/ori_llvm/tests/aot/panic.rs`

**Root cause (CONFIRMED + FIXED 2026-04-03):** Three sub-issues found and fixed:

1. **Main wrapper missing `invoke` for no-args `@main`** — `generate_main_wrapper` only used `invoke` when `args_cleanup.is_some()`. For `@main () -> void` (no args), it fell through to `emit_main_call_direct` (plain `call`, no landingpad). The unwinder found no handler → `_URC_END_OF_STACK` (code 5). Fix: always use `invoke` when `@main` can unwind, regardless of args.

2. **Intermediate Rust frames missing `extern "C-unwind"` ABI** — `dispatch_panic` and `aot_raise_exception` used default Rust ABI, which inserts abort guards for foreign exceptions. Fix: changed both to `extern "C-unwind" fn` with `#[expect(improper_ctypes_definitions)]` (the String parameter stays in Rust frames, not actual C interop).

3. **Panic trampoline PanicInfo field ordering mismatch** — The trampoline manually constructed PanicInfo using declaration-order field indices (message=0, location=1, ...), but the compiler reorders struct fields by descending alignment then descending size (location=0, message=1, ...). Fix: use `ReprPlan::get_repr()` → `StructRepr::memory_index()` to remap declaration indices to memory indices. Also use the compiler-resolved PanicInfo LLVM type via `type_resolver.resolve()` instead of manually constructing the struct type.

- [x] Investigated: 3 root causes found (main wrapper invoke, ABI, PanicInfo layout) (2026-04-03)
- [x] Fixed: all 3 sub-issues (entry_point.rs, io/mod.rs, panic_trampoline.rs) (2026-04-03)
- [x] Verify: `timeout 60 cargo test -p ori_llvm panic::test_panic_handler_receives_message -- --test-threads=1` passes (2026-04-03)
- [x] Verify: `timeout 60 cargo test -p ori_llvm panic::test_panic_handler_re_entrancy -- --test-threads=1` passes (2026-04-03)
- [x] Verify: `timeout 60 cargo test -p ori_llvm panic:: -- --test-threads=1` — all 11 panic tests pass (2026-04-03)

**Semantic pin:** AOT binary with `@panic` handler that prints `info.message` + `@main` that panics → handler prints the message AND process exits non-zero. All 11 panic tests pass.
**Negative pin:** `_Unwind_RaiseException returned (code 5)` no longer appears — exception caught by main wrapper's landingpad.

---

## 04.H Hygiene Observations (fix along the way)

These are not blocking bugs but should be fixed when touching the relevant files:

- [ ] **Dead code in `strategy.rs`**: `emit_coalesce()` (lines 134-148), `BinaryOp::And` (line 115), and `BinaryOp::Or` (line 116) in `emit_int_binary_op()` are dead code -- `And`/`Or`/`Coalesce` are all intercepted at ARC lowering (lines 413-424 of `lower/expr/mod.rs`) and never reach the LLVM `PrimOp` emission path. The `IntInstr` strategy routing on line 28 (`BinaryOp::And | BinaryOp::Or | BinaryOp::Coalesce => OpStrategy::IntInstr`) is also dead. These should be changed to `unreachable!()` to match their actual reachability, or removed entirely.
- [ ] **Decorative banners**: `tests/spec/types/integer_safety.ori` has 18 `// ====` banner lines. Per CLAUDE.md: "Banner removal: if you touch a file with decorative banners, remove them." Replace with plain `// Section Name` comments.
- [ ] **BLOAT**: `compiler/ori_arc/src/lower/expr/mod.rs` is 642 lines (exceeds 500-line limit). If 04.4b/04.4c add code, extract `lower_coalesce`, `lower_short_circuit_and`, `lower_short_circuit_or` (~180 lines, lines 440-620) into a sibling module `lower/expr/short_circuit.rs`.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [x] **Baseline captured**: LLVM spec tests crash (COW double-free), 26 pass/4 fail in `integer_safety.ori`, 13 pass/1 fail in `infinite_range.ori`, 15 pass/2 fail in `test_coalesce_copy.ori`, FATAL crash in `struct_layout.ori`. AOT: 24 failures. (2026-04-03)
- [x] **04.1** Division by zero emits runtime check in LLVM (`checked_div`/`checked_rem` in `checked_ops.rs`; 4 tests in `integer_safety.ori` pass: `test_div_by_zero`, `test_mod_by_zero`, `test_zero_div_zero`, `test_div_overflow_min`; all 30 tests pass; debug+release+interpreter parity confirmed) (2026-04-03)
- [ ] **04.2** COW nested collection double-free fixed (all tests in `cow/nested.ori` and `cow/sharing.ori` pass; `ORI_CHECK_LEAKS=1` clean on both) — NOT resolved by 04.5 (different root cause; needs separate investigation)
- [x] **04.3** Tuple/struct for-yield type confusion fixed (all 16 tests in `struct_layout.ori` pass through LLVM without crash; debug+release+interpreter parity confirmed; fix: override ARC pool_type_store_size with LLVM struct store size via for_yield_elem_size_types pre-scan) (2026-04-03)
- [x] **04.4a** Negative range iteration works (`test_neg_step_iter` passes, all 14 tests in `infinite_range.ori` pass; fix: recognize i64::MAX sentinel for descending unbounded ranges in `next_range`) (2026-04-03)
- [x] **04.4b** Coalesce ARC leak fixed (`test_list_coalesce` passes; all 17 tests pass; `ORI_CHECK_LEAKS=1` clean on `test_coalesce_copy.ori`; debug+release+interpreter parity confirmed; fix: `propagate_borrowed_closure` unanimity for merge block params) (2026-04-03)
- [x] **04.4c** Coalesce None path works (`test_none_evaluates_default` passes; 16 of 17 tests pass; fix: add `merge_mutable_vars` to `lower_coalesce`) (2026-04-03)
- [x] **04.5** AIMS borrowed-def propagation fixed (merge block params no longer over-conservatively marked borrowed; 04.4b resolved; 04.2 has separate root cause) (2026-04-03)
- [ ] **04.6** Panic handler exception propagation fixed (`test_panic_handler_receives_message` passes)
- [ ] ALL previously-failing LLVM tests now pass (compare against baseline captured at start)
- [x] AOT tests: 2093 passed, 0 failures (baseline was 24 failures — now 0) (2026-04-03)
- [ ] `./test-all.sh` passes clean — LLVM spec 5 pre-existing failures (BUG-04-029: shift overflow checks)
- [ ] Debug AND release builds pass for all affected tests
- [ ] `ORI_CHECK_LEAKS=1` reports 0 leaks on all affected test programs
- [ ] Interpreter and LLVM produce identical results for all affected tests (run `diagnostics/dual-exec-verify.sh` on each affected file)
- [ ] Plan annotation cleanup: remove any code annotations added during this section

**Exit Criteria:** `./test-all.sh` passes clean. All affected test files pass through LLVM backend in both debug and release builds. `ORI_CHECK_LEAKS=1` clean. Interpreter and LLVM produce identical output. (TPR and hygiene review are in Section 05.)
