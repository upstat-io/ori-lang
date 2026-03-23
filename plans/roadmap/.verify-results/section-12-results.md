# Section 12: Variadic Functions -- Verification Results

**Verified by**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-19
**Section status**: 0/86 (0%) -- all items `[ ]`
**Verdict**: PARTIALLY INCORRECT -- significant infrastructure already exists

---

## Summary

The section claims 0% completion ("not-started"), but investigation reveals substantial infrastructure is already implemented across the lexer, IR, and parser. The type checker and evaluator lack variadic-specific handling, confirming those phases are genuinely not started. The section status should be updated to reflect partial completion of infrastructure items.

---

## Spot-Check Results

### Item 12.1: Lexer -- `...` token (if not exists)

**Plan status**: `[ ]`
**Actual status**: IMPLEMENTED
**Classification**: STALE -- item is complete but not checked off

**Evidence**:
- `compiler/ori_lexer_core/src/tag/mod.rs`: `DotDotDot = 58` -- raw scanner tag exists
- `compiler/ori_lexer_core/src/raw_scanner/operators.rs`: scanner emits `RawTag::DotDotDot`
- `compiler/ori_lexer/src/trivial/mod.rs`: cooker maps `RawTag::DotDotDot` to `TokenKind::DotDotDot`
- `compiler/ori_ir/src/token/kind.rs:121`: `DotDotDot` token kind exists with tag `92`
- Tests: `compiler/ori_lexer_core/src/raw_scanner/tests.rs` asserts `scan_tags("...")` yields `DotDotDot`
- Tests: `compiler/ori_lexer_core/src/tag/tests.rs` asserts `DotDotDot.lexeme()` is `Some("...")`
- Distinguished from range `..` (`DotDot`) and `..=` (`DotDotEq`) in the raw scanner

**All sub-items complete**:
- [done] Three-dot token
- [done] Distinguish from range `..`

---

### Item 12.1: IR -- `is_variadic` flag on Param

**Plan status**: `[ ]` (implicit in "Sync Points" header, items under 12.1 Parser)
**Actual status**: IMPLEMENTED
**Classification**: STALE -- item is complete but not checked off

**Evidence**:
- `compiler/ori_ir/src/ast/items/function.rs:97`: `pub is_variadic: bool` field on `Param` struct
- Documentation: "If true, this is a variadic parameter (`nums: ...int`). Variadic params receive values as `[T]` inside the function."
- `compiler/ori_ir/src/ast/items/extern_def.rs:47`: `pub is_c_variadic: bool` on `ExternItem` for C variadics
- `compiler/ori_ir/src/ast/collections.rs:143`: `pub is_spread: bool` on `CallArg` for spread in calls

---

### Item 12.1: Parser -- Parse variadic parameters

**Plan status**: `[ ]`
**Actual status**: IMPLEMENTED
**Classification**: STALE -- item is complete but not checked off

**Evidence**:
- `compiler/ori_parse/src/grammar/item/function/mod.rs:487-492`: Parser checks for `DotDotDot` after `:` in parameter position, sets `is_variadic: true`
- `compiler/ori_parse/src/grammar/expr/postfix.rs:392-394`: Parser handles `...expr` spread syntax in call arguments, sets `is_spread: true`
- `compiler/ori_parse/src/tests/compositional.rs:487-490`: Tests parse `@collect<T> (items: ...T) -> [T] = items;` and `@print_all<T: Printable> (items: ...T) -> void = ();` successfully
- `compiler/ori_parse/src/incremental/copier.rs:794`: Incremental parser copies `is_variadic` field
- Tests pass: `cargo test -p ori_parse -- compositional` (76 passed, 0 failed)

**Sub-items**:
- [done] In function signatures -- parser handles `...Type` after colon
- [done] Spread in call expressions -- `...expr` parsed and `is_spread` set
- [todo] Validation (last param only) -- no validation found in parser; may be deferred to type checker

---

### Item 12.4: Parser -- Parse C variadics

**Plan status**: `[ ]`
**Actual status**: IMPLEMENTED
**Classification**: STALE -- item is complete but not checked off

**Evidence**:
- `compiler/ori_parse/src/grammar/item/extern_def.rs:184-224`: `parse_extern_params()` handles `...` in extern blocks, sets `is_c_variadic: true`
- `compiler/oric/tests/phases/parse/extern_def.rs:97-112`: Two tests verify C variadic parsing:
  - `test_extern_c_variadic`: `@printf (fmt: CPtr, ...) -> c_int` -- asserts `is_c_variadic` and 1 named param
  - `test_extern_c_variadic_no_params`: `@va_func (...) -> void` -- asserts `is_c_variadic` and empty params
- All 20 extern_def tests pass
- `compiler/ori_fmt/src/declarations/extern_def.rs:55`: Formatter handles `is_c_variadic` for output

**Sub-items**:
- [done] `...` without type in extern
- [done] Distinguish from Ori variadics -- separate `ExternParam`/`ExternItem` types vs `Param`

---

### Item 12.1: Type checker -- Variadic type rules

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- `is_variadic` only appears in `ori_types` in test helpers where it is hardcoded to `false`
- No code in `compiler/ori_types/src/infer/` references `is_variadic` or `variadic` outside tests
- The type checker does not convert `...T` to `[T]` internally
- No spread type compatibility checking
- No element type inference for variadic parameters

---

