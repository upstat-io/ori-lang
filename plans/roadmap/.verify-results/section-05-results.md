# Section 05 Verification Results: Type Declarations

**Verified**: 2026-03-28
**Verifier**: Claude Opus 4.6 (1M context)
**Branch**: dev (af8548b1)

## Files Loaded Before Verification

- `/home/eric/projects/ori_lang/CLAUDE.md` (full, 183 lines)
- All 19 rules files in `.claude/rules/`: types.md, typeck.md, eval.md, patterns.md, roadmap.md, spec.md, ori-lang.md, aot.md, llvm.md, diagnostic.md, parse.md, ir.md, compiler.md, cargo.md, registry.md, runtime.md, ori-syntax.md, arc.md, impl-hygiene.md, tests.md
- Section file: `plans/roadmap/section-05-type-declarations.md` (489 lines)
- Spec files: docs/ori_lang/v2026/spec/08-types.md (referenced)

---

## 5.1 Struct Types

### 5.1.1 Parse `type Name = { field: Type, ... }`

```
Tests found: tests/spec/declarations/struct_types.ori (488 lines, ~35 tests)
Tests run: 4181 passed, 0 failed, 42 skipped (full test suite via cargo st)
Audit: READ tests/spec/declarations/struct_types.ori — covers BasicStruct, SingleField,
  EmptyStruct, Nested, DeepNesting, ManyFields, MixedTypes, Shorthand, Spread (basic,
  override, multiple, nested, generic), Optional, List, Tuple, Function fields,
  Destructuring, Equality, Public, SpecialNames. 35+ test functions with assert_eq.
Parser Rust tests: Roadmap references test_parse_struct_type in grammar/attr.rs but this
  test does NOT exist. Closest: grammar/attr/tests.rs has test_parse_derive_attribute.
  tests/parser.rs has test_struct_literal_in_expression, test_struct_literal_in_if_then_body,
  test_if_condition_disallows_struct_literal. tests/compositional.rs has
  test_struct_literal_contexts, test_struct_literals_complex, test_struct_patterns_complex.
Matrix: int/float/bool/str/char types, nested structs, generics, Option/List/Tuple/Function fields.
  Missing: byte type, Duration/Size fields. Backend: eval only for spec tests.
AOT: compiler/ori_llvm/tests/aot/structs.rs — 30 tests all passing (2 string field tests,
  update syntax, nested, params/return, closures, control flow, derived Eq).
Semantic pin: test_struct_eq pins derived Eq behavior; test_spread_override_both pins spread semantics.
Status: VERIFIED — [x] items confirmed working. Roadmap claim test_parse_struct_type is STALE
  (test name does not exist in that file).
```

### 5.1.2 Register struct in type environment

```
Tests found: All struct tests verify type registration implicitly
Tests run: PASS (via struct_types.ori)
Audit: Type registration tested indirectly through all struct construction/access tests.
  No isolated Rust unit tests found for registration specifically.
Matrix: Covered by all struct variant tests (basic, nested, generic, etc.)
Semantic pin: NONE (implicit in all struct tests)
Status: VERIFIED — working but no isolated registration unit tests
```

### 5.1.3 Parse struct literals

```
Tests found: tests/spec/declarations/struct_types.ori, compiler/ori_parse/src/tests/parser.rs
Tests run: PASS
Audit: READ parser tests — test_struct_literal_in_expression, test_struct_literal_in_if_then_body,
  test_if_condition_disallows_struct_literal cover struct literal parsing.
Matrix: Basic, nested, computed fields, if/match contexts.
Semantic pin: test_if_condition_disallows_struct_literal is a negative pin.
Status: VERIFIED
```

### 5.1.4 Type check struct literals

```
Tests found: tests/spec/declarations/struct_types.ori, compiler/ori_llvm/tests/aot/structs.rs
Tests run: PASS (both eval and AOT)
Audit: AOT tests cover struct construction with int/bool/str/mixed fields, update syntax,
  nested structs, function params/returns, closures, derived Eq — 30 tests, 0 ignored.
Matrix: int/bool/str field types in AOT. Missing: float, char, byte in AOT.
Semantic pin: test_struct_derived_eq_string tests string field equality in AOT.
Status: VERIFIED — [x] items correct. [ ] LLVM Rust Tests claim "file does not exist" is
  accurate (no ori_llvm/tests/struct_tests.rs exists), but AOT tests exist in aot/structs.rs.
```

