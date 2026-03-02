# Code Journey Overview

Last updated: Journey 19 (2026-03-02)

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
| 13 | COW Lists | COW list push, list creation, `.length()`, `for..in` list iteration | 20 ✓ | 20 ✓ | Missing nounwind on alloc/iter fns (H3); unnecessary RC pair around `.length()`; 3.6x instruction overhead |
| 14 | COW Strings | string concat, substring (SSO), starts_with, `.length()` | 20 ✓ | 20 ✓ | Excessive RC dec on exception paths (M17); redundant SSO check sequences (M18) |
| 15 | COW Maps | map literal, COW insert, `.length()`, `for..in` map iteration, entry.1 | 63 ✓ | 63 ✓ | Redundant RC inc/dec pair on map data (M19); SSO-aware elem inc/dec correct |
| 16 | COW Sharing | shared list ref (RC>1), COW push clone, value semantics, dual iteration | 23 ✓ | 23 ✓ | COW sharing correct; `ori_rc_is_unique` in runtime (not inlined); no new findings |
| 17 | COW Slice + Combined | list take (seamless slice), string substring, chained push, mixed types | 10 ✓ | 10 ✓ | Seamless zero-copy slice correct; first push_cow uses dynamic check for fresh list (M20, M21) |
| 18 | String Sharing + SSO | SSO inline strings, heap strings, string sharing, concat value semantics | 48 ✓ | 48 ✓ | SSO guards correct but verbose (5.0x overhead); H3 expanded to string fns; M16/M18 confirmed |
| 19 | COW Comprehensive | all 3 COW types (list+str+map), sharing, mutation, cross-type ARC | 28 ✓ | 28 ✓ | No new findings; cross-collection ARC isolation correct; 8 orphaned landing pads (M11 high) |

## Deduplicated Findings by Severity

### CRITICAL (4)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| C1 | AOT crashes when non-capturing lambda + capturing closure coexist in same module | J5 | NEW |
| C2 | List indexing (`xs[0]`) crashes AOT — `__index` function unresolved in LLVM codegen | J10 | NEW |
| C3 | Derived `Eq` for payload sum types — `$eq` function not generated, silent wrong results | J11 | NEW |
| C4 | Built-in `Option<T>` match — switch tag numbering inverted, silent wrong results | J12 | NEW |

### HIGH (3)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| H1 | invoke + empty landing pads for ALL calls when any function is recursive | J3 | CONFIRMED |
| H2 | Potentially unsound `nounwind` on functions calling non-nounwind runtime (iterator fns) | J10 | CONFIRMED (J13) |
| H3 | Missing nounwind/noalias on allocation, iterator, and string runtime function declarations | J13 | CONFIRMED (J17, J18) |

