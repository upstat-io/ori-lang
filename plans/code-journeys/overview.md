# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-15 (AIMS branch `experiment/aims`)
**Previous run**: 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Old | Delta | Key Findings |
|---|------|----------|----------|------|-----|-------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 9.8/10 | 9.8 | — | Byte-identical IR, zero ARC ops |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 9.2/10 | 9.2 | — | Byte-identical IR, branchless select |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 8.9/10 | 8.9 | — | Byte-identical IR, TCO on gcd |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 9.7/10 | 9.7 | — | Byte-identical IR, OPTIMAL codegen |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 8.5/10 | 8.8 | **-0.3** | @apply improved (11→4 instr), @main regressed (new EH blocks, potential missing RC dec) |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 9.7/10 | 9.7 | — | Byte-identical IR, branchless select chains |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 9.2/10 | 9.2 | — | Byte-identical IR, correct phi lowering |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 9.8/10 | 9.8 | — | Byte-identical IR, zero-cost monomorphization |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 8.8/10 | 8.8 | — | New `ori_str_empty` for empty strings, ARC balanced |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 8.7/10 | 8.7 | — | Lost `drop_unique` fast path (REGRESSED), gained precise landingpads |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 9.7/10 | 9.7 | — | Byte-identical IR, excellent derived Eq |
| J12 | "I am an option" | option_type, pattern_matching, error_propagation | 33 | PASS | PASS | 9.2/10 | 9.1 | +0.1 | Byte-identical IR, scoring correction |
| J13 | "I am an iterator" | iterators, iterator_adapters, lists, closures | 55 | PASS | PASS | 9.4/10 | 7.5 | **+1.9** | Dead null-env rc_dec ELIMINATED by AIMS, ARC 3→10 |

**All 13 journeys pass on both eval and AOT backends.** No behavioral mismatches, no crashes, no wrong results.

## AIMS Impact Summary

| Category | Journeys Affected | Impact |
|----------|------------------|--------|
| **Improved** | J13 (+1.9), J12 (+0.1) | AIMS eliminated dead RC ops on null closure envs; scoring correction |
| **Unchanged** | J1-J4, J6-J9, J11 | Byte-identical IR for scalar-only and simple heap programs |
| **Regressed** | J5 (-0.3), J10 (score unchanged) | Closure EH blocks + missing RC dec; lost `drop_unique` fast path |

### What AIMS Got Right

1. **Dead code elimination for null closure environments** (J13): The unified lattice correctly identifies non-capturing closures as having null env pointers and omits the dead `br i1 true`-guarded `ori_rc_dec` blocks. This was the #1 known issue across all journeys.
2. **Scalar program parity** (J1-J4, J6-J8, J11): AIMS produces byte-identical IR for all programs with no heap allocations. The lattice correctly short-circuits to zero analysis.
3. **Improved function-level optimization** (J5 `@apply`, J13 `@main`): AIMS can eliminate entire RC sequences within single functions.

### What AIMS Regressed

1. **Closure environment RC in @main** (J5): `@_ori_main` grew from 19 to 24 instructions. New `invoke`/`landingpad` EH blocks appear with dead cleanup code including `load ptr, ptr null`. The live execution path appears to lack `ori_rc_dec` for the closure env allocated by `make_adder`. **Needs investigation — potential memory leak.**
2. **Lost `drop_unique` fast path** (J10): `@check_passing` switched from `call` + `ori_buffer_drop_unique` (20 instr) to `invoke` + landingpad + `ori_buffer_rc_dec` (28 instr). The unique-path optimization that skips the runtime refcount check is no longer emitted. Correct but slower.