### 5.1.5 Shorthand `Point { x, y }`

```
Tests found: tests/spec/declarations/struct_types.ori — test_shorthand_init, test_mixed_shorthand
Tests run: PASS
Audit: Two tests cover shorthand and mixed shorthand. No AOT coverage for shorthand.
Matrix: Only int types tested in shorthand. Missing: str, bool shorthand in dedicated tests.
  AOT: No shorthand tests in aot/structs.rs (roadmap correctly says [ ] for AOT).
Semantic pin: test_shorthand_init (value matched from variable name).
Status: VERIFIED for eval; INCOMPLETE MATRIX for AOT (no AOT shorthand tests)
```

### 5.1.6 Field access

```
Tests found: tests/spec/declarations/struct_types.ori — deep nesting, chaining
Tests run: PASS
Audit: test_field_chaining (c.ceo.name), test_deep_nesting (l3.inner.inner.value).
  AOT: All 30 struct tests exercise field access; test_struct_nested_basic,
  test_struct_nested_three_levels cover deep field chains.
Matrix: int/str fields chained. Good coverage in both eval and AOT.
Semantic pin: test_deep_nesting (3-level chain).
Status: VERIFIED
```

### 5.1.7 Destructuring

```
Tests found: tests/spec/declarations/struct_types.ori — test_destructure, test_destructure_partial,
  test_destructure_rename
Tests run: PASS
Audit: Three test patterns: full destructure, partial destructure, rename destructure.
  AOT: No dedicated destructuring tests in aot/structs.rs — roadmap correctly says [ ].
Matrix: Only BasicStruct (int fields) tested. Missing: str fields, nested struct destructure,
  generic struct destructure, destructure in match arms.
Semantic pin: test_destructure_rename pins rename semantics.
Status: WEAK — eval works but matrix is thin (only int fields), no AOT coverage
```

---

## 5.2 Sum Types (Enums)

### 5.2.1 Parse sum type syntax

```
Tests found: tests/spec/declarations/sum_types.ori (485 lines, 30+ tests)
Tests run: 4181 passed, 0 failed, 42 skipped
Audit: READ full file. Covers: Color (unit variants), Direction (4 unit), Toggle (binary),
  MyOption (single-field), Message (str payload), Shape (multi-field), Point3D (3 fields),
  Event (mixed variant types), nested sum types, generic sum types (MyResult, MyOptional),
  recursive types (LinkedList, Tree), exhaustive match, wildcard patterns, variable binding,
  same field names across variants, struct-like variants, public sum types, function params/returns,
  collections of sum types, boolean payloads. Recursive Expr eval is a strong test.
Matrix: int/str/char/bool payloads, unit/single/multi-field variants, generic, recursive.
Semantic pin: test_expr_eval pins recursive evaluation on sum types.
Status: VERIFIED — comprehensive coverage. Roadmap claims test_parse_sum_type exists in
  grammar/attr.rs but this test name does NOT exist. STALE reference.
```

### 5.2.2-5.2.5 Unit/Single-field/Multi-field/Struct variants

```
Tests found: All covered in sum_types.ori
Tests run: PASS
Audit: Each variant style thoroughly tested. See 5.2.1 audit.
Matrix: Good type coverage (int, str, char, bool). Missing: float payloads, byte payloads.
Status: VERIFIED
```

### 5.2.6 Variant constructors

```
Tests found: sum_types.ori — all tests construct variants; generic sum types tested
Tests run: PASS
Audit: Construction syntax verified for: unit (Red), single-field (MySome(value: 42)),
  multi-field (Rectangle(width: 10, height: 5)), generic (MyOk(value: 42)).
AOT: No AOT coverage (roadmap correctly says [ ]).
Status: VERIFIED for eval; NEEDS TESTS for AOT
```

### 5.2.7 Pattern matching on variants

