---
section: "02"
title: "Attribute Compliance"
status: not-started
goal: "All journeys attribute score ≥ 8/10, with compliance ≥ 80% (simple journeys ≥ 90%)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "noundef on Struct/Enum Params"
    status: not-started
  - id: "02.2"
    title: "uwtable on Main Wrapper"
    status: not-started
  - id: "02.3"
    title: "nounwind Improvements"
    status: not-started
  - id: "02.4"
    title: "memory(...) Annotations"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Attribute Compliance

**Status:** Not Started
**Goal:** Raise attribute compliance to ≥ 80% across all journeys, with simple journeys (J1, J4, J6, J8, J11) reaching ≥ 90%. This is the single highest-impact section — attribute score is the lowest-scoring category in 10 of 13 journeys.

**Context:** The LLVM codegen emits functions without several standard LLVM attributes that enable critical optimizations and improve debugging. The gaps are systematic — the same attributes are missing across all journeys because the infrastructure doesn't emit them. Four fixes cover 90%+ of the deductions.

**Current attribute scores:**
| Journey | Score | Compliance | Missing |
|---------|-------|-----------|---------|
| J1 | 8/10 | 92.3% | uwtable on main |
| J2 | 8/10 | 95.7% | uwtable on main |
| J3 | 6/10 | 77.8% | nounwind on recursive fns |
| J4 | 7/10 | 83.3% | noundef on struct params |
| J5 | 5/10 | 60.0% | fastcc on indirect, nounwind |
| J6 | 7/10 | 82.4% | noundef on struct/enum params |
| J7 | 8/10 | 94.1% | nounwind on panic callers |
| J8 | 8/10 | 91.7% | noundef on Box param |
| J9 | 6/10 | 73.9% | nounwind on string fns |
| J10 | 5/10 | 66.7% | nounwind on list fns |
| J11 | 7/10 | 81.1% | noundef on $eq params |
| J12 | 9/10 | 90.5% | memory(...) on pure fns |
| J13 | 4/10 | 52.6% | nounwind/fastcc on trampolines |

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/attributes.rs`: comprehensive attribute application per function
- **Zig** `src/codegen/llvm.zig`: `function_attributes()` — applies `nounwind`, `uwtable`, `noundef` systematically

**Depends on:** None.

---

## 02.1 noundef on Struct/Enum Params

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs` (`add_noundef_param_attribute`, `add_noundef_return_attribute`), `compiler/ori_llvm/src/codegen/ir_builder/calls.rs`

The compiler correctly applies `noundef` to primitive-typed parameters (i64, f64, i1, i32, i8) but not to user-defined struct/enum types passed by value. The gate is `is_llvm_scalar()` in `function_compiler/mod.rs:254` — this excludes all aggregate types even when passed `Direct` (i.e., small structs that fit in registers). Since Ori has no concept of poison values for user types, all values are `noundef`.

**Key distinction**: `noundef` on an aggregate value means no field is `undef`/`poison`. This is safe for Ori because all struct/enum fields are initialized at construction. However, `noundef` should NOT be applied to pointer params (Indirect passing) — those pointers are `nonnull` but the `noundef` attribute means the pointer value itself is defined, which is a weaker property already guaranteed by `nonnull`. The real optimization opportunity for indirect params is `dereferenceable` + `nonnull` + `readonly` (see below).

**Affected journeys:** J4, J6, J8, J11 (all currently 9.7, would reach 9.8+ with this fix alone)

- [ ] Find where `noundef` is applied: `function_compiler/mod.rs:247-266` — gated by `is_llvm_scalar()`
- [ ] Extend to apply `noundef` to `ParamPassing::Direct` params regardless of `is_llvm_scalar()` — small aggregates passed in registers are always fully defined in Ori
- [ ] Do NOT apply `noundef` to `ParamPassing::Indirect` pointer params — these need `nonnull` + `dereferenceable` instead
- [ ] Extend return value `noundef` to `ReturnPassing::Direct` for non-scalar returns
- [ ] Update the existing test `aggregate_params_no_noundef` in `function_compiler/tests.rs` — this test currently asserts aggregates DON'T get `noundef`, which needs to be updated for Direct aggregate params while keeping the assertion for Indirect aggregate params
- [ ] **Bonus**: For `ParamPassing::Indirect` params that are read-only (not mutated by the callee), add `readonly` attribute. This enables LLVM to hoist loads from these params and CSE across calls. Requires knowing which params are mutated — conservative: skip if any `Set` instruction targets a projected field of the param.
- [ ] Verify: `noundef` appears on struct/enum params in J4, J6, J8, J11 IR
- [ ] Verify: No behavioral changes (noundef is a hint, not a transformation)

