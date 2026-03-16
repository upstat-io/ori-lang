---
section: "01"
title: "Attribute Completion"
status: in-progress
goal: "100% attribute compliance on all 13 journeys — every applicable attribute present on every function"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "noundef on Main Wrapper Return"
    status: complete
  - id: "01.2"
    title: "nounwind on All Qualifying Functions"
    status: complete
  - id: "01.3"
    title: "noundef on Indirect Pointer Params"
    status: complete
  - id: "01.4"
    title: "nonnull + dereferenceable on Indirect Params"
    status: not-started
  - id: "01.5"
    title: "Construct Purity — memory(none) and memory(read) on Functions with Value-Type Construction"
    status: not-started
  - id: "01.6"
    title: "Closure/Iterator Trampoline Attributes (J13 gap)"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Attribute Completion

**Status:** Not Started
**Goal:** 100% attribute compliance across all 13 code journeys. Every function gets every applicable LLVM attribute. No gaps, no excuses.

**Context:** Attribute compliance is the weakest scoring dimension (avg 7.5/10, worst J13 at 4/10). The gaps are systematic — the same missing attributes repeat across journeys. Six targeted fixes cover every gap.

**Current state (completed in predecessor `aims-codegen-quality` Section 02):**
- `noundef` on Direct params: DONE
- `uwtable` on all functions including main wrapper: DONE
- `memory(none)` on pure scalar functions: DONE
- `memory(read)` on readonly functions with Indirect params: DONE
- `readonly` on Indirect borrowed params: DONE
- `nounwind` two-pass analysis + posthoc pass: DONE

**Remaining gaps (from code journey re-run 2026-03-16):**

| Gap | Impact | Journeys |
|-----|--------|----------|
| Missing `noundef` on C main wrapper `i32` return | All 13 | J1-J13 |
| Missing `nounwind` on some qualifying functions | 4-5 journeys | J5, J9, J10, J13 |
| Missing `noundef` on Indirect (ptr) params | 2 journeys | J4, J11 |
| Missing `nonnull` + `dereferenceable` on Indirect params | 2-4 journeys | J4, J10, J11 |
| Missing `memory(read)` on some readonly functions | 1 journey | J7 (@sum_for) |
| Low compliance from indirect call targets | 1 journey | J13 (trampolines) |

**Reference implementations:**
- **Rust** `compiler/rustc_codegen_llvm/src/attributes.rs`: `from_fn_attrs()` applies `nounwind`, `noundef`, `readonly`, `dereferenceable`, `nonnull` systematically
- **Zig** `src/codegen/llvm.zig`: `function_attributes()` applies all applicable attributes per function kind

**Depends on:** None.

---

## 01.1 noundef on Main Wrapper Return

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`

The C `main()` wrapper returns `i32` but lacks `noundef` on the return value. This is a single missing attribute that affects all 13 journeys.

- [x] In `generate_main_wrapper()`, after declaring the function with `declare_function("main", ...)`, add `noundef` to the return value attribute (2026-03-16)
  - Use `self.builder.add_noundef_return_attribute(c_main_id)` — the method already exists in `ir_builder/attributes.rs`
  - The return value is always a well-defined `i32` (exit code from `_ori_main`)
  - Insert after line `self.builder.add_uwtable_attribute(c_main_id);` (line ~65 of entry_point.rs)
  - Note: `c_main_id` is declared via `self.builder.declare_function()`, NOT via `declare_function_llvm()`, so it does NOT go through the standard param attribute loop. The `noundef` return attribute must be added explicitly here.
- [x] Test: Write a Rust unit test `main_wrapper_has_noundef_return` in `function_compiler/tests.rs` that compiles a trivial `@main () -> void` and checks the main wrapper's return attribute (2026-03-16)
- [x] Test: `ORI_DUMP_AFTER_LLVM=1 ./target/debug/ori build plans/code-journeys/01-arithmetic.ori 2>&1 | grep 'define.*@main'` shows `define noundef i32 @main()` (2026-03-16)
- [x] Verify: `timeout 150 ./test-all.sh` green — 12,888 tests, 0 failures (2026-03-16)

---

## 01.2 nounwind on All Qualifying Functions

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`, `compiler/ori_llvm/src/codegen/function_compiler/impls.rs`

The posthoc nounwind pass (`apply_posthoc_nounwind()`) catches impl methods that were compiled before the two-pass analysis. But some functions still lack `nounwind` because:

1. **Indirect call targets** (closure function pointers, iterator trampolines) — these CAN be nounwind if they contain no `invoke` instructions
2. **Functions calling `noreturn` callees** — if the only potentially-unwinding path ends in `unreachable` (after a `noreturn` call like panic), the function itself can be `nounwind` because the unwind is contained

