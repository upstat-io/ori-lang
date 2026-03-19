---
section: "04"
title: "Combinatorial Test Matrix"
status: complete
goal: "Every cell of {type categories} x {language features} is covered by an AOT test, ensuring no intersection of fat pointers with any feature is untested"
depends_on: ["01", "02", "03"]
third_party_review:
  status: resolved
  updated: 2026-03-19
sections:
  - id: "04.1"
    title: "Type Category Definitions"
    status: complete
  - id: "04.2"
    title: "Feature Dimension Definitions"
    status: complete
  - id: "04.3"
    title: "Matrix Implementation"
    status: complete
  - id: "04.4"
    title: "Valgrind Verification Layer"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "04.N"
    title: "Completion Checklist"
    status: complete
---

# Section 04: Combinatorial Test Matrix

**Status:** In Progress
**Goal:** Build a systematic test matrix covering `{type categories} x {language features}`. Every cell is an AOT test program that exercises a specific type in a specific feature context. All tests pass in both eval and AOT. All tests run clean under Valgrind.

**Context:** The original 13 code journeys all scored 10.0/10, yet 3 CRITICAL bugs lurked at feature intersections. The journeys tested features in isolation: J5 tested closures with `int` capture, J9 tested strings with `.length()`, but nobody tested closures capturing strings. The test matrix ensures this gap class is eliminated permanently — every type x feature intersection is tested.

**Design principle:** Tests target the **general type category**, not specific literal values. A test for "str x closures" proves that ALL string values work in closure capture, not just `"hello"`. The type categories and feature dimensions are defined below.

---

## 04.1 Type Category Definitions

These are the type categories that differ in LLVM representation, ARC strategy, or ABI treatment. Each category exercises a different codegen path.

| ID | Category | LLVM Type | RC Strategy | ABI | Example |
|----|----------|-----------|-------------|-----|---------|
| T1 | Scalar int | `i64` | None | Direct | `42` |
| T2 | Scalar float | `double` | None | Direct | `3.14` |
| T3 | Scalar bool | `i1` | None | Direct | `true` |
| T4 | String (SSO) | `{i64, i64, ptr}` | FatPointer (SSO skip) | Indirect (24B) | `"hello"` (<=23 bytes) |
| T5 | String (heap) | `{i64, i64, ptr}` | FatPointer (heap RC) | Indirect (24B) | `"abcdefghijklmnopqrstuvwxyz1234"` |
| T6 | List of scalars | `{i64, i64, ptr}` | HeapPointer | Indirect (24B) | `[1, 2, 3]` |
| T7 | List of fat ptrs | `{i64, i64, ptr}` | HeapPointer + elem RC | Indirect (24B) | `["a", "b"]` |
| T8 | Struct (scalar fields) | `{i64, i64}` | None | Direct (<=16B) or Indirect | `Point { x: 1, y: 2 }` |
| T9 | Struct (fat fields) | `{{i64,i64,ptr}, i64}` | AggregateFields | Indirect | `Named { name: "x", id: 1 }` |
| T10 | Sum type (unit variants) | `i64` (tag only) | None | Direct | `Red \| Green \| Blue` |
| T11 | Sum type (fat payload) | `{i64, {i64, i64, ptr}}` | InlineEnum | Indirect | `Some("hello")` / `None` |
| T12 | Closure (no capture) | `{ptr, ptr}` | Closure (null env) | Direct (16B) | `x -> x + 1` |
| T13 | Closure (scalar capture) | `{ptr, ptr}` | Closure (env RC) | Direct (16B) | `let n = 5; x -> x + n` |
| T14 | Closure (fat capture) | `{ptr, ptr}` | Closure (env RC + elem RC) | Direct (16B) | `let s = "hi"; x -> s.length() + x` |
| T15 | Option\<int\> | `{i64, i64}` | None | Direct | `Some(42)` / `None` |
| T16 | Option\<str\> | `{i64, {i64, i64, ptr}}` | InlineEnum + FatPointer | Indirect | `Some("hello")` / `None` |
| T17 | Map (str keys) | `{i64, i64, ptr}` | HeapPointer + key/val RC | Indirect (24B) | `{"a": 1, "b": 2}` |
| T18 | Tuple (mixed) | `{{i64, i64, ptr}, i64}` | AggregateFields | Indirect | `("hello", 42)` |

---

## 04.2 Feature Dimension Definitions

These are the language features that exercise different compiler paths (monomorphization, codegen patterns, ARC insertion, control flow).

