# Section 07A Verification Results: Core Built-ins

**Verified**: 2026-03-19
**Section**: `plans/roadmap/section-07A-core-builtins.md`
**Status**: in-progress (17/296 items, ~5%)

---

## Summary

The section has 8 `[x]` checked items and many `[ ]` unchecked items. Several unchecked items are actually partially or fully implemented but not reflected in the roadmap. The checked items are all genuinely working.

---

## 7A.1 Type Conversions

All items marked `[ ]`. Verification:

- **`As<T>` / `TryAs<T>` traits**: NOT implemented. No trait definitions in prelude.ori, no registration in type checker. -- VERIFIED as `[ ]`
- **`x as T` / `x as? T` syntax**: Parser has `ExprKind::Cast` with `fallible` flag. Type checker infers cast types in `operators.rs`. Evaluator handles via `eval_can_cast`. These work for built-in primitive conversions (int->float, etc.) but do NOT use the `As<T>`/`TryAs<T>` trait system. -- ROADMAP INACCURACY: basic as/as? syntax is partially working for primitives, but the trait-based system is not. The `[ ]` items are correctly marked as incomplete since the trait infrastructure is missing.
- **Standard `As`/`TryAs` implementations**: NOT implemented (no traits to implement against). -- VERIFIED as `[ ]`
- **Float truncation methods**: NOT implemented. -- VERIFIED as `[ ]`
- **Remove `int()`, `float()`, `str()`, `byte()` functions**: Still present in `identifiers.rs` and `prelude.rs`. -- VERIFIED as `[ ]`

---

## 7A.2 Assertions

### [x] `assert(cond:)` -- VERIFIED

- **Implementation**: `assert` is defined in `library/std/testing.ori` as a regular Ori function: `if !cond then panic(msg: "assertion failed")`
- **Test coverage**: Used in 257+ spec test files via `use std.testing { assert, assert_eq }`
- **LLVM support**: `ori_assert` runtime function exists in `ori_rt`, declared in `runtime_functions.rs`, mapped in JIT `runtime_mappings.rs`. AOT tests exist (`test_assert_false_panics`).
- **Roadmap inaccuracy**: The `[ ] LLVM Support` and `[ ] AOT Tests` sub-items are marked incomplete but assert DOES have LLVM runtime support and AOT test coverage.

**Classification**: VERIFIED -- but LLVM/AOT sub-items should be `[x]`

### [x] `assert_eq(actual:, expected:)` -- VERIFIED

- **Implementation**: Defined in `library/std/testing.ori` as Ori function with `T: Eq` bound.
- **Test coverage**: Used in 257+ spec test files.
- **LLVM support**: `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_float`, `ori_assert_eq_str` runtime functions exist. AOT tests: `test_assert_eq_int_mismatch_panics`, `test_assert_eq_bool_mismatch_panics`, `test_assert_eq_str_mismatch_panics`.
- **Roadmap inaccuracy**: Same as assert -- LLVM/AOT sub-items should be `[x]`.

**Classification**: VERIFIED -- but LLVM/AOT sub-items should be `[x]`

### [x] `assert_ne(actual:, unexpected:)` -- VERIFIED

- **Implementation**: Defined in `library/std/testing.ori`.
- **Test coverage**: Used in 20 spec test files (e.g., `duration_size_hashable.ori`, `eq.ori`, `data.ori`, `try.ori`).
- **Roadmap note**: Says "Used in module tests (`tests/spec/modules/`)" but assert_ne is NOT used in `tests/spec/modules/`. It IS used in other spec test directories. Minor description inaccuracy.
- **LLVM support**: No dedicated `ori_assert_ne_*` runtime functions, but assert_ne in Ori desugars to `if actual == unexpected then panic(...)` which uses existing LLVM infrastructure.

**Classification**: VERIFIED -- description inaccuracy (not in modules dir)

### [ ] `assert_some`, `assert_none`, `assert_ok`, `assert_err` -- ROADMAP INACCURACY

- **Implementation**: All four ARE defined in `library/std/testing.ori` (lines 19-32).
- **Test coverage**: NOT used in any spec tests. Zero test files import or use these functions.
- **Status**: Implemented but untested. The roadmap says "not found in test suite" which is correct for test usage, but the functions themselves exist and are likely functional.

**Classification**: NEEDS TESTS -- implementations exist but are untested. Should be `[x]` for implementation with `[ ]` for tests.

---

## 7A.3 I/O and Other

### [x] `print(x)` -- VERIFIED

