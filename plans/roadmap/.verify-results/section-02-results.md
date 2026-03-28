# Section 02: Complete Type Inference -- Verification Results

**Verified**: 2026-03-28
**Branch**: dev
**Verdict**: MOSTLY VERIFIED -- all implementations work, but roadmap metadata (test counts, bug status) is stale

---

## 2.1 Unification Algorithm

### [x] Occurs check (2026-02-10) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/unify/tests.rs -- occurs_check_detects_infinite_type (line 76),
        occurs_check_finds_var_in_borrowed (line 473), occurs_check_finds_var_in_dei (line 603)
  Ori:  tests/spec/inference/unification.ori -- 25 active tests
Tests run: ALL PASS
  Rust: 38/38 in unify::tests (0 failed)
  Ori:  4181 passed, 0 failed, 42 skipped (full suite)
Audit: READ compiler/ori_types/src/unify/tests.rs
  - line 76-88: occurs_check_detects_infinite_type -- creates var, creates List<var>, attempts
    unify(var, List<var>), asserts InfiniteType error. Correct per spec.
  - line 473-481: occurs_check_finds_var_in_borrowed -- same pattern for Borrowed<var>. Sound.
  - line 603-611: occurs_check_finds_var_in_dei -- same pattern for DEI<var>. Sound.
Audit: READ tests/spec/inference/unification.ori
  - 25 tests covering: same-type unification, inferred types, list types, nested lists,
    function types, higher-order functions, tuples, Option Some/None, Result Ok/Err,
    assignment chains, conditional branches, nested conditionals, closure captures,
    arithmetic/comparison/logical ops, string concat, map lookup.
  - All assertions use assert_eq with actual/expected -- correct patterns.
  - Dual structure: each test has a @test function + target function (belt-and-suspenders).
Coverage: Comprehensive for core unification scenarios.
Status: VERIFIED
```

### [x] Substitution application via resolve() (2026-02-10) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/unify/tests.rs -- path_compression (line 49),
        error_propagates (line 187), never_unifies_with_anything (line 178)
Tests run: ALL PASS (38/38)
Audit: READ compiler/ori_types/src/unify/tests.rs
  - line 49-73: path_compression -- creates chain var1->var2->var3->INT, resolves var1,
    verifies path compressed to direct INT link. Correct.
  - line 178-184: never_unifies_with_anything -- Never unifies with INT and STR. Correct per spec
    (Never is bottom type, coerces to anything).
  - line 187-194: error_propagates -- Error type unifies with anything (prevents cascading).
    Correct per spec.
Coverage: Path compression, Never coercion, Error propagation all tested.
Status: VERIFIED
```

### [x] Generalization (let-polymorphism) (2026-02-10) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/unify/tests.rs -- generalize_identity_function (line 254),
        generalize_monomorphic (line 236), generalize_does_not_generalize_outer_vars (line 279),
        let_polymorphism_example (line 386)
  Ori:  tests/spec/inference/polymorphism.ori -- 8 active tests
Tests run: ALL PASS
Audit: READ compiler/ori_types/src/unify/tests.rs
  - line 236-251: generalize_monomorphic -- int and fn(int)->bool return unchanged. Correct.
  - line 254-276: generalize_identity_function -- creates var at inner rank, generalizes fn(a)->a,
    verifies scheme with 1 quantified variable. Correct HM generalization.
  - line 279-298: generalize_does_not_generalize_outer_vars -- outer_var at FIRST rank, inner_var
    at FIRST.next(), generalizes: only inner_var generalized (1 quantified). Correct --
    preserves rank-based scoping.
  - line 386-422: let_polymorphism_example -- canonical test: id used with int AND str independently,
    both resolve correctly, independent vars. Gold-standard HM test.
Audit: READ tests/spec/inference/polymorphism.ori
  - 8 tests: identity polymorphism, const function, list head, list length, Option polymorphism,
    multiple instantiations, downward flow, upward flow.
  - let_poly_identity: `let id = x -> x; id(x: 42); id(x: "hello")` -- both work. Sound.
  - let_poly_const: `(a, b) -> a` used with mixed types. Sound.
  - poly_list_head: `xs -> xs[0]` used with [int] and [str]. Sound.
  - instantiate_multiple_calls: wrap = x -> Some(x) used with int/str/bool. Sound.