| ID | Feature | What It Tests | Compiler Path |
|----|---------|---------------|---------------|
| F1 | Let binding | Value construction and binding | Value emission, alloca/store |
| F2 | Function parameter | Passing values to functions | ABI, borrow elision, RC inc/dec |
| F3 | Function return | Returning values from functions | Return ABI (sret vs register) |
| F4 | Closure capture | Capturing values in closure env | Env alloc, type propagation |
| F5 | Closure parameter | Passing values through closure call | Indirect call, trampoline |
| F6 | Pattern matching | Match expressions on values | Decision tree, extractvalue |
| F7 | If/else branching | Using values in conditionals | Select vs branch, phi merge |
| F8 | For loop iteration | Iterating over collections of values | Iterator protocol, element borrow |
| F9 | Loop accumulation | Accumulating values across iterations | Phi nodes, mutable binding |
| F10 | Generic instantiation | Using values as generic type params | Monomorphization |
| F11 | Struct field | Storing values in struct fields | GEP, aggregate construction |
| F12 | Sum type payload | Values as enum variant payloads | Tag + payload layout |
| F13 | Derived Eq | Equality comparison on values | `$eq` method codegen |
| F14 | List element | Values stored in list elements | Element-level RC, iteration |
| F15 | ? propagation | Using ? on Option/Result containing values | Early return, cleanup |
| F16 | Recursion | Passing values through recursive calls | Stack frames, RC across calls |
| F17 | Higher-order | Values passed through fn-typed params | Indirect call, type erasure |
| F18 | Multiple values | Multiple values of same type in scope | RC tracking, drop ordering |
| F19 | Break/continue | Early exit from loops with fat values in scope | Cleanup on break, continue semantics |
| F20 | Derived Clone | Cloning values containing fat pointer fields | Clone codegen, RC increment |

---

## 04.3 Matrix Implementation

**File(s):** `compiler/ori_llvm/tests/aot/fat_matrix/`, `tests/spec/fat_matrix/`

Not every cell in the 18x20 matrix (360 cells) needs a separate test file. Group tests by feature dimension — each test file exercises one feature across multiple type categories.

**Test file structure:**

```
compiler/ori_llvm/tests/aot/fat_matrix/
  f01_let_binding.rs        # T4-T18 in let bindings
  f02_function_param.rs     # T4-T18 as function params
  f03_function_return.rs    # T4-T18 as return values
  f04_closure_capture.rs    # T4-T18 as closure captures
  f05_closure_param.rs      # T4-T18 through closure calls
  f06_pattern_matching.rs   # T4-T18 in match expressions
  f07_branching.rs          # T4-T18 in if/else
  f08_for_loop.rs           # T6-T7, T17 as iteration sources; T4-T18 as elements
  f09_loop_accumulation.rs  # T4-T18 accumulated in loops
  f10_generics.rs           # T4-T18 through generic functions
  f11_struct_field.rs       # T4-T18 as struct fields
  f12_sum_payload.rs        # T4-T18 as sum type payloads
  f13_derived_eq.rs         # T4-T18 in derived Eq
  f14_list_element.rs       # T4-T18 as list elements
  f15_question_mark.rs      # T4-T18 in ? propagation
  f16_recursion.rs          # T4-T18 through recursive calls
  f17_higher_order.rs       # T4-T18 through higher-order functions
  f18_multiple_values.rs    # Multiple T4-T18 in same scope
  f19_break_continue.rs     # T4-T18 in loops with break/continue
  f20_derived_clone.rs      # T4-T18 cloned via derived Clone
```

Each test file is a Rust AOT test that:
1. Compiles an Ori program exercising the feature with each type
2. Runs it via eval AND AOT
3. Asserts identical exit codes
4. Runs under Valgrind for fat pointer types (T4-T18) -- this is mandatory per Section 04.4

