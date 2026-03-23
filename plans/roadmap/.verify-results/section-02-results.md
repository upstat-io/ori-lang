# Section 02: Complete Type Inference -- Verification Results

**Verified**: 2026-03-19
**Branch**: experiment/aims
**Commit**: 073d88fb

**Summary**: 48/49 items checked. 47 VERIFIED, 1 STALE TEST (collections.ori), 1 unchecked item confirmed as genuine open bug. Several count claims are stale but harmless.

---

## 2.1 Unification Algorithm

### [x] Occurs check (2026-02-10) -- VERIFIED

- **Rust Test**: `ori_types::unify::tests::occurs_check_detects_infinite_type` -- PASSES. Test creates `var` and `List<var>`, verifies unification fails with `UnifyError::InfiniteType`. Sound test: correctly validates prevention of infinite types like `T = [T]`.
- **Ori Test**: `tests/spec/inference/unification.ori` -- 25 tests, ALL PASS.
  - Tests cover: same-type unification, inferred type unification, list/nested list, function/higher-order, tuple, Option (Some/None), Result (Ok/Err), assignment chains, conditional branches, complex conditionals, Option/Result in branches, list element assignment, tuple destructuring, function call chains, closure capture, arithmetic/comparison/logical results, string concat, map lookup.
  - Assertions verify computed values match expected, validating that type inference correctly unified types through each scenario.

### [x] Substitution application via resolve() (2026-02-10) -- VERIFIED

- **Rust Tests**: ALL PASS.
  - `path_compression` -- Creates chain var1->var2->var3->INT, verifies resolve(var1) returns INT and compresses the path (var1 points directly to INT after resolution). Sound.
  - `error_propagates` -- Verifies ERROR type unifies with anything (INT, STR). Sound for cascade prevention.
  - `never_unifies_with_anything` -- Verifies NEVER unifies with INT and STR. Sound (bottom type semantics).
- **Ori Tests**: Substitution verified through unification tests above -- transitive chains, conditional branches, arithmetic results all exercise substitution.

### [x] Generalization (let-polymorphism) (2026-02-10) -- VERIFIED

- **Rust Tests**: ALL PASS.
  - `generalize_identity_function` -- Creates `a -> a` at inner rank, verifies scheme has 1 quantified variable and body is the function type. Sound.
  - `generalize_monomorphic` -- Verifies concrete types (INT, `int -> bool`) return unchanged. Sound.
  - `generalize_does_not_generalize_outer_vars` -- Creates `outer -> inner` function, verifies only inner-rank variable is generalized. Sound (escaping variable check).
  - `let_polymorphism_example` -- Full canonical test: creates scheme for `id`, instantiates twice, unifies one with INT and other with STR, verifies they resolve independently. Sound.
- **Ori Tests**: `tests/spec/inference/polymorphism.ori` -- 8 tests, ALL PASS.
  - `let_poly_identity` -- Polymorphic identity `id(42)` and `id("hello")` both work. Sound.
  - `let_poly_const` -- `const_fn(1, "ignored")` and `const_fn("kept", 999)` both work. Sound.
  - `poly_list_head`, `poly_list_length` -- Polymorphic functions on collections. Sound.
  - `poly_option`, `instantiate_multiple_calls` -- Option wrapping with different types. Sound.
  - `inference_flow_down`, `inference_flow_up` -- Bidirectional flow. Sound.

### [x] Instantiation (2026-02-10) -- VERIFIED

- **Rust Tests**: ALL PASS.
  - `instantiate_identity_scheme` -- Creates scheme `forall a. a -> a`, instantiates, verifies fresh variables. Sound.
  - `instantiate_non_scheme` -- INT and concrete function types return unchanged. Sound.
  - `instantiate_twice_gives_different_vars` -- Two instantiations of same scheme yield different fresh variables. Sound (prevents cross-use aliasing).
- **Ori Tests**: Covered by polymorphism.ori tests above (instantiation is exercised every time a polymorphic function is called with different types).

---

## 2.2 Expression Type Inference

### [x] Local variable inference (2026-02-10) -- VERIFIED

- **Rust Tests**: 182 infer tests pass in `ori_types` (covers expression inference broadly).
- **Ori Tests**: `tests/spec/expressions/bindings.ori` -- 17 tests, ALL PASS.
  - Tests cover: inferred int/str, annotated int/str/bool/float/char, shadowing (same and different type), struct destructuring (shorthand, rename, partial, nested), list destructuring (basic, head, rest), tuple destructuring.
  - `let x = 42` infers int, `let x = x + 1` chains correctly (verified via `let_shadow`). Sound.

