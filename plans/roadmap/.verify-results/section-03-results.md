# Section 03: Traits and Implementations — Verification Results

**Verified**: 2026-03-28
**Verifier**: Claude Opus 4.6 (automated)
**Method**: Systematic — test discovery, execution, file audit, matrix assessment, semantic pin check

## Files Loaded

- `/home/eric/projects/ori_lang/CLAUDE.md` (full)
- All 20 files in `.claude/rules/` (roadmap.md, tests.md, registry.md, eval.md, etc.)
- `/home/eric/projects/ori_lang/plans/roadmap/section-03-traits.md` (full, 1696 lines)
- Spec files referenced in section header

## Test Execution Summary

All test suites **PASS** with 0 failures:

| Suite | Result |
|-------|--------|
| `cargo st tests/spec/traits/` (all subdirs) | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/compile-fail/` (trait-related, 15 files) | All pass |
| `cargo st tests/spec/types/ordering/` | All pass |
| `cargo st tests/lint/infinite_iteration.ori` | All pass |
| `cargo st tests/spec/expressions/infinite_range.ori` | All pass |
| `cargo test -p ori_llvm -- traits` | 88 passed, 0 failed |
| `cargo test -p ori_llvm -- derives` | 43 passed, 0 failed |
| `cargo test -p ori_llvm -- formattable` | 17 passed, 0 failed |
| `cargo test -p ori_llvm -- iterators` | 26 passed, 0 failed |
| `cargo test -p ori_types -- trait` | 24 passed, 0 failed |
| `cargo test -p ori_types -- object_safety` | 2 passed, 0 failed |
| `cargo test -p ori_ir -- derives` | 22 passed, 0 failed |
| `cargo test -p ori_patterns -- iterator` | 116 passed, 0 failed |

**42 skipped tests** are from other sections (patterns/for:7, const_generics:6, collect_set:6, declarations:9, etc.). Only 6 are trait-related (collect_set.ori — properly annotated as not-yet-implemented).

---

## 3.0 Core Library Traits

### 3.0.1 Len Trait

--- Verifying 3.0.1: Len Trait ---
Tests found: tests/spec/traits/core/len.ori (14+ tests), ori_types Rust tests, ori_llvm/tests/aot/traits.rs (5 AOT tests)
Tests run: ALL PASS
Audit: READ len.ori — proper assert_eq assertions on list, string, range, map, set, tuple (pair, triple, single). Tests use `use std.testing { assert, assert_eq }`. Each test has a target function and assertion test. Covers empty cases. Tuple Len includes pair/triple/single.
Matrix assessment: Types: list/str/range/map/set/tuple (6/6) | Patterns: basic/empty/nested (3) | Backend: eval + LLVM AOT (5 tests)
Semantic pin: `test_tuple_len` — tuple Len is a unique feature, pins bound recognition
Status: **VERIFIED** — Comprehensive coverage, good matrix breadth

### 3.0.2 IsEmpty Trait

--- Verifying 3.0.2: IsEmpty Trait (implemented part) ---
Tests found: tests/spec/traits/core/is_empty.ori (13 tests), ori_types Rust tests, ori_llvm AOT (4 tests)
Tests run: ALL PASS
Audit: READ is_empty.ori — proper assertions on list, string, map, set. Covers empty and non-empty cases.
Matrix assessment: Types: list/str/map/set (4/6) | Missing: Range, [T, max N] | Backend: eval + AOT
Semantic pin: None specific to IsEmpty beyond basic functionality
Status: **VERIFIED** (implemented items) — `[ ]` items for Range/fixed-list correctly marked as TODO

### 3.0.3 Option Methods

--- Verifying 3.0.3: Option Methods ---
Tests found: tests/spec/traits/core/option.ori (16 tests), ori_eval Rust tests, ori_llvm AOT (7 tests)
Tests run: ALL PASS
Audit: READ option.ori — is_some, is_none, unwrap, unwrap_or all tested. Edge cases: None.is_some, Some.is_none, unwrap_or on Some (returns value, not default).
Matrix assessment: Types: Some(int)/None (2) | Methods: 4 | Backend: eval + AOT
Semantic pin: `unwrap_or` on Some returns value not default — pins correct semantics
Status: **VERIFIED**

### 3.0.4 Result Methods

--- Verifying 3.0.4: Result Methods ---
Tests found: tests/spec/traits/core/result.ori (14 tests), ori_eval Rust tests, ori_llvm AOT (5 tests)
Tests run: ALL PASS
Audit: READ result.ori — is_ok, is_err, unwrap all tested for both Ok and Err variants.
Matrix assessment: Types: Ok(int)/Err(str) | Methods: 3 | Backend: eval + AOT
Semantic pin: is_ok(Err) returns false — pins variant discrimination
Status: **VERIFIED**

### 3.0.5 Comparable Trait

--- Verifying 3.0.5: Comparable Trait ---
Tests found: tests/spec/traits/core/comparable.ori (58 tests), ori_llvm AOT (7+ tests)
Tests run: ALL PASS
Audit: READ comparable.ori — int, float, bool, str, char, byte, Duration, Size comparisons. Operators <, >, <=, >= all tested. .compare() method tested returning Ordering. List/Option/Result comparison included.
Matrix assessment: Types: int/float/bool/str/char/byte/Duration/Size/list/Option/Result/Ordering (12/12) | Operators: 4 relational + .compare() | Backend: eval + AOT
Semantic pin: `test_float_nan_comparison` would pin NaN ordering; `test_ordering_compare` pins Ordering self-comparison
Status: **VERIFIED** — Excellent matrix coverage across all types

### 3.0.6 Eq Trait

--- Verifying 3.0.6: Eq Trait ---
Tests found: tests/spec/traits/core/eq.ori (23 tests), ori_llvm AOT (3 tests)
Tests run: ALL PASS
Audit: READ eq.ori — int, float, bool, str, char equality and inequality. Edge cases: 0.0 == 0.0, -0.0 handling.
Matrix assessment: Types: int/float/bool/str/char (5 primitives) | Operators: ==/!= | Backend: eval + AOT
Semantic pin: Structural equality tests on each type — basic but sufficient
Status: **VERIFIED**

---

## 3.1 Trait Declarations

--- Verifying 3.1: Trait Declarations ---
Tests found: tests/spec/traits/declaration.ori (16 tests), self_param.ori (9), self_type.ori (7), inheritance.ori (6)
Tests run: ALL PASS
Audit: READ declaration.ori — Greeter/Counter/Describable traits declared, Widget inherent+trait impls, default methods (summarize, is_large), static methods (Point.new, Point.origin). Covers required methods, default methods, impl blocks, inheritance.
Matrix assessment: Features: parse/required methods/default methods/self/Self/inheritance/static methods (7/7) | Backend: eval + AOT
Semantic pin: `test_default_method` — default summarize() calls describe() — pins default method dispatch
Status: **VERIFIED** — All 8 items marked [x], all tests pass, good feature coverage

---

## 3.2 Trait Implementations

--- Verifying 3.2: Trait Implementations ---
Tests found: declaration.ori (shared), generic_impl.ori (4 tests), method_call_test.ori (1 test), ori_llvm AOT (7+ tests)
Tests run: ALL PASS
Audit: READ generic_impl.ori — Box<T> with inherent/trait impls, type parameter threading. Coherence testing via Rust unit tests (3 tests in type_registry.rs).
Matrix assessment: Features: inherent/trait/generic/where/resolution/dispatch/coherence (7/7) | Backend: eval + AOT (except generic dispatch — no monomorphization in AOT)
Semantic pin: `test_method_resolution_inherent_over_trait` in AOT — pins priority order
Note: Generic impl LLVM items are correctly marked `[ ]` — no monomorphization pipeline
Status: **VERIFIED** — `[x]` items all confirmed; `[ ]` for LLVM generic impls correctly reflects missing monomorphization

---

## 3.3 Trait Bounds

--- Verifying 3.3: Trait Bounds ---
Tests found: ori_parse generics/tests.rs (5 tests), ori_types constraint tests (10+), associated_types.ori (compile_fail)
Tests run: ALL PASS (24 trait tests in ori_types, 5 in ori_parse generics)
Audit: Comprehensive parser tests for `<T: Trait>`, `<T: A + B>`, where clauses. Type checker constraint satisfaction verified.
Matrix assessment: Features: single bound/multiple bounds/constraint checking/E2009 (4/4) | Parseable syntaxes verified
Semantic pin: compile_fail in associated_types.ori pins bound violation detection (E2009)
Status: **VERIFIED** — All items complete

---

## 3.4 Associated Types

--- Verifying 3.4: Associated Types ---
Tests found: associated_types.ori (2 tests + 1 compile_fail), associated_types_verify.ori (2 tests), compile-fail/impl_missing_assoc_type.ori
Tests run: ALL PASS
Audit: READ associated_types.ori — Container<T>.Item resolution, where C.Item: Eq constraint, compile_fail for missing bound.
Matrix assessment: Features: declaration/Self.Item/where constraints/impl validation (4/4) | Negative test: impl_missing_assoc_type.ori
Semantic pin: compile_fail for violated where clause — pins constraint enforcement
Status: **VERIFIED**

---

## 3.5 Derive Traits

--- Verifying 3.5: Derive Traits ---
Tests found: all_derives.ori (7+ tests), eq.ori (13+), derive/default.ori (6), derive/comparable.ori, derive/hashable.ori, derive/clone.ori, derive/printable.ori, ori_llvm derives (43 AOT tests)
Tests run: ALL PASS (spec + AOT)
Audit: READ all_derives.ori — Point with Eq/Clone/Hashable/Printable. Tests: equality, clone identity, hash consistency, to_str output. Uses both assert_eq and print-based assertions. Nested struct (Line with Points), many-field struct (Config). Also read eq.ori (22 struct tests), eq_sum.ori (15 sum type tests).
Matrix assessment: Traits: Eq/Clone/Hashable/Printable/Default/Comparable/Debug (7/7) | Types: struct/sum/nested/single-field/empty/many-field/generic/recursive | Backend: eval + AOT (43 tests)
Semantic pin: `test_hash_consistency` — same value always hashes same — pins hash determinism. `test_nested_eq` — pins recursive field comparison.
Status: **VERIFIED** — Excellent coverage across traits and type shapes

---

## 3.6 Section Completion Checklist

--- Verifying 3.6: Section Completion Checklist ---
Status: This is a tracking checklist, not an implementation item. Reviewed for accuracy.
- `[ ]` for IsEmpty Range/fixed-list — CORRECT (not implemented)
- `[x]` for declarations/impls/bounds/associated/derives — CONFIRMED by test runs above
- `[ ]` for proposals 3.8-3.17 — accurate assessment of partial completion
Status: **VERIFIED** — Checklist accurately reflects implementation state

---

## 3.7 Clone Trait Formal Definition

--- Verifying 3.7: Clone Trait ---
Tests found: clone/definition.ori (6), clone/primitives.ori (13), clone/collections.ori (3), clone/wrappers.ori (4), clone/tuples.ori (2), ori_llvm AOT (10+ clone tests)
Tests run: ALL PASS
Audit: READ clone files — all 8 primitive types (int, float, bool, str, char, byte, Duration, Size). Collections: list clone. Wrappers: Option Some/None, Result Ok/Err. Tuples: pair/triple.
Matrix assessment: Types: 8 primitives + list + Option + Result + tuple (12) | Backend: eval + AOT | Hygiene: Phase boundary audit documented
Semantic pin: `test_clone_is_equal` on each type — pins that clone produces equal values
Status: **VERIFIED** — All items [x], comprehensive type coverage, hygiene reviews documented

---

## 3.8 Iterator Traits

--- Verifying 3.8: Iterator Traits ---
Tests found: iterator/iterator.ori (9), double_ended.ori (12), collect.ori (8), methods.ori (31), double_ended_methods.ori (21), builtin_impls.ori (13), infinite.ori (13), for_loop.ori (19+), prelude.ori (8), copy_elision.ori, double_ended_gating.ori (16), collect_set.ori (6 skipped), ori_patterns Rust (116 tests), ori_llvm AOT (26 tests)
Tests run: ALL PASS
Audit: READ multiple files — iterator.ori covers .iter() on list/range/str/map, fused guarantee, basic next(). methods.ori covers map/filter/fold/find/count/any/all/for_each/collect/take/skip/enumerate/zip/chain/flatten/flat_map/cycle. double_ended covers rev/last/rfind/rfold. for_loop covers for-do and for-yield with guards, break, option iteration.
Matrix assessment: Types: list/map/set/str/Range/Option (6 source types) | Methods: 20+ consumer/adapter methods | Patterns: for-do/for-yield/guard/break/chain | Backend: eval + AOT (26 AOT tests) | LLVM: many `[ ]` items for advanced features (DoubleEndedIterator, repeat, etc.)
Semantic pin: `test_list_iter_fused` — pins fused guarantee. `test_double_ended_gating` — pins DEI method restriction on plain Iterator.
Note: collect_set.ori has 6 properly-skipped tests (type-directed collect to Set not implemented). Many LLVM `[ ]` items are correctly tracked.
Status: **VERIFIED** — Evaluator implementation complete and well-tested; LLVM gaps correctly tracked as `[ ]`

---

## 3.8.1 Iterator Performance and Semantics

--- Verifying 3.8.1: Iterator Performance and Semantics ---
Tests found: copy_elision.ori, infinite_range.ori, tests/spec/expressions/infinite_range.ori, tests/lint/infinite_iteration.ori (14), ori_patterns unbounded range tests
Tests run: ALL PASS
Audit: READ infinite_range.ori — unbounded range syntax, step, iteration with take. Lint tests cover repeat().collect(), (start..).collect(), cycle().collect() warning detection (W2001).
Matrix assessment: Features: copy elision/infinite range/stepped/lint warnings (4/4 implemented) | Compiler optimizations `[ ]` correctly tracked
Semantic pin: W2001 lint for infinite collect — pins safety warning
Status: **VERIFIED** — Implemented items confirmed; optimizer items correctly `[ ]`

---

## 3.9 Debug Trait

--- Verifying 3.9: Debug Trait ---
Tests found: debug/definition.ori, debug/primitives.ori, debug/collections.ori, debug/wrappers.ori, debug/tuples.ori, debug/derive.ori, debug/escape.ori, debug/join.ori
Tests run: ALL PASS
Audit: READ debug files — all primitive types tested for .debug() output, collections (list, map, set), wrappers (Option, Result), tuples, derived Debug. str.escape() and Iterator.join() also tested.
Matrix assessment: Types: int/float/bool/str/char/byte/void/Duration/Size + collections + wrappers + tuples + user structs (14+) | Backend: eval only (all LLVM items are `[ ]`)
Semantic pin: Debug for str includes quotes (vs Printable without) — pins Debug vs Printable distinction
Status: **VERIFIED** — Evaluator implementation complete; LLVM correctly tracked as `[ ]`

---

## 3.10 Trait Resolution and Conflict Handling

--- Verifying 3.10: Trait Resolution ---
Tests found: resolution/diamond.ori (4), resolution/conflicting_defaults.ori, resolution/method_priority.ori (2), resolution/ambiguous_method.ori (1 compile_fail), resolution/inherited_defaults.ori, compile-fail/conflicting_defaults.ori, compile-fail/duplicate_impl.ori, ori_types trait registry tests (11+)
Tests run: ALL PASS
Audit: READ diamond.ori — 4-level diamond hierarchy (Base/Left/Right/Bottom), verifies single impl satisfies all paths. READ method_priority.ori — inherent method wins over trait method with same name. READ ambiguous_method.ori — compile_fail for ambiguous method call (E2023).
Matrix assessment: Features: diamond/conflicting defaults/method priority/ambiguous detection/E2010/E2021-E2023 (6/6 implemented) | Unimplemented: coherence/orphan rules, super trait calls, extension conflicts, assoc type disambiguation, specificity ori tests, overlapping ori tests (6 `[ ]` items)
Semantic pin: `compile_fail("ambiguous method")` — pins E2023 detection. Diamond test pins single-impl satisfaction.
Note: Specificity (3.10.10) and overlapping impl detection (3.10.11) have Rust unit tests but no Ori tests — marked as needing generic impls in type checker.
Status: **VERIFIED** (implemented items) — `[x]` items confirmed with tests; `[ ]` items correctly blocked by dependencies

---

## 3.11 Object Safety Rules

--- Verifying 3.11: Object Safety Rules ---
Tests found: object_safety.ori (1), compile-fail/object_safety_self_return.ori, object_safety_self_param.ori, object_safety_nested.ori, object_safety_trait_bounds.ori, object_safety_lambda_param.ori, object_safety_let_binding.ori, ori_types registration tests (2+)
Tests run: ALL PASS
Audit: READ object_safety.ori — verifies object-safe traits can be used as types. READ object_safety_self_return.ori — compile_fail for Self in return position. 6 compile-fail tests cover Rule 1 (Self return), Rule 2 (Self param), nested types, bounded trait objects, lambda params, let bindings. Rule 3 (generic methods) has detection code ready but cannot be triggered (no per-method generics syntax yet).
Matrix assessment: Rules: 1/2/3 (3/3) | Error code: E2024 | Sites: function params, lambda params, let bindings, nested types, bounded traits (5 usage sites) | Backend: type checker only (no runtime component)
Semantic pin: compile_fail("cannot be made into an object") — pins E2024 detection at all usage sites
Status: **VERIFIED** — Comprehensive negative test coverage

---

## 3.12 Custom Subscripting (Index Trait)

--- Verifying 3.12: Index Trait ---
Tests found: index/definition.ori (5), index/option_return.ori, index/multiple_impls.ori, index/builtin_impls.ori, compile-fail/index_no_impl.ori, compile-fail/index_wrong_key.ori
Tests run: ALL PASS
Audit: READ definition.ori — Pair indexed by int, Config indexed by str, arithmetic on indexed values, chained indexing (Grid[1][0]). Multiple impls (JsonValue with int and str keys). Built-in impls tested. Error messages E2025-E2027 covered by compile-fail tests.
Matrix assessment: Features: basic index/string key/arithmetic/chained/multiple impls/builtin/errors (7/7 implemented) | LLVM `[ ]` correctly tracked
Semantic pin: Chained indexing `g[1][0]` — pins nested Index dispatch
Status: **VERIFIED** — Feature complete in evaluator; LLVM gaps tracked

---

## 3.13 Additional Core Traits

--- Verifying 3.13: Additional Core Traits ---
Tests found: printable/definition.ori (8), printable/derive.ori (7), default/definition.ori (10), default/derive.ori (7), traceable/definition.ori (5), traceable/error_trace.ori (7), traceable/no_trace.ori (3), traceable/result_delegation.ori (6), compile-fail/default_sum_type.ori, compile-fail/interpolation_missing_printable.ori, ori_llvm AOT derives (5 Default AOT tests)
Tests run: ALL PASS
Audit: READ traceable/definition.ori — Error type construction, has_trace/trace/trace_entries methods, Ok variant returns empty trace. READ printable/definition.ori — primitives, Ordering, generic bound, interpolation. READ default/definition.ori — all primitive defaults via struct fields, nested structs, deep nesting, idempotency.
Matrix assessment: Printable: 8 types + derive | Default: 10 tests + derive + E2028 sum type rejection | Traceable: Error type + Result delegation + ? operator trace injection | Error messages: E2038/E2028 | LLVM: Printable/Default in AOT; Traceable `[ ]`
Semantic pin: E2028 compile_fail for Default on sum type — pins rejection. Traceable trace_entries().len() == 0 for fresh Error — pins initial state.
Status: **VERIFIED** — All `[x]` items confirmed; Traceable LLVM correctly `[ ]`

---

## 3.14 Comparable and Hashable Traits

--- Verifying 3.14: Comparable and Hashable Traits ---
Tests found: comparable.ori (58), compound_hash.ori (17), compound_equals.ori (12), tuple_compare.ori (6), ori_llvm AOT (25+ tests for compare/hash/equals on list/tuple/option/result/ordering/str/bool/float/char)
Tests run: ALL PASS
Audit: READ compound_hash.ori — hash consistency on primitives, list/tuple/set/map hash, hash_combine function, Option/Result hash, float hash (NaN and +0/-0 normalization). READ compound_equals.ori — list/map/Option/Result/tuple equals. All use assert_eq with expected values.
Matrix assessment: Types: all primitives + list + tuple + Option + Result + Ordering (12+) | Methods: compare + hash + equals | Derive: Comparable + Hashable for structs | Error codes: E2029/E2030/E2031 | LLVM: extensive AOT coverage (list/tuple/option/result compare/hash/equals)
Semantic pin: Float hash normalization (+0.0/-0.0 same hash, NaN canonical) — pins IEEE 754 compliance. E2029 compile_fail — pins Hashable requires Eq. hash_combine determinism tests.
Note: Map/set LLVM hash/equals pending AOT collection infrastructure — correctly tracked.
Status: **VERIFIED** — Excellent cross-phase coverage

---

## 3.15 Derived Traits Formal Semantics

--- Verifying 3.15: Derived Traits Formal Semantics ---
Tests found: derive/eq.ori (22), derive/eq_sum.ori (15), derive/hashable.ori (11), derive/comparable.ori (6), derive/comparable_sum.ori (10), derive/clone.ori (8), derive/default.ori (7), derive/debug.ori (5), derive/printable.ori (6), derive/generic.ori (5), derive/recursive.ori (8), compile-fail/derive_field_missing_trait.ori, compile-fail/derive_not_derivable.ori, ori_llvm AOT derives (43 tests)
Tests run: ALL PASS
Audit: READ eq_sum.ori — variant matching for sum types (same variant/different payload, different variants, payloadless). READ generic.ori — Pair<T> with Eq/Clone, nested generic (Box<Pair<int>>). READ recursive.ori — Tree type with recursive Eq/Clone/Printable.
Matrix assessment: Traits: Eq/Hashable/Comparable/Clone/Default/Debug/Printable (7/7) | Type shapes: struct/sum/generic/recursive/single-field/empty/nested (7) | Error codes: E2032/E2033/E2028 | LLVM: AOT for struct derives; sum type/generic/recursive LLVM `[ ]` correctly tracked
Semantic pin: Sum type variant matching — pins variant discrimination in Eq. Generic conditional derivation — pins bounded impl requirement.
Status: **VERIFIED** — Comprehensive evaluator coverage; LLVM gaps for sum/generic/recursive correctly tracked

---

## 3.16 Formattable Trait

--- Verifying 3.16: Formattable Trait ---
Tests found: formattable/definition.ori, formattable/int.ori, formattable/float.ori, formattable/blanket.ori, formattable/format_spec_type.ori, formattable/fill.ori, formattable/edge_cases.ori, formattable/user_impl.ori, ori_llvm AOT (17 tests)
Tests run: ALL PASS
Audit: READ formattable files — format spec parsing, width/alignment/precision/fill, int (binary/octal/hex/sign/zero-pad), float (fixed/precision/percent/scientific), str (width/precision), blanket for Printable types, user-defined Formattable impl, edge cases.
Matrix assessment: Types: int/float/str/bool/char + user types | Spec features: width/align/sign/#/0/precision/type (7) | Format types: b/o/x/X/e/E/f/% (8) | Backend: eval + AOT (17 AOT tests)
Semantic pin: `test_format_int_hex` with `0x` prefix — pins alternate form. User impl in formattable/user_impl.ori tests custom Formattable dispatch.
Note: GAP(formattable-aot) documented — user Formattable::format() in LLVM blocked on general trait method call codegen.
Status: **VERIFIED** — Comprehensive format spec coverage

---

## 3.17 Into Trait

--- Verifying 3.17: Into Trait ---
Tests found: into/definition.ori (2), into/str_to_error.ori, into/int_to_float.ori, into/set_to_list.ori, compile-fail/into_no_identity.ori, compile-fail/into_no_chaining.ori, ori_llvm AOT (3 int->float tests)
Tests run: ALL PASS
Audit: READ definition.ori — Celsius->str and Wrapper->int custom Into impls with .into() calls. READ str_to_error.ori — str.into() returns Error type. Negative tests: no identity impl, no chaining.
Matrix assessment: Conversions: str->Error, int->float, Set->List, custom user types (4) | Negative: no identity, no chaining | Backend: eval + AOT (int->float only) | Missing AOT: str->Error (blocked on Error LLVM repr), Set->List (blocked on Set construction)
Semantic pin: compile_fail for identity Into — pins no-identity rule. compile_fail for chaining — pins explicit-only rule.
Status: **VERIFIED** — All `[x]` items confirmed; orphan rule `[ ]` blocked by module system

---

## 3.18 Ordering Type

--- Verifying 3.18: Ordering Type ---
Tests found: tests/spec/types/ordering/methods.ori (32 tests), ordering/then_with.ori (9), ori_llvm AOT (7+ ordering tests)
Tests run: ALL PASS
Audit: Ordering predicate methods (is_less/is_equal/is_greater/is_less_or_equal/is_greater_or_equal), reverse, then, then_with (lazy chaining), trait methods (clone/to_str/hash/debug). Then_with tests laziness (closure not called when non-Equal).
Matrix assessment: Methods: 8 + trait methods (12 total) | Variants: Less/Equal/Greater (3) | Backend: eval + AOT
Semantic pin: `test_then_with_laziness` — closure not called on non-Equal — pins lazy evaluation. `test_ordering_involution` — reverse(reverse(x)) == x.
Note: `Ordering.default()` correctly marked `[ ]` — needs static method support.
Status: **VERIFIED** — All `[x]` items confirmed

---

## 3.19 Default Type Parameters on Traits

--- Verifying 3.19: Default Type Parameters ---
Tests found: default_type_params.ori (2 tests), ori_parse generics tests
Tests run: ALL PASS
Audit: READ default_type_params.ori — Addable<Rhs = Self> trait, Point impl omitting Rhs, Transform<Input = Self, Output = Input> for cascading defaults. Ordering constraint verified via E2015.
Matrix assessment: Features: parse/store/fill/Self-substitution/ordering/cascading (6/6) | Backend: eval
Semantic pin: Trait with cascading defaults (Output = Input) — pins later-references-earlier
Status: **VERIFIED** — All items complete

---

## 3.20 Default Associated Types

--- Verifying 3.20: Default Associated Types ---
Tests found: default_assoc_types.ori (4 tests), ori_parse trait tests
Tests run: ALL PASS
Audit: READ default_assoc_types.ori — Addable with `type Output = Self`, Point uses default, Number overrides to int. Self-substitution and override both tested.
Matrix assessment: Features: parse/store/fill/Self-substitution/reference (5/5 implemented) | `[ ]` for bounds checking on defaults — correctly deferred
Semantic pin: Number overrides Output=int while Point uses default=Self — pins override mechanism
Status: **VERIFIED** — All `[x]` items confirmed; bounds check `[ ]` noted

---

## 3.21 Operator Traits

--- Verifying 3.21: Operator Traits ---
Tests found: operators/user_defined.ori (16 tests), compile-fail/operator_trait_missing.ori (5), ori_llvm AOT (7 tests)
Tests run: ALL PASS
Audit: READ user_defined.ori — Point with Add/Sub/Neg/Mul/Div/Rem/FloorDiv/BitAnd/BitOr/BitXor/Shl/Shr/BitNot/Not implementations. Chaining (a + b + c) and double negation tested. Compile-fail tests for missing operator trait.
Matrix assessment: Operators: Add/Sub/Neg/Mul/Div/Rem/FloorDiv/BitAnd/BitOr/BitXor/Shl/Shr/BitNot/Not (14/14) | Types: user-defined Point | Error: E2020 | Backend: eval + AOT (7 tests)
Semantic pin: compile_fail for Add on type without impl — pins E2020. Chaining test — pins associativity.
Note: Derive for newtypes correctly marked `[ ]` as optional.
Status: **VERIFIED** — Complete operator coverage

### 3.21.1 MatMul Operator

--- Verifying 3.21.1: MatMul Operator ---
Tests found: NONE
Status: **NEEDS TESTS** — All items `[ ]`, not started. No code, no tests.

---

## 3.22 Bound Syntax (Capability Unification)

--- Verifying 3.22: Bound Syntax ---
Status: **ELIMINATED** — Section correctly marked as no longer needed per 2026-03-04 addendum. No implementation required. Verified.

---

## 3.23 Impl Colon Syntax

--- Verifying 3.23: Impl Colon Syntax ---
Tests found: NONE
Status: **NEEDS TESTS** — All items `[ ]`, not started. Parser change not yet implemented.

---

## 3.24 Value Trait (ARC-Free Value Types)

--- Verifying 3.24: Value Trait ---
Tests found: NONE
Status: **NEEDS TESTS** — All items `[ ]`, not started across all 5 phases.

---

## Summary

### Verified Items (all [x] items with tests)

| Subsection | Status | Eval Tests | AOT Tests | Rust Tests |
|------------|--------|------------|-----------|------------|
| 3.0.1 Len | VERIFIED | 14+ | 5 | Yes |
| 3.0.2 IsEmpty (impl) | VERIFIED | 13 | 4 | Yes |
| 3.0.3 Option | VERIFIED | 16 | 7 | Yes |
| 3.0.4 Result | VERIFIED | 14 | 5 | Yes |
| 3.0.5 Comparable | VERIFIED | 58 | 7+ | Yes |
| 3.0.6 Eq | VERIFIED | 23 | 3 | Yes |
| 3.1 Declarations | VERIFIED | 38 | 2+ | Yes |
| 3.2 Implementations | VERIFIED | 4+ | 7+ | Yes |
| 3.3 Bounds | VERIFIED | N/A | N/A | 15+ |
| 3.4 Associated Types | VERIFIED | 4+1cf | N/A | Yes |
| 3.5 Derives | VERIFIED | 7+ | 43 | Yes |
| 3.6 Checklist | VERIFIED | N/A | N/A | N/A |
| 3.7 Clone | VERIFIED | 28 | 10+ | Yes |
| 3.8 Iterators | VERIFIED | 100+ | 26 | 116 |
| 3.8.1 Iterator Perf | VERIFIED | 14+ | N/A | Yes |
| 3.9 Debug | VERIFIED | 40+ | N/A | Yes |
| 3.10 Resolution | VERIFIED | 7+1cf | N/A | 11+ |
| 3.11 Object Safety | VERIFIED | 1+6cf | N/A | 2+ |
| 3.12 Index | VERIFIED | 5+2cf | N/A | N/A |
| 3.13 Additional | VERIFIED | 46+1cf | 5 | Yes |
| 3.14 Comp/Hash | VERIFIED | 93 | 25+ | Yes |
| 3.15 Derived Formal | VERIFIED | 96+2cf | 43 | 22 |
| 3.16 Formattable | VERIFIED | 30+ | 17 | Yes |
| 3.17 Into | VERIFIED | 5+2cf | 3 | Yes |
| 3.18 Ordering | VERIFIED | 41 | 7+ | Yes |
| 3.19 Default Type Params | VERIFIED | 2 | N/A | 5+ |
| 3.20 Default Assoc Types | VERIFIED | 4 | N/A | Yes |
| 3.21 Operators | VERIFIED | 16+5cf | 7 | Yes |

### Not Started Items (all [ ])

| Subsection | Status | Notes |
|------------|--------|-------|
| 3.0.2 IsEmpty (Range/fixed) | NEEDS TESTS | Tracked TODO |
| 3.0.2 IsEmpty (generic prelude) | NEEDS TESTS | Tracked TODO |
| 3.2 Generic LLVM | NEEDS TESTS | Blocked on monomorphization |
| 3.8 Iterator LLVM (various) | NEEDS TESTS | Partially complete (26 AOT); DEI, repeat, etc. pending |
| 3.9 Debug LLVM | NEEDS TESTS | Evaluator-only currently |
| 3.10 Coherence/orphan | NEEDS TESTS | Blocked on module system |
| 3.10 Super trait calls | NEEDS TESTS | Blocked on parser |
| 3.10 Extension conflicts | NEEDS TESTS | Blocked on module system |
| 3.10 Assoc type disambiguation | NEEDS TESTS | Blocked on parser |
| 3.11 Rule 3 (generic methods) | N/A | Detection code ready; syntax not parseable yet |
| 3.12 Index LLVM | NEEDS TESTS | Evaluator-only currently |
| 3.13 Traceable LLVM | NEEDS TESTS | Evaluator-only currently |
| 3.15 Sum/Generic/Recursive LLVM derives | NEEDS TESTS | Evaluator-only currently |
| 3.17 Orphan rules for Into | NEEDS TESTS | Blocked on module system |
| 3.18 Ordering.default() | NEEDS TESTS | Blocked on static method support |
| 3.20 Bounds checking on defaults | NEEDS TESTS | Deferred |
| 3.21.1 MatMul | NEEDS TESTS | Not started |
| 3.22 Bound Syntax | ELIMINATED | No work needed |
| 3.23 Impl Colon Syntax | NEEDS TESTS | Not started |
| 3.24 Value Trait | NEEDS TESTS | Not started (5 phases) |

### Quality Assessment

**Strengths:**
- Excellent type matrix coverage in core traits (3.0.5 Comparable covers 12 types)
- Good negative testing with compile_fail tests (15+ compile-fail files)
- AOT parity for most implemented features (88 trait + 43 derive + 17 formattable + 26 iterator = 174 AOT tests)
- Rust unit tests provide phase-boundary verification (ori_types, ori_ir, ori_patterns)
- Iterator tests are particularly thorough (116 Rust + 100+ Ori)

**Weaknesses:**
- Some older tests use print-based assertions instead of assert_eq (all_derives.ori)
- collect_set.ori has 6 skipped tests with trivial `assert(cond: true)` bodies — these are effectively placeholder stubs
- Specificity and overlapping impl detection have Rust unit tests but no Ori spec tests
- No negative test for `Ordering.default()` error (blocked on static methods)

**Overall: Section 3 is well-verified.** All `[x]` items have passing tests with reasonable matrix coverage. The `[ ]` items are correctly tracked with appropriate blockers. No regressions found.