### Cleanup (02.1)

- [ ] **[BLOAT]** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` — Currently 506 lines, exceeds 500-line limit. The `declare_function_llvm_with_extra_params()` method (lines 171-269) handles both declaration and attribute application in one 100-line block. Extract attribute application into a helper `apply_function_attributes()` to bring the file under 500 lines.
- [ ] **[STYLE]** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs:501-505` — Change `#[allow(clippy::doc_markdown, clippy::default_trait_access, ...)]` on test module to `#[expect(...)]` per lint discipline

---

## 02.2 uwtable on Main Wrapper

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`

The C `main` wrapper function (which calls `_ori_main`) is missing the `uwtable` attribute. Note: `uwtable` is already applied to all user-defined Ori functions via `declare_function_llvm()` in `mod.rs:245`. The gap is specifically the C-ABI `@main` wrapper generated in `entry_point.rs`, which uses `declare_function()` directly and skips the standard attribute application path.

**Affected journeys:** J1, J2, J7, J8

- [ ] Find the main wrapper emission code in `entry_point.rs` — specifically `generate_main_wrapper()`, the `declare_function("main", ...)` call around line 63
- [ ] Add `self.builder.add_uwtable_attribute(c_main_id);` after the function declaration (similar to how `nounwind` is conditionally added at line 68-70)
- [ ] Verify: `uwtable` appears on `@main` in J1 IR
- [ ] J1 should reach 10.0/10 with this fix

---

## 02.3 nounwind Improvements

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` (primary — two-pass nounwind analysis with `compute_nounwind_set()`), `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

Functions that provably don't throw (no panicking operations, no `invoke` to throwing callees) should have `nounwind`. The nounwind analysis uses a two-pass system: (1) prepare all functions into `PreparedFunction` buffers, (2) fixed-point iteration via `compute_nounwind_set()` to build the complete nounwind set. **Known limitation**: impl methods are compiled via the old immediate-emit path before the two-pass analysis runs, so they may incorrectly use `invoke` even when callees are trivially nounwind.

**Affected journeys:** J3, J5, J9, J10, J13

- [ ] **Audit**: Review the current `nounwind` analysis — what prevents it from being applied?
- [ ] **Option A**: Bottom-up `nounwind` propagation — if all callees are `nounwind`, caller is too
- [ ] **CRITICAL constraint**: Ori's panic mechanism uses `_Unwind_RaiseException` (Itanium EH). This IS C++ exception unwinding. A function that calls a potentially-panicking callee via `invoke` IS an unwinding function and must NOT be marked `nounwind`. Only functions where ALL call paths use `call` (not `invoke`) are safe to mark `nounwind`. A blanket "mark all user functions nounwind" approach is INCORRECT.
- [ ] If safe: apply `nounwind` to all user functions that don't use `invoke` for exception propagation
- [ ] **Fix impl method gap**: Impl methods compiled via the immediate-emit path in `impls.rs` bypass the two-pass nounwind analysis. Two approaches:
  - (a) **Fold impl methods into the two-pass batch**: modify `compile_impls()` to use `prepare_all_cached()` + `compute_nounwind_set()` + `emit_prepared_functions()` instead of direct `emit_arc_function()`. This is the correct fix but requires refactoring.
  - (b) **Post-hoc nounwind**: After all functions are emitted, walk all LLVM functions and retroactively add `nounwind` to those that contain no `invoke` instructions. This is simpler but less precise (misses transitive nounwind).
- [ ] **Runtime function nounwind**: Verify that `ori_rc_inc`, `ori_rc_dec`, `ori_buffer_drop_unique`, `ori_str_empty`, `ori_list_rc_inc`, and other non-panicking runtime functions are declared with `nounwind` in `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`. Missing `nounwind` on runtime declarations causes callers to use `invoke` unnecessarily.
- [ ] Verify: compliance % improves for J3, J5, J9, J10, J13

### Cleanup (02.3)

- [ ] **[STYLE]** `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs:76` — Bare `TODO(typeck)` without plan/roadmap reference. Add a plan reference or convert to a tracked issue: "See roadmap section-XX" per comment hygiene rules.

---

## 02.4 memory(...) Annotations

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs` (already has `add_memory_argmem_readwrite` — needs `memory(none)` and `memory(read)` variants)