### Item 12.1: Evaluator -- Handle variadic calls

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- Zero references to `is_variadic`, `variadic`, or `is_spread` in `compiler/ori_eval/src/`
- `tests/spec/declarations/functions.ori` line 99: "STATUS: Lexer [OK], Parser [OK], Evaluator [BROKEN] - variadic calling not implemented"
- All variadic test cases in `functions.ori` are commented out with `#skip("variadic function calling not implemented in evaluator")`
- The evaluator does not pack multiple arguments into a list for variadic parameters
- The evaluator does not expand spread expressions

---

### Item 12.1: LLVM Support

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- Zero references to `is_variadic`, `is_spread`, or `is_c_variadic` in `compiler/ori_llvm/`
- No variadic codegen, no C variadic ABI (`va_list`) handling

---

### Item 12.4: Type checker -- C variadic rules

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- No type checking for `is_c_variadic` found in `ori_types`
- No unsafe context validation for C variadic calls

---

### Item 12.1: Test -- `tests/spec/functions/variadic.ori`

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED (partially exists as commented-out tests)
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- `tests/spec/functions/variadic.ori` does not exist
- `tests/spec/declarations/functions.ori` contains commented-out variadic tests (lines 95-118) with annotations explaining "variadic calling not implemented in evaluator"
- No dedicated variadic spec test file

---

### Item 12.2-12.3: Minimum args / Trait bounds

**Plan status**: `[ ]`
**Actual status**: NOT IMPLEMENTED
**Classification**: CONFIRMED `[ ]`

**Evidence**:
- No minimum arg validation code found in any crate
- No variadic trait bound validation code found
- No spec test files for these features

---

## Implementation Status Matrix

| Component | Lexer | IR | Parser | Type Checker | Evaluator | LLVM | Tests |
|-----------|-------|----|--------|-------------|-----------|------|-------|
| `...` token | [done] | [done] | [done] | n/a | n/a | n/a | [done] |
| Ori variadic params (`...T`) | n/a | [done] | [done] | [todo] | [todo] | [todo] | [partial] |
| Spread in calls (`...expr`) | n/a | [done] | [done] | [todo] | [todo] | [todo] | [todo] |
| C variadics (`...` in extern) | n/a | [done] | [done] | [todo] | n/a | [todo] | [done] |
| Spread in list/map/struct literals | n/a | [done] | [done] | [done] | [done] | [done] | [done] |
| Variadic `...T` -> `[T]` conversion | n/a | n/a | n/a | [todo] | [todo] | [todo] | [todo] |
| Min arg count validation | n/a | n/a | n/a | [todo] | [todo] | [todo] | [todo] |
| Trait bounds on variadics | n/a | n/a | n/a | [todo] | [todo] | [todo] | [todo] |
| va_list ABI codegen | n/a | n/a | n/a | n/a | n/a | [todo] | [todo] |

**Note**: Spread in list/map/struct literals is fully implemented (separate from variadic function calls). The `ListWithSpread`, `MapWithSpread`, `StructWithSpread` AST nodes exist, are canonicalized via `ori_canon`, and work end-to-end. This is related infrastructure but is NOT part of Section 12 (which covers function variadics).

---

## Corrected Status Assessment

The section should NOT be 0%. Based on evidence:

- **Lexer items**: 100% complete (2/2 sub-items)
- **IR items**: 100% complete (Param.is_variadic, ExternItem.is_c_variadic, CallArg.is_spread)
- **Parser items (12.1)**: ~90% complete (parsing works, validation not confirmed)
- **Parser items (12.4)**: 100% complete (C variadic parsing works with tests)
- **Type checker items**: 0% complete
- **Evaluator items**: 0% complete
- **LLVM items**: 0% complete
- **Spec test items**: 0% complete (commented-out code exists but no active tests)

Estimated true completion: approximately 15-20/86 items if lexer/IR/parser items were properly counted. The "not-started" status is inaccurate for the section as a whole; "in-progress" would be more accurate, with the caveat that the critical backend work (type checker, evaluator, LLVM) is genuinely not started.

---

## Findings

| # | Item | Classification | Details |
|---|------|---------------|---------|
| 1 | Lexer `...` token | STALE | Fully implemented but marked `[ ]` |
| 2 | IR `is_variadic` on Param | STALE | Fully implemented but not tracked |
| 3 | IR `is_c_variadic` on ExternItem | STALE | Fully implemented but not tracked |
| 4 | IR `is_spread` on CallArg | STALE | Fully implemented but not tracked |
| 5 | Parser variadic params | STALE | Parsing works, tests pass, marked `[ ]` |
| 6 | Parser spread in calls | STALE | Parsing works, marked `[ ]` |
| 7 | Parser C variadics | STALE | Parsing works with 2 dedicated tests, marked `[ ]` |
| 8 | Type checker variadic rules | CONFIRMED `[ ]` | No implementation found |
| 9 | Evaluator variadic handling | CONFIRMED `[ ]` | No implementation found, commented-out tests confirm |
| 10 | LLVM variadic codegen | CONFIRMED `[ ]` | No implementation found |
| 11 | C variadic type checking | CONFIRMED `[ ]` | No implementation found |
| 12 | Spec test files | CONFIRMED `[ ]` | No dedicated test files exist |

---

## Tests Executed

| Test Suite | Command | Result |
|-----------|---------|--------|
| Extern def parse tests | `cargo test -p oric --test phases -- extern_def` | 20 passed, 0 failed |
| Compositional parse tests | `cargo test -p ori_parse -- compositional` | 76 passed, 0 failed |
| Spec declarations/functions | `cargo st tests/spec/declarations/functions.ori` | 4181 passed, 0 failed, 42 skipped |
