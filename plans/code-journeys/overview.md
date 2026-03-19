# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-19 (AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-16 (re-run J1-J17), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | PERFECT — zero waste, 100% attributes, memory(none) |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 10.0/10 | PERFECT — branchless select, memory(none) on all |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 10.0/10 | PERFECT — TCO on gcd, loop entry block structural |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — nonnull dereferenceable(32), OPTIMAL GEP |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0/10 | PERFECT — AIMS RC elision, lambda naming improved |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | PERFECT — branchless select for tag-only enums |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 10.0/10 | PERFECT — range unused field extraction FIXED (was 9.8) |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — zero-cost abstraction, identity=1 instr |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 10.0/10 | Aggregate sret load optimization (-34% instructions) |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0/10 | Landing pad elimination, aggregate loads |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | Shape$eq 30% instruction reduction via aggregate load |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 10.0/10 | PERFECT — ? operator = 8-instruction optimal sequence |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 10.0/10 | PERFECT — null env for non-capturing, balanced ARC |
| J14 | "I am a fat pointer" | strings, arc, multiple_functions | 65 | PASS | PASS | 10.0/10 | 3 codegen improvements FIXED (was 9.4) |
| J15 | "I am nested fat" | lists, strings, arc, loops | 18 | PASS | PASS | 10.0/10 | PERFECT — all issues FIXED: option wrapping, nounwind (was 8.7) |
| J16 | "I am fat and moving" | strings, arc, multiple_functions | 42 | PASS | PASS | 10.0/10 | PERFECT — dead loads + sret copy + nounwind all FIXED (was 9.9) |
| J17 | "I am a captured fat pointer" | strings, closures, capture, arc | 10 | PASS | PASS | 10.0/10 | PERFECT — dead loads + nounwind FIXED (was 9.9) |

## Score Changes Since Previous Run (2026-03-19 codegen polish)

| Journey | Previous | Current | Delta | Reason |
|---------|----------|---------|-------|--------|
| J7 | 9.8 | 10.0 | +0.2 | Range unused field extraction FIXED (Section 05) |
| J15 | 8.7 | 10.0 | +1.3 | Option struct wrapping FIXED (Section 04), nounwind FIXED (Section 01) |
| J16 | 9.9 | 10.0 | +0.1 | Dead aggregate loads FIXED (Section 02), nounwind FIXED (Section 01) |
| J17 | 9.9 | 10.0 | +0.1 | Dead loads FIXED (Section 02), nounwind FIXED (Section 01) |

All 17 journeys now at 10.0/10. Previous: J1-J6, J8-J14 already at 10.0 (unchanged).

## Recurring Issues

All recurring issues have been resolved as of 2026-03-19:

| Issue | Status | Fixed By |
|-------|--------|----------|
| Missing nounwind on @_ori_main | RESOLVED | Section 01: Nounwind Propagation |
| Dead aggregate loads | RESOLVED | Section 02: Dead Aggregate Load Elimination |
| Option struct wrapping overhead | RESOLVED | Section 04: Iterator Option Wrapping |
| Range unused field extraction | RESOLVED | Section 05: Range Unused Field Extraction |
| Sret identity copy | RESOLVED | Section 03: Sret Identity Copy Elimination |

## Resolved Issues

### Aggregate field-by-field materialization — FIXED
**First seen**: J9, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: sret structs were materialized via 9-instruction GEP+load+insertvalue chains instead of single `load { i64, i64, ptr }`. Now uses aggregate loads everywhere, saving 6-24 instructions per load site.

### Landing pad over-generation — FIXED
**First seen**: J10, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: `invoke`+`landingpad`+`resume` used for calls to `nounwind` functions. Fixed-point nounwind analysis now correctly propagates, allowing plain `call` and eliminating personality/landingpad blocks.

### Closure capturing str — Idx leak — FIXED
**First seen**: J17 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: Closure capturing a `str` variable triggered an unresolved type variable (Idx(202)) at LLVM codegen, producing a phantom `_ori_drop$202` function and type mismatches in RC operations. Lambda parameter `s` was typed as `i64` instead of the correct fat pointer type.

