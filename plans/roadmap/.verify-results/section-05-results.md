# Section 05: Type Declarations -- Verification Results

**Verified**: 2026-03-19
**Branch**: experiment/aims
**Method**: Test execution + test source review + AOT coverage audit

---

## Summary

- **126/289 items checked (43%)** per plan
- Sampled all major checked subsections; verified all unchecked items in active subsections
- **Key findings**: Plan has significant staleness -- several `[ ]` items are actually working, and several `[x]` items reference Rust tests that do not exist. AOT coverage is much better than the plan records.

---

## 5.1 Struct Types

### `[x]` Parse struct type -- VERIFIED
- **Tests**: `tests/spec/declarations/struct_types.ori` -- 30+ tests covering basic, single field, empty, nested (3 levels), many fields, mixed types, generic, shorthand, spread (basic, override, multiple, nested, generic), destructuring, equality, public visibility, special names, expressions (if, match), function fields
- **Run**: 4181 passed, 0 failed, 42 skipped
- **Rust test issue**: Plan references `ori_parse/src/grammar/attr.rs -- test_parse_struct_type` but NO such test exists. STALE REFERENCE. Actual parser tests for structs are compositional tests in `ori_parse/src/tests/compositional.rs` (9 struct-related tests).

### `[x]` Register struct in type environment -- VERIFIED
- Tests verify struct creation and field access work, confirming registration.

### `[x]` Struct literals -- VERIFIED
- All tests construct struct literals. Comprehensive coverage.

### `[x]` Type check struct literals -- VERIFIED
- Mixed types, generic structs, nested structs all type check.

### `[x]` Shorthand `Point { x, y }` -- VERIFIED
- `test_shorthand_init`, `test_mixed_shorthand` both pass.

### `[x]` Field access -- VERIFIED
- Deep nesting (`l3.inner.inner.value`), field chaining (`c.ceo.name`) tested.

### `[x]` Destructuring -- VERIFIED
- `test_destructure`, `test_destructure_partial`, `test_destructure_rename` pass.

### `[x]` AOT Tests -- VERIFIED
- 30 AOT struct tests pass (construction, field access, update syntax, nested 3 levels, function params/returns, closures, derived Eq, string fields, loops, if expressions).
- Plan says "1 ignored" but `cargo test -p ori_llvm -- structs` shows 32 passed, 0 ignored (stress test included). No ignored tests found -- plan may be stale.

### `[ ]` LLVM codegen (shorthand, destructuring) -- CONFIRMED INCOMPLETE
- Plan correctly identifies shorthand and destructuring as missing from AOT. AOT struct tests use explicit field initialization, not shorthand. No destructuring tests in AOT.

---

## 5.2 Sum Types (Enums)

### `[x]` Parse sum types -- VERIFIED
- `tests/spec/declarations/sum_types.ori` -- 30+ tests covering unit variants (Color, Direction, Toggle, Status), single-field (MyOption, Message), multi-field (Shape, Point3D, Event, Response), nested sum types, generic sum types (MyResult, MyOptional), recursive (LinkedList, Tree), exhaustive match, wildcard, variable binding, public sum types, function params/returns, collections.
- **Run**: 4181 passed, 0 failed, 42 skipped

### `[x]` Unit, single-field, multi-field, struct variants -- VERIFIED
- Comprehensive test coverage for all variant types.

### `[x]` Pattern matching on variants -- VERIFIED
- Exhaustive match, wildcard, nested match, variable binding, recursive Expr eval all tested.

### PLAN ERROR: "Derive(Eq) for sum types NOT working"
- **WRONG** -- `tests/spec/declarations/sum_types.ori` lines 437-463 test `#[derive(Eq)]` on `EqColor` (unit variants) and `EqOption` (payload variants). Both pass.
- `tests/spec/declarations/attributes.ori` lines 105-131 test `DerivedColor` and `DerivedOption` with derive(Eq). Both pass.
- `tests/spec/traits/derive/eq_sum.ori` exists with dedicated sum type Eq tests.
- The plan's claim at lines 142 and 346 is STALE. Derive(Eq) for sum types IS working.

### PLAN ERROR: "No AOT coverage yet" for sum types
- **WRONG** -- `cargo test -p ori_llvm -- enum` shows 17 passing tests including `test_aot_enum_unit_variants`, `test_aot_enum_construction`, `test_aot_enum_mixed_variants`, `test_aot_recursive_enum_linked_list`, `test_aot_recursive_enum_tree`, `test_aot_enum_as_param_and_return`, `test_aot_derive_eq_payload_sum_type`, plus ARC and IR quality tests.

### Rust test references -- STALE
- Plan references `ori_parse/src/grammar/attr.rs -- test_parse_sum_type`. No such test exists.

---

## 5.3 Newtypes

### `[x]` Parse, distinct identity, wrapping/unwrapping -- VERIFIED
- `tests/spec/types/newtypes.ori` -- 9 tests (UserId, Email, Age, Score). Construction, `.unwrap()`, distinct types, function params, computation.
- **Run**: 4181 passed, 0 failed, 42 skipped

