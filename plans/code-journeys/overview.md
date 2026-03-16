# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-16 (AIMS branch `experiment/aims`, post Section 01+02 + CFG simplification)
**Previous runs**: 2026-03-16 (AIMS Section 02), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | PERFECT — zero waste, 100% attributes, memory(none) |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 10.0/10 | PERFECT — branchless select, memory(none) on all |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 9.7/10 | TCO on gcd, 1 redundant entry block from loop transform |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — nonnull dereferenceable(32), OPTIMAL GEP |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0/10 | PERFECT — AIMS RC elision, full noundef on closures |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | PERFECT — branchless select for tag-only enums |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.5/10 | Phi-based loops, 1 unused range field projection |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — zero-cost abstraction, identity=1 instr |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 10.0/10 | PERFECT — SSO guard correct, drop attrs fixed |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0/10 | PERFECT — borrow elision, drop_unique optimization |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | PERFECT — three Eq patterns OPTIMAL, memory(none) |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 10.0/10 | PERFECT — ? operator 8 instr, CFG simplified |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 10.0/10 | PERFECT — zero user RC, full trampoline attrs |

**All 13 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results. **11 of 13 journeys achieve perfect 10.0/10.**

## What Changed Since Previous Run

| Category | Change | Impact |
|----------|--------|--------|
| **CFG simplification** | Empty trampoline blocks eliminated | J2: 9.3->10.0, J3: 9.3->9.7, J7: 9.2->9.5, J9: 8.7->10.0, J12: 9.4->10.0 |
| **Posthoc purity** | `memory(none)` on pure functions | J6, J7, J8 improved — sum_for, extractvalue-only functions recognized |
| **noundef coverage** | Full annotation on closure/trampoline params | J5: 9.2->10.0, J9: drop attrs fixed, J13: 9.5->10.0 |
| **nonnull/deref** | Pointer validity annotations | J4, J10, J11 improved — struct ptr params fully annotated |

### Score Delta Summary

| Journey | Previous | Current | Delta |
|---------|----------|---------|-------|
| J1 | 9.8 | 10.0 | +0.2 |
| J2 | 9.3 | 10.0 | **+0.7** |
| J3 | 9.3 | 9.7 | +0.4 |
| J4 | 10.0 | 10.0 | — |
| J5 | 9.2 | 10.0 | **+0.8** |
| J6 | 10.0 | 10.0 | — |
| J7 | 9.2 | 9.5 | +0.3 |
| J8 | 10.0 | 10.0 | — |
| J9 | 8.7 | 10.0 | **+1.3** |
| J10 | 9.0 | 10.0 | **+1.0** |
| J11 | 10.0 | 10.0 | — |
| J12 | 9.4 | 10.0 | **+0.6** |
| J13 | 9.5 | 10.0 | **+0.5** |

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Redundant entry block from TCO | LOW | J3 | `@gcd` loop transform leaves 1 unnecessary `br label %bb0` |
| Range construct-then-destructure | LOW | J7 | `@sum_for` creates range struct then extracts fields with 1 unused projection |

Only 2 LOW-severity findings remain across all 13 journeys. Zero CRITICAL, HIGH, or MEDIUM findings.

## Resolved Issues

### Empty trampoline blocks — FIXED
**First seen**: J2 (prior run, 2026-03-16)
**Fixed in**: AIMS Section 01 (CFG simplification pass)
**Description**: Unconditional `br` to next sequential block eliminated. Previously affected J2, J3, J7, J9, J10, J12.

### Missing nounwind on user functions — FIXED
**First seen**: J1 (prior run, 2026-03-03)
**Fixed in**: AIMS Section 01 (posthoc nounwind analysis)
**Description**: Fixed-point analysis now propagates nounwind through call graphs, including indirect calls.

### Missing memory(...) attributes — FIXED
**First seen**: J1 (prior run)
**Fixed in**: AIMS Section 02 (posthoc readonly/memory analysis)
**Description**: Pure functions now receive `memory(none)` via two-pass purity analysis. Functions with by-value struct params via extractvalue correctly classified as pure.

### Missing noundef on closure/trampoline infrastructure — FIXED
**First seen**: J5 (prior run)
**Fixed in**: AIMS Section 01
**Description**: Lambda env pointer parameters and trampoline functions now carry full `noundef` annotations.

### Missing nonnull/dereferenceable on pointer params — FIXED
**First seen**: J4 (prior run)
**Fixed in**: AIMS Section 01.4
**Description**: Struct pointer parameters now carry `nonnull dereferenceable(N)` enabling LLVM to eliminate null checks and speculate loads.

### Missing uwtable on drop helpers — FIXED
**First seen**: J9 (prior run)
**Fixed in**: AIMS Section 01
**Description**: Drop helper functions now receive `uwtable` for proper stack unwinding support.

## Score Trend

| Difficulty | Journeys | Avg Score | Prev Avg | Range |
|------------|----------|-----------|----------|-------|
| Simple (J1-J4) | 4 | 9.9 | 9.5 | 9.7–10.0 |
| Moderate (J5-J8) | 4 | 9.9 | 9.6 | 9.5–10.0 |
| Complex (J9-J13) | 5 | 10.0 | 9.3 | 10.0–10.0 |
| **Overall** | **13** | **9.9** | **9.5** | **9.5–10.0** |

All 5 complex-difficulty journeys now achieve perfect 10.0/10 — the largest improvement tier (+0.7 avg). The AIMS passes had the most dramatic effect on programs with ARC, closures, iterators, and collections.

## Per-Category Averages

| Category | Weight | Avg Score | Perfect (10/10) |
|----------|--------|-----------|-----------------|
| Instruction Efficiency | 15% | 9.8 | 11/13 |
| ARC Correctness | 20% | 10.0 | 13/13 |
| Attributes & Safety | 10% | 10.0 | 13/13 |
| Control Flow | 10% | 10.0 | 13/13 |
| IR Quality | 20% | 9.8 | 11/13 |
| Binary Quality | 10% | 10.0 | 13/13 |
| Other Findings | 15% | 9.8 | 11/13 |

**Strengths**: ARC correctness, attributes, control flow, and binary quality are all perfect 10.0 averages (13/13 journeys at 10). Attribute compliance went from 9.2 avg (prior run) to 10.0 avg.

**Remaining gaps**: J3 and J7 each have 1 unjustified instruction (entry block from TCO, unused range projection) preventing perfect 10.0 on instruction efficiency and IR quality.

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