Pure functions (no side effects) and read-only functions should have `memory(none)` or `memory(read)` respectively. This enables LLVM's memory analysis passes to optimize more aggressively.

**LLVM encoding**: The `memory` attribute uses `MemoryEffects` bitfield (see `ModRef.h`):
- `memory(none)` = `0` (no memory access at all)
- `memory(read)` = `DefaultMem:Ref | ArgMem:Ref | InaccessibleMem:Ref` = `1 | (1 << 2) | (1 << 4)` = `21`
- `memory(argmem: readwrite)` = `12` (already implemented)

**Affected journeys:** J5, J7, J12

- [ ] **Add `add_memory_none_attribute()`** to `ir_builder/attributes.rs`: `create_enum_attribute(kind, 0)` — encoding value `0`
- [ ] **Add `add_memory_read_attribute()`** to `ir_builder/attributes.rs`: `create_enum_attribute(kind, 21)` — encoding value `21`
- [ ] **Identify candidates**: Functions with no stores, no calls to side-effecting functions
  - `@bool_to_int` (J9): pure, should be `memory(none)`
  - `@safe_div` (J12): pure, should be `memory(none)`
  - `@my_abs`, `@my_max`, `@my_sign` (J2): pure, should be `memory(none)`
- [ ] **Detection criteria for `memory(none)`**: A function is pure (no memory effects) when:
  1. All ARC IR instructions are scalar-only (no `Construct`, `Project`, `Set` on heap types)
  2. No `Apply`/`Invoke` to non-nounwind or non-pure callees
  3. No `RcInc`/`RcDec` (these touch refcount memory)
  4. No `print()`, `panic()`, or other side-effecting builtins
  - Functions with only arithmetic/comparison on scalars qualify automatically.
- [ ] **Detection criteria for `memory(read)`**: A function reads but doesn't write — e.g., functions that read from struct fields but don't allocate or mutate. Less common, lower priority.
- [ ] **Where to add the analysis**: The analysis must be a post-emission pass (walk LLVM IR instructions after `emit_function()`) or integrated into the nounwind two-pass pipeline (which already has the ARC IR available in `PreparedFunction`). Note: `declare_function_llvm()` runs BEFORE the ARC pipeline, so it cannot be used.
- [ ] **Approach**: Conservative — annotate functions with no `call`/`invoke` to external functions and no `load`/`store` instructions as `memory(none)`. Simple, safe, covers the common case. Can be refined later using AIMS `EffectClass` from the lattice.
- [ ] Add `memory(none)` to functions identified as pure by the codegen
- [ ] Verify: `memory(none)` appears on qualifying functions

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `noundef` on all Direct-passed parameters (scalar AND small aggregate) in all 13 journeys
- [ ] `uwtable` on main wrapper (J1 score = 10.0)
- [ ] J4, J6, J8, J11 attribute score ≥ 9/10
- [ ] J3, J5, J9, J10, J13 attribute compliance ≥ 80%
- [ ] `aggregate_params_no_noundef` test updated to reflect new behavior (Direct aggregates get `noundef`, Indirect don't)
- [ ] `memory(none)` on qualifying pure functions (verified by IR inspection)
- [ ] Runtime function declarations in `codegen/runtime_decl/runtime_functions.rs` verified for `nounwind` correctness
- [ ] No behavioral changes (all journeys still PASS)
- [ ] `./test-all.sh` green

**Exit Criteria:** All 13 journeys have attribute compliance ≥ 80%. J1, J4, J6, J8, J11 reach 10/10 or 9.8/10. No regressions in any test suite.