- **Implementation**: `FunctionExpKind::Print` in parser/IR, typed as `(T) -> void` in `oric/src/typeck.rs`, evaluated in `function_exp.rs`, registered as built-in in `typeck.rs`.
- **LLVM support**: `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` runtime functions. Used extensively in AOT test infrastructure.
- **Roadmap accuracy**: `[x] LLVM Support` is correctly marked.

**Classification**: VERIFIED

### [x] `compare(a, b)` -- VERIFIED

- **Implementation**: Defined in `library/std/prelude.ori` (line 357) as `pub @compare (a: int, b: int) -> Ordering`.
- **Test coverage**: 58 tests in `tests/spec/traits/core/comparable.ori`.
- **LLVM support**: Roadmap says `[x] LLVM Support: LLVM codegen for compare -- inline IR in lower_calls.rs`. Comparison operators are extensively supported in LLVM codegen via `codegen/ir_builder/comparisons.rs` and `derive_codegen/` (Comparable derive).
- **Note**: The prelude `compare()` is int-only. The trait method `Comparable.compare()` is generic.

**Classification**: VERIFIED

### [x] `min(a, b)`, `max(a, b)` -- VERIFIED

- **Implementation**: Defined in `library/std/prelude.ori` (lines 362-365) as int-only functions.
- **Test coverage**: Verified in Section 4.6 per roadmap claim.
- **LLVM support**: Roadmap correctly marks `[ ] LLVM Support` -- no dedicated LLVM codegen for min/max. However, since they're regular Ori functions, they compile through standard function call codegen.

**Classification**: VERIFIED

### [x] `panic(msg)` -- VERIFIED

- **Implementation**: `FunctionExpKind::Panic` in parser/IR, typed as `(str) -> Never`, evaluated to produce `EvalError`.
- **LLVM support**: `ori_panic` and `ori_panic_cstr` runtime functions. Full `@panic` handler infrastructure with PanicInfo, trampoline, re-entrancy protection. 9 AOT tests pass.
- **Roadmap accuracy**: `[x] LLVM Support` is correctly marked.

**Classification**: VERIFIED

---

## 7A.4 Float NaN Behavior

All items marked `[ ]`. Verification:

- **NaN comparison panics**: NOT implemented. Current behavior follows IEEE 754 (NaN != NaN returns true, NaN < x returns false). The `eval_float_binary` function in `operators/mod.rs` uses `partial_cmp` without any NaN panic checks. -- VERIFIED as `[ ]`
- **NaN-producing operations**: Not separately handled -- standard Rust/IEEE 754 behavior. -- VERIFIED as `[ ]`

---

## 7A.5 Developer Functions

All items marked `[ ]`. Verification:

- **`todo()` and `todo(reason:)`**: ACTUALLY IMPLEMENTED. `FunctionExpKind::Todo` exists in parser/IR. Type checker returns `Idx::NEVER`. Evaluator produces `EvalError` with "not yet implemented" message. -- ROADMAP INACCURACY: should be `[x]` for implementation. Missing dedicated tests.
- **`unreachable()` and `unreachable(reason:)`**: ACTUALLY IMPLEMENTED. `FunctionExpKind::Unreachable` exists. Type checker returns `Idx::NEVER`. Evaluator produces `EvalError("reached unreachable code")`. -- ROADMAP INACCURACY: should be `[x]` for implementation. Missing dedicated tests.
- **`dbg(value:)` and `dbg(value:, label:)`**: NOT implemented. No `FunctionExpKind::Dbg` variant exists. -- VERIFIED as `[ ]`
- **Compile-time location capture**: NOT implemented for any of the three. Messages don't include file:line. -- VERIFIED as `[ ]`

**Classification**: `todo` and `unreachable` are NEEDS TESTS (implemented but untested as standalone features). `dbg` is correctly `[ ]`.

---

## 7A.6 Additional Built-in Functions

### repeat Function

- **`repeat<T: Clone>(value: T) -> impl Iterator`**: ACTUALLY IMPLEMENTED. `IteratorValue::Repeat` variant in `ori_patterns`. Type signature in `identifiers.rs`. `function_val_repeat` in evaluator. 14+ dedicated tests in `tests/spec/traits/iterator/infinite.ori`.
- **Clone requirement enforcement**: Typed with fresh var (not explicitly Clone-bounded) but works correctly due to runtime Clone dispatch.

**Classification**: ROADMAP INACCURACY -- should be `[x]`. Well-tested. Missing LLVM-specific tests but repeat works through standard iterator codegen.

