# Code Journeys -- Overview

Code journeys trace a single Ori program through the entire compiler pipeline -- lexer, parser, type checker, canonicalization, interpreter, LLVM codegen, AOT binary -- and perform deep scrutiny on the generated output. Each journey targets a specific language feature set, scoring the compiler's codegen across seven dimensions: instruction efficiency, ARC correctness, attributes/safety, control flow, IR quality, binary quality, and other findings.

Journeys are the primary mechanism for validating that the AIMS (ARC Intelligent Memory System) pipeline produces correct, efficient, and well-attributed LLVM IR. Every journey compares interpreter (eval) results against AOT (compiled binary) results to verify semantic equivalence.

**Latest run**: 2026-03-22 (J15-J17 re-run with updated findings), 2026-03-20 (full re-run of all 20 journeys)
**Branch**: `experiment/aims`
**Previous runs**: 2026-03-19 (J1-J17 polish), 2026-03-16 (re-run J1-J17), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)
**Journeys**: 20 total (4 simple, 4 moderate, 12 complex)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings, int_literals | 33 | PASS | PASS | 10.0 | OPTIMAL codegen, memory(none) on pure functions, zero ARC |
| J2 | "I am a branch" | branching, comparison, function_calls | 17 | PASS | PASS | 10.0 | Branchless select for max, hybrid branch+select for nested if |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 10.0 | TCO on gcd (loop lowering), zero empty blocks in fib, leak detection |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0 | nonnull dereferenceable(32), memory(argmem: read) on ptr params |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0 | AIMS RC elision on @apply, uniform {ptr, ptr} closure ABI |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring, exhaustiveness | 41 | PASS | PASS | 10.0 | Branchless select chain for tag-only enums, unreachable default |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 10.0 | Range unused field extraction FIXED, phi-based loop lowering |
| J8 | "I am generic" | generics, monomorphization, generic_structs, type_inference | 57 | PASS | PASS | 10.0 | Zero-cost abstraction, all generics at 1.00x instruction ratio |
| J9 | "I am a string" | strings, string_methods, arc, branching | 13 | PASS | PASS | 10.0 | SSO guard pattern, nounwind propagation through string calls |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0 | Borrow elision on readonly params, drop_unique optimization |
| J11 | "I am a derived trait" | derived_traits, trait_methods, struct_construction, sum_types | 33 | PASS | PASS | 10.0 | Shape$eq 30% instruction reduction, short-circuit equality |
| J12 | "I am an option" | option_type, pattern_matching, error_propagation | 33 | PASS | PASS | 10.0 | ? operator OPTIMAL (zero overhead), CFG simplification |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures, higher_order | 55 | PASS | PASS | 10.0 | Null env for non-capturing closures, balanced ARC pipeline |
| J14 | "I am a fat pointer" | strings, arc, function_calls | 65 | PASS | PASS | 10.0 | Borrow elision, aggregate load 9:1 reduction, nounwind complete |
| J15 | "I am nested fat" | lists, strings, arc, loops | 18 | PASS | PASS | 9.7 | 2 LOWs: dead loop load, dead debug loads; SSO guard correct |
| J16 | "I am fat and moving" | strings, arc, function_calls | 42 | PASS | PASS | 10.0 | sret ownership transfer, ori_str_rc_dec consolidation |
| J17 | "I am a captured fat pointer" | strings, arc, closures, capture, higher_order | 10 | PASS | PASS | 9.8 | 1 LOW: dead param.load in lambda body |
| J18 | "I am a string builder" | strings, string_methods, loops, ranges, arc, lists | 67 | PASS | PASS | 10.0 | SSO-to-heap promotion, phi-based str accumulation |
| J19 | "I am a lifecycle" | struct_construction, nested_structs, loops, lists, strings, arc | 51 | PASS | PASS | 10.0 | Zero-cost pass_through, nested aggregate destruction cascade |
| J20 | "I am copy-on-write" | cow, strings, lists, loops, ranges, arc | 105 | PASS | PASS | 10.0 | Static uniqueness analysis, seamless slices, COW fork protocol |

## Recurring Issues

Active issues appearing in the current run:

| Issue | Severity | Journeys | Status | Description |
|-------|----------|----------|--------|-------------|
| Dead aggregate load on borrowed param | LOW | J15, J17 | OPEN | Lambda/loop loads full `{ i64, i64, ptr }` str fat pointer but only forwards the pointer to runtime. Harmless at -O1+ but present in debug builds. |
| Dead loads in debug disassembly | LOW | J15 | OPEN | Unoptimized native code loads data ptr and cap fields that are never used. Artifact of debug-mode aggregate loads; eliminated by LLVM optimization passes. |