### `[ ]` Change `.unwrap()` to `.inner` -- CONFIRMED INCOMPLETE
- All newtype tests use `.unwrap()`. No `.inner` accessor anywhere in test suite. Genuinely pending.

### `[ ]` No AOT coverage -- CONFIRMED INCOMPLETE
- `cargo test -p ori_llvm -- newtype` returns 0 tests.

### Rust test references -- STALE
- Plan references `ori_parse/src/grammar/attr.rs -- test_parse_newtype`. No such test exists.

---

## 5.4 Generic Types

### `[x]` Parse, multiple params, instantiation -- VERIFIED
- `tests/spec/types/generic.ori` -- 14 tests (Box<int/str/list>, Pair<A,B>, Container<T>, Wrapper<T>, nested generics, chained field access, method calls on generic fields, multiple instances).
- **Run**: 4181 passed, 0 failed, 42 skipped

### `[ ]` Constrained `<T: Trait>` -- PARTIALLY WORKING
- Plan says Ori tests exist: `tests/spec/declarations/attributes.ori` has `GenericDerived<T: Eq>` which works. However, no dedicated constrained generics test suite exists. The constraint checking is minimal.

### `[ ]` Multiple bounds `<T: A + B>` -- CONFIRMED INCOMPLETE
- `tests/spec/declarations/where_clause.ori` has commented-out tests for `T: Eq + Clone`. Genuinely pending.

### Rust test references -- STALE
- Plan references `ori_parse/src/grammar/attr.rs -- test_parse_generic_type_with_bounds`. No such test exists. Actual generic type parser tests are in `ori_parse/src/grammar/ty/tests.rs` (`test_parse_generic_type`).

---

## 5.5 Compound Types

### `[ ]` List, Map, Set, Range, Function type inference -- CONFIRMED INCOMPLETE
- `tests/spec/types/collections.ori` is entirely commented out (416 lines). Confirmed still pending.

### `[x]` Tuple parser + field access -- VERIFIED
- `tests/spec/types/tuple_types.ori` -- 33+ tests covering unit, single, pair, triple, quad, mixed, nested, collections, Option/Result, destructuring, function params/returns, type inference, equality, field access (.0, .1, nested with parens).
- Parser tests exist: `test_tuple_field_access`, `test_chained_tuple_field_access_with_parens` in `ori_parse/src/tests/parser.rs`.
- **Run**: 4181 passed, 0 failed, 42 skipped

### `[x]` AOT Tuple tests -- VERIFIED
- 27 passed, 2 ignored in `cargo test -p ori_llvm -- tuples`. Good coverage.

### `[ ]` Chained field access without parens -- CONFIRMED INCOMPLETE
- Known limitation documented in tuple_types.ori: `t.0.1` fails because lexer tokenizes `0.1` as float.

---

## 5.6 Built-in Generic Types

### `[x]` Option<T> -- VERIFIED
- Used throughout test suite. Comprehensive coverage.

### PLAN ERROR: "No AOT coverage yet" for Option
- **WRONG** -- `cargo test -p ori_llvm -- option` shows 94 passing tests (3 ignored). Extensive coverage including Option equals, unwrap, is_none, nested option hash, option tuple equals, and iterator RC matrix tests.

### `[x]` Result<T, E> -- VERIFIED
- Used in test suite.

### PLAN ERROR: "No AOT coverage yet" for Result
- **WRONG** -- `cargo test -p ori_llvm -- result` shows 48 passing tests. Coverage includes result compare, hash, is_ok/is_err, unwrap, try/result projection.

### `[x]` Ordering -- VERIFIED
- `tests/spec/types/ordering/methods.ori` -- 32 tests pass.

### PLAN ERROR: "No AOT coverage yet" for Ordering
- **WRONG** -- `cargo test -p ori_llvm -- ordering` shows 7 passing tests including `test_aot_ordering_compare`.

### `[ ]` Error type -- NOT VERIFIED
- No dedicated Error type tests found in spec. Genuinely incomplete.

### `[ ]` Channel<T> -- CONFIRMED INCOMPLETE
- No channel tests found.

---

## 5.7 Capability Unification (`: Trait` clause on types)

### `[ ]` All items -- CONFIRMED NOT STARTED
- No `:` trait clause syntax found anywhere in test suite. All derive tests use `#[derive(...)]` syntax.
- Section is genuinely not started.

### PLAN ERROR: "#derive(Printable) not fully implemented"
- **WRONG** -- `tests/spec/traits/derive/printable.ori` has 6 active tests that all pass (struct basic, str field, nested, payloadless variant, payload variant, multi-payload variant).
- `tests/spec/declarations/attributes.ori` has `test_derive_printable` (line 350) which runs and passes (NOT skipped).
- Plan line 362-368 says Printable is "skipped" and "not fully implemented" -- this is STALE.

