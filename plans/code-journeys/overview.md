# Code Journey Overview

Last updated: Journey 12 (2026-02-27)

## Journey Results Table

| # | Theme | Features Tested | Eval | AOT | Key Finding |
|---|-------|----------------|------|-----|-------------|
| 1 | Arithmetic | int literals, let bindings, arithmetic (+,-,*), function call | 33 ✓ | 33 ✓ | Prelude 31.7x overhead; no `nsw` |
| 2 | Branching | if/else, comparison, unary negation, nested if/else | 17 ✓ | 17 ✓ | Dead branches systematic; phi correct |
| 3 | Recursion | recursion, equality, modulo, tail calls | 61 ✓ | 61 ✓ | invoke+landingpad overhead; no tail call opt |
| 4 | Structs | struct types, nested structs, field access, pass-by-ref | 57 ✓ | 57 ✓ | Full struct load for partial access; constant folding |
| 5 | Closures | lambdas, higher-order functions, capturing closures | 27 ✓ | **CRASH** | **CRITICAL: mixed closure types crash AOT** |
| 6 | Sum Types | sum types, pattern matching, match expressions | 41 ✓ | 41 ✓ | select for unit variants; verbose construction |
| 7 | Loops | loop/break, for..in, ranges, mutable let, compound assign | 30 ✓ | 30 ✓ | Phi-based SSA loops correct; range overflow risk |
| 8 | Generics | generic functions, generic structs, type inference, monomorphization | 57 ✓ | 57 ✓ | Full monomorphization; zero-overhead generics |
| 9 | Strings | boolean `&&`/`||`, string literals, `.length()`, ARC lifecycle | 13 ✓ | 13 ✓ | Boolean constant folding; zero-cost `.length()`; orphaned landing pads (M11) |
| 10 | Lists | list literals, `.length()`, list params, `for..in` list, ARC for lists | 33 ✓ | 33 ✓ | **List indexing crashes AOT (C2)**; duplicate drop functions; nounwind unsoundness (H2) |
| 11 | Derived Traits | `#[derive(Eq)]`, struct eq, unit sum eq, payload sum eq | 33 ✓ | **18 (WRONG)** | **CRITICAL: payload sum type `$eq` not generated (C3)** — silent wrong answer |
| 12 | Option/Match | `Option<int>`, Some/None, match on Option, `?` propagation | 33 ✓ | **144 (WRONG)** | **CRITICAL: built-in Option match tag inversion (C4)** — silent wrong answer |

## Deduplicated Findings by Severity

### CRITICAL (4)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| C1 | AOT crashes when non-capturing lambda + capturing closure coexist in same module | J5 | NEW |
| C2 | List indexing (`xs[0]`) crashes AOT — `__index` function unresolved in LLVM codegen | J10 | NEW |
| C3 | Derived `Eq` for payload sum types — `$eq` function not generated, silent wrong results | J11 | NEW |
| C4 | Built-in `Option<T>` match — switch tag numbering inverted, silent wrong results | J12 | NEW |

### HIGH (2)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| H1 | invoke + empty landing pads for ALL calls when any function is recursive | J3 | CONFIRMED |
| H2 | Potentially unsound `nounwind` on functions calling non-nounwind runtime (iterator fns) | J10 | NEW |

### MEDIUM (14)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| M1 | Prelude overhead — 10,331 bytes constant for every program | J1 | CONFIRMED (7/7) |
| M2 | No `nsw` flags on integer arithmetic in LLVM IR | J1 | CONFIRMED |
| M3 | Unnecessary `br label` after function calls in LLVM IR | J1 | CONFIRMED (7/7) |
| M4 | Tail-recursive functions not compiled as tail calls | J3 | CONFIRMED |
| M5 | `align 4` on i64 struct/variant field loads — should be `align 8` | J4 | CONFIRMED (J4, J6) |
| M6 | Full struct loading for partial field access | J4 | CONFIRMED |
| M7 | Verbose variant construction — alloca+store+load roundtrip | J6 | NEW |
| M8 | Identical match arms not deduplicated | J6 | NEW |
| M9 | Inclusive range `..=` computes `end + step` — overflow for INT_MAX | J7 | NEW |
| M10 | `_ori_main` inconsistently missing `nounwind` attribute | J8 | CONFIRMED (J9) |
| M11 | Orphaned landing pads with no predecessors in ARC cleanup code | J9 | CONFIRMED (J10) |
| M12 | Duplicate identical drop functions — multiple copies of same layout's drop | J10 | NEW |
| M13 | Unnecessary Option-like `{ tag, value }` construction in iterator loop | J10 | NEW |
| M14 | None variant codegen loads uninitialized payload from alloca — LLVM UB (poison) | J12 | NEW |