### MEDIUM (21)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| M1 | Prelude overhead — 10,331 bytes constant for every program | J1 | CONFIRMED (19/19) |
| M2 | No `nsw` flags on integer arithmetic in LLVM IR | J1 | CONFIRMED |
| M3 | Unnecessary `br label` after function calls in LLVM IR | J1 | CONFIRMED (18/18) |
| M4 | Tail-recursive functions not compiled as tail calls | J3 | CONFIRMED |
| M5 | `align 4` on i64 struct/variant field loads — should be `align 8` | J4 | CONFIRMED (J4, J6, J13, J14, J15, J16, J17, J18) |
| M6 | Full struct loading for partial field access | J4 | CONFIRMED |
| M7 | Verbose variant construction — alloca+store+load roundtrip | J6 | CONFIRMED (J13, J18) |
| M8 | Identical match arms not deduplicated | J6 | NEW |
| M9 | Inclusive range `..=` computes `end + step` — overflow for INT_MAX | J7 | NEW |
| M10 | `_ori_main` inconsistently missing `nounwind` attribute | J8 | CONFIRMED (J9, J14, J15, J17) |
| M11 | Orphaned landing pads with no predecessors in ARC cleanup code | J9 | CONFIRMED (J10, J13, J14, J15, J16, J17, J18) |
| M12 | Duplicate identical drop functions — multiple copies of same layout's drop | J10 | NEW |
| M13 | Unnecessary Option-like `{ tag, value }` construction in iterator loop | J10 | CONFIRMED (J13, J15, J17) |
| M14 | None variant codegen loads uninitialized payload from alloca — LLVM UB (poison) | J12 | NEW |
| M15 | Unused collection struct carried through loop phi — 24-byte struct in hot loop registers | J13 | CONFIRMED (J15) |
| M16 | Unnecessary RC inc/dec pair around scalar field extraction (`.length()`) | J13 | CONFIRMED (J18) |
| M17 | Excessive RC dec on exception paths — 16 dec vs 3 inc in string-heavy function | J14 | NEW |
| M18 | Redundant SSO check sequences — guard checks for statically-known SSO strings | J14 | CONFIRMED (J18) |
| M19 | Redundant RC inc/dec pair on map data pointer | J15 | NEW |
| M20 | Redundant RC inc on slice data after original RC dec — canceling pair | J17 | NEW |
| M21 | First push_cow uses cow_mode=0 (dynamic) for freshly-allocated list — should be cow_mode=1 | J17 | NEW |

### LOW (10)

| ID | Description | First Seen | Status |
|----|-------------|------------|--------|
| L1 | Canonicalizer node expansion varies (0-36%) | J1 | CONFIRMED |
| L2 | 4 prelude decision trees | J1 | CONFIRMED |
| L3 | Trivial if/else → branch+phi instead of select | J2 | CONFIRMED |
| L4 | Single-predecessor phi nodes in match codegen | J6 | CONFIRMED (J7, J13, J16) |
| L5 | Range struct materialized then immediately destructured | J7 | NEW |
| L6 | Duplicate computation in loops (CSE handles it) | J7 | NEW |
| L7 | Dead phi values at loop exit (unused variables) | J7 | CONFIRMED (J15) |
| L8 | Duplicate string constants — identical literals not deduplicated in LLVM IR | J14 | NEW |
| L9 | Non-reusable temporary allocas for sequential `ori_str_len` calls | J14 | CONFIRMED (J18) |
| L10 | Repeated `movabs` SSO constant load in x86 — not hoisted by unoptimized LLVM | J18 | NEW |

## Findings by Compiler Phase

| Phase | Findings | Count |
|-------|----------|-------|
| Lexer | (none) | 0 |
| Parser | (none) | 0 |
| Type Checker | (none) | 0 |
| Canonicalizer | L1, L2 | 2 |
| Eval | (none — 19/19 correct) | 0 |
| ARC Pipeline | M16, M17, M20, M21 | 4 |
| LLVM Codegen | C1-C4, H1-H3, M2-M15, M18-M19, L3-L9 | 33 |
| Overall | M1 | 1 |

## What Works Well