The posthoc pass (`apply_posthoc_nounwind()`) iterates `codegen_ctx.functions`, which includes top-level functions and lambdas but does NOT include closure wrapper functions generated by `emit_partial_apply` in `arc_emitter/closures.rs` (e.g., `_ori_partial_N`). These wrappers receive nounwind inline during generation only if their callee is nounwind. Investigate whether case 2 or unregistered wrappers are causing the remaining gaps.

- [x] Audit: For each non-nounwind function in J5, J9, J10, J13, determine WHY it's not nounwind (2026-03-16)
  - J5: `apply` (indirect call), `main` (calls apply) — correctly non-nounwind
  - J9: `check_strings` (calls `ori_panic_cstr` on overflow) — correctly non-nounwind
  - J10: `count_items` (ARC Apply @length not recognized as nounwind) — BUG: posthoc pass missing from AOT pipeline
  - J10: `check_length`/`check_passing` (genuine invoke to unwinding callees) — correctly non-nounwind
  - J13: `main` (calls iterator ops that may unwind) — correctly non-nounwind
- [x] If the posthoc pass misses functions: fix `apply_posthoc_nounwind()` — **BUG FOUND AND FIXED**: the AOT codegen pipeline (`codegen_pipeline.rs`) was missing `apply_posthoc_nounwind()`. Only the JIT path (`evaluator/compile.rs`) had it. Added call in `codegen_pipeline.rs` after `emit_prepared_functions()`. This fixes `count_items` (J10) and any other function whose ARC IR Apply callees get inlined to non-call LLVM IR. (2026-03-16)
- [x] If functions are correctly non-nounwind: updated `attribute_metrics.py` scoring tool to use module-level callee analysis (`_all_callees_nounwind`) instead of the old leaf-only heuristic. The new rule: nounwind is applicable if ALL call/invoke targets in the LLVM IR are nounwind-attributed (verified by inspecting each callee in the module). This correctly credits compiler-proven nounwind on non-leaf functions AND excludes functions calling `ori_panic_cstr` or other unwinding runtime functions. (2026-03-16)
- [x] Verify: Attribute compliance improves for J5, J9, J10, J13 (2026-03-16)
  - J5: 82.1% (remaining gaps are closure wrapper noundef — covered by 01.3/01.6)
  - J9: 89.5% (remaining gaps are drop function uwtable/noundef)
  - J10: 90.0% (up from 85.0% — count_items now nounwind; remaining: noundef on ptr params)
  - J13: 60.7% (remaining gaps are trampoline uwtable/noundef — covered by 01.6)

**CONSTRAINT**: Never mark a function `nounwind` if it uses `invoke` to a callee that can unwind. Ori uses Itanium EH — `nounwind` on an unwinding function causes UB (immediate `std::terminate`).

---

## 01.3 noundef on Indirect Pointer Params

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

Currently `noundef` is only applied to `ParamPassing::Direct` params. For `ParamPassing::Indirect` (pointer params), the pointer value itself IS always defined (non-poison) — Ori never creates poison pointers. `noundef` on a pointer means the pointer value is defined, not the pointee data.

- [x] In `declare_function_llvm_with_extra_params()`, apply `noundef` to both `ParamPassing::Indirect` and `ParamPassing::Reference` pointer params (2026-03-16)
  - Added `add_noundef_param_attribute` call before the readonly check in the `else if !matches!(param.passing, ParamPassing::Void)` branch
- [x] Affected functions verified: `@_ori_area` (J4), `Shape$eq` (J11), `@_ori_count_items` (J10) — all now show `ptr noundef` (2026-03-16)
- [x] Test: Renamed `indirect_params_no_noundef` → `indirect_params_have_noundef`, updated `mixed_params_selective_noundef` (3→4 noundef), updated AOT `test_aggregate_params_lack_noundef` → `test_indirect_params_have_noundef` (2026-03-16)
- [x] **BUG FOUND**: posthoc `function_has_no_invoke` was checking only for `invoke` instructions, but `call` to non-nounwind functions (e.g., `ori_panic`) also propagates unwinding. Fixed to check ALL call/invoke targets via `LLVMGetCalledValue` + callee nounwind attribute inspection. This prevented the panicking main wrapper from being incorrectly marked nounwind. (2026-03-16)
- [x] Verify: `timeout 150 ./test-all.sh` green — 12,888 tests, 0 failures (2026-03-16)
- [x] Result: J04, J10, J11 all jump to 100% compliance. 10/13 journeys at 100%. (2026-03-16)

---