**Summary**: 18 of 20 journeys report zero defects. J15 has 2 LOW findings (dead loads in loop body and debug disassembly), J17 has 1 LOW finding (dead param load in lambda). All are debug-only artifacts eliminated by LLVM optimization passes.

## Resolved Issues

Issues found and fixed across journey runs:

### Range unused field extraction -- FIXED
**First seen**: Journey 7 (2026-03-16) | **Fixed in**: 2026-03-20 re-run
**Description**: `extractvalue` for unused inclusive flag in exclusive range. Now eliminated -- J7 score 9.8 -> 10.0.

### Missing nounwind on entry wrapper / @_ori_main -- FIXED
**First seen**: J15, J17 (2026-03-16) | **Fixed in**: 2026-03-20 re-run
**Description**: LLVM generated unnecessary EH tables for main wrapper lacking nounwind. Now correctly propagated through the entire call graph.

### Dead aggregate loads on borrowed params -- FIXED (partial)
**First seen**: J16, J17 (2026-03-19) | **Fixed in**: J16 fixed in 2026-03-20; J17 lambda still has 1 dead load
**Description**: Full struct loaded but value unused -- pointer forwarded directly to runtime.

### Option struct wrapping + alloca round-trip in iterator loop -- FIXED
**First seen**: Journey 15 (2026-03-16) | **Fixed in**: 2026-03-20 re-run
**Description**: Iterator next() wrapped has-next flag into option struct then immediately unwrapped. +7 unjustified instructions per iteration. Now eliminated -- J15 score 8.7 -> 9.7.

### Missing nounwind on specific functions -- FIXED
**First seen**: Journey 16 (2026-03-19) | **Fixed in**: 2026-03-20 re-run
**Description**: @check_multi lacked nounwind despite all callees being nounwind. Now correctly propagated.

### C15-1: Double-free on [str] elements -- FIXED
**First seen**: Journey 15 (2026-03-16) | **Fixed in**: Pre-J15 reanalysis
**Description**: Iterator consumption + list buffer drop both decremented string element RCs. Aggregate load codegen + RC balance fix resolved it.

### C17: Closure capturing str -- Idx leak -- FIXED
**First seen**: Journey 17 (2026-03-16) | **Fixed in**: Prior to f561649f (verified 2026-03-19)
**Description**: Closure capturing a `str` produced an unresolved type variable Idx(202) at LLVM codegen. Score went from 3.0 to 9.8.

### Aggregate field-by-field materialization -- FIXED
**First seen**: J9, J14, J15, J16 (2026-03-16) | **Fixed by**: 2026-03-19 run
**Description**: sret structs materialized via 9-instruction GEP+load+insertvalue chains. Now uses aggregate loads (9:1 reduction).

### Landing pad over-generation -- FIXED
**First seen**: J10, J14, J15, J16 (2026-03-16) | **Fixed by**: 2026-03-19 run
**Description**: `invoke`+`landingpad`+`resume` used for calls to `nounwind` functions. Eliminated by fixed-point nounwind analysis.

### SSO guard ptrtoint duplication -- FIXED
**First seen**: J14 (2026-03-16) | **Fixed by**: 2026-03-19 run
**Description**: SSO guard performed `ptrtoint` twice on the same pointer. Now single `ptrtoint` reused.

### Empty trampoline blocks -- FIXED
**First seen**: J5, J12 (2026-03-16) | **Fixed by**: CFG simplification pass
**Description**: Unconditional `br` to next sequential block. Eliminated.

## Score Trend by Difficulty Tier

| Difficulty | Journeys | Count | Avg Score | Range |
|------------|----------|-------|-----------|-------|
| Simple (J1--J4) | J1, J2, J3, J4 | 4 | 10.0 | 10.0--10.0 |
| Moderate (J5--J8) | J5, J6, J7, J8 | 4 | 10.0 | 10.0--10.0 |
| Complex (J9--J20) | J9--J20 | 12 | 9.96 | 9.7--10.0 |
| **Overall** | **J1--J20** | **20** | **9.98** | **9.7--10.0** |

**Perfect scores**: 18 of 20 journeys (90%) score 10.0.
**Near-perfect**: J15 at 9.7 (2 LOWs: dead loads in debug mode), J17 at 9.8 (1 LOW: dead param load in lambda).

