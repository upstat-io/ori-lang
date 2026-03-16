# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-16 (AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-16 (AIMS Section 02), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Prev | Delta | Key Findings |
|---|------|----------|----------|------|-----|-------|------|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | 9.8 | **+0.2** | PERFECT — zero waste, 100% attributes |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 9.3/10 | 9.2 | **+0.1** | Branchless select, 100% attributes, 5 CF defects |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 9.3/10 | 9.2 | **+0.1** | TCO on gcd, 100% attributes, 4 CF defects |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | 9.7 | **+0.3** | PERFECT — OPTIMAL codegen, memory(argmem:read) |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 9.2/10 | 9.2 | — | AIMS elision, 82% attributes (trampoline gaps) |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | 9.8 | **+0.2** | PERFECT — branchless select, zero waste |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.2/10 | 9.2 | — | Phi-based loops, memory(none) on sum_loop |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | 9.9 | **+0.1** | PERFECT — zero-cost abstraction, OPTIMAL |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 8.7/10 | 8.8 | -0.1 | SSO guard correct, 4 CF defects, 2 missing attrs |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 9.0/10 | 8.8 | **+0.2** | ARC balanced, 100% attributes, borrow elision |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | 9.8 | **+0.2** | PERFECT — three Eq patterns OPTIMAL |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 9.4/10 | 9.3 | **+0.1** | ? operator zero overhead, 100% attributes |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 9.5/10 | 9.4 | **+0.1** | OPTIMAL instructions, 61% attr (trampoline gaps) |

**All 13 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results. **5 journeys achieve perfect 10.0/10** (J1, J4, J6, J8, J11).

## What Changed Since Previous Run

| Category | Journeys Affected | Impact |
|----------|------------------|--------|
| **Improved** | J1 (+0.2), J2 (+0.1), J3 (+0.1), J4 (+0.3), J6 (+0.2), J8 (+0.1), J10 (+0.2), J11 (+0.2), J12 (+0.1), J13 (+0.1) | `noundef` on main wrapper, full attribute compliance |
| **Unchanged** | J5, J7 | Score parity |
| **Minor regression** | J9 (-0.1) | Metric extraction difference, not codegen regression |

### Key Improvements

1. **`noundef` on C main wrapper return** — All 13 journeys now emit `define noundef i32 @main(...)`, closing the last attribute gap for simple programs.
2. **Full attribute compliance in 10/13 journeys** — J1, J2, J3, J4, J6, J7, J8, J10, J11, J12 all hit 100% compliance. Only J5 (82%), J9 (89%), J13 (61%) have gaps from trampoline/lambda functions.
3. **5 perfect scores** — J1, J4, J6, J8, J11 all achieve 10.0/10 with zero unjustified instructions, zero ARC violations, 100% attributes, zero CF defects.
4. **`nounwind` propagation improved** — Fixed-point analysis now reaches indirect-call functions like `@apply` (J5) through proven nounwind callees.

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Empty trampoline blocks | LOW | J2, J3, J7, J9, J10, J12 | Unconditional `br` to next sequential block in if/else lowering |
| Missing trampoline attributes | LOW | J5, J9, J13 | Trampoline/lambda functions lack `uwtable`, `noundef` on env pointer |
| Redundant branches | LOW | J2, J3, J7, J10, J12 | Branches where both targets could be merged |
| Missing `memory(none)` on @sum_for | LOW | J7 | Range struct insertvalue/extractvalue confuses purity analysis |

## Score Trend

| Difficulty | Journeys | Avg Score (Current) | Avg Score (Prev) | Range (Current) |
|------------|----------|--------------------|--------------------|-----------------|
| Simple (J1-J4) | 4 | 9.7 | 9.5 | 9.3–10.0 |
| Moderate (J5-J8) | 4 | 9.6 | 9.4 | 9.2–10.0 |
| Complex (J9-J13) | 5 | 9.3 | 9.2 | 8.7–10.0 |
| **Overall** | **13** | **9.5** | **9.3** | **8.7–10.0** |

## Per-Category Averages

| Category | Weight | Avg Score | Perfect (10/10) | Lowest |
|----------|--------|-----------|-----------------|--------|
| Instruction Efficiency | 15% | 9.5 | 7/13 | 9 (six journeys) |
| ARC Correctness | 20% | 10.0 | 13/13 | — |
| Attributes & Safety | 10% | 9.2 | 10/13 | 5 (J13) |
| Control Flow | 10% | 8.6 | 5/13 | 7 (J2,J3,J7,J9,J10) |
| IR Quality | 20% | 9.4 | 7/13 | 8 (J9,J10) |
| Binary Quality | 10% | 10.0 | 13/13 | — |
| Other Findings | 15% | 9.8 | 11/13 | 9 (J7,J9,J10) |

**Strengths**: ARC correctness (perfect 10.0 across all 13) and binary quality (perfect 10.0) are flawless. Attribute compliance dramatically improved from 7.5 avg to 9.2 avg.

**Remaining gaps**: Control flow (avg 8.6) is the weakest category due to empty trampoline blocks in if/else and loop lowering. Trampoline/lambda attribute emission (J5, J9, J13) needs work for full compliance.

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
