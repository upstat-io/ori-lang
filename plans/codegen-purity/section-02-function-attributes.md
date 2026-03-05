---
section: "02"
title: "Function Attributes"
status: not-started
goal: "Every function and runtime declaration has complete, correct LLVM attributes — noreturn, nounwind, noundef"
inspired_by:
  - "Rust rustc_codegen_llvm/declare.rs — applies nounwind/noreturn/noundef systematically"
  - "Zig src/codegen.zig — tracks side effects and exception behavior per function"
depends_on: []
sections:
  - id: "02.1"
    title: "noreturn on Panic Functions"
    status: not-started
  - id: "02.2"
    title: "nounwind on C main Wrapper"
    status: not-started
  - id: "02.3"
    title: "nounwind on Derived Trait Methods"
    status: not-started
  - id: "02.4"
    title: "nounwind on Runtime Declarations"
    status: not-started
  - id: "02.5"
    title: "nounwind on Indirect-Call Functions"
    status: not-started
  - id: "02.6"
    title: "noundef on Integer Parameters"
    status: not-started
---

# Section 02: Function Attributes

**Status:** Not Started
**Goal:** Every function emitted by the compiler has complete LLVM attributes: `noreturn` on functions that never return, `nounwind` on functions that cannot throw, `noundef` on parameters that are always defined. No function is missing an attribute it qualifies for.

**Context:** A two-pass fixed-point nounwind analysis already exists in `nounwind.rs` (prepare → analyze → emit). This infrastructure correctly identifies and marks user functions as `nounwind`. However, it misses several categories outside its analysis scope: the C `main` wrapper, derived trait methods (emitted by `derive_codegen`), and certain runtime declarations. Missing `noreturn` on `ori_panic_cstr` prevents LLVM from eliminating dead code after panic calls. Missing `noundef` on scalar parameters leaves value-range optimizations on the table.

**Existing infrastructure:** `compute_nounwind_set()` in `nounwind.rs` handles user functions via fixed-point iteration. The work here extends coverage to categories currently outside the analysis scope.

**Journeys affected:** M-2 (J1, J5), L-1 (J1, J5, J6), L-2 (J11), L-3 (J9), L-11 (J1), L-12 (J5).

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/declare.rs`: Systematically applies attributes during function declaration.
- **Zig** `src/codegen.zig`: Every function declaration includes complete attribute metadata.

---

## 02.1 noreturn on Panic Functions

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

`ori_panic_cstr` is declared with `cold` but not `noreturn`. Since this function always aborts/unwinds and never returns to its caller, LLVM should know this to:
- Eliminate unreachable code after panic calls (§06 synergy)
- Improve branch prediction hints
- Enable dead code analysis in the optimizer

**Infrastructure gap:** The `Attr` enum currently has `Nounwind`, `Cold`, `NoaliasReturn`, and `MemArgmemRW`. There is **no `Noreturn` variant** — it must be added and wired through runtime declaration emission in `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`.

**Critical distinction:** `noreturn` (function never returns to caller) and `nounwind` (function never throws/unwinds) are **independent** LLVM attributes. `ori_panic_cstr` should get `noreturn` but **MUST NOT** get `nounwind` (runtime table comments already document that panic paths must unwind for RC cleanup).

**Sync points for adding `Attr::Noreturn`** (all must be updated together):
1. `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` — add `Noreturn` variant to `enum Attr`
2. `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs` — add match arm in `apply_attr()` for `Attr::Noreturn`
3. `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs` — add `add_noreturn_attribute()` method to `IrBuilder` (follows pattern of `add_nounwind_attribute`)
4. `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` — add `Attr::Noreturn` to `ori_panic` and `ori_panic_cstr` entries in `RT_FUNCTIONS`
5. `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` — consider adding `is_rt_fn_noreturn()` query (parallel to `is_rt_fn_nounwind()`) for use by §06.2 dead code pruning
6. `compiler/ori_llvm/src/codegen/runtime_decl/tests.rs` — add test: `ori_panic_cstr` has `Noreturn` attribute

> **TDD requirement:** Write an IR test asserting `@ori_panic_cstr` declaration LACKS `noreturn` BEFORE implementing. Verify it fails after the fix (attribute now present). This confirms the test captures the actual issue.

- [ ] Add `Noreturn` variant to the `Attr` enum in `runtime_functions.rs`
- [ ] Add match arm for `Attr::Noreturn` in `apply_attr()` in `runtime_decl/mod.rs`
- [ ] Add `add_noreturn_attribute()` to `IrBuilder` in `attributes.rs` (uses LLVM `noreturn` enum attribute kind)
- [ ] Wire `Attr::Noreturn` through the declaration machinery to emit the LLVM `noreturn` function attribute
- [ ] Add `Attr::Noreturn` to `ori_panic_cstr` declaration (alongside existing `Attr::Cold`)
- [ ] Add `Attr::Noreturn` to `ori_panic` declaration (alongside existing `Attr::Cold`)
- [ ] Add `is_rt_fn_noreturn()` query function (parallel to `is_rt_fn_nounwind()`) for §06.2 consumption
- [ ] Do NOT add `Attr::Nounwind` to panic functions — they must unwind for RC cleanup
- [ ] Verify: IR for `panic()` calls shows `call void @ori_panic_cstr(...) #noreturn`
- [ ] Verify: call sites to proven-`noreturn` panic functions terminate the path (`unreachable` terminator)

