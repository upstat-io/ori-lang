# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-20 (AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-19 (J1-J17 polish), 2026-03-16 (re-run J1-J17), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)
**Journeys**: 19 total (4 simple, 4 moderate, 11 complex)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | OPTIMAL codegen, memory(none), zero ARC |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 10.0/10 | Branchless select, hybrid branch+select strategy |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 10.0/10 | TCO on gcd, zero empty blocks, leak detection |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | nonnull dereferenceable(32), memory(read) on ptr params |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0/10 | AIMS RC elision on @apply, uniform {ptr,ptr} ABI |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | Branchless select for tag-only enums, unreachable default |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.8/10 | 1 LOW: range unused field extraction |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | Zero-cost abstraction, identity = 1 instruction |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 10.0/10 | Aggregate sret load (-34% instructions), SSO guard |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0/10 | Landing pad elimination, borrow elision, drop_unique |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | Shape$eq 30% reduction via aggregate load |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 10.0/10 | ? operator = 8-instruction optimal, CFG simplification |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 10.0/10 | Null env for non-capturing closures, balanced ARC |
| J14 | "I am a fat pointer" | strings, arc, multiple_functions | 65 | PASS | PASS | 10.0/10 | Borrow elision, aggregate load, EH elimination |
| J15 | "I am nested fat" | lists, strings, arc, loops | 18 | PASS | PASS | 8.7/10 | 1 MEDIUM: option wrapping, 2 LOW: nounwind, dead loads |
| J16 | "I am fat and moving" | strings, arc, multiple_functions | 42 | PASS | PASS | 9.9/10 | 3 LOW: dead loads, sret copy, missing nounwind |
| J17 | "I am a captured fat pointer" | strings, closures, capture, arc | 10 | PASS | PASS | 9.9/10 | 2 LOW: dead loads, missing nounwind; C17 bug FIXED |
| J18 | "I am a string builder" | strings, loops, ranges, arc, lists | 67 | PASS | PASS | 10.0/10 | SSO-to-heap promotion, phi-based str accumulation |
| J19 | "I am a lifecycle" | nested_structs, loops, lists, strings, arc | 51 | PASS | PASS | 10.0/10 | Zero-cost pass_through, nested aggregate destruction |

## Recurring Issues

Issues appearing across multiple journeys in the current run:

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Missing nounwind on entry wrapper / @_ori_main | LOW | J15, J17 | LLVM generates unnecessary EH tables for main wrapper lacking nounwind |
| Dead aggregate loads on borrowed params | LOW | J16, J17 | Full struct loaded via `load { i64, i64, ptr }` but value unused -- pointer forwarded directly to runtime |
| Range construct-then-destructure with unused field | LOW | J7 | `extractvalue` for unused inclusive flag in exclusive range; LLVM DCE removes it |
| Option struct wrapping + alloca round-trip in iterator loop | MEDIUM | J15 | Iterator next() wraps has-next flag into option struct then immediately unwraps; +7 unjustified instr/iter |
| Missing nounwind on specific functions | LOW | J16 | @check_multi lacks nounwind despite all callees being nounwind (noreturn panic not recognized) |
| Missed transitive memory(none) propagation | NOTE | J13 | Lambda calling only memory(none) functions could itself be memory(none) |

**NOTE**: 16 of 19 journeys report zero issues. All CRITICAL and HIGH severity findings from previous runs have been resolved.

## Resolved Issues

### C15-1: Double-free on [str] elements — FIXED
**First seen**: Journey 15 (2026-03-16)
**Fixed in**: Pre-J15 reanalysis (aggregate load codegen + RC balance fix)
**Description**: Iterator consumption + list buffer drop both decremented string element RCs, causing double-free on `[str]` iteration.

### C15-2: Double ori_buffer_rc_dec in unwind path — FIXED
**First seen**: Journey 15 (2026-03-16)
**Fixed in**: Reanalysis confirmed 2 decs for RC=2 is correct unwind behavior
**Description**: Originally assessed as double-free in unwind path of @main; re-analysis determined the 2 decrements matched the RC=2 from two sharing sites.

### C17: Closure capturing str — Idx leak — FIXED
**First seen**: Journey 17 (2026-03-16)
**Fixed in**: Prior to f561649f (verified 2026-03-19)
**Description**: Closure capturing a `str` (fat pointer) produced an unresolved type variable Idx(202) at LLVM codegen, causing IR verification failure. Lambda parameter typed as `i64` instead of `ptr`, phantom `_ori_drop$202` emitted, AOT exit code 1. Score went from 3.0 to 9.9.

### Aggregate field-by-field materialization — FIXED
**First seen**: J9, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: sret structs were materialized via 9-instruction GEP+load+insertvalue chains instead of single `load { i64, i64, ptr }`. Now uses aggregate loads everywhere, saving 6-24 instructions per load site.

### Landing pad over-generation — FIXED
**First seen**: J10, J14, J15, J16 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: `invoke`+`landingpad`+`resume` used for calls to `nounwind` functions. Fixed-point nounwind analysis now correctly propagates, allowing plain `call` and eliminating personality/landingpad blocks.

### SSO guard ptrtoint duplication — FIXED
**First seen**: J14 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: SSO guard performed `ptrtoint` twice on the same pointer (once for bit-63 check, once for null check). Now performs a single `ptrtoint` and reuses the result.

### Empty trampoline blocks — FIXED
**First seen**: J5, J12 (2026-03-16)
**Fixed by**: CFG simplification pass
**Description**: Unconditional `br` to next sequential block in if/then/else and match codegen. Empty blocks eliminated; predecessors now branch directly to merge blocks.

### Redundant unconditional br in sso_len/heap_len — FIXED
**First seen**: J14 (2026-03-16)
**Fixed by**: 2026-03-19 run
**Description**: bb0 and bb1 in single-block functions were separated by an unnecessary unconditional branch. Blocks merged.

## Score Trend

| Difficulty | Journeys | Count | Avg Score | Range |
|------------|----------|-------|-----------|-------|
| Simple (J1-J4) | J1, J2, J3, J4 | 4 | 10.0 | 10.0-10.0 |
| Moderate (J5-J8) | J5, J6, J7, J8 | 4 | 9.95 | 9.8-10.0 |
| Complex (J9-J19) | J9-J19 | 11 | 9.9 | 8.7-10.0 |
| **Overall** | **J1-J19** | **19** | **9.9** | **8.7-10.0** |

**Perfect scores**: 16 of 19 journeys (84%) score 10.0/10.
**Near-perfect**: J16 and J17 at 9.9/10 (LOW findings only).
**Lowest**: J15 at 8.7/10 (1 MEDIUM + 2 LOW findings remaining).

**Comparison with pre-polish baseline (2026-03-16):**

| Difficulty | Baseline Avg | Current Avg | Delta |
|------------|-------------|-------------|-------|
| Simple (J1-J4) | 10.0 | 10.0 | 0.0 |
| Moderate (J5-J8) | 10.0 | 9.95 | -0.05 |
| Complex (J9-J17) | 8.8 | 9.9 | +1.1 |
| Complex (J18-J19) | N/A | 10.0 | new |
| **Overall** | **9.5** | **9.9** | **+0.4** |

The codegen polish plan (Sections 01-05) resolved the majority of quality findings. J18 and J19 are new journeys added on 2026-03-19/20 and both achieve perfect scores, validating that the polish improvements hold for new code patterns.

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