## Recurring Issues

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Empty trampoline/passthrough blocks | LOW | J2, J3, J5, J7, J9, J10, J12 | Blocks containing only `br label %next` |
| Missing `noundef` on struct params | LOW | J4, J6, J8, J11 | Struct-typed and Box-typed params missing `noundef` annotation |
| Missing `uwtable` on C main wrapper | LOW | J1, J2, J7, J8 | The `@main` entry wrapper lacks `uwtable` |
| Missing `memory(...)` annotations | LOW | J5, J7, J12 | Pure/read-only functions lack memory annotations |
| Attribute compliance below 80% | LOW | J3 (77.8%), J5 (~60%), J13 (~53%) | Closures' indirect targets and iterator trampolines lower compliance |
| Redundant entry block branch | LOW | J3, J7, J12 | TCO/loop lowering emits entry block with only `br label %header` |
| New invoke/landingpad EH overhead | MEDIUM | J5, J10 | **NEW in AIMS** — functions gain invoke+landingpad where call sufficed |
| Missing `drop_unique` optimization | MEDIUM | J10 | **NEW in AIMS** — unique-path drop fast path no longer emitted |

## Resolved Issues

### Dead null-env rc_dec with `br i1 true` — FIXED
**First seen**: J13 (previous run, 2026-03-10)
**Fixed in**: AIMS branch (2026-03-15)
**Description**: Non-capturing closures got null env pointers; old ARC pipeline emitted dead `ori_rc_dec` guarded by constant-true branches. AIMS lattice analysis recognizes FatVal with null env and omits cleanup entirely. ARC correctness: 3/10 → 10/10.

### Empty string `ori_str_from_raw(ptr, 0)` — IMPROVED
**First seen**: J9 (previous run, 2026-03-08)
**Improved in**: AIMS branch (2026-03-15)
**Description**: Empty strings now use `ori_str_empty` instead of `ori_str_from_raw(ptr, 0)`, eliminating unnecessary global constant.

## Score Trend

| Difficulty | Journeys | Avg Score (AIMS) | Avg Score (Old) | Range (AIMS) |
|------------|----------|-----------------|-----------------|--------------|
| Simple (J1-J4) | 4 | 9.4 | 9.4 | 8.9–9.8 |
| Moderate (J5-J8) | 4 | 9.2 | 9.4 | 8.5–9.8 |
| Complex (J9-J13) | 5 | 9.2 | 8.8 | 8.7–9.7 |
| **Overall** | **13** | **9.2** | **9.1** | **8.5–9.8** |

### Score Distribution

- **9.5+** (near-perfect): J1 (9.8), J4 (9.7), J6 (9.7), J8 (9.8), J11 (9.7) — arithmetic, structs, pattern matching, generics, derived traits
- **9.0–9.4** (strong): J2 (9.2), J7 (9.2), J12 (9.2), J13 (9.4) — branching, loops, options, **iterators (up from 7.5)**
- **8.5–8.9** (solid): J3 (8.9), J5 (8.5), J9 (8.8), J10 (8.7) — recursion, **closures (down from 8.8)**, strings, lists

### Observations

- **AIMS biggest win is J13** — iterator journey jumped from 7.5 to 9.4, the largest single improvement. Dead null-env RC ops were the #1 cross-journey issue.
- **AIMS regression in closures** — J5 dropped 0.3 points. The `@apply` improvement (11→4 instr) is real, but `@main` grew with new EH blocks and a potential missing RC dec.
- **Scalar programs unchanged** — 8 of 13 journeys produce byte-identical LLVM IR. AIMS correctly identifies scalar-only programs and applies zero overhead.
- **`drop_unique` lost in J10** — the old ARC system's unique-path fast path is not reproduced by AIMS. This is a targeted optimization opportunity.
- **Overall average improved** — 9.1 → 9.2 despite the J5 regression, driven by J13's massive improvement.

## Action Items for Merge Readiness

1. **Investigate J5 closure env RC dec** — is the missing `ori_rc_dec` in `@main`'s live path a leak, or is it handled by caller convention? If it's a leak, this blocks merge.
2. **Restore `drop_unique` optimization** — AIMS should emit `ori_buffer_drop_unique` when uniqueness analysis proves the value is unique. Currently using generic `ori_buffer_rc_dec`.
3. **Reduce invoke/landingpad overhead** — functions that provably don't throw should use `call` not `invoke`. The new EH blocks in J5/J10 are dead code.

## Tooling Notes

### `effect_summaries.py` — `ori_str_empty` added
**Fixed in**: J9 re-run (2026-03-15)
**Description**: The AIMS branch introduced `ori_str_empty` for empty string allocation. Added to effect summaries so the ARC metrics tool correctly accounts for +1 allocation effect.

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
