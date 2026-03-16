# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-16 (AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Prev | Delta | Key Findings |
|---|------|----------|----------|------|-----|-------|------|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 9.8/10 | 9.8 | — | memory(none) on @add, uwtable on main wrapper |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 9.2/10 | 9.2 | — | memory(none) on all pure helpers, branchless select |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 9.2/10 | 8.9 | **+0.3** | memory(none) on @gcd, TCO correct |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 9.7/10 | 9.7 | — | memory(read) on @area, OPTIMAL codegen |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 9.2/10 | 8.5 | **+0.7** | Dead EH blocks ELIMINATED, RC dec FIXED, memory(none) on lambdas |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 9.8/10 | 9.7 | **+0.1** | noundef on struct params FIXED, memory(read) on to_code |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.2/10 | 9.2 | — | memory(none) on @sum_loop, phi lowering correct |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 9.9/10 | 9.8 | **+0.1** | memory(none) on identity/first, zero-cost monomorphization |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 8.8/10 | 8.8 | — | memory(none) on @bool_to_int, ARC balanced |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 8.8/10 | 8.7 | **+0.1** | readonly on @count_items, drop_unique RESTORED |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 9.8/10 | 9.7 | **+0.1** | All 7 functions OPTIMAL, three Eq patterns |
| J12 | "I am an option" | option_type, pattern_matching, error_propagation | 33 | PASS | PASS | 9.3/10 | 9.2 | **+0.1** | uwtable on main wrapper, ? operator OPTIMAL |
| J13 | "I am an iterator" | iterators, iterator_adapters, lists, closures | 55 | PASS | PASS | 9.4/10 | 9.4 | — | memory(none) on pure functions, zero RC ops |

**All 13 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results.

## AIMS Section 02 Impact (This Run vs Previous)

| Category | Journeys Affected | Impact |
|----------|------------------|--------|
| **Improved** | J3 (+0.3), J5 (+0.7), J6 (+0.1), J8 (+0.1), J10 (+0.1), J11 (+0.1), J12 (+0.1) | memory attributes, noundef on structs, dead EH elimination, drop_unique restored |
| **Unchanged** | J1, J2, J4, J7, J9, J13 | Score parity (some gained memory attrs but stayed in same tier) |
| **Regressed** | None | Zero regressions this run |

### What Improved Since Last Run

1. **Closure EH blocks eliminated** (J5): The `invoke`/`landingpad` dead EH blocks in `@_ori_main` are gone. Simple `call` instructions are used again. Score jumped 8.5 → 9.2.
2. **Closure RC dec fixed** (J5): The missing `ori_rc_dec` on the live path for closure env is now present. The potential memory leak is resolved.
3. **`drop_unique` restored** (J10): `@check_passing` correctly uses `ori_buffer_drop_unique` again instead of generic `ori_buffer_rc_dec`. The unique-path fast path is back.
4. **`memory(none)` attribute propagation** (J1-J3, J5, J7-J9): AIMS Section 02 posthoc analysis correctly identifies pure functions and applies `memory(none)`. New on: `@add`, `@my_abs`, `@my_max`, `@my_sign`, `@gcd`, `@bool_to_int`, `@identity`, `@first`, `@sum_loop`, `@square`, lambda functions.
5. **`memory(read)` on readonly functions** (J4, J6): `@area` and `@to_code` correctly get `memory(argmem: read, inaccessiblemem: read, errnomem: read)`.
6. **`noundef` on struct params** (J6): `@to_code` and `@extract` now carry `noundef` on struct-typed parameters.
7. **`uwtable` on C main wrapper** (J1, J12): The entry `main()` wrapper now consistently has `uwtable`.

### Previous Run's Action Items — Status

| Action Item | Status | Resolution |
|-------------|--------|------------|
| Investigate J5 closure env RC dec leak | **FIXED** | RC dec now present on live path |
| Restore `drop_unique` optimization | **FIXED** | Back in J10's `@check_passing` |
| Reduce invoke/landingpad overhead | **FIXED** | J5 `@main` uses `call` again |

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Missing `noundef` on C main wrapper return | LOW | J1-J13 | `i32` return missing `noundef` (OS ignores, negligible) |
| Empty trampoline/passthrough blocks | LOW | J2, J3, J5, J7, J9, J10, J12 | Blocks containing only `br label %next` |
| Missing `nounwind` on functions calling panic | MEDIUM | J5, J7, J9, J10 | Conservative — functions with overflow checks lack `nounwind` |
| Missing `noundef` on struct ptr params | LOW | J4, J11 | Large structs passed by pointer lack `noundef` |
| Attribute compliance below 80% | LOW | J5 (78.6%), J13 (57.1%) | Indirect call targets and iterator trampolines lower compliance |
| Missing `memory(none)` on @sum_for | MEDIUM | J7 | Range struct insertvalue/extractvalue confuses purity analysis |

## Score Trend

| Difficulty | Journeys | Avg Score (Current) | Avg Score (Prev) | Range (Current) |
|------------|----------|--------------------|--------------------|-----------------|
| Simple (J1-J4) | 4 | 9.5 | 9.4 | 9.2–9.8 |
| Moderate (J5-J8) | 4 | 9.4 | 9.2 | 9.2–9.9 |
| Complex (J9-J13) | 5 | 9.2 | 9.0 | 8.8–9.8 |
| **Overall** | **13** | **9.4** | **9.2** | **8.8–9.9** |

### Score Distribution

- **9.5+** (near-perfect): J1 (9.8), J4 (9.7), J6 (9.8), J8 (9.9), J11 (9.8) — arithmetic, structs, pattern matching, generics, derived traits
- **9.0–9.4** (strong): J2 (9.2), J3 (9.2), J5 (9.2), J7 (9.2), J12 (9.3), J13 (9.4) — branching, recursion, closures, loops, options, iterators
- **8.5–8.9** (solid): J9 (8.8), J10 (8.8) — strings, lists (heap-heavy programs with more ARC complexity)

### Per-Category Averages

| Category | Avg Score | Perfect (10/10) | Weakest |
|----------|-----------|-----------------|---------|
| Instruction Efficiency | 9.5 | 7/13 | J2,J3,J5,J7,J9,J10,J12 (9) |
| ARC Correctness | 10.0 | 13/13 | — |
| Attributes & Safety | 7.5 | 0/13 | J13 (4) |
| Control Flow | 8.6 | 7/13 | J2,J3,J7,J9,J10 (7) |
| IR Quality | 9.3 | 9/13 | J9,J10 (8) |
| Binary Quality | 10.0 | 13/13 | — |
| Other Findings | 10.0 | 13/13 | — |

### Key Observations

- **ARC is perfect across all 13 journeys** — every program has balanced RC operations. Zero leaks, zero double-frees, zero scalar RC violations. This is the strongest dimension.
- **Attribute compliance is the weakest dimension** — averaging 7.5/10. The main gaps are `nounwind` on functions that call panic paths, and `noundef` on various parameters. AIMS Section 02 improved this significantly but there's room for more.
- **Zero regressions this run** — all three action items from the previous run are resolved. J5 went from the only regression to the biggest improvement (+0.7).
- **Overall average: 9.2 → 9.4** — steady improvement driven by AIMS Section 02 attribute work and the J5 closure fixes.

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
