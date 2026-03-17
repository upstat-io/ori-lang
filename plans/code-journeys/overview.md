# Code Journeys — Overview

Code journeys trace a single Ori program through the entire compiler pipeline (lexer, parser, typeck, canonicalization, interpreter, LLVM codegen, AOT binary) and perform deep scrutiny on the generated output. Each journey tests a specific language feature set.

**Run date**: 2026-03-16 (AIMS branch `experiment/aims`)
**Previous runs**: 2026-03-16 (re-run J1-J13), 2026-03-15 (AIMS initial), 2026-03-07 to 2026-03-10 (old ARC system, `master`)

## Journey Index

| # | Name | Features | Expected | Eval | AOT | Score | Key Findings |
|---|------|----------|----------|------|-----|-------|--------------|
| J1 | "I am arithmetic" | arithmetic, function_calls, let_bindings | 33 | PASS | PASS | 10.0/10 | PERFECT — zero waste, 100% attributes, memory(none) |
| J2 | "I am a branch" | branching, comparison | 17 | PASS | PASS | 10.0/10 | PERFECT — branchless select, memory(none) on all |
| J3 | "I am recursive" | recursion, comparison, arithmetic | 61 | PASS | PASS | 10.0/10 | PERFECT — TCO on gcd, loop entry block structural |
| J4 | "I am a struct" | struct_construction, field_access, nested_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — nonnull dereferenceable(32), OPTIMAL GEP |
| J5 | "I am a closure" | closures, higher_order, capture | 27 | PASS | PASS | 10.0/10 | PERFECT — AIMS RC elision, full noundef on closures |
| J6 | "I am a match" | pattern_matching, sum_types, destructuring | 41 | PASS | PASS | 10.0/10 | PERFECT — branchless select for tag-only enums |
| J7 | "I am a loop" | loops, ranges, break_continue | 30 | PASS | PASS | 10.0/10 | PERFECT — phi-based loops, structural entry block |
| J8 | "I am generic" | generics, monomorphization, generic_structs | 57 | PASS | PASS | 10.0/10 | PERFECT — zero-cost abstraction, identity=1 instr |
| J9 | "I am a string" | strings, string_methods, arc | 13 | PASS | PASS | 10.0/10 | PERFECT — SSO guard correct, drop attrs fixed |
| J10 | "I am a list" | lists, list_methods, loops, arc | 33 | PASS | PASS | 10.0/10 | PERFECT — borrow elision, drop_unique optimization |
| J11 | "I am a derived trait" | derived_traits, trait_methods, sum_types | 33 | PASS | PASS | 10.0/10 | PERFECT — three Eq patterns OPTIMAL, memory(none) |
| J12 | "I am an option" | option_type, error_propagation | 33 | PASS | PASS | 10.0/10 | PERFECT — ? operator 8 instr, CFG simplified |
| J13 | "I am an iterator" | iterators, iterator_adapters, closures | 55 | PASS | PASS | 10.0/10 | PERFECT — zero user RC, full trampoline attrs |
| **J14** | **"I am a fat pointer"** | strings, arc, fat_pointer | 65 | PASS | PASS | **9.4/10** | SSO guard works, 2 CF defects, duplicate ptrtoint |
| **J15** | **"I am nested fat"** | lists, strings, arc, nested_fat | 18 | PASS | PASS | **6.2/10** | **2 CRITICAL: double-free on [str] elements, double drop in unwind** |
| **J16** | **"I am fat and moving"** | strings, arc, ownership_transfer | 42 | PASS | PASS | **9.4/10** | HIGH: field-by-field aggregate materialization (3-6x bloat), sret ABI correct |
| **J17** | **"I am a captured fat pointer"** | strings, closures, capture | 10 | PASS | **FAIL** | **3.0/10** | **CRITICAL: closure capturing str — unresolved type variable at codegen** |

J1-J13: All PASS, all 10.0/10.
**J14-J17 (Fat Pointer Series): Exposed 3 CRITICAL bugs and 1 HIGH codegen issue.**

## Fat Pointer Findings (J14-J17)

The fat pointer journeys were specifically designed to stress-test the `FatPointer` RC strategy (`{i64 len, i64 cap, ptr data}` representation for strings). They revealed a **class of bugs** that the original J1-J13 journeys missed because those journeys either used scalar types or tested fat pointers in isolation (J9 `.length()` only, J10 `[int]` not `[str]`).