### Comparison with Previous Runs

| Run | Avg Score | Perfect | Lowest | Key Change |
|-----|-----------|---------|--------|------------|
| 2026-03-16 (initial) | 9.5 | 8/17 | 3.0 (J17) | Baseline |
| 2026-03-19 (polish) | 9.9 | 16/19 | 8.7 (J15) | Aggregate load, nounwind, CFG simplify |
| 2026-03-20 (full re-run) | 10.0 | 19/20 | 9.8 (J17) | Iterator loop fix, range field elimination, nounwind propagation |
| **2026-03-22 (latest)** | **9.98** | **18/20** | **9.7 (J15)** | **J15 re-scored with 2 LOWs; J16 improved to 10.0** |

## Positive Patterns Across Journeys

Recurring strengths observed consistently:

| Pattern | Journeys | Description |
|---------|----------|-------------|
| `memory(none)` on pure functions | J1--J8, J11, J12 | Posthoc purity analysis correctly identifies functions with no memory effects |
| `nounwind` propagation | J1--J20 | Fixed-point analysis propagates nounwind through entire call graph including string/ARC functions |
| SSO guard correctness | J9, J14--J20 | Bit 63 check correctly discriminates SSO-inline vs heap-allocated strings for RC operations |
| Borrow elision | J10, J14, J16--J19 | Read-only parameters receive `readonly dereferenceable` with zero RC overhead |
| Branchless select | J2, J6, J12, J16 | Simple if/then/else with trivial arms compiles to `select` instead of branch |
| Phi-based loop state | J3, J7, J10, J18 | Mutable loop accumulators use SSA phi nodes, not stack allocas |
| Zero-cost abstractions | J5, J8, J12, J13 | Generics, closures, Option, and iterators compile with no overhead vs hand-written code |
| Attribute compliance 100% | J1--J20 | All applicable LLVM attributes (noundef, nonnull, dereferenceable, fastcc, cold) correctly placed |

## Tooling Notes

- `effect_summaries.py` was missing 8 iterator consumer functions (`ori_iter_fold`, `ori_iter_collect`, etc.). Fixed during J13 re-analysis -- these functions consume their iterator param (param[0] = -1).

## Results Files

- [Journey 1: "I am arithmetic"](01-arithmetic-results.md) -- arithmetic, function_calls, let_bindings
- [Journey 2: "I am a branch"](02-branching-results.md) -- branching, comparison
- [Journey 3: "I am recursive"](03-recursion-results.md) -- recursion, comparison, arithmetic
- [Journey 4: "I am a struct"](04-structs-results.md) -- struct_construction, field_access, nested_structs
- [Journey 5: "I am a closure"](05-closures-results.md) -- closures, higher_order, capture
- [Journey 6: "I am a match"](06-pattern-matching-results.md) -- pattern_matching, sum_types, destructuring
- [Journey 7: "I am a loop"](07-loops-results.md) -- loops, ranges, break_continue
- [Journey 8: "I am generic"](08-generics-results.md) -- generics, monomorphization, type_inference
- [Journey 9: "I am a string"](09-strings-results.md) -- strings, string_methods, arc
- [Journey 10: "I am a list"](10-lists-results.md) -- lists, list_methods, loops, arc
- [Journey 11: "I am a derived trait"](11-derived-traits-results.md) -- derived_traits, trait_methods, sum_types
- [Journey 12: "I am an option"](12-options-results.md) -- option_type, error_propagation
- [Journey 13: "I am an iterator"](13-iterators-results.md) -- iterators, iterator_adapters, closures
- [Journey 14: "I am a fat pointer"](14-fat-string-sharing-results.md) -- strings, arc, fat pointers
- [Journey 15: "I am nested fat"](15-fat-nested-collections-results.md) -- lists, strings, arc, loops
- [Journey 16: "I am fat and moving"](16-fat-ownership-transfer-results.md) -- strings, arc, ownership transfer
- [Journey 17: "I am a captured fat pointer"](17-fat-closure-capture-results.md) -- strings, closures, capture, arc
- [Journey 18: "I am a string builder"](18-string-builder-results.md) -- strings, loops, ranges, arc, lists
- [Journey 19: "I am a lifecycle"](19-rc-lifecycle-results.md) -- nested_structs, loops, lists, strings, arc
- [Journey 20: "I am copy-on-write"](20-cow-patterns-results.md) -- cow, strings, lists, slices, arc