## 01.4 nonnull + dereferenceable on Indirect Params

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs`

Indirect params are always non-null pointers to valid memory of a known size. LLVM can use `nonnull` to eliminate null checks and `dereferenceable(N)` to enable speculative loads.

- [ ] Add `add_nonnull_param_attribute(fn_value, param_index)` to `attributes.rs`
- [ ] Add `add_dereferenceable_param_attribute(fn_value, param_index, size_bytes)` to `attributes.rs`
  - `dereferenceable(N)` uses `Attribute::get_named_enum_kind_id("dereferenceable")` with `create_enum_attribute(kind, N)` where N is the byte size
  - Get byte size from `abi_size(ty, store)` in `compiler/ori_llvm/src/codegen/abi/mod.rs` for the Ori type being passed
- [ ] In `declare_function_llvm_with_extra_params()`, for each `ParamPassing::Indirect` or `ParamPassing::Reference` param:
  - Add `nonnull` (always — Ori never passes null pointers to functions)
  - Add `dereferenceable(N)` where N = byte size of the pointed-to type
  - `abi_size()` is in `crate::codegen::abi::abi_size` — import it in `function_compiler/mod.rs`. It takes `(Idx, &TypeInfoStore)`, both available in `FunctionCompiler`.
  - The `abi_size` FIXME (no alignment padding) is safe for `dereferenceable` — underestimating is legal (LLVM treats it as minimum). But document this in a comment.
  - `param.ty` gives the Ori `Idx` for the pointed-to type
- [ ] Test: Write `indirect_params_have_nonnull_dereferenceable` test
- [ ] Test: Verify `dereferenceable(N)` values match expected sizes for known types (e.g., list struct = 24 bytes = `dereferenceable(24)`)
- [ ] Verify: `timeout 150 ./test-all.sh` green

### Cleanup (01.4)

- [ ] **[STYLE]** `abi/mod.rs:138` — `FIXME` comment lacks a plan/roadmap reference. Per hygiene rules, all TODOs/FIXMEs must reference a plan or roadmap item. Update to `// FIXME(roadmap): abi_size_inner sums field sizes WITHOUT alignment padding — needs LLVM TargetData query when user-defined structs land in Pool. See roadmap section-05.` This is the same FIXME the plan already references (01.4 notes it's safe for dereferenceable).

---

## 01.5 Construct Purity — memory(none) and memory(read) on Functions with Value-Type Construction

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` (analysis implementations), `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` (invocation in `compute_nounwind_set`)

The `is_arc_function_readonly()` and `is_arc_function_pure()` analyses (defined in `define_phase.rs`) detect functions that only read memory or have no memory effects. The purity/readonly analysis runs inside `compute_nounwind_set()` in `nounwind.rs`. But some functions are missed.

- [ ] Investigate: Why does `@sum_for` (J7) not get `memory(none)` despite being a pure function?
  - The range struct `insertvalue`/`extractvalue` operates on SSA values, not memory
  - `is_arc_function_pure()` (in `define_phase.rs`) only allows `ArcInstr::Let` and `ArcInstr::Select` instructions — `ArcInstr::Construct` is excluded, which blocks purity for range-constructing functions
  - Range lowering (`ori_arc/src/lower/collections/mod.rs`) emits `Construct { ctor: CtorKind::Tuple, ... }` for range structs
  - Likely fix: `Construct` of value types (all scalar fields, no heap pointers) should be treated as pure — it creates an SSA aggregate, not a heap allocation
- [ ] Fix: Update `is_arc_function_pure()` in `define_phase.rs` to recognize `ArcInstr::Construct` of value types as pure
  - A `Construct` of a value type (all fields are scalars, no heap pointers) is pure — it creates an SSA aggregate, not a heap allocation
  - **Key insight**: `ArcInstr::Construct` emits LLVM `insertvalue` instructions — pure SSA manipulation with no memory access. The heap allocation (if any) is done by separate `RcAlloc`/`Apply` instructions, not by `Construct`. So `Construct` is always safe to allow in `is_arc_function_pure()` regardless of field types.
  - Additionally check: `is_arc_function_readonly()` should also allow `ArcInstr::Construct` (it's already weaker than pure — Construct doesn't read memory either).
  - Signature change: `is_arc_function_pure()` remains a static method — no classifier needed since the fix is unconditional.
- [ ] Fix: Also update `is_arc_function_readonly()` in `define_phase.rs` to allow `ArcInstr::Construct`
- [ ] Verify: `@sum_for` gets `memory(none)` in J7 IR dump
- [ ] Verify: Check whether `is_abi_memory_free()` in `nounwind.rs` still correctly gates the attribute (a function with Construct but Indirect params is readonly, not pure, because of the param loads — `is_abi_memory_free` returns false for Indirect params)
- [ ] Test: Write unit test `construct_does_not_block_purity` — create an ArcFunction with `Let` + `Construct` + `Return`, verify `is_arc_function_pure()` returns true

### Cleanup (01.5)

- [ ] **[BLOAT]** `define_phase.rs` — Currently 508 lines, exceeds 500-line limit. This section adds `ArcInstr::Construct` to both `is_arc_function_pure()` and `is_arc_function_readonly()`. After the change, consider extracting `is_arc_function_pure()`, `is_arc_function_readonly()`, and `remap_partial_apply_names()` into a new `purity_analysis.rs` submodule (~70 lines) to bring `define_phase.rs` under the limit.
- [ ] **[WASTE]** `define_phase.rs:260` — `let _ = name;` is dead code. The `name` parameter to `process_arc_function()` is suppressed but never used. Either remove the parameter (if callers don't need to pass it) or use it in the tracing `debug!()` call (it was likely intended for logging and accidentally silenced).

---

## 01.6 Closure/Iterator Trampoline Attributes

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/closures.rs`