```
Tests found: sum_types.ori — exhaustive match, wildcard, nested match, variable binding,
  recursive Expr eval
Tests run: PASS
Audit: Strong coverage of match patterns. Wildcard, binding, nested match, exhaustiveness.
AOT: No AOT coverage for sum type pattern matching (roadmap correctly says [ ]).
Status: VERIFIED for eval; NEEDS TESTS for AOT
Note: Roadmap says "Derive(Eq) for sum types NOT working" — this is STALE. See 5.7 below.
```

---

## 5.3 Newtypes

### 5.3.1 Parse `type Name = ExistingType`

```
Tests found: tests/spec/types/newtypes.ori (110 lines, 9 tests)
Tests run: 4181 passed, 0 failed, 42 skipped
Audit: READ full file. Tests: UserId, Email, Age (str/int underlying), construction, unwrap,
  distinct type identity (nominal), equality via unwrap, function params, computation (Score).
  Roadmap claims test_parse_newtype exists in grammar/attr.rs — NOT FOUND. STALE reference.
Matrix: str and int underlying types. Missing: bool, float, char, collection underlying types.
Semantic pin: test_newtype_computation pins Score arithmetic via unwrap.
Status: VERIFIED but INCOMPLETE MATRIX (only str/int underlying types tested)
```

### 5.3.2 Distinct type identity (nominal)

```
Tests found: newtypes.ori — validate_user/validate_email show separate types
Tests run: PASS
Audit: Nominal typing verified by having separate functions accepting UserId vs Email.
  No compile_fail test proving you CANNOT pass UserId where Email is expected.
Semantic pin: NONE — no negative pin exists.
Status: WEAK — no negative test that rejects cross-newtype assignment
```

### 5.3.3 Wrapping/unwrapping

```
Tests found: newtypes.ori — test_newtype_unwrap, test_int_newtype_unwrap
Tests run: PASS
Audit: .unwrap() works for str and int newtypes.
Status: VERIFIED
```

### 5.3.4 Change .unwrap() to .inner accessor

```
Tests found: NONE — not implemented yet
Status: NOT VERIFIED (not implemented) — roadmap correctly shows [ ]
```

---

## 5.4 Generic Types

### 5.4.1 Parse `type Name<T> = ...`

```
Tests found: tests/spec/types/generic.ori (138 lines, 14 tests),
  compiler/ori_parse/src/grammar/ty/tests.rs — test_parse_generic_type
Tests run: 4181 passed, 0 failed, 42 skipped
Audit: READ full file. Tests: Box<T> with int/str/list, Pair<A,B> with int-str/str-bool,
  nested generics (Box of Pair, Pair of Boxes), Container<T> with list, Wrapper<T> chained
  field access, method calls on generic fields, multiple instances.
Matrix: int/str/bool/list element types. Missing: float, char, map, set.
Semantic pin: test_deep_nesting pins nested generic field access.
Status: VERIFIED — good eval coverage
```

### 5.4.2 Multiple parameters `<T, U>`

```
Tests found: generic.ori — test_pair_int_str, test_pair_str_bool
Tests run: PASS
Matrix: Two combinations tested. Adequate for 2-param generics.
Status: VERIFIED
```

### 5.4.3 Constrained `<T: Trait>`

```
Tests found: tests/spec/declarations/attributes.ori — GenericDerived<T: Eq> works
Tests run: PASS
Audit: Only one test of constrained generics (GenericDerived<T: Eq>). Roadmap correctly
  notes [ ] for full implementation — bound checking not fully implemented.
Status: WEAK — only one test covering constrained generics
```

### 5.4.4 Multiple bounds `<T: A + B>`

```
Tests found: NONE
Status: NOT VERIFIED (not tested) — roadmap correctly shows [ ]
```

### 5.4.5 Generic application/instantiation

```
Tests found: generic.ori — 14 tests covering instantiation
Tests run: PASS
Status: VERIFIED
```

### 5.4.6 Constraint checking

```
Tests found: attributes.ori — GenericDerived<T: Eq> constraint checked,
  compile-fail/derive_field_missing_trait.ori (E2032)
Tests run: PASS
Audit: compile_fail test pins that field types must implement derived trait.
Status: WEAK — only Eq constraint checked, no tests for non-Eq bounds
```

---

## 5.5 Compound Types

### 5.5.1 List `[T]` type inference

```
Tests found: tests/spec/types/collections.ori — ENTIRELY COMMENTED OUT
Tests run: N/A (all code commented)
Status: NOT VERIFIED — roadmap correctly shows [ ]. File exists but all tests are commented out.
```