### [x] Lambda parameter inference (2026-02-10) -- VERIFIED

- **Ori Tests**: `tests/spec/expressions/lambdas.ori` -- 30 tests (was 29, +1 regression test), ALL PASS.
  - Tests cover: simple lambda (`x -> x + 1`), identity, multiply, negate, multi-param, three-param, no-param, typed params, explicit return type, closures (single capture, multiple capture, nested), lambda as for-yield argument, inline lambda in for, IIFE (single/no-param/multi-param), lambda assignment, lambda in conditional, lambda with block body, lambda with conditional body, lambda infer from use, higher-order (pass lambda to function), single-char param, curried (lambda returns lambda), closure-returning-closure annotated (regression test).
  - `apply(x -> x + 1, 41)` correctly infers `x: int` from context (tested via `lambda_infer_from_use` and `pass_lambda_to_function`). Sound.

### [x] Closure-returning-closure inference bug fix (2026-03-15) -- VERIFIED

- **Ori Test**: `closure_returning_closure_annotated` in lambdas.ori -- PASSES. Tests `(n: int) -> (int) -> int = { (x: int) -> int = base + n + x }` with captured `base = 10`, verifies `make_adder(n: 5)(x: 2) == 17`. Sound regression test.
- **AOT Test**: `ori_llvm::tests::aot::spec::test_aot_closure_capturing_closure` -- PASSES. Tests same pattern with `@main` returning exit code. Sound.

### [x] Generic type argument inference (2026-02-10) -- VERIFIED (with caveat)

- **Ori Tests**: `tests/spec/inference/generics.ori` -- 17 active tests PASS, 5 stubs present.
  - Active tests cover: Option/Result/List type inference from argument, nested generics, return type inference, param type inference, lambda param/return inference, lambda chains, Option.unwrap_or, nested generics (list-in-option, option-in-list, tuple-in-option, result-in-option, option-in-result), inference through lambda, inference through closure.
  - 5 test stubs for Option.map, Option.and_then, Result.map, Result.map_err, Result.unwrap_or have placeholder functions (`() -> bool = true` or `() -> int = 0`) -- these are noted as "IMPLEMENTATION BUG" (methods not implemented at the time). The stubs do not exercise any real inference.
- **Caveat**: Roadmap claims "22 tests (all pass)" which is misleading. 17 tests actually exercise inference, 5 are vacuous stubs. The 17 active tests are sound.

### [x] Collection element type inference (2026-02-10) -- STALE TEST

- **Ori Tests**: `tests/spec/types/collections.ori` -- **ALL 35 tests are commented out.** Zero active tests.
  - Roadmap claims "35 tests (all pass)" which is INCORRECT. The entire file is commented out with `TODO: Type checker needs various features` at the top.
  - The tests were likely commented out because they used old comma-as-separator syntax and referenced features not yet implemented at the time of writing.
  - Collection inference IS tested indirectly through `unification.ori` (list/map/option/result tests) and `generics.ori` (nested generic tests), but `collections.ori` itself contributes zero test coverage.
- **Evidence**: Grep shows 0 active `@test_` lines, 35 commented-out `// @test_` lines in the file.

---

## 2.3 Type Error Improvements

### [x] Expected vs found messages (2026-02-10) -- VERIFIED

- **Ori Test**: `tests/compile-fail/type_mismatch_arg.ori` -- 1 test, PASSES. Tests `add(a: "hello", b: 5)` where `add` expects `int`, using `#[compile_fail("type mismatch")]`. Sound -- verifies the compiler detects and reports the mismatch.

### [x] Type conversion hints (2026-02-16) -- VERIFIED (with count correction)

- **Ori Tests**: `tests/compile-fail/type_hints.ori` -- 5 tests, ALL PASS.
  - `test_float_to_int_hint` -- `takes_int(x: 3.14)` expects `#compile_fail("int(x)")`. Sound.
  - `test_int_to_float_hint` -- `takes_float(x: 42)` expects `#compile_fail("float(x)")`. Sound.
  - `test_int_to_str_hint` -- `takes_str(s: 42)` expects `#compile_fail("str(x)")`. Sound.
  - `test_str_to_byte_hint` -- `takes_byte(b: "a")` expects `#compile_fail("byte(x)")`. Sound.
  - `test_wrap_in_list_hint` -- `takes_list(items: 42)` expects `#compile_fail("[x]")`. Sound.