- [x] Create the `fat_matrix/` test directory structure — `fat_matrix/mod.rs` + `main.rs` registration (2026-03-18)
- [x] Implement F01 (let binding) tests — 15 tests: T4-T18 (SSO, heap str, list scalar/fat, struct scalar/fat, Option int/str, map, tuple, closure no/scalar/fat capture, multi-fat, rebind). All pass debug+release. (2026-03-18)
- [x] Implement F02 (function parameter) tests — 12 tests: T4-T18 as params, plus heap str reuse (RC inc) and multi-fat params. All pass debug+release. (2026-03-18)
- [x] Implement F03 (function return) tests — 11 tests: T4-T18 returned from functions, plus chained return. All pass debug+release. (2026-03-18)
- [x] Implement F04 (closure capture) tests — 12 tests: T4-T18 captured in closures (SSO, heap, list scalar/fat, struct scalar/fat, Option int, map, multi, passed-as-arg, in-loop). **BUG FOUND AND FIXED**: closure env drop function used `ori_rc_dec` on collection data ptrs — drop function expected `{len, cap, data}` struct but received raw buffer pointer → SIGSEGV. Fix: dispatch to `emit_buffer_rc_dec_list_or_set`/`emit_buffer_rc_dec_map` for collection captures in `closures.rs:generate_env_drop_fn()`. All pass debug+release. (2026-03-18)
- [x] Implement F05 (closure parameter) tests — 12 tests written. 5 pass, **7 FAIL (BUG-04-01)**: heap str, list scalar, list fat, struct scalar, struct fat, Option<int>, map all leak 1 RC allocation. SSO str, tuple, multi-fat SSO, higher-order, fat-capture+param pass. (2026-03-18)
- [x] Implement F06 (pattern matching) tests — 14 tests. All pass after BUG-04-02 fix. 6 new matrix tests for multi-field variant offset: str-first, fat-middle, fat-last, multi-fat, fat-scalar-fat, heap str. (2026-03-18)
- [x] Implement F07 (branching) tests — 11 tests: T4-T18 in if/else (str, heap str, list scalar/fat, struct scalar/fat, Option int/str, map, tuple, nested). All pass debug+release. (2026-03-18)
- [x] Implement F08 (for loop iteration) tests — 10 tests: T6-T17 iterating collections (list scalar/fat do/yield/break/two-iter, struct scalar/fat, map, nested, yield transform). All pass debug+release. (2026-03-18)
- [x] Implement F09 (loop accumulation) tests — 4 tests: scalar sum, list lengths, map values, function calls on fat values. All pass debug+release. (2026-03-18)
- [x] Implement F10 (generic instantiation) tests — 10 tests written. 9 pass, **1 FAIL (BUG-04-01)**: `test_fm_generic_with_operation` leaks — list passed through generic `apply<T>(f: (T) -> int, x: T)` leaks 1 RC allocation. Identity generics with all fat types pass. (2026-03-18)
- [x] Implement F11 (struct field) tests — 8 tests: str, heap str, list scalar/fat, nested fat, multi fat, field passed to fn, map field. **All pass.** (2026-03-18)
- [x] Implement F12 (sum type payload) tests — 8 tests: str, heap str, list scalar/fat, struct fat, multi-variant, None variant, payload passed to fn. **All pass.** Note: variant punning `Text(content:)` doesn't parse — used positional `Text(content)` workaround (BUG-04-06). (2026-03-18)
- [x] Implement F13 (derived Eq) tests — 8 tests written. 4 pass, **4 FAIL (BUG-04-03)**: struct with `[int]` field, struct with `[str]` field, nested struct with fat Inner, Option<str> comparison — all return wrong exit codes. Struct with str+int, direct str, multi-fat-field struct, heap str struct all pass. (2026-03-18)
- [x] Implement F14 (list element) tests — 6 tests: [str], [[int]], [Named], [Option<str>], two-iterations, yield. All pass debug+release. (2026-03-18)
- [x] Implement F15 (? propagation) tests — 7 tests written. 5 pass, **2 FAIL (BUG-04-04)**: Option<str> with `?` — LLVM module verification failure: return type mismatch `{i64, {i64, i64, ptr}}` vs `{i64, i64}`. Option<int> with `?`, fat-in-scope cleanup, multiple `?` all pass. (2026-03-18)
- [x] Implement F16 (recursion) tests — 6 tests: str in scope, str param, list param, struct fat return, Option return, mutual recursion. **All pass.** (2026-03-18)
- [x] Implement F17 (higher-order) tests — 8 tests: str fn, list fn, lambda fat capture, called-twice, compose, struct fat, map, different fns. **All pass.** (2026-03-18)
- [x] Implement F18 (multiple values) tests — 5 tests: multi-str, multi-list, multi-struct, multi-map, mixed fat types. All pass debug+release. (2026-03-18)
- [x] Implement F19 (break/continue) tests — 6 tests: break from [str], continue [str], break with inner fat, continue with inner fat, break in for-yield, break nested loops. All pass debug+release. (2026-03-18)
- [x] Implement F20 (derived Clone) tests — 8 tests written. 4 pass, **4 FAIL (BUG-04-05)**: Clone of struct with `[int]`, struct with `[str]`, nested struct with fat Inner, struct with map — all double-free (`ori_rc_dec called on already-freed allocation`). Struct with str, heap str, multi-fat-fields, independence test all pass. (2026-03-18)
- [x] All tests pass in both eval and AOT (2026-03-19) — 181/181 pass, 0 ignored

### Bugs Found by Matrix (2026-03-18)

All 20 feature test files written. 181 total tests: **181 pass, 0 fail, 0 ignored** (after fixes). 6 distinct bugs found, all 6 fixed:

- [x] **BUG-04-01**: Closure/generic parameter RC leak — **FULLY FIXED** (2026-03-19). Phase 1: Changed `is_owned_position` for `ApplyIndirect` to return `false` — lambda callees don't own params, caller must emit RcDec. Fix: `ori_arc/src/ir/instr.rs`. All 12 F05 tests pass. Phase 2 (2026-03-19): Fixed monomorphized generic RC leak. Root cause: ARC IR call sites use original name `"apply"`, but interprocedural contracts are keyed under monomorphized name `"apply$m$Lint"` → ownership lookup falls to default `Owned` for all args → caller doesn't emit RcDec → leak. Fix: `emit_arg_ownership()` now builds a reverse mapping from monomorphized names to original names, adding contract entries under both. Conservative merge when multiple monomorphizations exist. Fix: `ori_arc/src/aims/emit_rc/arg_ownership.rs`. All 181 fat_matrix tests pass (0 ignored).
- [x] **BUG-04-02**: Multi-field variant match crash — **FIXED** (2026-03-18). Root cause: 5 codegen locations used field INDEX as i64 slot offset, but fat types (str = 3 slots) need cumulative byte offsets. Fix: use `compute_variant_field_offsets()` (already correct in `drop_enum.rs`) across construction (`construction.rs`), projection fast+slow paths (`instr_dispatch.rs`), and RC inc/dec (`rc_helpers.rs`). All use `gep(i8_ty, ...)` with byte offsets instead of `gep(i64_ty, ...)` with slot indices. 6 new matrix tests added: str-first, fat-middle, fat-last, multi-fat, fat-scalar-fat interleave, heap str. All 14 F06 tests pass debug+release. Valgrind clean.
- [x] **BUG-04-03**: Derived Eq wrong results on collections/nested — **PARTIALLY FIXED** (2026-03-19, reopened by TPR: BUG-04-03b list/set non-scalar, BUG-04-03c map composite thunks). Phase 1: Added `ori_list_eq_scalar` runtime function + `TypeInfo::List/Set` handling in `emit_field_operation` + ABI fixup in `emit_method_call_for_derive` for Indirect params. Phase 2 (2026-03-19): Fixed Option<str>/Result/List inline `==` by adding `emit_element_equals()` dispatch in `emit_comparison_via_trait()` — handles compound types that lack compiled derived Eq methods. Fixed map `==` by adding `ori_map_eq` runtime function (entry-by-entry comparison with `key_eq`/`key_hash`/`val_eq` callbacks) + `TypeInfo::Map` arm in `emit_field_operation` + thunk generation for derive codegen. Files: `ori_rt/src/map/mod.rs`, `ori_llvm/src/codegen/arc_emitter/operators/mod.rs`, `ori_llvm/src/codegen/arc_emitter/builtins/compound_traits.rs`, `ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls.rs`, `ori_llvm/src/codegen/derive_codegen/field_ops.rs`, `ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`. All 3 previously-ignored BUG-04-03 tests now pass (180/181, 1 remaining = BUG-04-01).
- [x] **BUG-04-04**: Option<str> with `?` LLVM IR type mismatch — **FIXED** (2026-03-19). `lower_try` used scrutinee type (`Option<str>`) instead of function return type (`Option<int>`) for early-return None construction. Fix: added `return_type` field to `ArcLowerer`, used it in the Option branch of `lower_try`. Files: `ori_arc/src/lower/{expr/mod.rs, mod.rs, calls/lambda.rs, collections/mod.rs}`. All 7 F15 tests pass.
- [x] **BUG-04-05**: Derived Clone double-free on collections/nested/map — **FIXED** (2026-03-19). Clone codegen `compile_clone_fields` was an identity-return stub. Fix: iterate struct fields and emit per-field RC increment (SSO-aware for str, `ori_list_rc_inc` for list/set, `ori_rc_inc` for map, recursive for nested structs/tuples/options). Fix: `ori_llvm/src/codegen/derive_codegen/bodies.rs`. RC trace confirms perfect balance. Remaining F20 test failures are BUG-04-03 (Eq interference).
- [x] **BUG-04-06**: Variant punning in match patterns doesn't parse — **FIXED** (2026-03-19). Added punning detection in `parse_variant_inner_patterns()`: when `ident:` is followed by `,` or `)`, desugars to `Binding(name)`. Also implemented call argument punning (`f(x:)` → `f(x: x)`) in `parse_call_args()` per the approved argument-punning proposal. Parser-only changes — no IR/type checker/evaluator modifications needed. Files: `ori_parse/src/grammar/expr/postfix.rs`, `ori_parse/src/grammar/expr/patterns/match_patterns.rs`. Spec tests: `tests/spec/declarations/argument_punning.ori`, `tests/spec/patterns/variant_punning.ori`. Note: full named field access (`Circle(radius: r)` with reordering support) requires IR changes — tracked separately.

