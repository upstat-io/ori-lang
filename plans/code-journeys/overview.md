# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 9.8/10 | OPTIMAL codegen on both functions, zero ARC ops |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 9.2/10 | Branchless select for my_max, 5 empty blocks from if/else lowering |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 8.9/10 | Tail-call optimization on gcd, 77.8% attribute compliance |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 9.7/10 | All functions OPTIMAL, correct by-pointer passing for aggregates |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 8.8/10 | Clean {ptr,ptr} representation, indirect call targets lack fastcc |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 9.7/10 | Branchless select chains for tag-only enums, near-perfect codegen |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.2/10 | Correct phi-based loop lowering, empty trampoline blocks |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 9.8/10 | Zero-cost monomorphization, all functions at 1.00x ratio |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 8.8/10 | SSO gating correct, ARC balanced, branchless bool_to_int |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 8.7/10 | Borrow elision works, ARC balanced, correct iterator protocol |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 9.7/10 | Excellent derived Eq for structs/unit-sums/payload-sums |
| J12 | "I am an option" | option_type, pattern_matching, error_propagation | 33 | PASS | PASS | 9.1/10 | ? operator zero overhead, zero ARC for scalar Options |
| J13 | "I am an iterator" | iterators, iterator_adapters, lists, closures | 55 | PASS | PASS | 7.5/10 | Runtime delegation sound, dead null-env rc_dec caps ARC score |

**All 13 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results.

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Empty trampoline/passthrough blocks | LOW | J2, J3, J5, J7, J9, J10, J12, J13 | Blocks containing only `br label %next` — could be eliminated at emission time |
| Missing `uwtable` on C main wrapper | LOW | J1, J2, J7, J8 | The `@main` entry wrapper uses attribute group without `uwtable` |
| Missing `noundef` on some parameters | LOW | J4, J6, J8, J11 | Struct-typed and Box-typed params missing `noundef` annotation |
| Missing `memory(...)` annotations | LOW | J5, J7, J12 | Pure read-only or side-effect-free functions lack `memory(read)` or `memory(none)` |
| Redundant entry block branch | LOW | J3, J7, J12 | TCO loop lowering and loop/for emit an entry block with only `br label %loop.header` |
| Attribute compliance below 80% | LOW | J3 (77.8%), J5 (~58%), J13 (~44%) | Closures' indirect call targets, recursive functions, and iterator trampolines have lower compliance |
| Dead null-env rc_dec with `br i1 true` | MEDIUM | J13 | Non-capturing closures get null env pointers; ARC pipeline emits dead `ori_rc_dec` guarded by constant-true branch |
## Resolved Issues

### ARC metrics false-positive on heap types — FIXED
**First seen**: J9 (7.4/10), J10 (7.2/10) — ARC score capped at 3/10 by unbalanced gate
**Fixed in**: Tooling rewrite (2026-03-07) — two root causes eliminated:
1. **Drop function inclusion** (J9): `_ori_drop$*` functions passed `is_user_function` check despite being compiler-generated destructors. They naturally have 0 inc / 1 dec (calling `ori_rc_free`), which is correct behavior but flagged as unbalanced. Fix: exclude `_ori_drop$` from user function set in `ir_parser.py`.
2. **Landingpad block inflation** (J10): `invoke` instructions create landingpad cleanup blocks with RC operations that execute *instead of* normal continuation during stack unwinding. Naive counting treats them as cumulative. Fix: skip landingpad blocks in `arc_metrics.py::_count_rc_ops()`.
3. **Effect summaries** (both): `effect_summaries.py` correctly accounts for implicit allocations by runtime functions like `ori_str_from_raw` (+1 return effect) and `ori_list_alloc_data` (+1 return effect).

### `noreturn` on `ori_panic_cstr` — FIXED
**First seen**: Previous journey run
**Fixed in**: Current run (J1 confirms `noreturn` + `cold`)
**Description**: The `ori_panic_cstr` runtime function declaration now correctly has both `cold` and `noreturn` attributes.

### `nounwind` on user functions — FIXED
**First seen**: Previous journey run (J1 originally missing)
**Fixed in**: Current run (J2 confirms all user functions have `nounwind`)
**Description**: User-defined functions now correctly propagate `nounwind` via attribute groups.

