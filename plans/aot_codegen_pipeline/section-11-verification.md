---
section: "11"
title: "Comprehensive Verification"
status: in-progress
goal: "Verify the complete AOT pipeline against the full language surface area"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"]
sections:
  - id: "11.1"
    title: "AOT test matrix"
    status: in-progress
  - id: "11.2"
    title: "Dual-execution verification"
    status: not-started
  - id: "11.3"
    title: "Memory safety verification"
    status: not-started
  - id: "11.4"
    title: "Performance validation"
    status: not-started
  - id: "11.5"
    title: "Documentation"
    status: not-started
---

# Section 11: Comprehensive Verification

**Status:** In Progress (11.1 underway — 862 passed, 0 failed, 58 ignored as of 2026-02-23)
**Goal:** Every language feature compiles through AOT, matches JIT behavior, and has zero memory leaks.

**Context:** This is the capstone section. All architectural improvements are in place. Now we prove the system works as one cohesive whole by testing every language feature through the AOT pipeline and verifying behavioral equivalence with the JIT evaluator.

**Depends on:** All previous sections.

---

## 11.1 AOT Test Matrix

**File:** `compiler/ori_llvm/tests/aot/`

Build a comprehensive test matrix covering every language feature through AOT compilation.

- [x] **Literals & primitives:** (2026-02-23)
  - int, float, bool, char, byte, str, unit — all covered in spec.rs
  - Arithmetic, bitwise, comparison, logical operators — all covered
  - String concatenation, escapes — covered
  - String interpolation — known AOT gap (#[ignore])
  - Duration, Size literals and arithmetic — covered

- [x] **Control flow:** (2026-02-23)
  - if/else (expression-valued) — covered
  - while-pattern loops (loop+break+continue) — covered
  - loop (infinite with break, break value) — covered
  - for-in loops over Range, List, String, Option, Map — covered (for_loops.rs)
  - for-yield (generator expressions, with filter and transform) — covered
  - Pattern matching (match: int, bool, char, wildcard) — covered
  - Nested control flow (match inside if inside loop) — covered

- [ ] **Tuples:** (2026-02-23)
  - [x] Pair and triple construction, single-element, bool pair, mixed types — covered (tuples.rs)
  - [x] Destructuring (pair, triple, mixed, from variable) — covered (tuples.rs)
  - [x] Field access (.0, .1, .2, .3), in expressions, as function args — covered (tuples.rs)
  - [x] Tuple as function param and return value — covered (tuples.rs)
  - [x] Tuple return from function (min_max, stats triple) — covered (tuples.rs)
  - [x] Nested tuple destructuring `((a, b), (c, d))` — covered (tuples.rs)
  - [x] Tuples with string fields (one, two strings) — covered (tuples.rs)
  - [x] Tuple from if expression — covered (tuples.rs)
  - [x] Closure capturing tuple, closure returning tuple — covered (tuples.rs)
  - [ ] Chained nested tuple field access `t.0.0` — Parser gap: lexed as float (Section 05 § 5.7, #[ignore])
  - [ ] List of tuples iteration — AOT gap: heap corruption (#[ignore])
  - [ ] Tuple destructuring in for loop `for (a, b) in ...` — Parser gap (#[ignore])
  - [ ] Tuple == equality comparison — AOT gap: compiles but returns wrong result (#[ignore])

- [ ] **Structs:** (2026-02-23)
  - [x] Basic construction (1/2/3 fields), bool fields, mixed fields — covered (structs.rs)
  - [x] String fields (one, two string fields, method on string field) — covered (structs.rs)
  - [x] Update syntax (one field, all fields, preserves original, chain) — covered (structs.rs)
  - [x] Nested structs (2 levels, 3 levels, with string) — covered (structs.rs)
  - [x] Struct as function param, return, param+return, multiple params — covered (structs.rs)
  - [x] Struct from if expression, in loop — covered (structs.rs)
  - [x] Closure capturing struct field access, closure returning struct — covered (structs.rs)
  - [x] Derived Eq on struct (int fields), derived Eq on struct (string fields) — covered (structs.rs)
  - [x] Multiple struct types in same program — covered (structs.rs)
  - [x] Struct with list field — covered (structs.rs)
  - [x] Computed fields, fields from function calls — covered (structs.rs)
  - [ ] Struct update with string field — AOT gap: GEP on non-pointer causes segfault (#[ignore])

- [ ] **Data structures (other):**
  - [ ] Enum construction and pattern matching — AOT gap: variant constructors (#[ignore])
  - [ ] Recursive types (tree, linked list) — blocked by enum constructors
  - [ ] Generic types — AOT gap: generic function resolution (#[ignore])

- [ ] **Collections:**
  - [x] List: literal, length, iter, map, filter, collect — covered
  - [ ] List: push, pop, index, first, last — AOT gap: methods not in builtin table (#[ignore])
  - [x] Map: literal, length, for-loop iteration — covered
  - [ ] Map: insert, get, remove, is_empty, keys, values — AOT gap: methods not in builtin table
  - [ ] Set: all operations — not yet in AOT
  - [x] Range: `0..5`, `0..=5`, iter, yield, guard — covered

- [ ] **Functions & closures:** (2026-02-23)
  - [x] Direct function calls — covered
  - [x] Method calls on types — covered (via traits)
  - [x] Closures with 0, 1, N captures — covered (arc.rs)
  - [x] Higher-order functions (passing lambdas to function params) — covered (higher_order.rs)
  - [x] Two function params, apply-twice, composition, pipeline — covered (higher_order.rs)
  - [x] Functions returning closures (make_adder, make_multiplier, make_predicate) — covered (higher_order.rs)
  - [x] Closure in conditional (if/else selecting closure) — covered (higher_order.rs)
  - [x] Manual fold with HOF (sum, product accumulation) — covered (higher_order.rs)
  - [x] Multi-param lambdas (`(int, int) -> int`) — covered (higher_order.rs)
  - [x] Closure capture from function result — covered (higher_order.rs)
  - [x] Nested closures (3 levels deep with captures) — covered (higher_order.rs)
  - [x] Closure chaining (step1→step2→step3 without HOF wrapper) — covered (higher_order.rs)
  - [x] Callback pattern (function takes value + callback) — covered (higher_order.rs)
  - [x] Predicate filter with closure in loop — covered (higher_order.rs)
  - [x] Two closures sharing same capture — covered (higher_order.rs)
  - [x] Bool-returning closures (local use works) — covered (higher_order.rs)
  - [x] Recursive functions — covered
  - [x] Mutually recursive functions — covered
  - [ ] Named function → closure coercion — AOT gap: ABI mismatch i64 vs { ptr, ptr } (#[ignore])
  - [ ] Function taking `(int) -> bool` param — AOT gap: bool return ABI for closures (#[ignore])
  - [ ] Closures capturing closures — AOT gap for complex nesting (#[ignore])

- [ ] **Error handling:** (2026-02-23)
  - [x] Result basics (Ok/Err construction, is_ok/is_err, unwrap, unwrap_or) — covered (error_handling.rs)
  - [x] Option basics (Some/None construction, is_some/is_none, unwrap, unwrap_or) — covered (error_handling.rs)
  - [x] `?` propagation (Result ok/err, Option some/none) — covered (error_handling.rs)
  - [x] `?` chaining (multi-step pipeline, early exit) — covered (error_handling.rs)
  - [x] Deep `?` chains (4 levels) — covered (error_handling.rs)
  - [x] Result with conditional logic (validate, range check) — covered (error_handling.rs)
  - [x] Result in loops (validate each iteration) — covered (error_handling.rs)
  - [x] Option chain unwrap_or — covered (error_handling.rs)
  - [x] Mixed Result<Option<int>, str> — covered (error_handling.rs)
  - [x] Result<int, int> (non-string error type) — covered (error_handling.rs)
  - [x] panic in unreachable branch — covered (2026-02-23)
  - [ ] catch(expr:) — AOT gap: not lowered through ARC pipeline (#[ignore])
  - [ ] @panic handler — not specifically tested

- [x] **Traits & derived:** (2026-02-23)
  - Eq, Comparable, Hashable, Printable, Clone — covered (traits.rs, derives.rs)
  - Debug — interpreter-only (#[ignore])
  - Derived Eq on structs (==, !=, string fields) — covered (2026-02-23, fixed emit_comparison_via_trait)
  - Derived Comparable on structs (<, >, <=, >=) — covered (2026-02-23, fixed emit_ordering_comparison)
  - Derived on enums — blocked by enum constructors
  - Operator overloading through traits (arithmetic, bitwise, boolean) — covered
  - Formattable (hex, binary, octal, padding, alignment) — covered (formattable.rs)

- [x] **Iterator pipeline:** (2026-02-23)
  - map, filter, take, skip, enumerate, zip, chain — all covered (iterators.rs)
  - collect, fold, count, find, any, all, for_each — all covered
  - Chained adapters (map → filter → take → count) — covered
  - Nested iterators (flat_map, flatten) — not tested (likely not in AOT)

- [ ] **ARC-specific:**
  - [x] Shared references (multiple owners) — covered (arc.rs)
  - [x] Shared references with many aliases (6+ refs to same struct) — covered (stress.rs, 2026-02-23)
  - [ ] Last-reference optimization (in-place mutation when RC=1)
  - [x] Drop ordering (nested structs, loop allocations, block scopes) — covered
  - [x] Collections of RC'd values (list of strings, struct with list field) — covered (2026-02-23)
  - [x] Enum basic drop and enum with string payload — covered (2026-02-23, un-ignored)
  - [x] 1000+ struct allocations in loop (int fields + string fields) — covered (stress.rs, 2026-02-23)
  - [x] Nested struct allocation in loop (500 iterations) — covered (stress.rs, 2026-02-23)
  - [x] String concatenation stress (100+ iterations) — covered (stress.rs, 2026-02-23)
  - [ ] Reset/reuse (constructing same-shape value after match)

- [x] **Recursion:** (2026-02-23)
  - Direct recursion: factorial, fibonacci, sum_to, power, GCD — covered (recursion.rs)
  - Tail-recursive accumulator patterns: factorial_acc, sum_acc, count_digits — covered (recursion.rs)
  - Mutual recursion: is_even/is_odd, mutual countdown — covered (recursion.rs)
  - Recursion with Result and ? operator — covered (recursion.rs)
  - Recursion with match (countdown, Collatz steps) — covered (recursion.rs)
  - Recursion depth: 100 levels (direct), 1000 levels (accumulator) — covered (recursion.rs)
  - Recursion with struct parameters (move towards origin) — covered (recursion.rs)
  - Recursive computation: binary search, Ackermann, Tower of Hanoi — covered (recursion.rs)
  - Recursion with Option (find_first_above) — covered (recursion.rs)

- [x] **Scale & stress:** (2026-02-23)
  - Large lists (100-500 elements via for-yield) — covered (stress.rs)
  - Large tuples (5-6 elements, mixed types) — covered (stress.rs)
  - Structs with 5-8 fields, mixed field types — covered (stress.rs)
  - Deeply nested structs (4 levels, strings at every level) — covered (stress.rs)
  - Struct update syntax with multiple field overrides — covered (stress.rs)
  - Deep recursion with struct parameters (200-500 levels) — covered (stress.rs)
  - Long iterator pipeline chains (5 adapters) — covered (stress.rs)
  - Struct passed through multi-function chain — covered (stress.rs)
  - 200 function calls in loop (struct alloc + field sum) — covered (stress.rs)

- [ ] **Depth & complexity:** (2026-02-23)
  - [x] Match with 20+ arms — covered (depth.rs)
  - [x] Match in loop (100 iterations) — covered (depth.rs)
  - [x] Nested if 5 levels deep — covered (depth.rs)
  - [x] Nested loops with break/continue — covered (depth.rs)
  - [x] Match inside loop inside match — covered (depth.rs)
  - [x] Break with value from nested loop — covered (depth.rs)
  - [x] Complex for-yield guards (multi-condition) — covered (depth.rs)
  - [x] Deep ? chains (5 levels, Result and Option) — covered (depth.rs)
  - [x] unwrap_or with computed defaults — covered (depth.rs)
  - [x] Closure capturing struct fields — covered (depth.rs)
  - [x] Closure capturing single string — covered (depth.rs)
  - [ ] Closure capturing 3+ strings — AOT gap: heap corruption (#[ignore])
  - [x] Closure passed through 3 function levels — covered (depth.rs)
  - [x] Multiple closures in same scope — covered (depth.rs)
  - [x] Closure with mixed capture types (struct + bool) — covered (depth.rs)
  - [x] Multi-derive (Eq+Comparable+Hashable, 5-trait combo) — covered (depth.rs)
  - [x] Derive Eq on struct with int+bool+str fields — covered (depth.rs)
  - [x] Derive Comparable with string fields — covered (depth.rs)
  - [x] Derive Hashable consistency contract — covered (depth.rs)
  - [x] Nested Option types — covered (depth.rs)
  - [x] Result with Option payload — covered (depth.rs)
  - [x] Fibonacci via recursive match — covered (depth.rs)
  - [ ] Mutation in match arms inside for-do — AOT gap: ArcIrEmitter variable not defined (#[ignore])
  - [ ] Bool comparison stored as struct field in for-yield — AOT gap: always false (#[ignore])
  - [ ] Iterator .map closure accessing struct fields — AOT gap: invalid LLVM IR (#[ignore])

- [x] **Operator edge cases:** (2026-02-23)
  - Integer boundary values (billion-scale, negative large, zero boundary) — covered (operators.rs)
  - Division truncation (positive, negative dividend/divisor, both negative, exact) — covered (operators.rs)
  - Modulo edge cases (negative dividend/divisor, both negative, zero remainder) — covered (operators.rs)
  - Unary negation combos (double boolean, triple int/bool) — covered (operators.rs)
  - Complex precedence chains (mixed arith, parenthesized, bitwise) — covered (operators.rs)
  - Comparison in boolean chain (&&/|| precedence) — covered (operators.rs)
  - Float edge cases (negative zero, very small/large, precision 0.1+0.2, division) — covered (operators.rs)
  - Empty string operations (equality, concat left/right/both, inequality) — covered (operators.rs)
  - Boolean short-circuit (&&, ||, chained) — covered (operators.rs)
  - Char equality, byte arithmetic — covered (operators.rs)
  - Duration arithmetic (500ms + 500ms == 1s) — covered (operators.rs)
  - [ ] Size cross-unit arithmetic (512kb + 512kb == 1mb) — AOT gap: unit normalization (#[ignore])

- [x] **Pattern matching extensions:** (2026-02-23)
  - Or-patterns (int literals, char, bool, in loop) — covered (patterns.rs)
  - Guard clauses (basic, with binding, complex condition, in loop) — covered (patterns.rs)
  - Tuple patterns in match (basic, second arm, wildcard, 3 elements, all wildcards, from function) — covered (patterns.rs)
  - Binding patterns (capture, mixed with literals) — covered (patterns.rs)
  - Combined patterns (guard+tuple, result dispatch, nested match, fizzbuzz) — covered (patterns.rs)
  - Exhaustiveness (bool cases, many char literals) — covered (patterns.rs)

- [ ] **Type conversions:** (2026-02-23)
  - [x] int.to_float, int.to_float (negative, zero, large) — covered (conversions.rs)
  - [x] float.to_int (basic, truncation, negative truncation, zero, negative zero) — covered (conversions.rs)
  - [x] int.into (int -> float) — covered (conversions.rs)
  - [x] bool.to_int (true=1, false=0) — covered (conversions.rs)
  - [x] char.to_int (ASCII 'A'=65, '0'=48, ' '=32) — covered (conversions.rs)
  - [x] byte.to_int (0, 200, 255) — covered (conversions.rs)
  - [x] int.abs, float.abs (positive, negative, zero) — covered (conversions.rs)
  - [x] int.to_str (positive, negative, zero), bool.to_str, float.to_str — covered (conversions.rs)
  - [x] Ordering.to_int — covered (conversions.rs)
  - [x] Chained conversions (int->float->int roundtrip, bool->int->float) — covered (conversions.rs)
  - [x] Conversions in expressions (comparison, arithmetic, concat) — covered (conversions.rs)
  - [x] int.f() shorthand — **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS
  - [x] int.byte() and byte roundtrip — **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS

- [ ] **Variable scoping & block expressions:** (2026-02-23)
  - [x] Let bindings (basic, type annotation, chain) — covered (scoping.rs)
  - [x] Variable shadowing (same type, different type, uses previous, many shadows) — covered (scoping.rs)
  - [x] Block expressions as values (basic, single, nested, with side effects) — covered (scoping.rs)
  - [x] If-else as expression (basic, computed, nested, block branches, string value) — covered (scoping.rs)
  - [x] Match as expression (int values, string values, block arms, bool, nested in if) — covered (scoping.rs)
  - [x] Expressions in complex positions (function args, arithmetic, comparison) — covered (scoping.rs)
  - [x] Closure captures outer scope, shadow before closure — covered (scoping.rs)
  - [x] Tuple destructuring, struct from block, let in branches — covered (scoping.rs)
  - [ ] Nested block shadowing (inner let x leaks to outer) — AOT gap: block scoping (#[ignore])

- [ ] **String methods:** (2026-02-23)
  - [x] str.length / str.len (basic, empty, single char, spaces, escapes) — covered (strings.rs)
  - [x] str.is_empty (true, false, space) — covered (strings.rs)
  - [x] str.clone (basic, independence) — covered (strings.rs)
  - [x] str.iter (count, empty, for-loop) — covered (strings.rs)
  - [x] String == and != comparison — covered (strings.rs)
  - [x] String + operator concat and chain — covered (strings.rs)
  - [x] String in tuples and structs, struct field length — covered (strings.rs)
  - [x] String + to_str concat (int, bool) — covered (strings.rs)
  - [x] str.concat() method — **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS
  - [x] str.to_str() identity — **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS
  - [x] String ordering (<, >) — **FIXED** (2026-02-23) — wired emit_str_cmp_predicate in emit_binary_op
  - [ ] str.contains, starts_with, ends_with — not in builtin table (#[ignore])
  - [ ] str.trim, to_uppercase, to_lowercase — not in builtin table (#[ignore])
  - [ ] str.replace, split, repeat, chars — not in builtin table (#[ignore])

- [ ] **Collection methods:** (2026-02-23)
  - [x] List: length/len (empty, one, many), is_empty, clone — covered (collections_ext.rs)
  - [x] List: iter (count, fold sum, filter count, map+collect, any/all) — covered (collections_ext.rs)
  - [x] List: for-yield (basic, with filter guard) — covered (collections_ext.rs)
  - [x] List: string elements (length, iter count) — covered (collections_ext.rs)
  - [x] List: tuples as elements — covered (collections_ext.rs)
  - [x] Map: length/len (basic, one, alias), iter (count, for-loop), int keys — covered (collections_ext.rs)
  - [ ] List: push, pop, first, last, index, reverse, contains — not in builtin table (#[ignore])
  - [ ] Map: get, contains_key, keys, values, insert, remove — not in builtin table (#[ignore])

- [x] **Mutation & reassignment:** (2026-02-23)
  - Simple reassignment (single, multiple, self-reference) — covered (mutations.rs)
  - Loop patterns (counter, accumulator, product, conditional) — covered (mutations.rs)
  - Loop with break (manual loop, Collatz sequence) — covered (mutations.rs)
  - Conditional reassignment (if branch, if-else) — covered (mutations.rs)
  - String reassignment, bool reassignment — covered (mutations.rs)
  - Swap values, Fibonacci via mutation — covered (mutations.rs)
  - Min/max tracking over list iteration — covered (mutations.rs)
  - Nested loops with outer mutation — covered (mutations.rs)
  - String builder (concat in loop with to_str) — covered (mutations.rs)
  - Reassignment with function call results — covered (mutations.rs)

### 11.1.1 Discovered AOT Gaps (2026-02-23)

Gaps discovered during verification. **All gaps cross-referenced to real roadmap** (2026-02-23):
- §21A sections: 21.2, 21.3, 21.4, 21.5, 21.7, 21.10, 21.11, 21.12 updated with `[ ]` items
- §02 (Type Inference): reopened for closure-returning-closure inference bug
- §00 (Parser): tuple destructuring in for-loops marked as regression (was FIXED, now broken)

| Gap | Roadmap Location | Test | Severity |
|-----|-----------------|------|----------|
| Enum variant constructors | 21A § 21.2 | `test_aot_enum_construction` | Blocks enums, recursive types |
| Generic monomorphization | 21A § 21.7 | `test_aot_generic_identity` | **CRITICAL** — blocks 2,472+ sites |
| `catch(expr:)` lowering | 21A § 21.5 | `test_aot_catch_success` | Blocks panic recovery |
| String interpolation | 21A § 21.3 | `test_aot_string_interpolation` | Cosmetic |
| ~~Derive Eq struct (icmp)~~ | ~~21A § 21.19~~ | ~~`test_aot_derive_eq_struct`~~ | **FIXED** (2026-02-23) — emit_comparison_via_trait |
| List methods (push/first/last) | 21A § 21.10, 21.12 | `test_aot_list_push` et al. | Builtin table gap |
| Map methods (is_empty/get) | 21A § 21.10, 21.12 | `test_aot_map_is_empty` | Builtin table gap |
| `list[index]` subscript | 21A § 21.10 | `test_aot_list_index` | Index trait codegen |
| Closure returning closure | **Type checker bug** (§ 02) | `test_aot_closure_capturing_closure` | Inference regression |
| Bool in for-yield struct field | 21A codegen | `test_depth_combined_struct_iter_match` | **NEW** (2026-02-23) — comparison always stores false |
| Closure capturing 3+ strings | 21A closure codegen | `test_depth_closure_capturing_multiple_strings` | **NEW** (2026-02-23) — heap corruption |
| Mutation in match arms in for-do | 21A ArcIrEmitter | `test_depth_combined_match_closure_result` | **NEW** (2026-02-23) — variable not yet defined |
| Iterator .map struct field access | 21A closure codegen | `test_stress_combined_struct_closure_iteration` | **NEW** (2026-02-23) — invalid LLVM IR |
| Size cross-unit comparison | 21A literal codegen | `test_op_size_arithmetic` | **NEW** (2026-02-23) — unit normalization not applied in AOT equality |
| ~~`int.f()` return type tracking~~ | ~~21A builtin codegen~~ | ~~`test_conv_int_f_shorthand`~~ | **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS |
| ~~`int.byte()` type tracking~~ | ~~21A builtin codegen~~ | ~~`test_conv_int_to_byte`~~ | **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS |
| Block scope variable leak | 21A ARC IR scoping | `test_scope_shadow_in_nested_block` | **NEW** (2026-02-23) — inner `let x` overwrites outer scope |
| ~~`str.concat()` return type~~ | ~~21A builtin codegen~~ | ~~`test_str_concat_basic`~~ | **FIXED** (2026-02-23) — added to TYPECK_BUILTIN_METHODS |
| ~~String ordering (`<`, `>`)~~ | ~~21A comparison codegen~~ | ~~`test_str_compare_less`~~ | **FIXED** (2026-02-23) — wired emit_str_cmp_predicate |
| String methods (contains, trim, etc.) | 21A § 21.10 | `test_str_contains` et al. | Builtin table gap — 8 methods missing |
| List methods (push, first, last, etc.) | 21A § 21.10, 21.12 | `test_coll_list_push` et al. | Builtin table gap — 7 methods missing |
| Map methods (get, keys, values, etc.) | 21A § 21.10, 21.12 | `test_coll_map_get` et al. | Builtin table gap — 6 methods missing |
| Named fn → closure coercion | 21A closure codegen | `test_hof_apply_identity` et al. | **NEW** (2026-02-23) — named functions can't be passed as closure-typed params (ABI mismatch) |
| `(int) -> bool` as function param | 21A closure ABI | `test_hof_bool_lambda` | **NEW** (2026-02-23) — bool return from closure used as i64, branch expects i1 |
| Chained tuple field access `t.0.0` | Section 05 § 5.7 | `test_tuple_nested_pair_of_pairs` | **NEW** (2026-02-23) — parser lexes `t.0.0` as float literal, not chained field access |
| List-of-tuples iteration | 21A codegen | `test_tuple_in_loop` | **NEW** (2026-02-23) — heap corruption (malloc assertion failure) during for-loop over `[(1, 10), ...]` |
| Tuple destructuring in for loops | Parser / Section 00 | `test_tuple_destructure_in_loop` | **NEW** (2026-02-23) — `for (a, b) in ...` rejected: "for pattern requires named properties" |
| Tuple `==` equality | 21A comparison codegen | `test_tuple_equality` | **NEW** (2026-02-23) — compiles but returns wrong result (comparison codegen incorrect) |
| Struct update with string field | 21A struct codegen | `test_struct_update_with_string` | **NEW** (2026-02-23) — GEP on non-pointer value during spread, causes runtime segfault |

---

## 11.2 Dual-Execution Verification

Verify that AOT-compiled programs produce identical output to JIT-interpreted programs.

- [ ] Build a test harness that runs each test program twice:
  1. `ori run test.ori` → capture stdout, stderr, exit code (JIT)
  2. `ori build test.ori -o test && ./test` → capture stdout, stderr, exit code (AOT)
  3. Assert outputs are identical

- [ ] Apply to all spec tests in `tests/spec/`:
  ```bash
  for test in tests/spec/**/*.ori; do
      jit_output=$(ori run "$test" 2>&1) || true
      aot_output=$(ori build "$test" -o /tmp/test && /tmp/test 2>&1) || true
      diff <(echo "$jit_output") <(echo "$aot_output") || echo "MISMATCH: $test"
  done
  ```

- [ ] Track mismatches and investigate each one:
  - If JIT is correct and AOT differs → AOT bug
  - If AOT is correct and JIT differs → JIT bug
  - If both wrong → spec or type checker bug

- [ ] Create a CI-runnable script for this dual-execution check

---

## 11.3 Memory Safety Verification

- [ ] **Leak detection:** For every AOT test, verify `ori_rc_live_count()` returns 0 after `main` completes
  - Add a runtime hook that checks live count at exit
  - Any non-zero count indicates a leak
  - Report which types have leaked references

- [ ] **Use-after-free detection:** Compile and run tests under AddressSanitizer (ASan):
  ```bash
  CFLAGS="-fsanitize=address" cargo bl
  ./llvm-test.sh
  ```

- [ ] **Double-free detection:** Run under ASan — any double-free will be caught

- [ ] **Overflow detection:** Compile with refcount overflow checks enabled:
  - `ori_rc_inc` should panic (not wrap) if refcount exceeds `isize::MAX`

- [ ] **Stress test:** Create programs that exercise:
  - 10,000+ allocations/deallocations
  - Deep recursion (100+ levels)
  - Large collections (10,000+ elements)
  - Complex ownership patterns (diamond sharing, passing through multiple functions)

---

## 11.4 Performance Validation

- [ ] **Compile time:** Measure `ori build` time for programs of various sizes:
  - Small: 100 lines
  - Medium: 1,000 lines
  - Large: 10,000 lines (when available)
  - Track as baseline for future optimization

- [ ] **Runtime performance:** Compare AOT vs JIT execution time:
  - AOT should be significantly faster for compute-heavy programs
  - Measure with `time` or internal timing
  - Document the speedup ratio

- [ ] **RC overhead:** Measure the impact of RC operations:
  - Count total RcInc/RcDec executed at runtime (add counters)
  - Compare with and without RC elimination enabled
  - Report elimination effectiveness (% of ops removed)

- [ ] **Binary size:** Track compiled binary sizes:
  - Minimal program (hello world)
  - Medium program (data structure operations)
  - Record as baseline

---

## 11.5 Documentation

- [ ] Update `plans/arc_optimization/` to point to this plan as the superseding document
- [ ] Update `plans/arc_codegen_unification/` similarly
- [ ] Update `CLAUDE.md` if any new commands, paths, or patterns were introduced
- [ ] Update `.claude/rules/arc.md` with final pipeline description
- [ ] Add a brief architecture overview to `compiler/ori_arc/src/lib.rs` module doc
- [ ] Add a brief architecture overview to `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` module doc

---

## 11.6 Completion Checklist

- [ ] AOT test matrix covers all language features (every checkbox in 11.1 checked)
- [ ] Dual-execution script passes on all spec tests
- [ ] Zero memory leaks detected (live count = 0 at exit)
- [ ] ASan clean (no use-after-free, double-free)
- [ ] Stress tests pass
- [ ] Compile time baselined
- [ ] Runtime AOT > JIT performance verified
- [ ] RC elimination effectiveness measured and documented
- [ ] Binary sizes baselined
- [ ] All documentation updated
- [ ] Superseded plans marked as superseded
- [ ] `./test-all.sh` green
- [ ] `./llvm-test.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** Every Ori language feature compiles through AOT and produces identical results to JIT interpretation, with zero memory leaks, under all test conditions. The AOT pipeline is the single, unified codegen path.