### Bugs Found by Valgrind Layer (2026-03-19)

- [x] **BUG-04-07**: AIMS lambda naming collision — **FIXED** (2026-03-19). Root cause: all lambdas were named `__lambda_{idx}` per parent function, colliding in the interprocedural AIMS contract map. LLVM codegen's global counter renaming created a mismatch between the contract map keys and the renamed function names, causing wrong/missing ownership annotations → double-free. Fix: lambda names now include parent function name at lowering time (`__lambda_{parent}_{idx}`), making them globally unique. Removed the global counter renaming in LLVM define_phase (no longer needed). Files: `ori_arc/src/lower/calls/lambda.rs`, `ori_llvm/src/codegen/function_compiler/define_phase.rs`, `ori_llvm/src/codegen/function_compiler/mod.rs` (removed `lambda_counter` field).
- [x] **BUG-04-08**: AIMS FIP verification panic with multiple lambdas — **FIXED** (2026-03-19). Same root cause as BUG-04-07 — contract map collision caused wrong ownership → inflated reuse counts → FIP assertion failure. Fixed by the same globally unique lambda naming in BUG-04-07.

### Bugs Reopened by TPR (2026-03-19)

BUG-04-03 was marked "FULLY FIXED" but TPR review found two remaining gaps. Both are eval/AOT parity failures — AOT silently returns wrong results.

- [x] **BUG-04-03b**: List/set derived equality fails for non-scalar elements — **FIXED** (2026-03-19). Added `ori_list_eq_deep(a, b, elem_size, elem_eq_fn)` runtime function with per-element callback comparison. Updated `emit_list_eq_call()` to use `ori_list_eq_deep` when element type needs deep comparison (str, list, set, map, struct, enum) via `needs_deep_comparison()` check. Reuses existing `get_or_create_derive_eq_thunk()` for thunk generation (str → `ori_str_eq`). Files: `ori_rt/src/list/query.rs`, `ori_llvm/src/codegen/derive_codegen/field_ops.rs`, `ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`.
  - [x] Implement `ori_list_eq_deep(a, b, elem_size, elem_eq_fn)` runtime function in `ori_rt/src/list/query.rs`
  - [x] Declare in `ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`
  - [x] Update `emit_list_eq_call()` in `field_ops.rs` to use element-wise comparison when element type is non-scalar (str, nested struct, Option, etc.)
  - [x] Reuse existing `get_or_create_derive_eq_thunk()` for elem_eq thunks (str already supported)
  - [x] AOT test: `#derive(Eq)` struct with `[str]` field using heap-backed strings (>23 bytes) — `test_fm_eq_list_heap_str`
  - [x] AOT test: `#derive(Eq)` struct with `[str]` multiple heap strings + inequality — `test_fm_eq_list_multiple_heap_str`
  - [x] AOT test: `#derive(Eq)` empty list equality — `test_fm_eq_list_empty`
  - [x] AOT test: `#derive(Eq)` mixed SSO and heap strings — `test_fm_eq_list_mixed_str`
  - [x] Valgrind test: list equality with heap strings — 0 errors, 0 leaks (f13_derived_eq.ori updated)
  - [x] Dual-exec verify: eval == AOT for all new test cases
  - Note: `[[int]]` and `[Option<str>]` element types require BUG-04-03c (composite thunk extension) — thunk generation returns None for those types
- [x] **BUG-04-03c**: Map derived equality fails for ALL non-primitive value types — **FIXED** (2026-03-19). Extended `get_or_create_derive_eq_thunk()` to handle List/Set (generates thunks calling `ori_list_eq_scalar`/`ori_list_eq_deep`), Struct/Enum (generates thunks calling compiled derived `eq` method). Extended `get_or_create_derive_hash_thunk()` with constant-0 hash thunk for composite types (correct but O(n) — acceptable since composite map keys are rare). Files: `ori_llvm/src/codegen/derive_codegen/field_ops.rs`.
  - [x] Extend `get_or_create_derive_eq_thunk()` — added `get_or_create_list_eq_thunk()` for List/Set elements, `get_or_create_user_type_eq_thunk()` for Struct/Enum
  - [x] Extend `get_or_create_derive_hash_thunk()` — added `get_or_create_constant_hash_thunk()` for composite types (returns 0)
  - [x] AOT test: `#derive(Eq)` struct with `{str: [int]}` map field — `test_fm_eq_map_composite_list_val`
  - [x] AOT test: `#derive(Eq)` struct with `{int: str}` map field (heap strings) — `test_fm_eq_map_str_val`
  - [x] AOT test: `#derive(Eq)` struct with `{str: int}` map field (base case) — `test_fm_eq_map_primitive_val`
  - [x] Valgrind test: map equality with composite values — 0 errors, 0 leaks (f13_derived_eq.ori updated)
  - [x] Dual-exec verify: eval == AOT for all new test cases

