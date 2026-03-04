# Code Journeys -- Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Issues |
|---|------|----------|----------|------|-----|-------|--------|
| J1 | "I am arithmetic" | int literals, let bindings, arithmetic ops, function call | 33 | PASS | PASS | 8.8/10 | M1: redundant branch, M2: missing noreturn |
| J2 | "I am a branch" | if/else, comparison ops, boolean logic, nested conditionals, unary minus | 17 | PASS | PASS | -- | Results file exists, no scored scrutiny |
| J3 | "I am recursive" | recursion, equality, modulo, boolean operators | 61 | PASS | PASS | -- | Results file exists, no scored scrutiny |
| J4 | "I am a struct" | struct types, construction, field access, nested structs | 57 | PASS | PASS | 9.0/10 | M1: dead struct field loads, M2: overflow msg dedup |
| J5 | "I am a closure" | lambdas, higher-order functions, closures capturing variables | 27 | PASS | PASS | 8.0/10 | M1: redundant branches, M2: missing noreturn, M3: closure env RC leak |
| J6 | "I am a match" | sum types, pattern matching, match expressions, variant destructuring | 41 | PASS | PASS | 8.5/10 | M1: redundant branches, NEW: payload extraction via alloca |
| J7 | "I am a loop" | loop/break, for..in, ranges, mutable let, compound assignment | 30 | PASS | PASS | 8.5/10 | M1: duplicate i+1 CSE, M2: redundant break bridge blocks, L1: 6x overflow msg dedup, L2: dead code after panic |
| J8 | "I am generic" | generic functions, generic structs, type inference, monomorphization | 57 | PASS | PASS | 9.5/10 | L1: sequential block merging, L2: overflow msg dedup (both pre-existing) |
| J9 | "I am a string" | boolean ops (`&&`/`||`), string literals, `.length()`, ARC lifecycle, SSO | 13 | PASS | PASS | 9.0/10 | L1: overflow msg dedup, L2: missing nounwind on `ori_str_from_raw` |
| J10 | "I am a list" | list literals, list length, list as parameter, for..in list, ARC | 33 | PASS | PASS | 9.0/10 | L1: dead list field loads, L2: loop-invariant phi, P1: static uniqueness opt, P2: exception-safe ARC |
| J11 | "I am a derived trait" | #derive(Eq), struct equality, sum type equality, == and != | 33 | PASS | PASS | 9.0/10 | L1: missing nounwind on derived $eq, L2: alloca round-trip in enum $eq; C3 FIX confirmed |
| J12 | "I am an option" | Option, Some/None, match on Option, ? propagation | 33 | PASS | PASS | 8.8/10 | No new findings; C4 FIX confirmed |

## Resolved Critical Issues

### C1: AOT closure crash (Journey 5) -- FIXED

Previously, Journey 5 triggered a crash in the AOT backend when compiling closures. This has been resolved. Both interpreter and LLVM native now produce the correct result (27). The closure representation uses a clean two-tier design: non-capturing lambdas are zero-cost (`ptr null` environment), capturing closures use RC-managed heap environments with a dispatcher/destructor pair.

### C3: Payload sum type `$eq` not generated (Journey 11) -- FIXED

Previously, `#[derive(Eq)]` on sum types with payload variants (record fields) did not generate the `$eq` method, causing AOT failures. Now fixed: `_ori_Shape$eq` is correctly emitted with tag-first comparison, switch dispatch to variant-specific blocks, and per-variant short-circuit field comparison. Both eval and AOT return 33.

### C4: Option match tag inversion in decision tree (Journey 12) -- FIXED

Previously, Option match arms were swapped in the decision tree: `Some` mapped to tag 1 and `None` to tag 0, but construction used `Some=0, None=1`. This caused silent miscompilation -- any `match` on `Option<T>` returned the wrong arm's value. Fixed in commit `77fe984c` across 3 locations: `flatten.rs` (decision tree compilation), `emit.rs` (variant field lookup), and the eval decision tree walker. Resolved 114 previously-failing spec tests. Both eval and AOT now return 33.

## Recurring Issues Across Journeys

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Redundant unconditional branches | MEDIUM | J1, J5, J6, J7, J8, J12 | `br label %bbN` emitted at let-binding boundaries, loop break paths, and `?` propagation continuations; LLVM backend eliminates them |
| Missing `noreturn` on `ori_panic_cstr` | MEDIUM | J1, J5 | Only marked `cold`, should also be `noreturn` |
| Missing `nounwind` on `main` wrapper | LOW | J1, J5, J6 | Transitively nounwind from `_ori_main` |
| Closure env RC leak | MEDIUM | J5 | ARC pipeline does not emit `ori_rc_dec` at end of closure live range |
| Dead struct/list field loads | LOW | J4, J10 | Full aggregate loaded before extracting single field (J4: Rect fields, J10: list len/cap/ptr); DCE removes at -O1+ |
| Overflow message dedup | LOW | J4, J6, J7, J9, J10, J12 | Identical overflow message constants not deduplicated (2x in J4, 6x in J7, 7x in J9, 6x in J10, 6x in J12) |
| Duplicate subexpression in loops | LOW | J7 | `i + 1` computed twice per iteration (for `total += i+1` and `i += 1`); CSE opportunity |
| Dead code after noreturn call | LOW | J7 | RC cleanup code emitted after `ori_panic()` which never returns |
| Payload extraction via alloca | MEDIUM | J6 | Record variant destructuring uses alloca+store+GEP+load (5 instr/arm) where extractvalue (2 instr) would suffice; ~2.5x IR overhead |
| Missing nounwind on runtime decls | LOW | J9 | `ori_str_from_raw` declared without `nounwind`, blocks propagation to callers like `check_strings` |
| Missing nounwind on derived methods | LOW | J11 | Derived `$eq` methods emitted by `derive_codegen` lack `nounwind`; not included in nounwind fixed-point analysis |
| Alloca round-trip in enum derived methods | LOW | J11 | Enum `$eq` loads params into SSA, stores to alloca, then GEPs back; SROA eliminates at -O1+ |

## Results Files

- [Journey 1 Results](journey1-results.md)
- [Journey 2 Results](journey2-results.md)
- [Journey 3 Results](journey3-results.md)
- [Journey 4 Results](journey4-results.md)
- [Journey 5 Results](journey5-results.md)
- [Journey 6 Results](journey6-results.md)
- [Journey 7 Results](journey7-results.md)
- [Journey 8 Results](journey8-results.md)
- [Journey 9 Results](journey9-results.md)
- [Journey 10 Results](journey10-results.md)
- [Journey 11 Results](journey11-results.md)
- [Journey 12 Results](journey12-results.md)