### 02.1 Completion Checklist

- [ ] `Attr::Noreturn` variant exists and emits the LLVM `noreturn` function attribute
- [ ] `ori_panic_cstr` declaration has `noreturn` + `cold` but NOT `nounwind`
- [ ] All panic-path runtime functions (`ori_panic`, `ori_panic_cstr`) have `noreturn`
- [ ] IR test: `@ori_panic_cstr` declaration includes `noreturn` attribute
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` test for noreturn on panic functions
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 02.2 nounwind on C main Wrapper

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs` (primary — `generate_main_wrapper()`), `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` (nounwind set)

The C `main()` wrapper function calls `ori_run_main` (which calls `_ori_main`). The wrapper itself is never marked `nounwind`. This causes unnecessary exception table generation.

**Note:** The C `main` wrapper calls `ori_run_main` (a runtime function), not `_ori_main` directly. `ori_run_main` catches all panics internally:
- **Non-MSVC (Linux, macOS, MinGW):** Uses `std::panic::catch_unwind()` (line 388 of `ori_rt/src/lib.rs`)
- **MSVC (Windows):** Uses `ori_try_call()` with `__try/__except` SEH (line 375)

Since `ori_run_main` catches all unwinding internally and returns an `i32` exit code, it IS `nounwind` from the caller's perspective. The C `main` wrapper can be marked `nounwind`.

- [ ] Add `Attr::Nounwind` to `ori_run_main` in `RT_FUNCTIONS` table (verified: it catches all panics internally)
- [ ] Mark C `main` wrapper as `nounwind` in `entry_point.rs` (`generate_main_wrapper()`)
- [ ] Verify: `main` function in IR has `nounwind` attribute

### 02.2 Completion Checklist