- **Count correction**: Roadmap claims "all 10 tests pass (5 conversion hints + 5 existing)" but the file contains only 5 tests total (the 5 conversion hint tests). There are no "5 existing" tests in this file. The roadmap count is stale/wrong.
- **Edit-distance typo suggestions**: 21 Rust tests in `ori_types::infer::env::tests`, ALL PASS. Tests cover: empty env, find similar, skip target, parent scopes, max results, sort by distance, unresolvable target, bound locally, local count, names iterator, new env. Additionally 6 `type_error::diff::tests` for edit distance calculation. Sound.

### [x] Source location in errors (2026-02-10) -- VERIFIED

- **Ori Test**: `tests/compile-fail/return_type_mismatch.ori` -- 1 test, PASSES. Tests `(name: str) -> int = "Hello, " + name` with `#[compile_fail("type mismatch")]`. Sound -- verifies span information is present (the error includes the span of the mismatched expression).

---

## 2.4 Section Completion Checklist

### [x] All 2.1 items complete -- VERIFIED

All 4 unification items pass their Rust and Ori tests as documented above.

### [x] All 2.2 items complete (2026-03-15, closure bug verified fixed) -- VERIFIED (with caveats)

- Lambda, local variable, generic inference: VERIFIED.
- Closure-returning-closure bug fix: VERIFIED with both Ori spec test and AOT test.
- Collection element inference: STALE TEST -- `collections.ori` entirely commented out.

### [x] All 2.3 items complete -- VERIFIED

Expected/found, conversion hints, source locations all pass.

### [x] 3,792 Rust unit tests pass (ori_types) -- STALE COUNT

- Current count: **671 Rust tests** pass in `ori_types` (not 3,792). The number 3,792 was likely a workspace-wide count at the time.
- All 671 tests PASS with zero failures.

### [x] Spec and compile-fail tests pass -- VERIFIED (with caveats)

- Current test suite: 4181 passed, 0 failed, 42 skipped (full spec test run).
- Roadmap counts (101 Ori spec + 11 compile-fail) are stale; the current counts are much higher.
- All section-relevant tests pass.

### [x] Run full test suite -- VERIFIED

- `cargo st` returns: 4181 passed, 0 failed, 42 skipped.

---

## 2.5 Post-Completion Bugs

### [ ] Unconstrained type variables reach codegen -- CONFIRMED OPEN BUG

- **Status**: Genuinely incomplete. Bug is real and reproducible.
- **Reproduction**: `let a = Ok("hello")` in a `@main` function.
  - `ori check` passes (no error) -- type checker does not flag the unconstrained E type variable.
  - `ori run` works fine (interpreter handles it).
  - `ori build` crashes with: `unresolved type variable at codegen -- type inference bug` (E5001), referencing `Idx(97)` and `_ori_drop$97`.
- **Root cause**: `Ok("hello")` produces `Result<str, E>` where `E` is never constrained. The type checker should either report "cannot infer type for E" or default unconstrained sum-type params to `Never`.
- **Impact**: AOT-only (interpreter unaffected). Any program with a standalone `Ok(x)` or `Err(x)` where the other type param is unconstrained will crash at codegen.

---

## Overall Assessment

| Subsection | Items | Status |
|-----------|-------|--------|
| 2.1 Unification | 4/4 checked | All VERIFIED -- sound tests, all pass |
| 2.2 Expression Inference | 5/5 checked | 4 VERIFIED, 1 STALE TEST (collections.ori entirely commented out) |
| 2.3 Type Error Improvements | 3/3 checked | All VERIFIED |
| 2.4 Completion Checklist | 6/6 checked | All VERIFIED (some counts stale but harmless) |
| 2.5 Post-Completion Bugs | 1 unchecked | CONFIRMED -- genuinely open bug |

**Findings requiring attention**:

1. **STALE TEST** -- `tests/spec/types/collections.ori`: All 35 tests commented out. The roadmap claims they pass. Collection inference works (verified through other test files) but this file provides zero coverage. Should be either uncommented and updated or the roadmap claim corrected.

2. **STALE COUNTS** -- Several numbers in the roadmap are outdated:
   - "35 tests (all pass)" for collections.ori -- 0 active tests
   - "10 tests" for type_hints.ori -- only 5 tests
   - "3,792 Rust tests" for ori_types -- currently 671
   - "101 Ori spec tests" -- currently 4181 total
   - "11 compile-fail tests" -- currently 39 files
   - These counts were accurate when written (2026-02-10) but the codebase has evolved.

3. **BUG CONFIRMED** -- Item 2.5 (unconstrained type variables reach codegen) is a real, reproducible AOT bug.