J13 has the worst attribute compliance (57.1%) because trampoline functions (`_ori_tramp_N`) and closure wrapper/lambda functions (`_ori_partial_N`) lack some standard attributes. The posthoc nounwind pass should catch these, but other attributes (like `memory(none)` on pure trampolines) may be missed.

- [ ] Audit: List every function in J13 IR and its attributes — identify each gap
- [ ] For each trampoline/lambda function, verify the posthoc passes cover it:
  - Posthoc nounwind: walks `codegen_ctx.functions` only. Lambdas ARE registered there, but closure wrappers (`_ori_partial_N`) generated by `emit_partial_apply` are NOT. Wrappers get nounwind inline during `generate_closure_wrapper()` only if their callee is nounwind.
  - Memory analysis: runs inside `compute_nounwind_set()` (no separate `compute_memory_attributes()` function). Only covers functions in the two-pass prepared set and their lambdas — NOT closure wrappers.
- [ ] Fix: Apply attributes to closure wrappers. Recommended approach: **(b) apply inline during `generate_closure_wrapper()`** in `closures.rs`:
  - `nounwind`: already done inline — wrapper checks if callee is nounwind and applies
  - `uwtable`: already applied by `declare_function_llvm()` which the wrapper calls
  - `memory(none)`: wrappers that only extract fields from an env struct and call the lambda are NOT memory(none) — they load from the env pointer (argmem read). They could get `memory(argmem: read)` if the lambda is pure, but this is a new attribute not yet in the attributes API. Skip this — the scoring tool should not count memory attributes on trampoline wrappers.
  - `noundef`: apply to all Direct params. Wrapper params are typed (not opaque void*).
  - The cost of approach (a) is that `codegen_ctx.functions` is a Name-keyed map, but wrappers don't have interned Names — they have ad-hoc symbol strings like `_ori_partial_0`. This would require interning the wrapper name, which adds complexity for little benefit.
  - Approach (c) is a sledgehammer — walking all LLVM module functions requires re-deriving attribute applicability from LLVM IR without Ori type info.
- [ ] If `fastcc` is missing on indirect call targets: this is correct — indirect calls use C calling convention for compatibility. Mark as N/A in the scoring tool rather than counting as a gap.
  - Update `.claude/skills/code-journey/attribute_metrics.py`: in the `fastcc` applicability rule (line ~123), ensure closure wrappers (`_ori_partial_N`) are excluded since they use `ccc` for closure ABI compatibility
- [ ] Verify: J13 attribute compliance ≥ 90%

### Cleanup (01.6)

- [ ] **[BLOAT]** `nounwind.rs` — Currently 547 lines, exceeds 500-line limit. The `compute_nounwind_set()` method (lines 306-425, ~120 lines) combines nounwind analysis, purity analysis, and readonly analysis in a single function. Consider extracting the purity+readonly analysis into a separate `compute_memory_attributes()` method, or moving the `PreparedFunction`/`PreparedLambda` types and the prepare phase methods into a new `prepare.rs` submodule.

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `noundef` on C main wrapper `i32` return value — all 13 journeys
- [ ] `nounwind` on every function with no `invoke` to unwinding callees
- [ ] `noundef` on all Indirect and Reference pointer params
- [ ] `nonnull` on all Indirect and Reference pointer params
- [ ] `dereferenceable(N)` on all Indirect and Reference pointer params with known size
- [ ] `memory(none)` on all pure functions including those with value-type construction
- [ ] `memory(read)` on all readonly functions including those with value-type construction
- [ ] Trampoline/lambda functions covered by posthoc attribute passes
- [ ] `.claude/skills/code-journey/attribute_metrics.py` updated: closure wrappers excluded from fastcc check; non-leaf nounwind functions given credit when compiler proves them nounwind
- [ ] All 13 journeys attribute compliance ≥ 95%
- [ ] All 13 journeys attribute score 10/10 (or 9/10 with documented justification for N/A attributes)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `cargo b --release && timeout 150 ./test-all.sh` green

**Exit Criteria:** `extract-metrics.py` reports ≥ 95% attribute compliance on all 13 journeys. No applicable attribute is missing from any emitted function.