### 5.5.2 Map `{K: V}` type inference

```
Tests found: collections.ori — ENTIRELY COMMENTED OUT
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.5.3 Set `Set<T>` type inference

```
Tests found: collections.ori — ENTIRELY COMMENTED OUT
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.5.4 Tuple `(T, U)`

```
Tests found: tests/spec/types/tuple_types.ori (368 lines, 30+ tests),
  compiler/ori_llvm/tests/aot/tuples.rs (430 lines, 28 tests, 2 ignored),
  compiler/ori_parse/src/tests/parser.rs — test_tuple_field_access,
  test_chained_tuple_field_access_with_parens
Tests run: 4181 eval passed; 66 AOT passed, 2 AOT ignored
Audit: READ both files. Eval tests cover: unit type, single/pair/triple/quad tuples,
  mixed types, nested, collections, Option/Result, destructuring, function params/returns,
  type inference, equality, field access (.0, .1). AOT tests cover: construction, destructuring,
  field access, params/returns, nested, strings, control flow, closures, equality.
  Two AOT tests ignored: chained tuple field access without parens (t.0.1 lexed as float).
Matrix: int/str/bool/float/char types, nested tuples, collections, Option/Result — COMPREHENSIVE.
Semantic pin: test_nested_tuple_field_access pins parens-required workaround for nested access.
  AOT ignored tests document the parser gap correctly.
Status: VERIFIED — comprehensive coverage. [x] items confirmed. [ ] items correctly identified.
  BUG FOUND in roadmap (decision tree column count mismatch) is documented.
```

### 5.5.5 Range `Range<T>` type inference

```
Tests found: tests/spec/expressions/loops.ori referenced but not specific range type inference
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.5.6 Function `(T) -> U` type inference

```
Tests found: Function types tested indirectly in struct_types.ori (WithFunction),
  generic.ori, lambdas tests
Status: NOT VERIFIED for dedicated type inference — roadmap correctly shows [ ]
```

---

## 5.6 Built-in Generic Types

### 5.6.1 `Option<T>`

```
Tests found: Used throughout: struct_types.ori (OptionalFields), tuple_types.ori,
  sum_types.ori, many trait/derive tests
Tests run: PASS (all use sites)
Audit: Option is extensively tested through indirect usage. No dedicated Option type test file.
Status: VERIFIED for eval usage; NEEDS TESTS for dedicated LLVM/AOT
```

### 5.6.2 `Result<T, E>`

```
Tests found: Used in tuple_types.ori (tuple_with_result), trait tests
Tests run: PASS
Status: VERIFIED for eval usage; NEEDS TESTS for dedicated LLVM/AOT
```

### 5.6.3 `Ordering`

```
Tests found: tests/spec/types/ordering/methods.ori (32+ tests),
  tests/spec/types/ordering/then_with.ori, tests/spec/types/ordering/match_ordering.ori
Tests run: 4181 passed, 0 failed, 42 skipped
Audit: READ methods.ori — covers is_less/is_equal/is_greater predicates on all three
  Ordering variants, is_less_or_equal/is_greater_or_equal compound predicates, reverse method.
  Comprehensive predicate coverage.
Status: VERIFIED — strong eval coverage; NEEDS TESTS for dedicated LLVM/AOT
```

### 5.6.4 `Error` type

```
Tests found: NONE specific
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.6.5 `Channel<T>`

```
Tests found: NONE
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

---

## 5.7 `with` Clause on Type Declarations (Capability Unification)

### Syntax Migration (`:` trait clause)

```
Tests found: NONE — not implemented
Status: NOT VERIFIED — roadmap correctly shows all [ ]
```

### Existing Derive Functionality

#### `#derive(Eq)` — including sum types