### PLAN ERROR: "#derive(Default) not tested"
- **WRONG** -- `tests/spec/traits/derive/default.ori` has 6 active tests that all pass (basic int fields, multiple types, single field, float fields, Eq integration, nested default).
- Plan line 370-375 says Default is "Not tested" -- this is STALE.

---

## 5.8 Visibility

### `[x]` pub type, private by default -- VERIFIED
- `tests/spec/declarations/struct_types.ori` has `pub type PublicStruct`.
- `tests/spec/declarations/sum_types.ori` has `pub type PublicStatus`.
- `tests/spec/modules/use_imports.ori` tests cross-module visibility.
- **Run**: 4181 passed, 0 failed, 42 skipped

---

## 5.9 Associated Functions

### `[x]` Type.method() for user types -- VERIFIED
- `tests/spec/types/associated_functions.ori` -- 10 tests covering Point.origin(), Point.new(), Builder pattern (new + chaining), Counter (zero, starting_at, increment, value), Rectangle (square, from_dimensions, unit), Pair (create vs sum/swap), Duration.from_seconds, Size.from_megabytes.
- **Run**: 4181 passed, 0 failed, 42 skipped

### `[x]` Self return type -- VERIFIED
- Counter.zero(), Point.origin(), Builder.new() all return Self and work.

### `[x]` Instance vs associated distinction -- VERIFIED
- `test_instance_vs_associated` -- Pair.create() (type) vs p.sum() (value).

### `[ ]` Generic associated functions -- CONFIRMED INCOMPLETE
- No test for `Option<int>.some(value: 42)` pattern. Genuinely untested.

### `[ ]` Trait associated functions -- CONFIRMED INCOMPLETE
- No test for `trait Default { @default () -> Self }` pattern. Genuinely untested.

### `[ ]` AOT tests -- CONFIRMED INCOMPLETE
- No dedicated AOT associated function tests found.

---

## 5.10 Section Completion Checklist

### Corrections needed to checklist:

1. `[ ] Derive(Eq) for sum types -- not working` -- **WRONG**, it IS working (tests pass in both sum_types.ori and eq_sum.ori)
2. `[ ] Derive: Printable, Default -- not working` -- **WRONG**, both work and have dedicated test files with passing tests
3. `[ ] LLVM codegen for all type declarations -- no dedicated test files` -- **PARTIALLY WRONG**, significant AOT coverage exists: 30 struct tests, 17 enum tests, 27 tuple tests, 94 Option tests, 48 Result tests, 7 Ordering tests, 39 derive tests

---

## Stale Rust Test References

The following Rust tests are referenced in the plan but DO NOT EXIST:

| Referenced Test | Referenced Location | Actual Status |
|---|---|---|
| `ori_parse/src/grammar/attr.rs -- test_parse_struct_type` | 5.1 | Does not exist |
| `ori_parse/src/grammar/attr.rs -- test_parse_sum_type` | 5.2 | Does not exist |
| `ori_parse/src/grammar/attr.rs -- test_parse_newtype` | 5.3 | Does not exist |
| `ori_parse/src/grammar/attr.rs -- test_parse_generic_type_with_bounds` | 5.4 | Does not exist |

The actual parser tests are:
- Derive: `grammar/attr/tests.rs -- test_parse_derive_attribute`
- Type parsing: `grammar/ty/tests.rs -- test_parse_generic_type`, `test_parse_named_type`, etc.
- Struct literals: `tests/compositional.rs -- test_struct_literals_complex`, `test_struct_literal_contexts`

---

## Verification Summary by Classification

| Classification | Count | Items |
|---|---|---|
| VERIFIED | 28 | 5.1 structs (all checked items), 5.2 sum types (all), 5.3 newtypes (checked items), 5.4 generics (checked items), 5.5 tuples (checked items), 5.6 Option/Result/Ordering, 5.8 visibility, 5.9 associated functions (checked items) |
| STALE TEST REF | 4 | Four non-existent Rust test names referenced in plan |
| PLAN STALE | 6 | Derive(Eq) sum types marked broken (works), Printable marked not implemented (works), Default marked not tested (has tests), Option/Result/Ordering AOT marked missing (extensive coverage exists) |
| CONFIRMED INCOMPLETE | 12 | `.inner` migration, multiple bounds, compound type inference, chained tuple field access, Error type, Channel, capability unification syntax, generic associated functions, trait associated functions, newtype AOT, shorthand/destructure AOT |

---

## Recommendations

1. **Update plan staleness**: Fix 6 stale claims about Derive(Eq) sum types, Printable, Default, and AOT coverage for Option/Result/Ordering/enums
2. **Fix Rust test references**: 4 non-existent test names need correction to actual test names
3. **Update completion percentage**: With the stale items corrected, the actual completion is higher than 43%
4. **Priority gaps**: Newtype AOT tests and compound type inference are the biggest genuine gaps