- [ ] `ori_run_main` has `Attr::Nounwind` in `RT_FUNCTIONS`
- [ ] C `main` wrapper has `nounwind` attribute
- [ ] IR test: `define i32 @main(...)` includes `nounwind` for a trivial program
- [ ] `./test-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 02.3 nounwind on Derived Trait Methods

**File(s):** `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`, `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`, `compiler/ori_llvm/src/evaluator/compile.rs` (pipeline ordering)

Derived methods (`$eq`, `$compare`, `$hash`, `$clone`, `$debug`, `$to_str`) are emitted by `derive_codegen` outside the standard nounwind fixed-point analysis pipeline. Methods like `$eq` perform only pure comparisons (`extractvalue`, `icmp`, `load`, `switch`, `br`) and should be marked `nounwind`.

**Pipeline ordering issue:** In `evaluator/compile.rs`, `compile_derives()` (step 8b) runs BEFORE `compute_nounwind_set()` (step 8d). Derived methods emit LLVM IR directly — they are not processed through `prepare_all_cached()` / `prepare_mono_cached()`. This means approach (a) requires refactoring the pipeline to either:
- Move `compile_derives` after `emit_prepared_functions`, or
- Create a parallel `PreparedDerived` type and include derived methods in the two-pass analysis.

**Similarly:** `compile_impls()` (step 8a) also runs before nounwind analysis. The existing `nounwind.rs` comment (line 14-19) documents this as a known limitation for impl methods.

Two approaches:
- **(a)** Include derived methods in the nounwind fixed-point analysis (preferred — single source of truth, but requires pipeline refactor)
- **(b)** Mark derived methods as `nounwind` in `derive_codegen` directly (simpler but duplicates logic — must enumerate which traits are pure)
- **(b.1)** Hybrid: mark pure derived methods (`$eq`, `$compare`, `$hash`) as `nounwind` directly in `derive_codegen`; leave impure ones (`$to_str`, `$debug`) unmarked. Revisit approach (a) as a follow-up.

- [ ] Choose approach: (a) pipeline refactor to include derives in nounwind analysis, (b) direct annotation in `derive_codegen`, or (b.1) hybrid (pure derives annotated directly, impure left for later). Document choice in a code comment at the implementation site.
- [ ] If approach (b) or (b.1): add `nounwind` directly to `$eq`, `$compare`, `$hash` in `derive_codegen` after function declaration
- [ ] If approach (b) or (b.1): document in `nounwind.rs` that derived methods are handled separately with a `// NOTE:` comment citing this section
- [ ] If approach (a): refactor pipeline ordering in `evaluator/compile.rs` to include derived methods in two-pass analysis
- [ ] Verify: `$eq` methods in J11 IR have `nounwind` attribute
- [ ] Verify: `$compare`, `$hash` methods also get `nounwind` where applicable
- [ ] Negative test: `$to_str` (allocates strings) does NOT get `nounwind` unless proven safe
- [ ] Negative test: `$debug` (allocates strings) does NOT get `nounwind` unless proven safe

### 02.3 Completion Checklist

- [ ] All derived trait methods that are pure (`$eq`, `$compare`, `$hash`) have `nounwind`
- [ ] Derived methods that may allocate or call user code (`$to_str`, `$debug`) are correctly excluded or included based on analysis
- [ ] IR test: `$eq` for a simple struct has `nounwind`
- [ ] J11 `_ori_Shape$eq` has `nounwind` in emitted IR
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 02.4 nounwind on Runtime Declarations

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

`ori_str_from_raw` is declared without `nounwind`, while `ori_str_len` and `ori_rc_dec` have it. This prevents functions that call `ori_str_from_raw` (like string comparison) from being marked `nounwind`.

- [ ] Audit all runtime function declarations for missing `nounwind`
- [ ] Add `nounwind` to `ori_str_from_raw` and any other safe runtime functions
- [ ] For each runtime function left without `nounwind`, add or confirm rationale (may panic, may allocate, or otherwise may unwind)
- [ ] Verify: Functions calling `ori_str_from_raw` can now be marked `nounwind` by the fixed-point analysis

> **Note:** `runtime_functions.rs` is 1464 lines but has a documented exemption from the 500-line limit (it is a pure static data table with no logic). Adding `Nounwind` attrs to entries will not meaningfully change its size. The exemption comment is at the top of the file.

**Scope:** The `RT_FUNCTIONS` table has ~141 entries. Currently 47 have `Attr::Nounwind` and 94 have empty attrs. Not all 94 should get `Nounwind` — many may panic (assertions, OOB access, allocations that can OOM). The audit should categorize each into:
- **Can add `Nounwind`:** pure accessor functions (e.g., `ori_str_from_raw`, `ori_str_len` — already has it)
- **Cannot add `Nounwind`:** functions that may panic or allocate (e.g., `ori_assert`, `ori_list_push`)
- **Needs investigation:** functions where it's unclear (e.g., `ori_print` — does it unwind on I/O error?)

### 02.4 Completion Checklist