```
Tests found: tests/spec/declarations/attributes.ori, tests/spec/traits/derive/eq.ori,
  tests/spec/traits/derive/eq_sum.ori (142 lines, 16 tests),
  tests/spec/declarations/sum_types.ori (EqColor, EqOption),
  tests/compile-fail/derive_field_missing_trait.ori,
  compiler/ori_llvm/tests/aot/structs.rs (test_struct_derived_eq, test_struct_derived_eq_string)
Tests run: ALL PASS
Audit: READ eq_sum.ori — covers same variant same/different payload, different variants,
  payloadless variants (Red==Red, Red!=Green), multi-payload, three-payload, nested struct
  in sum type, reflexivity, symmetry. VERY STRONG coverage.
  sum_types.ori also has EqColor and EqOption with derive(Eq) passing.
Matrix: float/str/int payloads, payloadless, nested struct, 1/2/3-field variants.
Semantic pin: test_reflexive, test_symmetric pin algebraic properties.
  Negative pin: derive_field_missing_trait.ori pins E2032.
Status: VERIFIED
STALE ROADMAP NOTE: Section 5.2 note "Derive(Eq) for sum types NOT working" and
  Section 5.10 "Derive(Eq) for sum types -- not working" are STALE.
  Derive(Eq) for sum types IS working — 16 dedicated tests in eq_sum.ori + tests in
  sum_types.ori all pass. These roadmap notes must be updated.
```

#### `#derive(Clone)`

```
Tests found: tests/spec/declarations/attributes.ori — ClonePoint, SingleFieldDerived
  tests/spec/traits/derive/clone.ori, tests/spec/traits/derive/all_derives.ori
Tests run: PASS
Audit: Clone on Point with field equality verification. Nested struct clone in all_derives.ori.
Matrix: int fields in dedicated tests. Nested structs tested in all_derives.
Semantic pin: test_derive_clone + assertion cloned.x == original.x.
Status: VERIFIED for eval; no AOT coverage
```

#### `#derive(Hashable)`

```
Tests found: tests/spec/declarations/attributes.ori — HashPoint, MultiAttrPoint
  tests/spec/traits/derive/hashable.ori, tests/spec/traits/derive/all_derives.ori,
  tests/compile-fail/hashable_without_eq.ori
Tests run: PASS
Audit: Equal values hash equal, consistency tested. Negative pin: hashable_without_eq.ori
  correctly rejects Hashable without Eq.
Matrix: int fields. Missing: str, bool, float field types in dedicated hash tests.
Semantic pin: hash consistency test (same value hashes same).
Status: VERIFIED — good positive and negative pins
```

#### `#derive(Printable)`

```
Tests found: tests/spec/traits/derive/printable.ori (substantial tests),
  tests/spec/traits/derive/all_derives.ori — Point with Printable,
  tests/spec/declarations/attributes.ori — PrintablePoint
Tests run: ALL PASS
Audit: READ printable.ori — covers struct basic (Point -> "Point(10, 20)"), single str field,
  nested struct, payloadless variants (Red -> "Red"), sum type variants with payloads.
  assertions use assert_eq with exact expected strings.
Matrix: int/str fields, nested structs, sum type variants (payloadless + payload).
Semantic pin: Exact output format pins (e.g., "Point(10, 20)", "Red").
Status: VERIFIED
STALE ROADMAP NOTE: Section 5.7 says "derive(Printable) not fully implemented" and marks
  [ ] for all items. Section 5.10 says "Derive: Printable -- not working."
  This is STALE — derive(Printable) IS working with comprehensive tests.
  attributes.ori notes "skipped" but the actual test_derive_printable in attributes.ori PASSES.
```

#### `#derive(Default)`

```
Tests found: tests/spec/traits/derive/default.ori (substantial tests),
  tests/compile-fail/default_sum_type.ori
Tests run: ALL PASS
Audit: READ default.ori — covers basic Point (int fields -> 0), multi-type Config
  (str->"", int->0, bool->false), single field, float fields.
  Negative pin: default_sum_type.ori rejects derive(Default) on sum types.
Matrix: int/str/bool/float field types all tested with correct defaults.
Semantic pin: Exact default values pinned (0, "", false, 0.0).
Status: VERIFIED
STALE ROADMAP NOTE: Section 5.7 says "derive(Default) not tested" and marks [ ] for all items.
  Section 5.10 says "Derive: Default -- not working."
  This is STALE — derive(Default) IS working with comprehensive tests.
```

#### `#derive(Debug)`

```
Tests found: tests/spec/declarations/attributes.ori — DebugPoint,
  tests/spec/traits/derive/debug.ori
Tests run: PASS
Audit: DebugPoint test_derive_debug verifies debug_str.len() > 0 (weak assertion).
Status: VERIFIED but WEAK assertion (only checks non-empty, not format)
```

