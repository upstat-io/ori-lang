# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 9.8/10 | OPTIMAL codegen on both functions, zero ARC ops |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 9.2/10 | Branchless select for value selection, 5 CF defects (empty blocks) |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 8.9/10 | Tail-call optimization on gcd, 77.8% attribute compliance |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 8.5/10 | Redundant insertvalue/extractvalue round-trip, missing constant folding |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 8.8/10 | Clean {ptr, ptr} representation, 60% attribute compliance (structural) |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 9.7/10 | Branchless select chains for tag-only enums, near-perfect codegen |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.2/10 | Correct phi-based loop lowering, empty trampoline blocks |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 9.8/10 | Zero-cost monomorphization, all functions at 1.00x ratio |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 7.5/10 | SSO gating correct, ARC false-positive from metrics tool |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 8.2/10 | Borrow elision, unnecessary invoke/landing pad (HIGH) |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 9.8/10 | All 7 functions OPTIMAL, three distinct Eq patterns |
| J12 | "I am an option" | option_type, pattern_matching, error_propagation | 33 | PASS | PASS | 9.1/10 | ? operator zero-overhead, 3 empty blocks |

**All 12 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results.

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Empty trampoline/passthrough blocks | LOW | J2, J3, J5, J7, J9, J10, J12 | Blocks containing only `br label %next` — could be eliminated at emission time |
| Missing `uwtable` on C main wrapper | LOW | J1, J11 | The `@main` entry wrapper uses attribute group without `uwtable` |
| Missing `noundef` on some parameters | LOW | J6, J8 | Struct-typed and Box-typed params missing `noundef` annotation |
| Missing `memory(...)` annotations | LOW-MEDIUM | J2, J4, J12 | Pure read-only or side-effect-free functions lack `memory(read)` or `memory(none)` |
| Redundant entry block branch | LOW | J3, J7 | TCO loop lowering and loop/for emit an entry block with only `br label %loop.header` |
| Missing `nounwind` on `ori_panic_cstr` | LOW | J7 | Runtime panic function declaration missing `nounwind` in some journeys |
| Attribute compliance below 80% | LOW | J3 (77.8%), J5 (60%) | Structural issue — closures' indirect call targets and recursive functions have lower compliance |

### Highest-Severity Finding

**HIGH — Unnecessary invoke/landing pad for non-unwinding functions** (J10): The compiler emits `invoke` + landing pad infrastructure for calls to `count_items`, which provably never unwinds. This adds code size and prevents inlining optimizations. Root cause: nounwind analysis doesn't propagate through simple read-only user functions.

## Resolved Issues

### `noreturn` on `ori_panic_cstr` — FIXED
**First seen**: Previous journey run
**Fixed in**: Current run (J1 confirms)
**Description**: The `ori_panic_cstr` runtime function declaration now correctly has both `cold` and `noreturn` attributes, allowing LLVM to optimize code paths after panic calls.

### `nounwind` on user functions — FIXED
**First seen**: Previous journey run (J1 originally missing)
**Fixed in**: J2 confirms all user functions have `nounwind`
**Description**: User-defined functions now correctly propagate `nounwind` via attribute groups.

### `noundef` on function parameters — FIXED
**First seen**: Previous journey run
**Fixed in**: J1 confirms `noundef` present on params and returns
**Description**: Function parameters and return values now carry `noundef` annotations for integer types.

## Score Trend

| Difficulty | Journeys | Avg Score | Range |
|------------|----------|-----------|-------|
| Simple (J1-J4) | 4 | 9.1 | 8.5–9.8 |
| Moderate (J5-J8) | 4 | 9.2 | 8.8–9.8 |
| Complex (J9-J12) | 4 | 8.7 | 7.5–9.8 |
| **Overall** | **12** | **9.0** | **7.5–9.8** |

### Score Distribution

- **9.5+** (near-perfect): J1 (9.8), J6 (9.7), J8 (9.8), J11 (9.8) — arithmetic, pattern matching, generics, derived traits
- **9.0–9.4** (strong): J2 (9.2), J7 (9.2), J12 (9.1) — branching, loops, options
- **8.5–8.9** (solid): J3 (8.9), J4 (8.5), J5 (8.8) — recursion, structs, closures
- **< 8.5** (room to improve): J9 (7.5), J10 (8.2) — strings, lists (heap-allocated types with ARC)

### Observations

- **Scalar-only journeys score highest** — when no ARC is needed, the compiler's codegen is near-perfect (J1, J6, J8, J11 all 9.7+)
- **Heap-allocated types drop scores** — strings (J9: 7.5) and lists (J10: 8.2) introduce ARC complexity that surfaces attribute and control flow issues
- **Attribute compliance is the most common deduction** — across all journeys, missing attributes (memory, noundef, readonly) are the primary source of non-NOTE findings
- **Instruction efficiency is excellent** — 8 of 12 journeys have average instruction ratio ≤ 1.03x, with 5 at exactly 1.00x (OPTIMAL)

## Tooling Notes

### `extract-metrics.py` — Quoted Function Names
The `_FUNC_NAME_RE` regex in `ir_parser.py` cannot parse LLVM quoted function names like `@"_ori_first$24m$24int_int"` produced by monomorphized generics. Journey 8 (generics) works around this because `extract-metrics.py` still extracts enough functions to compute scores, but a proper fix should update the regex to handle `@"..."` names.

### `extract-metrics.py` — ARC False Positive on Strings
Journey 9's ARC score (3/10) reflects a metrics tool false positive: 0 `rc_inc` / 3 `rc_dec` appears unbalanced, but the initial RC increment is hidden inside `ori_str_from_raw` (a runtime function, not visible in the IR). The actual runtime behavior is leak-free and correctly balanced. The metrics tool should be taught to recognize paired runtime construction/destruction patterns.

### Multi-line Switch Parsing — FIXED
The `ir_parser.py` was splitting multi-line LLVM `switch` instructions into separate instructions, causing `extract_branch_targets` to miss case labels. This produced false `cf_incorrect: true` flags. Fixed during this journey run by joining continuation lines between `[` and `]`.

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