### LOW (7)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| L1 | Canonicalizer node expansion varies (0-25%) | J1 | CONFIRMED |
| L2 | 4 prelude decision trees | J1 | CONFIRMED |
| L3 | Trivial if/else → branch+phi instead of select | J2 | CONFIRMED |
| L4 | Single-predecessor phi nodes in match codegen | J6 | CONFIRMED (J7) |
| L5 | Range struct materialized then immediately destructured | J7 | NEW |
| L6 | Duplicate computation in loops (CSE handles it) | J7 | NEW |
| L7 | Dead phi values at loop exit (unused variables) | J7 | NEW |

## Findings by Compiler Phase

| Phase | Findings | Count |
|-------|----------|-------|
| Lexer | (none) | 0 |
| Parser | (none) | 0 |
| Type Checker | (none) | 0 |
| Canonicalizer | L1, L2 | 2 |
| Eval | (none — 12/12 correct) | 0 |
| LLVM Codegen | C1-C4, H1-H2, M2-M14, L3-L7 | 25 |
| Overall | M1 | 1 |

## What Works Well

- **Eval path: 12/12 correct** — interpreter is rock solid
- **AOT: 9/12 correct** — closures crash (C1), payload Eq wrong (C3), Option match inverted (C4)
- **Monomorphization**: Zero-overhead generics — specialized functions are minimal
- **Generic struct specialization**: `Box<T>` → `Box<int>` → `{ i64 }` correctly
- **Type inference**: All generic type arguments inferred from call sites
- **SSA loop compilation**: Correct phi nodes for mutable variables
- **Range iteration**: Inclusive→exclusive conversion works
- **Pattern matching**: `select` for unit, `switch` for payload variants
- **Struct constant folding**: Field values propagated at compile time
- **Calling convention**: Consistent `fastcc`/`nounwind` analysis
- **Boolean constant folding**: `true && true` → `i1 true` at compile time
- **Zero-cost `.length()`**: Compiles to `extractvalue` field extraction — no function call
- **String ARC lifecycle**: Correct create/use/free pattern for all strings
- **List fat pointer `{ i64, i64, ptr }`**: Clean representation with O(1) length
- **for..in list compilation**: Runtime iterator with correct SSA phi loop
- **List ARC lifecycle**: Correct RC inc/dec for multi-use lists across calls
- **Pass-by-reference for lists**: Correctly uses alloca+store for >16-byte structs
- **Derived Eq for structs**: Textbook field-by-field comparison with early exit chain
- **Derived Eq for unit sum types**: Minimal tag-only comparison (4 instructions)
- **`!=` as `xor i1 %eq, true`**: Simple, correct negation
- **Pure value programs**: 0 runtime declarations when no ARC types used
- **`?` propagation codegen**: Correct `icmp eq tag, 0` pattern — avoids match switch bug
- **Option<int> as { i64, i64 }**: 16-byte representation, passed by value in fastcc
- **AOT cache**: Reliable across all 12 journeys

## Coverage Map

### Working (Both Paths): 27 features
- [x] Integer literals, let bindings, blocks
- [x] Arithmetic: `+`, `-`, `*`, `%`
- [x] Functions (1-3 args), recursion
- [x] `if`/`then`/`else` (simple, nested)
- [x] Comparison: `<`, `>`, `<=`, `>=`, `==`
- [x] Structs (simple, nested), field access
- [x] Sum types (unit + payload), match
- [x] Non-capturing lambdas (alone)
- [x] Capturing closures (alone)
- [x] `loop` + `break`
- [x] `for..in` with ranges (`..`, `..=`)
- [x] Mutable `let` + compound assignment (`+=`)
- [x] Unary negation
- [x] Expression-based return
- [x] Generic functions (monomorphization)
- [x] Generic structs (`Box<T>` → `Box<int>`)
- [x] Type inference for generics
- [x] Boolean operators (`&&`, `||`) — constant-folded
- [x] String literals, `.length()`, ARC lifecycle
- [x] List literals, `.length()`, ARC lifecycle
- [x] List as function parameter (pass-by-reference)
- [x] `for..in` with lists (runtime iterator)
- [x] List construction with constants
- [x] `#[derive(Eq)]` on structs — field-by-field comparison
- [x] `#[derive(Eq)]` on unit-only sum types — tag comparison
- [x] `!=` operator (negated `==`)