### `noundef` on function parameters — FIXED
**First seen**: Previous journey run
**Fixed in**: Current run (J1 confirms `noundef` present on int params and returns)
**Description**: Function parameters and return values now carry `noundef` annotations for integer types.

### Struct codegen — IMPROVED
**First seen**: Previous run (J4 scored 8.5)
**Improved in**: Current run (J4 scores 9.7)
**Description**: Struct construction and field access now score OPTIMAL — the redundant insertvalue/extractvalue round-trip noted previously has been resolved.

## Score Trend

| Difficulty | Journeys | Avg Score | Range |
|------------|----------|-----------|-------|
| Simple (J1-J4) | 4 | 9.4 | 8.9–9.8 |
| Moderate (J5-J8) | 4 | 9.4 | 8.8–9.8 |
| Complex (J9-J13) | 5 | 8.8 | 7.5–9.7 |
| **Overall** | **13** | **9.1** | **7.5–9.8** |

### Score Distribution

- **9.5+** (near-perfect): J1 (9.8), J4 (9.7), J6 (9.7), J8 (9.8), J11 (9.7) — arithmetic, structs, pattern matching, generics, derived traits
- **9.0–9.4** (strong): J2 (9.2), J7 (9.2), J12 (9.1) — branching, loops, options
- **8.5–8.9** (solid): J3 (8.9), J5 (8.8), J9 (8.8), J10 (8.7) — recursion, closures, strings, lists
- **7.0–8.4** (needs work): J13 (7.5) — iterators (dead null-env rc_dec caps ARC score)

### Observations

- **Scalar-only journeys score highest** — when no ARC is needed, the compiler's codegen is near-perfect (J1, J4, J6, J8, J11 all 9.7+)
- **Heap-allocated types now score well** — strings (J9: 8.8) and lists (J10: 8.7) score 10/10 ARC after tooling fixes (effect summaries + drop exclusion + landingpad exclusion); remaining deductions are attribute compliance and control flow
- **Attribute compliance is the most common deduction** — across all journeys, missing attributes (memory, noundef, uwtable) are the primary source of non-NOTE findings
- **Instruction efficiency is excellent** — 8 of 13 journeys have functions at exactly 1.00x ratio (OPTIMAL), and none exceed 1.14x maximum
- **Struct improvement** — J4 jumped from 8.5 to 9.7 since the previous run, confirming a codegen improvement in struct handling
- **? operator is zero-overhead** — J12 confirms that `?` propagation compiles to the same code as manual match

## Tooling Notes

### `extract-metrics.py` — ARC False Positive on Heap Types — RESOLVED
Three fixes in the tooling rewrite eliminated all ARC false positives on J9 and J10:
1. `effect_summaries.py` — runtime function RC effect declarations (e.g., `ori_str_from_raw` returns +1)
2. `ir_parser.py` — `_ori_drop$*` exclusion from user function set
3. `arc_metrics.py` — landingpad block exclusion from RC counting

J9 and J10 re-scored with fixed tooling: J9 7.4→8.8, J10 7.2→8.7 (ARC 3/10→10/10 on both).

### `extract-metrics.py` — Quoted Function Names — RESOLVED
The `ir_parser_internal.py` now handles both bare `@name` and quoted `@"name"` LLVM function name formats, supporting monomorphized generics like `@"_ori_first$24m$24int_int"`.

## Results Files

- [Journey 1: "I am arithmetic"](01-arithmetic-results.md)
- [Journey 2: "I am a branch"](02-branching-results.md)
- [Journey 3: "I am recursive"](03-recursion-results.md)
- [Journey 4: "I am a struct"](04-structs-results.md)
- [Journey 5: "I am a closure"](05-closures-results.md)
- [Journey 6: "I am a match"](06-pattern-matching-results.md)
- [Journey 7: "I am a loop"](07-loops-results.md)
- [Journey 8: "I am generic"](08-generics-results.md)
- [Journey 9: "I am a string"](09-strings-results.md)
- [Journey 10: "I am a list"](10-lists-results.md)
- [Journey 11: "I am a derived trait"](11-derived-traits-results.md)
- [Journey 12: "I am an option"](12-options-results.md)
- [Journey 13: "I am an iterator"](13-iterators-results.md)