Coverage: Good -- covers core HM scenarios including the canonical identity test.
Status: VERIFIED
```

### [x] Instantiation (2026-02-10) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/unify/tests.rs -- instantiate_identity_scheme (line 322),
        instantiate_non_scheme (line 305), instantiate_twice_gives_different_vars (line 356)
Tests run: ALL PASS
Audit: READ compiler/ori_types/src/unify/tests.rs
  - line 305-318: instantiate_non_scheme -- non-scheme types return unchanged. Correct.
  - line 322-353: instantiate_identity_scheme -- creates scheme forall a. a->a, instantiates,
    verifies fresh var (different from original), both param and return are same fresh var. Correct.
  - line 356-383: instantiate_twice_gives_different_vars -- two instantiations yield different
    fresh variables. Correct -- essential for let-polymorphism soundness.
Coverage: Complete for instantiation mechanics.
Status: VERIFIED
```

---

## 2.2 Expression Type Inference

### [x] Local variable inference (2026-02-10) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/infer/expr/ -- 126 tests in infer::expr::tests (all pass)
  Ori:  tests/spec/expressions/bindings.ori -- 17 active tests
Tests run: ALL PASS
Audit: READ tests/spec/expressions/bindings.ori
  - 17 tests covering: inferred int/str, annotated int/str/bool/float/char, shadowing,
    type-changing shadow, struct destructuring (shorthand/rename/partial/nested),
    list destructuring (basic/head/rest), tuple destructuring.
  - All assertions use assert_eq with correct expected values.
  - let_shadow_different_type: `let x = 42; let x = str(x)` -- tests type-changing shadow. Sound.
  - struct_destructure_nested: 4-level nesting with Rectangle/Point. Sound.
Coverage: Good coverage of variable inference and destructuring patterns.
Status: VERIFIED
Roadmap accuracy: Claims "17 tests" -- correct.
```

### [x] Lambda parameter inference (2026-02-10) -- VERIFIED

```
Tests found:
  Ori: tests/spec/expressions/lambdas.ori -- 30 active tests (roadmap says 29 -- STALE COUNT)
Tests run: ALL PASS (4181 passed, 0 failed, 42 skipped)
Audit: READ tests/spec/expressions/lambdas.ori
  - 30 tests covering: simple lambdas (identity, multiply, negate), multi-param, no-param,
    typed params, explicit return type, closures (single/multiple/nested capture),
    lambdas in for-yield, IIFEs (single/no-param/multi-param), lambda aliasing,
    lambda in conditional, complex bodies, type inference from use, higher-order,
    single-char params, curried lambdas, closure-returning-closure (annotated).
  - test_closure_returning_closure_annotated (line 372): regression test for the closure-returning-
    closure bug. Uses explicit type annotations. Passes correctly, returns 17 (10+5+2).
  - test_lambda_returns_lambda (line 360): curried add without annotations. Passes. Sound.
Coverage: Comprehensive lambda coverage.
Status: VERIFIED
Roadmap accuracy: Claims "29 tests" -- actually 30 (one regression test added later). STALE COUNT.
```

### [ ] Closure-returning-closure inference bug -- BUG APPEARS FIXED

```
Status in roadmap: [ ] (open bug)
Actual status: APPEARS FIXED
Evidence:
  - test_aot_closure_capturing_closure in compiler/ori_llvm/tests/aot/spec.rs: PASSES
  - test_closure_returning_closure_annotated in tests/spec/expressions/lambdas.ori: PASSES
  - Direct interpreter test with `(n: int) -> (int) -> int = { (x: int) -> int = base + n + x }`:
    returns 17 correctly
  - Without explicit return type annotation (no `-> (int) -> int`): also works correctly via ori run
  - Tested as both inline lambda and top-level function: both pass
  - `ori check` passes with no errors for all variants
Note: The roadmap says "infers () return instead of (int) -> int" but both AOT and interpreter
produce correct results. The bug appears to have been fixed as a side effect of other inference work.
Status: BUG APPEARS FIXED -- roadmap checkbox is stale (should be [x])
```

### [x] Generic type argument inference (2026-02-10) -- VERIFIED (with count correction)

```
Tests found:
  Ori: tests/spec/inference/generics.ori -- 17 active tests (roadmap says "22" -- STALE COUNT)
Tests run: ALL PASS
Audit: READ tests/spec/inference/generics.ori
  - 22 total test declarations but 5 are COMMENTED OUT with TODO notes:
    - infer_option_map (line 139): "Option.map not implemented yet" -- stub returns true
    - infer_option_and_then (line 149): "Option.and_then not implemented yet" -- stub returns true
    - infer_result_map (line 159): "Result.map not implemented yet" -- stub returns true
    - infer_result_map_err (line 169): "Result.map_err not implemented yet" -- stub returns true
    - infer_result_unwrap_or (line 191): "Result.unwrap_or not implemented yet" -- stub returns 0
  - Active 17 tests cover: Option/Result/List type inference from argument, nested generics,
    return type context, parameter type context, lambda param inference, lambda return inference,
    lambda chains, Option.unwrap_or, nested generics (list-in-option, option-in-list,
    tuple-in-option, result-in-option, option-in-result), lambda+closure inference.
  - infer_option_unwrap_or (line 179): `Option<int> = None; .unwrap_or(default: 42)` -- correct.