- **Eval path: 19/19 correct** — interpreter is rock solid
- **AOT: 16/19 correct** — closures crash (C1), payload Eq wrong (C3), Option match inverted (C4)
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
- **AOT cache**: Reliable across all 19 journeys
- **Seamless zero-copy list slicing**: `ori_list_slice_take` produces view with SLICE_FLAG cap, no element copying
- **Mixed collection type discrimination**: Lists, strings, and iterators in same function with correct per-type RC protocols
- **SSO-aware substring**: Short substrings produce SSO results (zero heap allocation, zero RC)
- **Chained push uniqueness analysis**: Second push correctly gets cow_mode=1 (static unique fast path)
- **SSO-aware RC guards**: Correct MSB-based SSO detection skips RC operations for inline strings
- **String concat via runtime**: `ori_str_concat` with sret return, SSO-safe
- **Zero-copy `starts_with`**: Delegates to Rust `core::str::starts_with` via `deref_str`
- **Substring SSO handling**: Runtime correctly copies for SSO, zero-copy slice for heap
- **SSO-safe `.length()`**: Uses `ori_str_len` runtime call (not raw field extraction) respecting SSO invariant
- **OriStr fat pointer layout**: Consistent `{ i64, i64, ptr }` with dual heap/SSO interpretation
- **COW list push codegen**: Correct `ori_list_push_cow` calls with null inc_fn for scalar elements
- **COW uniqueness delegation**: Runtime handles uniqueness check -- correct separation of concerns
- **Iterator cleanup**: `ori_iter_drop` correctly called at loop exit
- **RC balance for COW lists**: Balanced inc/dec pairs, no leaks
- **Map literal construction**: `ori_map_literal_alloc` + `ori_map_literal_put` pattern — clean batch insert (J15)
- **COW map insert**: `ori_map_insert_cow` with 13 args including cow_mode, key_eq, key_hash, elem inc/dec — correct protocol (J15)
- **Map iteration codegen**: `ori_iter_from_map` with owns_data=true, correct extractvalue for entry tuples (J15)
- **SSO-aware elem inc/dec thunks**: Map key RC callbacks correctly check MSB SSO flag before RC operations (J15)
- **Map struct layout**: Consistent `{ i64, i64, ptr }` (len, cap, data) — same as list/set (J15)
- **Cross-collection ARC isolation**: List, string, and map RC operations are completely independent — no shared state, no cross-type callbacks (J19)
- **Type-specific drop dispatch**: `ori_buffer_rc_dec` (lists), `ori_rc_dec` with SSO guard (strings), `ori_map_buffer_rc_dec` with elem callbacks (maps) — correct per-type cleanup (J19)
- **Universal `_ori_drop$3`**: Single drop function parameterized by size/align shared across all heap types — efficient code reuse (J19)
- **COW comprehensive stress test**: All 3 COW types in one function, 0 new findings — architecture is sound (J19)

## Coverage Map

### Working (Both Paths): 43 features
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
- [x] COW list push (`.push()` with uniqueness check) — J13
- [x] COW list push chaining (sequential pushes) — J13
- [x] List `.length()` after COW mutation — J13
- [x] String concatenation (`+` operator) — J14
- [x] String `.substring(start:, end:)` — J14
- [x] String `.starts_with(prefix:)` — J14
- [x] String `.length()` (SSO-safe via runtime call) — J14
- [x] Map literal construction, COW insert, map iteration — J15
- [x] COW list sharing (RC>1 → clone on push, value semantics) — J16
- [x] Seamless list slice via `.take()` (zero-copy, SLICE_FLAG cap) — J17
- [x] String `.substring()` with SSO-aware result — J17
- [x] Mixed collection types in same function (list + string + iterator) — J17
- [x] COW sharing semantics (shared list ref, clone on push when RC>1) — J16
- [x] Dual list iteration (independent iterators on original + modified) — J16
- [x] Cross-collection ARC isolation (list + string + map in same function, no interference) — J19
- [x] COW map insert with SSO-aware element inc/dec thunks for string keys — J19

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
- [ ] Set operations (Set<T>, union, intersection)
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
5. **H2/H3**: Add `nounwind` to runtime alloc/iterator functions — **eliminates EH overhead for COW programs**
6. **M14**: Fix None variant uninitialized payload load — **LLVM UB (poison)**
7. **M9**: Range overflow for `..=INT_MAX` — **correctness**
8. **H1**: `nounwind` propagation — **performance**
9. **M5**: Fix `align 4` → `align 8` — **trivial fix, measurable impact**
10. **M16**: Eliminate unnecessary RC inc/dec around `.length()` — **performance, easy ARC pipeline fix**
11. **M7**: `insertvalue` for variant construction — **code quality**
12. **M2**: `nsw` flags / checked arithmetic — **semantic correctness**
13. **M13/M15**: Eliminate iterator loop struct overhead — **hot loop performance**
14. **M6/M8**: Lazy struct load / dedup match arms — **optimization**