---

## 5.8 Visibility

### 5.8.1 Parse `pub type Name = ...`

```
Tests found: tests/spec/declarations/struct_types.ori — PublicStruct,
  tests/spec/declarations/sum_types.ori — PublicStatus
Tests run: PASS
Audit: Both pub struct and pub sum type tested.
Status: VERIFIED
```

### 5.8.2 Public visible from other modules

```
Tests found: tests/spec/modules/use_imports.ori — `pub type Point` defined and used
Tests run: PASS (4181 passed)
Audit: READ use_imports.ori — defines pub type Point, pub functions, pub constants.
  Tests verify self-module usage. No cross-file import test that actually imports this module.
Matrix: Only single-module visibility tested. No cross-module import test found that proves
  pub type can be imported from a different file.
Semantic pin: NONE for cross-module visibility.
Status: WEAK — pub type parsed and self-tested but no cross-module import verification
```

### 5.8.3 Private only in declaring module

```
Tests found: tests/spec/modules/use_imports.ori — type InternalPoint (private)
Tests run: PASS
Audit: InternalPoint defined without pub. No negative test proving it cannot be imported.
Semantic pin: NONE — no negative pin rejecting private type import.
Status: WEAK — no negative test for private visibility enforcement
```

---

## 5.9 Associated Functions

### 5.9.1 Remove hardcoded checks

```
Tests found: tests/spec/types/associated_functions.ori (164 lines, 12+ tests)
Tests run: 4181 passed, 0 failed, 42 skipped
Audit: READ full file. Tests: Point.origin(), Point.new(x:, y:), Builder pattern with
  chaining, Counter.zero()/starting_at()/increment(), Rectangle.square/from_dimensions/unit(),
  Duration.from_seconds(s: 5), Size.from_megabytes(mb: 2), instance vs associated distinction
  (Pair.create() vs p.sum()).
Matrix: Multiple user types (Point, Builder, Counter, Rectangle, Pair), built-in types
  (Duration, Size). Various param counts (0, 1, 2 params). Self return type tested.
Semantic pin: test_instance_vs_associated pins the distinction between Type.method() and value.method().
Status: VERIFIED — comprehensive coverage of associated functions
```

### 5.9.2 Parse `Type.method(...)` syntax

```
Tests found: associated_functions.ori — all tests use Type.method() syntax
Tests run: PASS
Status: VERIFIED
```

### 5.9.3 Distinguish type name vs value

```
Tests found: associated_functions.ori — test_instance_vs_associated
Tests run: PASS
Audit: Pair.create() (type) vs p.sum() (value) distinction explicitly tested.
Status: VERIFIED
```

### 5.9.4 Track methods without `self` in impl blocks

```
Tests found: associated_functions.ori — all associated function tests
Tests run: PASS
Status: VERIFIED
```

### 5.9.5 Built-in associated functions (Duration, Size)

```
Tests found: associated_functions.ori — Duration.from_seconds(s: 5), Size.from_megabytes(mb: 2)
Tests run: PASS
Audit: One test each for Duration and Size. Tests verify correct values.
Status: VERIFIED
```

### 5.9.6 Generic associated functions with type args

```
Tests found: NONE — not tested
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.9.7 Trait associated functions without `self`

```
Tests found: NONE — not tested
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

### 5.9.8 LLVM support for associated functions

```
Tests found: NONE — no AOT tests
Status: NOT VERIFIED — roadmap correctly shows [ ]
```

---

## 5.10 Section Completion Checklist

### STALE Items Found

1. **"Derive(Eq) for sum types -- not working"** — STALE. tests/spec/traits/derive/eq_sum.ori
   has 16 tests all passing. tests/spec/declarations/sum_types.ori also has EqColor/EqOption
   with derive(Eq) passing. This works.

2. **"Derive: Printable, Default -- not working"** — STALE. Both have dedicated test files
   in tests/spec/traits/derive/ with comprehensive tests all passing.

3. **Parser test references** — Multiple roadmap items reference test names like
   `test_parse_struct_type`, `test_parse_sum_type`, `test_parse_newtype` in
   `ori_parse/src/grammar/attr.rs`. These specific test names do NOT exist.
   Parser tests exist under different names in different files.