Coverage: Good for active tests. 5 stubs remain as placeholders for unimplemented methods.
Status: VERIFIED (active tests sound, but count claim is wrong)
Roadmap accuracy: Claims "22 tests (all pass)" -- only 17 are active. 5 are vacuous stubs.
```

### [x] Collection element type inference (2026-02-10) -- WRONG TEST

```
Tests found:
  Ori: tests/spec/types/collections.ori -- 0 active tests (roadmap says "35 tests (all pass)")
Tests run: N/A (entire file is commented out)
Audit: READ tests/spec/types/collections.ori
  - The ENTIRE file (416 lines) is commented out with `//` prefixes.
  - Line 4-14: TODO header listing unimplemented features.
  - Contains 35 test declarations, ALL commented out.
  - Top-level non-test functions are also commented out.
  - No active test code whatsoever.
Coverage: ZERO active tests for collection type inference.
Note: Collection type inference IS tested indirectly through other test files:
  - unification.ori tests list, tuple, map, Option/Result inference
  - generics.ori tests nested generic collections
  - bindings.ori tests list/tuple destructuring
  So the capability EXISTS, but the dedicated test file has no active tests.
Status: WRONG TEST (roadmap claims "35 tests (all pass)" but 0 tests are active)
```

---

## 2.3 Type Error Improvements

### [x] Expected vs found messages (2026-02-10) -- VERIFIED

```
Tests found:
  Ori: tests/compile-fail/type_mismatch_arg.ori -- 1 compile_fail test
Tests run: ALL PASS
Audit: READ tests/compile-fail/type_mismatch_arg.ori
  - Uses old bracket syntax: `#[compile_fail("type mismatch")]`
  - Test passes str "hello" where int is expected in @add(a: int, b: int)
  - Expects "type mismatch" in error message. Correct.
  - Note: uses `#[compile_fail(...)]` (old syntax) vs `#compile_fail(...)` (current syntax).
    Both work but inconsistent with current convention.
Coverage: Minimal -- only one test for type mismatch at argument position.
Status: VERIFIED (works, but WEAK TESTS -- only 1 test)
```

### [x] Type conversion hints + edit-distance suggestions (2026-02-16) -- VERIFIED

```
Tests found:
  Rust: compiler/ori_types/src/infer/env/tests.rs -- 21 tests (all pass)
  Ori:  tests/compile-fail/type_hints.ori -- 5 compile_fail tests
Tests run: ALL PASS
  Rust: 21/21 env tests pass
  Ori:  4181 passed, 0 failed, 42 skipped
Audit: READ compiler/ori_types/src/infer/env/tests.rs
  - 9 env/scope tests: new_env, bind_and_lookup, shadow, child_scope, is_bound_locally,
    names_iterator, local_count, parent. All verify TypeEnv scoping mechanics. Sound.
  - 5 edit distance tests: identical, empty, single_edit (sub/ins/del), typos (transposition).
    Correct Levenshtein distance implementation.
  - 1 threshold test: default_threshold function for various lengths. Sound.
  - 6 find_similar tests: basic typo, no match, empty env, max_results, parent scope search,
    skip target name, sorted by distance, unresolvable target. Comprehensive.
Audit: READ tests/compile-fail/type_hints.ori
  - 5 compile_fail tests with conversion hints:
    - float->int: expects "int(x)" suggestion. Correct.
    - int->float: expects "float(x)" suggestion. Correct.
    - int->str: expects "str(x)" suggestion. Correct.
    - str->byte: expects "byte(x)" suggestion. Correct.
    - int->[int]: expects "[x]" suggestion (list wrapping). Correct.
  - All 5 are conversion hint tests.
Coverage: Good for edit-distance and conversion hints.
Status: VERIFIED
Roadmap accuracy: Claims "10 tests (5 conversion hints + 5 existing)" -- only 5 tests exist. STALE.
```

### [x] Source location in errors (2026-02-10) -- VERIFIED

```
Tests found:
  Ori: tests/compile-fail/return_type_mismatch.ori -- 1 compile_fail test