- [ ] Full audit of all ~141 runtime functions completed
- [ ] Every runtime function declaration has either `nounwind` or a documented rationale for omitting it
- [ ] `ori_str_from_raw` has `nounwind`
- [ ] Functions that transitively call only `nounwind` runtime functions are now caught by the fixed-point analysis
- [ ] Runtime declaration table in `runtime_functions.rs` has comments explaining nounwind status for each function without `Nounwind`
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 02.5 nounwind on Indirect Closure Calls

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs`

Indirect calls through closure function pointers (e.g., `ApplyIndirect` in ARC IR) are conservatively excluded from nounwind analysis because the callee isn't statically known. This is correct in general but pessimistic when all actual callees are known to be nounwind at the module level.

**Note:** The original finding referenced `_ori_apply`, but this function does not exist in the current codebase. The issue applies to all indirect closure calls emitted by `ArcIrEmitter::emit_instr()` for `ApplyIndirect` instructions.

> **Complexity note:** Interprocedural nounwind proof for indirect calls requires whole-module closure analysis — tracking all possible callees for every closure variable. This is a significant analysis investment for a LOW-severity finding (L-12). The conservative approach (document limitation, add comment) is likely the right choice for this cycle.

- [ ] Decide policy explicitly: conservative (document limitation) vs interprocedural proof
- [ ] If implementing interprocedural proof, require whole-module evidence that all possible callees are nounwind
- [ ] Add negative test where one closure target may unwind; indirect calls must remain without `nounwind`

### 02.5 Completion Checklist

- [ ] Policy documented: either interprocedural proof implemented, or conservative limitation explicitly documented with rationale
- [ ] If interprocedural: functions with all-nounwind closure targets get `nounwind`; mixed targets do not
- [ ] If conservative: comment in `nounwind.rs` explains why indirect calls are excluded
- [ ] Negative test: function with a may-unwind closure target is NOT marked `nounwind`
- [ ] `./test-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 02.6 noundef on Integer Parameters

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` (parameter emission), `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs` (new `add_noundef_param_attribute()` method needed)

Ori has no undefined scalar values at language level. Adding `noundef` tells LLVM that passing `undef`/`poison` is UB, enabling additional optimization and cleanup opportunities.

**Sync points for adding `noundef`:**
1. `compiler/ori_llvm/src/codegen/ir_builder/attributes.rs` — add `add_noundef_param_attribute(func, param_index)` method
2. `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` — call it for each scalar parameter during function definition
3. Potentially `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs` — derived methods also have scalar params

> **File size warning:** `define_phase.rs` is 461 lines. Adding noundef annotation logic should be minimal (a few lines per param type), but if it requires new helper functions, consider extracting parameter attribute logic into a sibling module to stay under 500 lines.

- [ ] Add `add_noundef_param_attribute()` to `IrBuilder` in `attributes.rs`
- [ ] Add `noundef` to scalar ABI parameters (`i64`, `i1`, `double`) in `define_phase.rs` during function definition
- [ ] Add `noundef` to scalar parameters of derived methods in `derive_codegen`
- [ ] Add `noundef` to scalar return values where guaranteed defined
- [ ] Do not blanket-annotate aggregate/pointer values without proof obligations
- [ ] Verify: IR shows `noundef i64 %param` in function signatures
- [ ] Verify: No test regressions (noundef should be a pure optimization hint)
- [ ] Verify: `opt-21 -passes=verify` clean (noundef contract violation = UB, must be correct)

### 02.6 Completion Checklist

- [ ] All scalar parameters (`i64`, `i1`, `double`) have `noundef` in LLVM IR
- [ ] Scalar return values have `noundef` where guaranteed defined
- [ ] Aggregate/pointer values do NOT have `noundef` (unless proven)
- [ ] IR test: function with `int` parameter shows `noundef i64` in signature
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`
- [ ] `opt-21 -passes=verify` clean on representative journey IR

---

## Section 02 Exit Criteria

All six subsections complete. `grep` for function declarations in emitted IR shows: all panic functions have `noreturn`, all pure functions have `nounwind`, all integer parameters have `noundef`. Zero attribute gaps across all 12 code journeys.