### Status Summary Table

| Item | Roadmap | Actual | Classification |
|------|---------|--------|----------------|
| 5.1 Struct types | [x] | All passing (eval + AOT) | VERIFIED |
| 5.2 Sum types | [x] | All passing (eval) | VERIFIED, no AOT |
| 5.3 Newtypes | [x] | Passing (eval) | VERIFIED, thin matrix |
| 5.3 .inner accessor | [ ] | Not implemented | correctly [ ] |
| 5.4 Generic types | [x] partial | Passing (eval) | VERIFIED for basics |
| 5.4 Constrained generics | [ ] | Only 1 test | WEAK |
| 5.4 Multiple bounds | [ ] | No tests | correctly [ ] |
| 5.5 Compound type inference | [ ] | All commented out | correctly [ ] |
| 5.5 Tuple types | [x] partial | Comprehensive (eval + AOT) | VERIFIED |
| 5.6 Option/Result/Ordering | [x] | All passing | VERIFIED |
| 5.6 Error/Channel | [ ] | Not implemented | correctly [ ] |
| 5.7 Capability unification | [ ] | Not started | correctly [ ] |
| 5.7 derive(Eq) structs | [x] | Passing (eval + AOT) | VERIFIED |
| 5.7 derive(Eq) sum types | "not working" | PASSING (16 tests) | STALE -- actually works |
| 5.7 derive(Clone) | [x] | Passing (eval) | VERIFIED |
| 5.7 derive(Hashable) | [x] | Passing (eval) | VERIFIED |
| 5.7 derive(Printable) | [ ] "not working" | PASSING (eval) | STALE -- actually works |
| 5.7 derive(Default) | [ ] "not tested" | PASSING (eval) | STALE -- actually works |
| 5.7 derive(Debug) | exists | Passing but WEAK | WEAK assertion |
| 5.8 Visibility | [x] | Passing but WEAK | WEAK -- no cross-module/negative tests |
| 5.9 Associated functions | [x] | Comprehensive (eval) | VERIFIED |
| 5.9 Generic assoc fns | [ ] | Not tested | correctly [ ] |
| 5.9 Trait assoc fns | [ ] | Not tested | correctly [ ] |
| LLVM/AOT overall | [ ] | Structs+tuples have AOT | Partial -- struct/tuple AOT good |

### Tests Run

| Test Command | Result |
|---|---|
| `cargo st tests/spec/declarations/struct_types.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/declarations/sum_types.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/newtypes.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/generic.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/declarations/attributes.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/tuple_types.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/associated_functions.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/types/ordering/` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/traits/derive/` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/traits/derive/eq_sum.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/traits/derive/printable.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/traits/derive/default.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/modules/use_imports.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/compile-fail/derive_*.ori hashable*.ori never*.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/compile-fail/default_sum_type.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo test -p ori_llvm -- struct` | 190 passed, 0 failed, 2 ignored |
| `cargo test -p ori_llvm -- tuple` | 68 passed, 0 failed, 2 ignored |
| `cargo test -p ori_parse -- test_tuple_field_access` | 1 passed |
| `cargo test -p ori_parse -- test_parse_derive` | 2 passed |

### Key Findings

1. **3 STALE roadmap entries**: derive(Eq) for sum types, derive(Printable), derive(Default) are
   all marked as "not working" or [ ] but are actually working with passing tests.

2. **STALE parser test references**: Multiple items reference parser test names that do not exist
   (test_parse_struct_type, test_parse_sum_type, test_parse_newtype, test_parse_generic_type_with_bounds).

3. **AOT coverage gap**: Only structs and tuples have AOT tests. Sum types, newtypes, generics,
   derives (except struct Eq), associated functions, visibility all lack AOT coverage.

4. **Missing negative tests**: Newtype nominal identity has no compile_fail test rejecting
   cross-type assignment. Visibility has no test rejecting private type import from another module.

5. **Compound type inference (5.5)**: Entirely pending as stated. The collections.ori file
   exists but is 100% commented out.

6. **42 skipped tests**: Consistent across all spec test runs. These are from the full test
   suite, not section-05-specific. Not investigated further as they are suite-wide.