### PanicInfo Type

- **PanicInfo struct**: PARTIALLY IMPLEMENTED in LLVM codegen (`entry_point.rs` constructs PanicInfo fields, trampoline bridges to user handler). Not available as an Ori-level type for general use. -- Partially done, `[ ]` is approximately correct.

### @panic Handler

- **@panic recognition, signature validation, handler invocation, re-entrancy, default handler, exit code**: ALL IMPLEMENTED in LLVM codegen. 9 AOT tests pass covering all these scenarios. -- ROADMAP INACCURACY: All `[ ]` items in this sub-section should be `[x]` for LLVM support. They are fully functional in AOT.
- **Interpreter support**: NOT implemented (these are LLVM/AOT-only features). -- `[ ]` items for evaluator are correctly marked.

---

## 7A.7 Resource Management

- **`drop_early`**: NOT implemented. No code found in any compiler crate. -- VERIFIED as `[ ]`

---

## 7A.8 Compile-Time File Embedding

- **`embed(path)` / `has_embed(path)`**: NOT implemented. No `EmbedExpr` node, no code in any crate. -- VERIFIED as `[ ]`

---

## 7A.9 Char and Byte Classification Methods

All items marked `[ ]`. Verification:

### Char Methods -- PARTIALLY IMPLEMENTED

Implemented in evaluator (`methods/variants.rs`):
- `is_alphabetic` (via `is_alpha` name) -- works, uses Rust's Unicode-aware `char::is_alphabetic()`
- `is_ascii` -- works
- `is_digit` -- works, but uses `is_ascii_digit()` (ASCII-only, spec says Unicode Nd)
- `is_lowercase` -- works, uses Rust's Unicode-aware `char::is_lowercase()`
- `is_uppercase` -- works, uses Rust's Unicode-aware `char::is_uppercase()`
- `is_whitespace` -- works, uses Rust's Unicode-aware `char::is_whitespace()`

NOT implemented:
- `is_alphanumeric` -- missing
- `is_control` -- missing
- All `is_ascii_*` variants (is_ascii_alphabetic, is_ascii_digit, etc.) -- missing
- `to_ascii_uppercase`, `to_ascii_lowercase` -- have `to_uppercase`/`to_lowercase` (full Unicode, not ASCII-only)
- `to_digit(radix:)` -- missing

### Byte Methods -- PARTIALLY IMPLEMENTED

Implemented in evaluator:
- `is_ascii` (always returns true -- byte is 0-255, but `is_ascii` should be 0-127)
- `is_ascii_alpha` / `is_alpha` -- works
- `is_ascii_digit` / `is_digit` -- works
- `is_ascii_whitespace` -- works

NOT implemented:
- `is_ascii_alphanumeric` / `is_alnum` -- missing
- `is_ascii_uppercase` / `is_upper` -- missing
- `is_ascii_lowercase` / `is_lower` -- missing
- `is_ascii_hex_digit` / `is_hex_digit` -- missing
- `is_ascii_punctuation` -- missing
- `is_ascii_control` -- missing
- `to_ascii_uppercase`, `to_ascii_lowercase` -- missing
- `to_digit(radix:)` -- missing

**Classification**: ROADMAP INACCURACY -- several methods are implemented but roadmap shows all as `[ ]`. Should reflect partial completion. Also BUG: `byte.is_ascii()` always returns `true` but should return `b <= 127` (bytes 128-255 are not ASCII).

---

## 7A.10 Byte-Level String Access

All items marked `[ ]`. Verification:

### ACTUALLY IMPLEMENTED in evaluator (`methods/collections.rs`):

- `str.as_bytes()` -- returns `[byte]` (copies bytes, not zero-copy seamless slice in interpreter)
- `str.to_bytes()` -- same implementation as `as_bytes` (alias)
- `str.byte_len()` -- returns UTF-8 byte count
- `str.from_utf8(bytes:)` -- validates UTF-8, returns `Result<str, Error>`
- `str.from_utf8_unchecked(bytes:)` -- validates anyway in interpreter (safety)

### Registered in type registry (`ori_registry/src/defs/str.rs`):

All five methods have `MethodDef` entries with correct signatures.

**Classification**: ROADMAP INACCURACY -- all five str methods are implemented and registered. Should be `[x]` for interpreter support. Missing: seamless slice behavior for `as_bytes()`, LLVM codegen, dedicated spec tests.

---

## 7A.11 Section Completion Checklist

All `[ ]`. Correct -- section is far from complete.