### Broken (AOT only): 4 features
- [!] Mixed closures (non-capturing + capturing same module) → crash (C1)
- [!] List indexing (`xs[0]`) → crash (C2)
- [!] `#[derive(Eq)]` on payload sum types → silent wrong results (C3)
- [!] `match` on built-in `Option<T>` → tag inversion, silent wrong results (C4)

### Partially Working (AOT): 2 features
- [~] `Option<T>` construction (Some/None) — construction correct, match broken (C4)
- [~] `?` propagation — operator itself correct (`icmp eq`), but match-based unwrapping broken (C4)

### Not Yet Tested: 6 features
- [ ] Iterators (`.map()`, `.filter()`, `.collect()`)
- [ ] Collections (maps, sets)
- [ ] Derived traits (`Printable`, `Clone`, `Hashable`)
- [ ] `Result<T, E>`, `?` on Result
- [ ] Modules / `use` imports
- [ ] ARC lifecycle (shared references)
- [ ] Integer overflow behavior
- [ ] `for..yield` (list comprehension)

## Recommended Fix Priority

1. **C4**: Fix Option match tag inversion — **blocks ALL Option-based programs in AOT**
2. **C3**: Fix payload sum type `$eq` codegen — **silent wrong answers are worst severity**
3. **C1**: Fix closure argument mismatch — **blocks real programs**
4. **C2**: Fix list indexing (`__index` mono registration) — **blocks list element access**
5. **H2**: Audit `nounwind` analysis for runtime function calls — **unsoundness risk**
6. **M14**: Fix None variant uninitialized payload load — **LLVM UB (poison)**
7. **M9**: Range overflow for `..=INT_MAX` — **correctness**
8. **H1**: `nounwind` propagation — **performance**
9. **M5**: Fix `align 4` → `align 8` — **trivial fix, measurable impact**
10. **M7**: `insertvalue` for variant construction — **code quality**
11. **M2**: `nsw` flags / checked arithmetic — **semantic correctness**
12. **M6/M8**: Lazy struct load / dedup match arms — **optimization**

## Trend Analysis

- **Eval**: 100% correct, 12/12 — no regressions
- **AOT**: 75% correct (9/12) — 3 journey failures: closures crash (C1), derived Eq wrong (C3), Option match wrong (C4); list indexing crash is separate (C2)
- **LLVM codegen**: 25 of 28 findings — overwhelmingly the weakest phase
- **Prelude**: Stable 10,331 bytes (12 journeys)
- **Dead branches (M3)**: Universal — present in every journey (CONFIRMED 12/12)
- **Alignment (M5)**: Expanding — struct fields, variant fields, list elements, derived eq stores, Option variant stores
- **Canon expansion**: 0-25% (struct=0, match=10.7%, loop=11.1%, bool+string=25%, list=11.4%, derive=17.6%, option=11.4%)
- **Eval efficiency**: J12=223 calls — Option construction/matching through function calls
- **ARC features**: J9 (strings) + J10 (lists) — both correct; J11-J12 need 0 ARC (pure value types)
- **Silent wrong answers**: J11 and J12 both produce wrong results without crashing — **two consecutive silent miscompilation journeys**. C3 (payload sum Eq) and C4 (Option match tags) are independent bugs with the same symptom class.
- **Built-in vs user-defined asymmetry**: User-defined sum types work correctly in match (J6=41 ✓), but built-in generic Option has inverted tags. The monomorphization path for built-in types diverges from user-defined types.
- **`?` vs `match` inconsistency**: In the SAME module, `?` uses correct tag interpretation (`icmp eq tag, 0`) while `match` switch has inverted labels. Two different codegen paths for the same type.
- **New per-journey**: J1(5), J2(1), J3(3), J4(3), J5(1), J6(4), J7(4), J8(1), J9(1), J10(4), J11(1), J12(2) — steady discovery rate