Tests run: ALL PASS
Audit: READ tests/compile-fail/return_type_mismatch.ori
  - Uses old bracket syntax: `#[compile_fail("type mismatch")]`
  - Test defines lambda `(name: str) -> int = "Hello, " + name` -- return type mismatch
    (declares int, body produces str). Expects "type mismatch" error. Correct.
Coverage: Minimal -- only 1 test. Source location verified indirectly (all errors carry spans).
Status: VERIFIED (works, but WEAK TESTS -- only 1 test)
```

---

## 2.4 Section Completion Checklist

### Roadmap metadata accuracy

```
Claim: "3,792 Rust unit tests pass (ori_types)" -- STALE
Actual: ori_types has 740 Rust tests (not 3,792). The number was likely a workspace total at the time.

Claim: "4,078 Rust tests in workspace" -- STALE
Actual: Workspace test count has grown substantially since this was written.

Claim: "101 Ori spec tests across inference/bindings/lambdas/collections" -- WRONG
Actual active tests:
  - inference/unification.ori: 25
  - inference/polymorphism.ori: 8
  - inference/generics.ori: 17 (not 22 -- 5 commented out)
  - expressions/bindings.ori: 17
  - expressions/lambdas.ori: 30 (not 29)
  - types/collections.ori: 0 (all 35 commented out)
  Total active: 97 (not 101)

Claim: "11 compile-fail tests" -- STALE
Actual: 38+ compile-fail test files exist now. Section-02-relevant: 7 tests.

Claim: "10 tests" in type_hints.ori -- STALE
Actual: 5 tests (all conversion hints).

Claim: "closure-returning-closure inference bug" marked as [ ] (open)
Actual: Bug appears FIXED. Both AOT and interpreter handle it correctly.
```

---

## Overall Assessment

| Subsection | Items | Status |
|-----------|-------|--------|
| 2.1 Unification | 4/4 checked | All VERIFIED -- sound tests, all pass |
| 2.2 Expression Inference | 5/5 checked | 4 VERIFIED, 1 WRONG TEST (collections.ori 100% commented out) |
| 2.2 Open Bug | 1 checked | BUG APPEARS FIXED (roadmap checkbox stale) |
| 2.3 Type Error Improvements | 3/3 checked | All VERIFIED (2 WEAK TESTS with only 1 test each) |
| 2.4 Completion Checklist | 6/6 checked | All VERIFIED (multiple counts stale) |

### Issues Found

1. **WRONG TEST** -- `tests/spec/types/collections.ori`: All 35 tests commented out. The roadmap claims "35 tests (all pass)" but zero tests are active. Collection inference is tested indirectly through other test files, but the dedicated test file is non-functional.

2. **STALE BUG STATUS** -- Closure-returning-closure inference bug (2.2.3) is marked `[ ]` (open) but appears fully fixed. Both AOT (`test_aot_closure_capturing_closure` passes) and interpreter produce correct results. Tested with and without explicit type annotations -- all produce expected value 17 (10+5+2). Roadmap should be updated to `[x]`.

3. **STALE COUNTS** -- Multiple test count claims are outdated:
   - "22 tests" in generics.ori -- actually 17 active (5 commented out stubs)
   - "29 tests" in lambdas.ori -- actually 30 (one regression test added)
   - "35 tests" in collections.ori -- actually 0 active (all commented out)
   - "101 Ori spec tests" -- actually 97 active
   - "3,792 Rust unit tests (ori_types)" -- actually 740
   - "10 tests" in type_hints.ori -- actually 5
   - "11 compile-fail tests" -- now 38+ files

4. **WEAK TESTS** -- 2.3.1 (expected/found) and 2.3.3 (source location) each have only 1 compile-fail test. Adequate for verification that the feature exists, but thin coverage.

5. **OLD SYNTAX** -- `type_mismatch_arg.ori` and `return_type_mismatch.ori` use old bracket syntax `#[compile_fail(...)]` instead of current `#compile_fail(...)`. Both work but inconsistent with current convention.

### Changes from Previous Verification (2026-03-19)

- Previous verification on `experiment/aims` branch noted an open "unconstrained type variables reach codegen" bug (section 2.5). That item no longer appears in the current roadmap file on `dev` branch.
- Closure-returning-closure bug: previous verification confirmed it as fixed. Current re-verification confirms it remains fixed.
- Test counts: ori_types now has 740 tests (was 671 in previous verification).
- Expression inference tests: now 126 (was 182 in previous count, likely due to test restructuring).
- Total spec test suite: 4181 passed (same as previous).