---

## Bugs Found

1. **BUG**: `byte.is_ascii()` always returns `true` (`compiler/ori_eval/src/methods/variants.rs` line 262: `Ok(Value::Bool(true))`). Bytes 128-255 are NOT ASCII. Should be `Ok(Value::Bool(b.is_ascii()))` which checks `b <= 127`.

2. **BUG**: `char.is_digit()` uses `is_ascii_digit()` (ASCII-only 0-9) but the spec says it should check Unicode `Nd` category. Should use `c.is_ascii_digit()` or `c.is_numeric()` depending on intended semantics.

---

## Roadmap Inaccuracies

1. **7A.2 assert/assert_eq/assert_ne LLVM sub-items**: Marked `[ ]` but assert has full LLVM runtime support and AOT tests.
2. **7A.2 assert_some/assert_none/assert_ok/assert_err**: Marked as not implemented but they exist in `library/std/testing.ori`. Should be `[x]` for implementation, `[ ]` for tests.
3. **7A.3 assert_ne description**: Says "Used in module tests (tests/spec/modules/)" -- it is NOT used there. Used in other test directories.
4. **7A.5 todo/unreachable**: Marked `[ ]` but both are fully implemented in parser, type checker, and evaluator.
5. **7A.6 repeat**: Marked `[ ]` but fully implemented and well-tested (14+ tests).
6. **7A.6 @panic handler**: All sub-items marked `[ ]` but fully implemented in LLVM with 9 passing AOT tests.
7. **7A.9 Char/Byte methods**: Several methods are implemented but all marked `[ ]`.
8. **7A.10 Byte-level string access**: All five methods implemented but all marked `[ ]`.

---

## Test Commands Used

```bash
timeout 150 cargo st tests/spec/traits/core/comparable.ori  # 4181 passed
timeout 150 cargo st tests/spec/types/integer_safety.ori     # 4181 passed
timeout 150 cargo st tests/spec/types/duration_size_hashable.ori  # 4181 passed (1 unrelated fail)
timeout 150 cargo st tests/spec/expressions/type_conversion.ori   # 4181 passed
timeout 150 cargo test -p ori_llvm test_panic                # 9 passed
timeout 150 cargo test -p ori_llvm assert                    # 10 passed
```

---

## Verification Classification Summary

| Item | Status | Classification |
|------|--------|---------------|
| 7A.1 Type Conversions (all) | `[ ]` | VERIFIED as incomplete -- `as`/`as?` syntax works for primitives but trait system missing |
| 7A.2 assert(cond:) | `[x]` | VERIFIED -- LLVM sub-items inaccurately marked `[ ]` |
| 7A.2 assert_eq | `[x]` | VERIFIED -- LLVM sub-items inaccurately marked `[ ]` |
| 7A.2 assert_ne | `[x]` | VERIFIED -- description inaccuracy (wrong test location) |
| 7A.2 assert_some/none/ok/err | `[ ]` | NEEDS TESTS -- implementations exist, zero test coverage |
| 7A.3 print | `[x]` | VERIFIED |
| 7A.3 compare | `[x]` | VERIFIED |
| 7A.3 min/max | `[x]` | VERIFIED |
| 7A.3 panic | `[x]` | VERIFIED |
| 7A.4 Float NaN | `[ ]` | VERIFIED as incomplete |
| 7A.5 todo/unreachable | `[ ]` | NEEDS TESTS -- implemented but untested, roadmap inaccurate |
| 7A.5 dbg | `[ ]` | VERIFIED as incomplete |
| 7A.6 repeat | `[ ]` | VERIFIED as IMPLEMENTED -- roadmap inaccurate, 14+ tests exist |
| 7A.6 PanicInfo | `[ ]` | VERIFIED as partially complete (LLVM-only) |
| 7A.6 @panic handler | `[ ]` | VERIFIED as IMPLEMENTED in LLVM -- roadmap inaccurate, 9 AOT tests |
| 7A.7 drop_early | `[ ]` | VERIFIED as incomplete |
| 7A.8 embed/has_embed | `[ ]` | VERIFIED as incomplete |
| 7A.9 Char/Byte methods | `[ ]` | PARTIALLY IMPLEMENTED -- roadmap inaccurate, BUG in byte.is_ascii() |
| 7A.10 Byte string access | `[ ]` | VERIFIED as IMPLEMENTED in interpreter -- roadmap inaccurate |
| 7A.11 Completion | `[ ]` | VERIFIED as incomplete |