**Priority ordering:** F04 (closure capture) and F08/F14 (iteration/list elements) first -- these are the known bug areas. Then F02/F03 (function param/return) as the most common fat pointer operations. Then the rest.

### Coverage Tracking

Maintain a coverage matrix in this file. Mark each cell as:
- `PASS` -- test exists and passes
- `FAIL` -- test exists and fails (with bug ID)
- `N/A` -- combination doesn't apply (e.g., T1 scalar int x F08 for loop iteration -- tested elsewhere)
- `---` -- not yet implemented

Initial state: all `---`. Target state: all `PASS` or `N/A`.

**Final state (2026-03-19): All applicable cells PASS. 181 tests, 0 failures.**

| Feature | T4 SSO | T5 Heap | T6 [int] | T7 [str] | T8 Struct | T9 FatStruct | T10-T11 Sum | T12-T14 Closure | T15 Opt\<int\> | T16 Opt\<str\> | T17 Map | T18 Tuple |
|---------|--------|---------|----------|----------|-----------|-------------|-------------|-----------------|---------------|---------------|---------|-----------|
| F01 Let | PASS | PASS | PASS | PASS | PASS | PASS | N/A | PASS | PASS | PASS | PASS | PASS |
| F02 Param | PASS | PASS | PASS | PASS | PASS | PASS | N/A | N/A | N/A | N/A | PASS | PASS |
| F03 Return | PASS | PASS | PASS | PASS | N/A | PASS | N/A | N/A | N/A | PASS | PASS | PASS |
| F04 Capture | PASS | PASS | PASS | PASS | PASS | PASS | N/A | N/A | PASS | N/A | PASS | PASS |
| F05 ClosParam | PASS | PASS | PASS | PASS | N/A | PASS | N/A | N/A | N/A | N/A | PASS | PASS |
| F06 Match | PASS | PASS | N/A | N/A | N/A | PASS | PASS | N/A | N/A | PASS | N/A | N/A |
| F07 Branch | PASS | PASS | PASS | PASS | N/A | PASS | N/A | N/A | PASS | PASS | PASS | PASS |
| F08 ForLoop | N/A | N/A | PASS | PASS | N/A | PASS | N/A | N/A | N/A | N/A | PASS | N/A |
| F09 Accum | N/A | N/A | PASS | PASS | N/A | PASS | N/A | N/A | N/A | N/A | PASS | N/A |
| F10 Generic | PASS | PASS | PASS | PASS | N/A | PASS | N/A | N/A | N/A | N/A | PASS | PASS |
| F11 Field | PASS | PASS | PASS | PASS | N/A | PASS | N/A | N/A | N/A | N/A | PASS | N/A |
| F12 Payload | PASS | PASS | PASS | N/A | N/A | N/A | PASS | N/A | N/A | N/A | N/A | N/A |
| F13 Eq | PASS | PASS | PASS | N/A | N/A | PASS | N/A | N/A | N/A | N/A | N/A | N/A |
| F14 ListElem | N/A | N/A | PASS | PASS | N/A | PASS | N/A | N/A | N/A | PASS | N/A | N/A |
| F15 ? | N/A | N/A | PASS | N/A | N/A | N/A | N/A | N/A | PASS | PASS | N/A | N/A |
| F16 Recurse | PASS | PASS | PASS | N/A | N/A | PASS | N/A | N/A | N/A | PASS | N/A | N/A |
| F17 HigherOrd | PASS | PASS | PASS | N/A | N/A | PASS | N/A | PASS | N/A | N/A | PASS | N/A |
| F18 MultiVal | N/A | PASS | PASS | N/A | N/A | PASS | N/A | N/A | N/A | N/A | PASS | N/A |
| F19 Break | N/A | N/A | N/A | PASS | N/A | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| F20 Clone | PASS | PASS | PASS | N/A | N/A | PASS | N/A | N/A | N/A | N/A | N/A | N/A |

---

## 04.4 Valgrind Verification Layer

**File(s):** `tests/valgrind/fat_matrix/`

Spec tests and AOT tests verify behavioral correctness (right exit code). Valgrind verifies memory correctness (no leaks, no double-frees, no use-after-free).

For every test in the matrix that involves fat pointer types (T4-T18), create a corresponding Valgrind test:

- [x] Create `tests/valgrind/fat_matrix/` directory (2026-03-19)
- [x] Write Valgrind test runner that builds each `.ori` program and runs under `valgrind --leak-check=full --show-leak-kinds=all` — 20 standalone `.ori` files (f01-f20), one per feature dimension, each testing T4-T18 fat pointer types. Uses existing `diagnostics/valgrind-aot.sh` infrastructure. (2026-03-19)
- [x] All T4-T18 tests pass Valgrind with "0 errors from 0 contexts" — 20/20 PASS (2026-03-19)
- [x] Add to `diagnostics/valgrind-aot.sh` so the fat matrix is included in manual Valgrind runs — updated default behavior to recursively find `.ori` files in `tests/valgrind/` subdirectories (2026-03-19)

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-005][high]` `compiler/ori_llvm/src/codegen/derive_codegen/field_ops/mod.rs:424` — Derived equality still mis-sizes struct list/map elements that contain fat fields, so AOT deep equality walks collection storage with the wrong stride and can return the wrong answer even though Section 04 is marked complete.
  Evidence: `compute_elem_size()` hard-codes `TypeInfo::Struct` as `fields.len() as i64 * 8`, but fat-field structs are stored at their real LLVM layout size, not "8 bytes per field". For `type Named = { name: str, id: int }`, list storage is 32 bytes per element (`str` = 24 bytes, `int` = 8), while the derived-equality callback path advances by 16 bytes.
  Resolved: Fixed on 2026-03-19. Two-part fix: (1) `compute_elem_size()` now routes `TypeInfo::Struct` through `TypeLayoutResolver::type_store_size()` instead of `fields.len() * 8`, matching Option/Result/Tuple handling. (2) `emit_element_equals()`, `emit_element_compare()`, and `emit_element_hash()` in `compound_traits.rs` now handle `TypeInfo::Struct` and `TypeInfo::Enum` by calling compiled derived methods via `ctx.method_functions` lookup with proper ABI parameter passing. 5 new AOT tests (list of fat struct, struct with list of fat struct, empty list of fat struct, map with fat struct value, direct list equality). Valgrind clean, dual-exec verified, 13,312 tests pass.

- [x] `[TPR-04-001][high]` `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:94` — BUG-04-03 is not fully fixed for derived equality on lists/sets whose elements are non-scalar or heap-backed.
  Evidence: `emit_field_operation()` routes every list/set field through `emit_list_eq_call()`, which hard-wires `ori_list_eq_scalar`; both the helper comment and the runtime implementation state this is only correct for scalar elements (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:222`, `compiler/ori_rt/src/list/query.rs:208`). Fresh verification on 2026-03-19 showed an eval/AOT parity failure for `#derive(Eq) type Words = { items: [str] }` when the list holds two independently allocated heap strings: `timeout 150 ori run /tmp/derive-eq.ori` exited 0, while `timeout 150 ori build /tmp/derive-eq.ori -o /tmp/derive-eq && /tmp/derive-eq` exited 1 and logged `icmp on non-int operands — returning false`. The current F13 matrix never covers this case: `test_fm_eq_struct_list_fat` only exercises SSO strings (`compiler/ori_llvm/tests/aot/fat_matrix/f13_derived_eq.rs:55`) and the Valgrind twin only uses `[int]` lists (`tests/valgrind/fat_matrix/f13_derived_eq.ori:10`).
  Resolved: Validated and accepted on 2026-03-19. BUG-04-03 reopened — implementation tasks added to 04.3 as BUG-04-03b (list/set deep equality).