### Double-free on [str] elements — FIXED
**First seen**: J15 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: Iterating over `[str]` with for-loop caused double-free of string elements. The unwind path RC cleanup was emitting duplicate `ori_buffer_rc_dec` calls. Now correctly balanced.

### SSO guard ptrtoint duplication — FIXED
**First seen**: J14 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: SSO guard performed `ptrtoint` twice on the same pointer (once for bit-63 check, once for null check). Now performs a single `ptrtoint` and reuses the result.

### Missing nounwind on @_ori_main — FIXED
**First seen**: J15, J16, J17 (2026-03-16)
**Fixed by**: Codegen Polish Section 01 (Nounwind Propagation)
**Description**: Entry main wrapper and user functions lacked `nounwind` attribute. Two-pass nounwind analysis with fixed-point post-hoc pass now correctly propagates nounwind through call chains including builtin methods, closures, and derived trait methods.

### Dead aggregate loads — FIXED
**First seen**: J16, J17 (2026-03-16)
**Fixed by**: Codegen Polish Section 02 (Dead Aggregate Load Elimination)
**Description**: Borrowed parameters loaded as full aggregates but the loaded value was never used — downstream runtime calls forwarded the pointer directly. `compute_pointer_only_params()` now identifies these parameters and skips the dead load.

### Sret identity copy — FIXED
**First seen**: J16 (2026-03-16)
**Fixed by**: Codegen Polish Section 03 (Sret Identity Copy Elimination)
**Description**: Functions returning their own parameter by value via sret performed a redundant memcpy from parameter alloca to sret slot. Now detected and eliminated when source and destination types match.

### Option struct wrapping overhead — FIXED
**First seen**: J15 (2026-03-16)
**Fixed by**: Codegen Polish Section 04 (Iterator Option Wrapping)
**Description**: For-loop iterator next() wrapped the has-next flag and element into a `{i64, T}` option struct via `insertvalue`, then immediately extracted them. Side-channel decomposition now stores tag and scratch pointer separately, eliminating the struct round-trip.

### Range unused field extraction — FIXED
**First seen**: J7 (2026-03-16)
**Fixed by**: Codegen Polish Section 05 (Range Unused Field Extraction)
**Description**: Range inclusive flag (field 3 of `{start, end, step, inclusive}`) was extracted via `extractvalue` but never used in exclusive ranges. Now only extracted when the range is inclusive.

## Score Trend

| Difficulty | Journeys | Avg Score | Range |
|------------|----------|-----------|-------|
| Simple (J1-J4) | 4 | 10.0 | 10.0–10.0 |
| Moderate (J5-J8) | 4 | 10.0 | 10.0–10.0 |
| Complex (J9-J17) | 9 | 10.0 | 10.0–10.0 |
| **Overall** | **17** | **10.0** | **10.0–10.0** |

**Comparison with pre-polish baseline (2026-03-16):**

| Difficulty | Baseline Avg | Current Avg | Delta |
|------------|-------------|-------------|-------|
| Simple (J1-J4) | 10.0 | 10.0 | 0.0 |
| Moderate (J5-J8) | 10.0 | 10.0 | 0.0 |
| Complex (J9-J17) | 8.8 | 10.0 | +1.2 |
| **Overall** | **9.5** | **10.0** | **+0.5** |

All 17 journeys now score 10.0/10 — PERFECT across the board. The codegen polish plan (Sections 01-05) resolved all remaining quality findings: nounwind propagation, dead aggregate load elimination, sret identity copy elimination, iterator option wrapping, and range unused field extraction.

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
- [Journey 14: "I am a fat pointer"](14-fat-string-sharing-results.md)
- [Journey 15: "I am nested fat"](15-fat-nested-collections-results.md)
- [Journey 16: "I am fat and moving"](16-fat-ownership-transfer-results.md)
- [Journey 17: "I am a captured fat pointer"](17-fat-closure-capture-results.md)