### CRITICAL-C15a: Double-free on `[str]` element cleanup
**Journey**: J15 | **Status**: OPEN
**Root cause**: Both `ori_iter_drop` (iterator cleanup) and `ori_buffer_rc_dec` (list destructor calling `_ori_elem_dec`) free the same string elements. The iterator takes ownership of elements during iteration but the list destructor doesn't know this.
**Impact**: Memory corruption at runtime. Correct exit code masks the bug.

### CRITICAL-C15b: Double `ori_buffer_rc_dec` in unwind path
**Journey**: J15 | **Status**: OPEN
**Root cause**: Landing pad in `@main` emits two `ori_buffer_rc_dec` calls on the same list buffer.
**Impact**: Double-free if `count_chars` panics during iteration.

### CRITICAL-C17: Closure capturing str — codegen crash
**Journey**: J17 | **Status**: OPEN
**Root cause**: Monomorphization fails to propagate the concrete `str` type for closure parameters when the closure captures a fat pointer. Type variable `Idx(N)` leaks into LLVM codegen, causing: (1) lambda param lowered as `i64` instead of `{i64, i64, ptr}`, (2) `.length()` dispatch fails, (3) `ori_rc_dec` called with wrong type.
**Impact**: AOT compilation fails. Eval works correctly.

### HIGH-H16: Field-by-field aggregate materialization
**Journey**: J16 | **Status**: OPEN
**Root cause**: 24-byte `str` values are copied via 10-instruction sequences (3 GEP + 3 load + 3 insertvalue + 1 store) instead of single aggregate load/store (2 instructions).
**Impact**: 3-6x instruction bloat per str operation. Correct behavior, but wasteful.

## Recurring Issues

### Active Issues (from fat pointer series)

| Issue | Severity | Journeys | Description |
|-------|----------|----------|-------------|
| Double-free on `[str]` elements | CRITICAL | J15 | Iterator and list destructor both free string elements |
| Closure capturing str | CRITICAL | J17 | Unresolved type variable leaks into codegen |
| Aggregate materialization bloat | HIGH | J14, J16 | Field-by-field copy instead of aggregate load/store |
| Duplicate ptrtoint in SSO guard | LOW | J14 | Same pointer converted to integer twice per guard |
| Empty blocks in string functions | LOW | J14 | Redundant unconditional branches |

### Resolved Issues (from J1-J13)

- **Empty trampoline blocks** — FIXED (AIMS Section 01, CFG simplification)
- **Missing nounwind** — FIXED (AIMS Section 01, posthoc nounwind analysis)
- **Missing memory(...)** — FIXED (AIMS Section 02, posthoc purity analysis)
- **Missing noundef on closures** — FIXED (AIMS Section 01)
- **Missing nonnull/dereferenceable** — FIXED (AIMS Section 01.4)
- **Missing uwtable on drop helpers** — FIXED (AIMS Section 01)

## Score Trend

| Difficulty | Journeys | Avg Score | Range |
|------------|----------|-----------|-------|
| Simple (J1-J4) | 4 | 10.0 | 10.0–10.0 |
| Moderate (J5-J8) | 4 | 10.0 | 10.0–10.0 |
| Complex (J9-J13) | 5 | 10.0 | 10.0–10.0 |
| **Fat Pointer (J14-J17)** | **4** | **7.0** | **3.0–9.4** |
| **Overall** | **17** | **9.3** | **3.0–10.0** |

The fat pointer series drops the overall average from 10.0 to 9.3, revealing that the original 13 journeys were testing a "happy path" that avoided the compiler's weakest area: **fat pointer types crossing feature boundaries** (closures, nested collections, ownership transfer).

## Per-Category Averages (All 17 Journeys)

| Category | Weight | Avg Score | Perfect (10/10) | Failing |
|----------|--------|-----------|-----------------|---------|
| Instruction Efficiency | 15% | 9.8 | 15/17 | — |
| ARC Correctness | 20% | 9.2 | 13/17 | J15 (3/10) |
| Attributes & Safety | 10% | 9.9 | 16/17 | — |
| Control Flow | 10% | 9.6 | 13/17 | — |
| IR Quality | 20% | 9.6 | 13/17 | — |
| Binary Quality | 10% | 8.8 | 15/17 | J17 (0/10) |
| Other Findings | 15% | 9.6 | 15/17 | — |

**ARC Correctness and Binary Quality are the weakest categories**, dragged down by the fat pointer bugs.

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
