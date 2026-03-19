---
section: "04"
title: "Combinatorial Test Matrix"
status: in-progress
goal: "Every cell of {type categories} x {language features} is covered by an AOT test, ensuring no intersection of fat pointers with any feature is untested"
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Type Category Definitions"
    status: not-started
  - id: "04.2"
    title: "Feature Dimension Definitions"
    status: not-started
  - id: "04.3"
    title: "Matrix Implementation"
    status: in-progress
  - id: "04.4"
    title: "Valgrind Verification Layer"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Combinatorial Test Matrix

**Status:** Not Started
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
- [x] Implement F06 (pattern matching) tests — 8 tests written. 7 pass, **1 FAIL (BUG-04-02)**: `test_fm_match_multi_arm_fat` crashes with misaligned pointer dereference in `ori_rc_dec` — multi-field variant `Rect(name: str, w: int, h: int)` with fat payload. Unit variants, Option, struct destructure, tuple, nested match all pass. (2026-03-18)
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
- [ ] All tests pass in both eval and AOT

### Bugs Found by Matrix (2026-03-18)

All 20 feature test files written. 175 total tests: **169 pass, 6 fail** (after fixes). 6 distinct bugs found, 3 fixed:

- [x] **BUG-04-01**: Closure/generic parameter RC leak — **FIXED** (2026-03-19). Changed `is_owned_position` for `ApplyIndirect` to return `false` — lambda callees don't own params, caller must emit RcDec. Fix: `ori_arc/src/ir/instr.rs`. All 12 F05 tests pass. F10 `generic_with_operation` still leaks (different root cause: monomorphized generic with indirect call inside — tracked separately).
- [ ] **BUG-04-02**: Multi-field variant match crash — `Rect(name: str, w: int, h: int)` match causes misaligned pointer dereference in `ori_rc_dec`. Affects 1 test in F06. Single-field payload variants and unit variants work. Root cause: likely wrong GEP offset calculation for fat pointer fields in multi-field sum type variants during match destructure.
- [x] **BUG-04-03**: Derived Eq wrong results on collections/nested — **PARTIALLY FIXED** (2026-03-19). Added `ori_list_eq_scalar` runtime function + `TypeInfo::List/Set` handling in `emit_field_operation` + ABI fixup in `emit_method_call_for_derive` for Indirect params. Fixes: `ori_rt/src/list/query.rs`, `ori_llvm/src/codegen/derive_codegen/field_ops.rs`, `ori_llvm/src/codegen/derive_codegen/mod.rs`. 6 more tests pass. Remaining: `eq_option_str` (Option<str> == uses ARC IR `==` primop → icmp on aggregate, separate from derived Eq), `clone_map_field` (map clone RC inc emitted but map equality not implemented for verification).
- [ ] **BUG-04-04**: Option<str> with `?` LLVM IR type mismatch — `?` on `Option<str>` produces "Function return type does not match operand type of return inst" — generates `{i64, {i64, i64, ptr}}` return but function signature expects `{i64, i64}`. Affects 2 tests in F15. `Option<int>` with `?` works fine. Root cause: `?` lowering doesn't account for fat Option payload when generating the early-return path.
- [x] **BUG-04-05**: Derived Clone double-free on collections/nested/map — **FIXED** (2026-03-19). Clone codegen `compile_clone_fields` was an identity-return stub. Fix: iterate struct fields and emit per-field RC increment (SSO-aware for str, `ori_list_rc_inc` for list/set, `ori_rc_inc` for map, recursive for nested structs/tuples/options). Fix: `ori_llvm/src/codegen/derive_codegen/bodies.rs`. RC trace confirms perfect balance. Remaining F20 test failures are BUG-04-03 (Eq interference).
- [ ] **BUG-04-06**: Variant punning in match patterns doesn't parse — spec says `Circle(radius:)` = `Circle(radius: radius)` as punning syntax, but parser rejects with "expected `,` or `)`". All F06/F12 tests work around with positional binding `Text(content)` instead. Root cause: parser `parse_match_pattern` doesn't handle trailing `:` as punning in variant patterns.

**Priority ordering:** F04 (closure capture) and F08/F14 (iteration/list elements) first -- these are the known bug areas. Then F02/F03 (function param/return) as the most common fat pointer operations. Then the rest.

### Coverage Tracking

Maintain a coverage matrix in this file. Mark each cell as:
- `PASS` -- test exists and passes
- `FAIL` -- test exists and fails (with bug ID)
- `N/A` -- combination doesn't apply (e.g., T1 scalar int x F08 for loop iteration -- tested elsewhere)
- `---` -- not yet implemented

Initial state: all `---`. Target state: all `PASS` or `N/A`.

---

## 04.4 Valgrind Verification Layer

**File(s):** `tests/valgrind/fat_matrix/`

Spec tests and AOT tests verify behavioral correctness (right exit code). Valgrind verifies memory correctness (no leaks, no double-frees, no use-after-free).

For every test in the matrix that involves fat pointer types (T4-T18), create a corresponding Valgrind test:

- [ ] Create `tests/valgrind/fat_matrix/` directory
- [ ] Write Valgrind test runner that builds each `.ori` program and runs under `valgrind --leak-check=full --show-leak-kinds=all`
- [ ] All T4-T18 tests pass Valgrind with "0 errors from 0 contexts"
- [ ] Add to `diagnostics/valgrind-aot.sh` so the fat matrix is included in manual Valgrind runs

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All 20 feature test files created
- [ ] All applicable type x feature cells are PASS
- [ ] No FAIL cells remain
- [ ] Valgrind clean on all fat pointer tests (T4-T18)
- [ ] `./test-all.sh` green (includes all new tests) -- debug AND release
- [ ] Coverage matrix in this file is fully populated
- [ ] No `---` (not yet implemented) cells remain for applicable combinations
- [ ] `diagnostics/dual-exec-verify.sh` passes on all fat matrix `.ori` programs (eval == AOT)
- [ ] `ORI_CHECK_LEAKS=1` reports 0 leaks on all fat matrix AOT binaries

**Exit Criteria:** `timeout 150 cargo test -p ori_llvm fat_matrix` passes all tests (0 failures) AND `diagnostics/valgrind-aot.sh tests/valgrind/fat_matrix/` reports "0 errors" for every test program AND `diagnostics/dual-exec-verify.sh` reports 0 mismatches.