## Trend Analysis

- **Eval**: 100% correct, 19/19 — no regressions
- **AOT**: 84% correct (16/19) — 3 journey failures: closures crash (C1), derived Eq wrong (C3), Option match inverted (C4); list indexing crash is separate (C2)
- **LLVM codegen**: 33 of 37 findings — overwhelmingly the weakest phase
- **Prelude**: Stable 10,331 bytes (19 journeys)
- **Dead branches (M3)**: Universal — present in every journey (CONFIRMED 18/18 non-crash)
- **Alignment (M5)**: Expanding — struct fields, variant fields, list elements, derived eq stores, Option variant stores, COW push elems/outputs, iterator scratch, string concat sret, map entries
- **Canon expansion**: 0-25% (struct=0, match=10.7%, loop=11.1%, bool+string=25%, list=11.4%, derive=17.6%, option=11.4%, cow_list=20%, cow_string=16%)
- **Eval efficiency**: J14=40 events (string ops), J16=64 calls (COW sharing), J17=compact (slice+combined), J18=48 (string sharing)
- **ARC features**: J9 (strings) + J10 (lists) + J13 (COW lists) + J14 (COW strings) + J15 (COW maps) + J16 (COW sharing) + J17 (COW slice) + J18 (string sharing) — all correct; J11-J12 need 0 ARC (pure value types)
- **COW correctness**: J13-J18 validate COW semantics across all collection types — lists, strings, maps, sharing, slices, string sharing. COW uniqueness check delegated to runtime, not inlined.
- **SSO guard overhead**: J14 revealed 133 instructions of SSO check sequences across 19 RC sites. All 3 strings fit SSO (<23 bytes), so all RC guards resolve to "skip" at runtime. J18 confirmed with 5.0x instruction overhead (new highest).
- **J9 vs J14 `.length()` divergence**: J9 used zero-cost `extractvalue` (pre-SSO-aware), J14 correctly uses `ori_str_len` runtime call (SSO-safe). The J14 approach is correct but J9 is stale.
- **Silent wrong answers**: J11 and J12 both produce wrong results without crashing — **two consecutive silent miscompilation journeys**. C3 (payload sum Eq) and C4 (Option match tags) are independent bugs with the same symptom class.
- **Built-in vs user-defined asymmetry**: User-defined sum types work correctly in match (J6=41), but built-in generic Option has inverted tags. The monomorphization path for built-in types diverges from user-defined types.
- **`?` vs `match` inconsistency**: In the SAME module, `?` uses correct tag interpretation (`icmp eq tag, 0`) while `match` switch has inverted labels. Two different codegen paths for the same type.
- **Instruction overhead trend**: J18 has 5.0x overhead (new highest) — SSO guard verbosity compounds with COW runtime calls + alloca roundtrips + unnecessary RC pairs. J13 previously held record at 3.6x.
- **COW series complete (J13-J19)**: 7 COW-specific journeys, ALL PASS on both paths. Cross-collection isolation validated in J19 (0 new findings). COW architecture is sound.
- **Orphaned landing pad growth**: J19 has 8 orphaned landing pads (new high) — scales with number of collection types per function
- **Redundant RC pattern**: M16 (length), M19 (map insert), M20 (slice) — all are the same class: ARC pipeline emits separate inc/dec for binding transitions without recognizing canceling pairs
- **cow_mode analysis gap**: M21 shows the ARC analysis recognizes `push_cow` output as unique but not `ori_list_alloc_data` output
- **New per-journey**: J1(5), J2(1), J3(3), J4(3), J5(1), J6(4), J7(4), J8(1), J9(1), J10(4), J11(1), J12(2), J13(3), J14(2), J15(1), J16(0), J17(2), J18(0), J19(0) — discovery rate plateauing for COW-related code
