# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-20 (full re-run of all 20 journeys, AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-19 (J1-J17 polish), 2026-03-16 (re-run J1-J17), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)
**Journeys**: 20 total (4 simple, 4 moderate, 12 complex)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | OPTIMAL codegen, memory(none), zero ARC |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 10.0/10 | Branchless select, hybrid branch+select strategy |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 10.0/10 | TCO on gcd, zero empty blocks, leak detection |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | nonnull dereferenceable(32), memory(read) on ptr params |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0/10 | AIMS RC elision on @apply, uniform {ptr,ptr} ABI |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | Branchless select for tag-only enums, unreachable default |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 10.0/10 | Range unused field extraction FIXED (was 9.8) |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | Zero-cost abstraction, identity = 1 instruction |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 10.0/10 | Aggregate sret load, SSO guard, nounwind propagation |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0/10 | Landing pad elimination, borrow elision, drop_unique |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | Shape$eq 30% reduction via aggregate load |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 10.0/10 | ? operator = 8-instruction optimal, CFG simplification |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 10.0/10 | Null env for non-capturing closures, balanced ARC |
| J14 | "I am a fat pointer" | strings, arc, multiple_functions | 65 | PASS | PASS | 10.0/10 | Borrow elision, dead load eliminated, nounwind complete |
| J15 | "I am nested fat" | lists, strings, arc, loops | 18 | PASS | PASS | 10.0/10 | nounwind + iterator loop fix (was 8.7) |
| J16 | "I am fat and moving" | strings, arc, multiple_functions | 42 | PASS | PASS | 10.0/10 | Dead loads + nounwind FIXED (was 9.9) |
| J17 | "I am a captured fat pointer" | strings, closures, capture, arc | 10 | PASS | PASS | 9.8/10 | 1 LOW: dead param load in lambda body |
| J18 | "I am a string builder" | strings, loops, ranges, arc, lists | 67 | PASS | PASS | 10.0/10 | SSO-to-heap promotion, phi-based str accumulation |
| J19 | "I am a lifecycle" | nested_structs, loops, lists, strings, arc | 51 | PASS | PASS | 10.0/10 | Zero-cost pass_through, nested aggregate destruction |
| J20 | "I am copy-on-write" | cow, strings, lists, slices, sharing, rc | 105 | PASS | PASS | 10.0/10 | Static uniqueness, seamless slices, SSO guard, COW fork |

## Recurring Issues

Issues appearing across multiple journeys in the current run:

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Dead aggregate load on borrowed param | LOW | J17 | Lambda loads full `{ i64, i64, ptr }` str but only forwards the pointer to runtime |

**NOTE**: 19 of 20 journeys report zero defects. The only remaining issue is a single dead load in J17's lambda body — harmless at -O1+ but present in debug builds.

## Resolved Issues

### Range unused field extraction — FIXED
**First seen**: Journey 7 (2026-03-16)
**Fixed in**: 2026-03-20 re-run (compiler improvement)
**Description**: `extractvalue` for unused inclusive flag in exclusive range. Now eliminated — J7 score 9.8 → 10.0.

### Missing nounwind on entry wrapper / @_ori_main — FIXED
**First seen**: Journey 15, J17 (2026-03-16)
**Fixed in**: 2026-03-20 re-run (fixed-point nounwind analysis improvement)
**Description**: LLVM generated unnecessary EH tables for main wrapper lacking nounwind. Now correctly propagated through the entire call graph.

### Dead aggregate loads on borrowed params — FIXED (partial)
**First seen**: Journey 16, J17 (2026-03-19)
**Fixed in**: J16 fixed in 2026-03-20; J17 lambda still has 1 dead load
**Description**: Full struct loaded but value unused — pointer forwarded directly to runtime.

### Option struct wrapping + alloca round-trip in iterator loop — FIXED
**First seen**: Journey 15 (2026-03-16)
**Fixed in**: 2026-03-20 re-run (iterator loop body simplified)
**Description**: Iterator next() wrapped has-next flag into option struct then immediately unwrapped. +7 unjustified instr/iter. Now eliminated — J15 score 8.7 → 10.0.

### Missing nounwind on specific functions — FIXED
**First seen**: Journey 16 (2026-03-19)
**Fixed in**: 2026-03-20 re-run (nounwind fixed-point analysis)
**Description**: @check_multi lacked nounwind despite all callees being nounwind. Now correctly propagated.

### C15-1: Double-free on [str] elements — FIXED
**First seen**: Journey 15 (2026-03-16)
**Fixed in**: Pre-J15 reanalysis (aggregate load codegen + RC balance fix)
**Description**: Iterator consumption + list buffer drop both decremented string element RCs.

### C17: Closure capturing str — Idx leak — FIXED
**First seen**: Journey 17 (2026-03-16)
**Fixed in**: Prior to f561649f (verified 2026-03-19)
**Description**: Closure capturing a `str` produced an unresolved type variable Idx(202) at LLVM codegen. Score went from 3.0 to 9.9.

### Aggregate field-by-field materialization — FIXED
**First seen**: J9, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: sret structs materialized via 9-instruction GEP+load+insertvalue chains. Now uses aggregate loads.

### Landing pad over-generation — FIXED
**First seen**: J10, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: `invoke`+`landingpad`+`resume` used for calls to `nounwind` functions. Eliminated by fixed-point nounwind analysis.

### SSO guard ptrtoint duplication — FIXED
**First seen**: J14 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: SSO guard performed `ptrtoint` twice on the same pointer. Now single `ptrtoint` reused.

### Empty trampoline blocks — FIXED
**First seen**: J5, J12 (2026-03-16)
**Fixed by**: CFG simplification pass
**Description**: Unconditional `br` to next sequential block. Eliminated.

## Score Trend

| Difficulty | Journeys | Count | Avg Score | Range |
|------------|----------|-------|-----------|-------|
| Simple (J1-J4) | J1, J2, J3, J4 | 4 | 10.0 | 10.0–10.0 |
| Moderate (J5-J8) | J5, J6, J7, J8 | 4 | 10.0 | 10.0–10.0 |
| Complex (J9-J20) | J9-J20 | 12 | 10.0 | 9.8–10.0 |
| **Overall** | **J1-J20** | **20** | **10.0** | **9.8–10.0** |

**Perfect scores**: 19 of 20 journeys (95%) score 10.0/10.
**Near-perfect**: J17 at 9.8/10 (1 LOW: dead param load in lambda).

**Comparison with previous runs:**

| Run | Avg Score | Perfect | Lowest | Key Change |
|-----|-----------|---------|--------|------------|
| 2026-03-16 (initial) | 9.5 | 8/17 | 3.0 (J17) | Baseline |
| 2026-03-19 (polish) | 9.9 | 16/19 | 8.7 (J15) | Aggregate load, nounwind, CFG simplify |
| **2026-03-20 (full re-run)** | **10.0** | **19/20** | **9.8 (J17)** | **Iterator loop fix, range field elimination, nounwind propagation** |

## Tooling Notes

- `effect_summaries.py` was missing 8 iterator consumer functions (`ori_iter_fold`, `ori_iter_collect`, etc.). Fixed during J13 re-analysis — these functions consume their iterator param (param[0] = -1).

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
- [Journey 18: "I am a string builder"](18-string-builder-results.md)
- [Journey 19: "I am a lifecycle"](19-rc-lifecycle-results.md)
- [Journey 20: "I am copy-on-write"](20-cow-patterns-results.md)