- [x] `[TPR-04-002][high]` `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:272` — Derived equality on map fields still fails for composite key/value types because the new map callback path only supports primitive and `str` thunks.
  Evidence: `emit_map_eq_call()` requires `key_eq`, `key_hash`, and `val_eq` thunks, but `get_or_create_derive_eq_thunk()` / `get_or_create_derive_hash_thunk()` return `None` for anything outside `{int,float,bool,char,byte,str}` (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:340`, `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:400`). Fresh verification on 2026-03-19 showed another eval/AOT mismatch for `#derive(Eq) type Wrapper = { m: {str: Option<str>} }`: `timeout 150 ori run /tmp/map-eq.ori` exited 0, while `timeout 150 ori build /tmp/map-eq.ori -o /tmp/map-eq && /tmp/map-eq` exited 1. The added F13 coverage only exercises maps with primitive values (`compiler/ori_llvm/tests/aot/fat_matrix/f11_struct_field.rs:122` and the Section 04 summary at `plans/fat-pointer-hardening/section-04-test-matrix.md:165`), so this unsupported composite-value path is still untested.
  Resolved: Validated and accepted on 2026-03-19. BUG-04-03 reopened — implementation tasks added to 04.3 as BUG-04-03c (map composite thunks).
- [x] `[TPR-04-003][high]` `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:276` — The reopened BUG-04-03b fix still misses non-scalar wrapper elements such as `Option<str>`, so Section 04 is being marked complete while eval/AOT parity remains broken for list equality.
  Evidence: the new deep-comparison gate only treats `Str`, `List`, `Set`, `Map`, `Struct`, and `Enum` as non-scalar (`needs_deep_comparison()`), and the thunk generator still only handles `str`, `List/Set`, `Struct/Enum`, and primitives (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:278`, `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:369`). Fresh verification on 2026-03-19 still reproduces the bug for `#derive(Eq) type Wrap = { items: [Option<str>] }`: `timeout 150 ori run /tmp/list-opt-eq.ori` exited 0, while `timeout 150 ori build /tmp/list-opt-eq.ori -o /tmp/list-opt-eq && /tmp/list-opt-eq` exited 1. The newly added F13/Valgrind coverage only exercises `[str]` lists, not wrapper element types (`compiler/ori_llvm/tests/aot/fat_matrix/f13_derived_eq.rs:180`, `tests/valgrind/fat_matrix/f13_derived_eq.ori:55`).
  Resolved: Fixed on 2026-03-19. Extended `needs_deep_comparison()`, `get_or_create_derive_eq_thunk()`, `get_or_create_derive_hash_thunk()`, and `emit_field_operation()` to handle `TypeInfo::Option`, `TypeInfo::Result`, and `TypeInfo::Tuple`. Added structural equality thunks for all three wrapper types. 6 new AOT tests + 3 Valgrind tests added.
- [x] `[TPR-04-004][high]` `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:295` — The reopened BUG-04-03c fix still does not support wrapper-valued maps such as `{str: Option<str>}` even though the section now marks the map thunk work complete.
  Evidence: `emit_map_eq_call()` still depends on `get_or_create_derive_eq_thunk()` / `get_or_create_derive_hash_thunk()` for key/value callbacks, but those helpers still return `None` for `Option`, `Result`, and tuple payloads (`compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:327`, `compiler/ori_llvm/src/codegen/derive_codegen/field_ops.rs:389`). Fresh verification on 2026-03-19 still shows eval/AOT divergence for `#derive(Eq) type Wrap = { m: {str: Option<str>} }`: `timeout 150 ori run /tmp/map-opt-eq.ori` exited 0, while `timeout 150 ori build /tmp/map-opt-eq.ori -o /tmp/map-opt-eq && /tmp/map-opt-eq` exited 1. The new tests stop at `{str: [int]}` and `{int: str}` (`compiler/ori_llvm/tests/aot/fat_matrix/f13_derived_eq.rs:262`, `compiler/ori_llvm/tests/aot/fat_matrix/f13_derived_eq.rs:283`), so the wrapper-value case remains uncovered.
  Resolved: Fixed on 2026-03-19. Same fix as TPR-04-003 — wrapper type thunk generation now covers map key/value comparison.

---

## 04.N Completion Checklist

- [x] All 20 feature test files created — f01-f20 in `compiler/ori_llvm/tests/aot/fat_matrix/`, 204 tests total (2026-03-19, +10 from TPR-04-003/004/005)
- [x] All applicable type x feature cells are PASS — 204/204 pass (2026-03-19)
- [x] No FAIL cells remain — all 10 bugs (BUG-04-01 through BUG-04-08, BUG-04-03b/c) plus TPR-04-003/004/005 fixed (2026-03-19)
- [x] Valgrind clean on all fat pointer tests (T4-T18) — 20/20 Valgrind tests in `tests/valgrind/fat_matrix/` pass with 0 errors (2026-03-19)
- [x] `./test-all.sh` green (includes all new tests) — 13,312 pass, 0 fail (2026-03-19)
- [x] Coverage matrix in this file is fully populated — 20x12 matrix, all cells PASS or N/A (2026-03-19)
- [x] No `---` (not yet implemented) cells remain for applicable combinations (2026-03-19)
- [x] `diagnostics/dual-exec-verify.sh` passes on all fat matrix `.ori` programs (eval == AOT) — 20/20 verified, 0 mismatches (2026-03-19)
- [x] `ORI_CHECK_LEAKS=1` reports 0 leaks on all fat matrix AOT binaries — 20/20 clean (2026-03-19)

**Exit Criteria:** `timeout 150 cargo test -p ori_llvm fat_matrix` passes all tests (0 failures) AND `diagnostics/valgrind-aot.sh tests/valgrind/fat_matrix/` reports "0 errors" for every test program AND `diagnostics/dual-exec-verify.sh` reports 0 mismatches.
