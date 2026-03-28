# Section 7A Verification Results: Core Built-ins

**Verified by**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-28
**Section file**: `plans/roadmap/section-07A-core-builtins.md`

## Files Loaded Before Verification

- `/home/eric/projects/ori_lang/CLAUDE.md` (full)
- All 20 files in `.claude/rules/`: aot.md, arc.md, cargo.md, compiler.md, diagnostic.md, eval.md, impl-hygiene.md, ir.md, llvm.md, ori-lang.md, ori-syntax.md, parse.md, patterns.md, registry.md, roadmap.md, runtime.md, spec.md, tests.md, typeck.md, types.md
- `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md` (full)
- `plans/roadmap/section-07A-core-builtins.md` (full)

---

## Summary

| Status | Count |
|--------|-------|
| Items marked `[x]` in roadmap | 10 |
| Items marked `[ ]` in roadmap | ~80 |
| VERIFIED | 7 |
| WEAK | 3 |
| NOT STARTED (confirmed) | ~73 |
| STALE (roadmap out of date) | 7 |

Most of this section is not started (`[ ]`), which I confirmed. The focus of this audit is on the 10 `[x]` items and identifying roadmap inaccuracies where features are partially implemented but not reflected.

---

## 7A.1 Type Conversions

All items are `[ ]` (not started). Confirmed: `As<T>` and `TryAs<T>` traits do not exist. No `as`/`as?` syntax desugaring exists.

**STALE FINDING**: The roadmap includes "Remove: `int()`, `float()`, `str()`, `byte()` function syntax" as a future item. These functions ARE currently implemented in `compiler/ori_eval/src/function_val.rs` and registered in `compiler/ori_eval/src/interpreter/prelude.rs`. They work correctly. The spec (`annex-c-built-in-functions.md`) actually still documents them as the current conversion mechanism. The `as`/`as?` proposal would replace them, but the spec and implementation agree on the current state.

**Status**: NOT STARTED (confirmed) for all 9 sub-items.

---

## 7A.2 Assertions

### `assert(cond:)` -- `[x]` (2026-02-10)

**Implementation**: Implemented in Ori as `library/std/testing.ori` line 7: `pub @assert (cond: bool) -> void = if !cond then panic(msg: "assertion failed");`

**Test coverage**: Used in 310+ occurrences across 20+ test files in `tests/spec/`. Exercised with int, float, bool, str, list, map, set, Option, Result, struct, newtype types.

**Matrix coverage**: Excellent -- exercised across all primitive types, collections, and user-defined types through the broader test suite.

**Semantic pin**: Assertion behavior is implicitly pinned by hundreds of passing tests.

**LLVM support**: Roadmap says `[ ]` for LLVM. This is STALE. `print` and `panic` both have LLVM support (`_ori_print`, `_ori_panic` in `ori_rt`). Since `assert` desugars to `if !cond then panic(...)`, it works transitively through LLVM when `panic` works. AOT tests in `compiler/ori_llvm/tests/aot/panic.rs` verify panic behavior.

**Verdict**: **VERIFIED** (interpreter). **STALE** (LLVM checkbox should be partially checked -- works transitively through panic).

### `assert_eq(actual:, expected:)` -- `[x]` (2026-02-10)

**Implementation**: `library/std/testing.ori` line 10: `pub @assert_eq<T: Eq> (actual: T, expected: T) -> void = if actual != expected then panic(msg: "assertion failed: " + str(actual) + " != " + str(expected));`

**Test coverage**: Used in 78+ occurrences across 20+ test files. Dedicated tests in `tests/spec/traits/core/eq.ori` lines 154-175 test `assert_eq` with int and str.

**Matrix coverage**: Good -- tested with int, str, bool, float, list, map, tuple, Option, Result, struct.

**Semantic pin**: The `assert_eq` tests in `eq.ori` are semantic pins.

**LLVM support**: Same as `assert` -- works transitively through `panic`. AOT tests in `panic.rs` lines 150-201 test assertion-like patterns in LLVM (manually inlined, not using `std.testing` due to monomorphization bug noted in the test comments).

**Spec discrepancy**: Spec says `assert_eq<T: Eq + Debug>` but implementation is `assert_eq<T: Eq>` (uses `str()` instead of `debug()` for value display). This is a minor spec divergence.

**Verdict**: **VERIFIED** (interpreter). **WEAK** (spec requires `Debug` bound, impl uses `str()`).

### `assert_ne(actual:, unexpected:)` -- `[x]` (2026-02-10)

**Implementation**: `library/std/testing.ori` line 15.

**Test coverage**: Used in 3 places: `tests/spec/types/duration_size_hashable.ori` and `tests/spec/traits/core/eq.ori` (lines 178-200, dedicated tests with int and str).

**Matrix coverage**: Minimal -- only int and str tested. Missing: bool, float, list, map, struct, Option.

**Semantic pin**: The tests in `eq.ori` serve as pins.

**Verdict**: **WEAK** -- limited type coverage for `assert_ne`. Only int and str tested.

---

### `assert_some`, `assert_none`, `assert_ok`, `assert_err` -- `[ ]`

**Implementation**: ACTUALLY IMPLEMENTED in `library/std/testing.ori` lines 19-32. All four functions exist with correct signatures.

**Test coverage**: ZERO -- no test file in the entire `tests/` directory uses any of these four functions.

**STALE FINDING**: The roadmap says "Implement: assert_some(x)" with "Not verified -- not found in test suite". The implementation EXISTS but the roadmap is correct that no tests use them. These should be marked as "implemented but untested" rather than "not implemented".

**Verdict**: **STALE** -- functions are implemented in Ori stdlib but marked as `[ ]`. They need spec tests to be verified.

### `assert_panics`, `assert_panics_with`

Not listed in roadmap section 7A.2 but ARE implemented in `library/std/testing.ori` lines 35-50 and ARE used in 4 test files (`tests/spec/control_flow/for/range_edge_cases.ori`, `tests/spec/types/integer_safety.ori`, `tests/spec/expressions/operators_bitwise.ori`, `tests/spec/expressions/ranges.ori`).

**Note**: `assert_panics` is listed in the spec (annex-c) but missing from the roadmap section. This is a roadmap gap.

---

## 7A.3 I/O and Other

### `print(x)` -- `[x]` (2026-02-10)

**Implementation**: `FunctionExpKind::Print` in `compiler/ori_eval/src/interpreter/can_eval/function_exp.rs` line 178.

**Test coverage**: Used pervasively. LLVM has `_ori_print` runtime function with tests in `compiler/ori_llvm/src/tests/runtime_tests.rs`.

**LLVM support**: Roadmap says `[x]` for LLVM -- VERIFIED. `ori_print` is a C-ABI runtime function. AOT test `test_aot_hello_world` in `spec.rs` line 318 tests `print(msg: "Hello AOT!")`.

**Verdict**: **VERIFIED**

### `compare(a, b)` -- `[x]` (2026-02-10)

**Implementation**: Implemented as Ori function in `library/std/prelude.ori` line 357: `pub @compare (a: int, b: int) -> Ordering = { ... }`. Only for `int` -- NOT the generic `compare<T: Comparable>` specified in the spec.

**Test coverage**: 58 tests in `tests/spec/traits/core/comparable.ori` for `.compare(other:)` method. The `compare()` free function is tested via the `Comparable` trait tests.

**LLVM support**: Roadmap says `[x]` -- comparison operators work in LLVM via inline IR. The free function `compare` works transitively since it's an Ori function that uses comparison operators.

**Spec discrepancy**: Spec says `compare<T: Comparable>(left: T, right: T) -> Ordering` (generic). Implementation is `compare(a: int, b: int)` (int-only). The roadmap should note this gap.

**Verdict**: **WEAK** -- works for `int` only, not the generic version from spec.

### `min(a, b)`, `max(a, b)` -- `[x]` (2026-02-10)

**Implementation**: Ori functions in `library/std/prelude.ori` lines 362-365. Int-only, not generic.

**Test coverage**: `tests/spec/traits/core/comparable.ori` lines 174-206 -- dedicated tests for `min()` and `max()` with int values including equal values and negatives.

**Spec discrepancy**: Same as `compare` -- spec says generic `min<T: Comparable>`, implementation is `int`-only.

**Verdict**: **VERIFIED** for int. **WEAK** for spec compliance (not generic).

### `panic(msg)` -- `[x]` (2026-02-10)

**Implementation**: `FunctionExpKind::Panic` in `compiler/ori_eval/src/interpreter/can_eval/function_exp.rs` line 183.

**Test coverage**: Used extensively in `#fail` test attributes and `assert_panics` calls. Dedicated Never type tests in `tests/spec/types/never.ori` (20+ tests using `panic`). LLVM runtime: `ori_panic` and `ori_panic_cstr` in `ori_rt`.

**LLVM support**: Roadmap says `[x]` -- VERIFIED. 33 AOT tests pass for panic-related functionality including panic handler, re-entrancy protection, assertion failures.

**AOT tests**: 9 dedicated panic tests in `compiler/ori_llvm/tests/aot/panic.rs` covering default behavior, handler invocation, message passing, re-entrancy, and assertion patterns.

**Verdict**: **VERIFIED**

---

## 7A.4 Float NaN Behavior

All items are `[ ]`. Confirmed NOT STARTED.

**FINDING**: Current behavior follows IEEE 754 (NaN == NaN returns false, NaN != NaN returns true, no panic). Tests in `tests/spec/expressions/operators_comparison.ori` lines 165-184 explicitly assert IEEE 754 behavior. When 7A.4 is implemented (NaN comparisons panic), these tests will need to be changed.

**Status**: NOT STARTED (confirmed). Existing tests contradict the planned behavior.

---

## 7A.5 Developer Functions

### `todo()` / `todo(reason:)`

**Implementation**: `FunctionExpKind::Todo` in `function_exp.rs` lines 187-196. Returns Never, panics with "not yet implemented" or "not yet implemented: {reason}".

**Test coverage**: `tests/spec/types/never.ori` lines 76-89 and 179-185 test `todo()` and `todo(reason:)` as Never-producing expressions. Tests verify coercion to int and str types.

**STALE FINDING**: Roadmap marks as `[ ]` but `todo()` IS implemented and tested. The implementation matches spec behavior.

**Verdict**: **STALE** -- should be `[x]`. Implementation exists and passes tests.

### `unreachable()` / `unreachable(reason:)`

**Implementation**: `FunctionExpKind::Unreachable` in `function_exp.rs` line 199. Returns Never.

**Test coverage**: `tests/spec/types/never.ori` lines 92-105 test both forms as Never-producing expressions.

**STALE FINDING**: Roadmap marks as `[ ]` but `unreachable()` IS implemented and tested.

**Verdict**: **STALE** -- should be `[x]`.

### `dbg(value:)` / `dbg(value:, label:)`

**Implementation**: NOT IMPLEMENTED. No `FunctionExpKind::Dbg` variant exists. No `Dbg` in `ori_ir`.

**Status**: NOT STARTED (confirmed).

### Compile-time location capture

**Status**: NOT STARTED (confirmed). No compile-time location capture infrastructure exists.

---

## 7A.6 Additional Built-in Functions

### `repeat` Function

**Implementation**: `function_val_repeat` in `compiler/ori_eval/src/function_val.rs` line 115. Registered in prelude. Creates `IteratorValue::from_repeat()`.

**Test coverage**: `tests/spec/traits/iterator/infinite.ori` -- dedicated test file with tests for repeat with int, str, bool, take(0), chaining with adapters.

**Type signature**: `compiler/ori_types/src/infer/expr/identifiers.rs` line 80 -- registered type.

**STALE FINDING**: Roadmap marks all repeat items as `[ ]` but repeat IS implemented in the evaluator and has spec tests. Missing: LLVM codegen, Clone requirement enforcement.

**Verdict**: **STALE** -- repeat is partially implemented (eval works, tests exist). LLVM codegen and Clone enforcement are not done.

### `PanicInfo` Type

**LLVM only**: `PanicInfo` struct is constructed in LLVM IR in `compiler/ori_llvm/src/codegen/function_compiler/panic_trampoline.rs`. AOT tests in `panic.rs` construct it manually in test source code. No interpreter support for the PanicInfo type.

**Status**: Partially implemented (LLVM only, not interpreter).

### `@panic` Handler

**STALE FINDING**: Multiple items marked `[ ]` are actually implemented in LLVM:
- `@panic` recognition: IMPLEMENTED in `panic_trampoline.rs` and `entry_point.rs`
- Signature validation: IMPLEMENTED (checks for PanicInfo param)
- Runtime panic hook: IMPLEMENTED (`ori_register_panic_handler` in `ori_rt`)
- Re-panic detection: IMPLEMENTED and tested (`test_panic_handler_re_entrancy`)
- Default handler: IMPLEMENTED (`test_panic_default_nonzero_exit`)
- Exit with non-zero code: IMPLEMENTED and tested

**Missing**: Interpreter support, `print()` stderr redirection in handler, concurrent panic handling, multiple `@panic` error detection.

**Verdict**: **STALE** -- multiple `[ ]` items should be partially checked for LLVM.

### `compile_error(msg:)`

**Status**: NOT STARTED (confirmed). Not implemented in any compiler phase.

---

## 7A.7 Resource Management

### `drop_early`

**Status**: NOT STARTED (confirmed). No implementation in compiler.

---

## 7A.8 Compile-Time File Embedding

### `embed(path)` and `has_embed(path)`

**Status**: NOT STARTED (confirmed). No implementation in compiler.

---

## 7A.9 Char and Byte Classification Methods

### Char Methods (Partial Implementation Found)

**Registry**: `compiler/ori_registry/src/defs/char.rs` defines: `is_alpha`, `is_ascii`, `is_digit`, `is_lowercase`, `is_uppercase`, `is_whitespace` (6 methods).

**Evaluator**: `compiler/ori_eval/src/methods/variants.rs` implements all 6 char methods.

**Missing from registry/eval (per roadmap spec)**:
- `is_alphabetic` (roadmap) -- partially covered by `is_alpha` (different name)
- `is_alphanumeric`, `is_control` (Unicode methods)
- `is_ascii_alphabetic`, `is_ascii_digit`, `is_ascii_alphanumeric`, `is_ascii_whitespace`, `is_ascii_uppercase`, `is_ascii_lowercase`, `is_ascii_hex_digit`, `is_ascii_punctuation`, `is_ascii_control` (ASCII methods)
- `to_ascii_uppercase`, `to_ascii_lowercase`, `to_digit` (conversion methods)
- Unicode lookup tables

**Test coverage**: ZERO Ori spec tests for char classification. Rust evaluator tests in `compiler/ori_eval/src/methods/tests.rs` verify method names are registered.

**STALE FINDING**: Roadmap marks all char methods as `[ ]` but 6 methods ARE implemented (eval + registry). The naming differs from spec (`is_alpha` vs `is_alphabetic`).

### Byte Methods (Partial Implementation Found)

**Registry**: `compiler/ori_registry/src/defs/byte.rs` defines: `is_ascii`, `is_ascii_alpha`, `is_ascii_digit`, `is_ascii_whitespace` (4 methods).

**Evaluator**: `compiler/ori_eval/src/methods/variants.rs` implements all 4 byte methods.

**Missing**: 6+ full byte methods, 7 short aliases, conversion methods (`to_ascii_uppercase`, `to_ascii_lowercase`, `to_digit`).

**Test coverage**: ZERO Ori spec tests. Rust evaluator tests cover method name registration only.

**STALE FINDING**: Roadmap marks all as `[ ]` but 4 methods ARE implemented.

---

## 7A.10 Byte-Level String Access

### str Methods (Partial Implementation Found)

**Registry**: `compiler/ori_registry/src/defs/str.rs` defines: `as_bytes`, `byte_len`, `to_bytes`, `from_utf8`, `from_utf8_unchecked`.

**Evaluator**: `compiler/ori_eval/src/methods/collections.rs` implements:
- `as_bytes` / `to_bytes` / `bytes` -- converts str to `[byte]` (line 174)
- `byte_len` -- returns byte length (line 180)
- `from_utf8` / `from_utf8_unchecked` -- converts `[byte]` to str (line 220)

**Test coverage**: Only `byte_len` tested indirectly in `tests/spec/types/primitives.ori` line 664 (`.len()` returns byte count). No dedicated spec tests for `as_bytes`, `to_bytes`, `from_utf8`, `from_utf8_unchecked`.

**STALE FINDING**: Roadmap marks all as `[ ]` but ALL 5 methods are implemented in the evaluator. Missing: dedicated spec tests, LLVM codegen, `as_bytes()` seamless slicing behavior.

---

## 7A.11 Section Completion Checklist

All items are `[ ]`. This section is clearly not complete.

---

## Bugs Found

### BUG-7A-01: Spec divergence on NaN comparison behavior
- **Location**: `tests/spec/expressions/operators_comparison.ori` lines 165-184
- **Issue**: Tests assert IEEE 754 NaN behavior (NaN == NaN is false, no panic). The roadmap section 7A.4 plans NaN comparisons to panic. Current tests will conflict with planned behavior.
- **Severity**: Informational (planned change, not a bug in current behavior)

### BUG-7A-02: `assert_eq` missing Debug bound per spec
- **Location**: `library/std/testing.ori` line 10
- **Issue**: Spec says `assert_eq<T: Eq + Debug>(actual: T, expected: T)` but implementation is `assert_eq<T: Eq>`. Uses `str(actual)` instead of `actual.debug()` for error messages.
- **Severity**: Minor spec divergence

### BUG-7A-03: `compare`/`min`/`max` are int-only, not generic
- **Location**: `library/std/prelude.ori` lines 357-365
- **Issue**: Spec defines `compare<T: Comparable>`, `min<T: Comparable>`, `max<T: Comparable>` but implementation only handles `int`. Users cannot call `min("apple", "banana")`.
- **Severity**: Medium -- spec promises generic behavior, implementation is int-only

### BUG-7A-04: `assert_some`/`assert_none`/`assert_ok`/`assert_err` return void, not inner value
- **Location**: `library/std/testing.ori` lines 19-32
- **Issue**: Spec says `assert_some<T>(option: Option<T>) -> T` (returns inner value on success) and `assert_ok<T, E>(result: Result<T, E>) -> T`. Implementation returns `void` for all four. `assert_err` should return `E`.
- **Severity**: Medium -- spec promises return values, implementation is void-only

---

## Stale Items Summary

The following items are marked `[ ]` in the roadmap but have partial or full implementations:

1. **todo() / unreachable()** -- Fully implemented as `FunctionExpKind` (eval + typeck). Tested in `tests/spec/types/never.ori`.
2. **repeat()** -- Implemented in eval, registered in typeck, has spec tests.
3. **@panic handler** -- LLVM support exists (trampoline, re-entrancy, PanicInfo construction). 9 AOT tests pass.
4. **PanicInfo type** -- Constructed in LLVM IR for panic handler.
5. **assert_some/none/ok/err** -- Implemented in `library/std/testing.ori` but never tested.
6. **assert_panics/assert_panics_with** -- Implemented and used in 4 test files. Not listed in roadmap at all.
7. **Char classification** -- 6 methods implemented (eval + registry). No spec tests.
8. **Byte classification** -- 4 methods implemented (eval + registry). No spec tests.
9. **str.as_bytes/to_bytes/byte_len/from_utf8/from_utf8_unchecked** -- All 5 implemented in eval. Minimal spec tests.
10. **LLVM assert/assert_eq/assert_ne** -- Work transitively through panic LLVM support.

---

## Detailed Item-by-Item Verification

### `[x]` Items

| Item | Verdict | Notes |
|------|---------|-------|
| assert(cond:) | VERIFIED | 310+ usages, all types |
| assert_eq(actual:, expected:) | VERIFIED | 78+ usages, spec divergence on Debug bound |
| assert_ne(actual:, unexpected:) | WEAK | Only 3 usages, only int+str tested |
| print(x) | VERIFIED | FunctionExpKind::Print + LLVM _ori_print |
| compare(a, b) | WEAK | Int-only, not generic per spec |
| min(a, b) / max(a, b) | WEAK | Int-only, tested with int edge cases |
| panic(msg) | VERIFIED | FunctionExpKind::Panic + LLVM _ori_panic, 33 AOT tests |
| print LLVM Support | VERIFIED | _ori_print in runtime, AOT test passes |
| compare LLVM Support | VERIFIED | Works transitively (Ori function using operators) |
| panic LLVM Support | VERIFIED | _ori_panic + panic_trampoline, 9 dedicated AOT tests |

### `[ ]` Items (Not Started, Confirmed)

- All of 7A.1 (As/TryAs traits, as/as? syntax, standard impls, float truncation methods) -- NOT STARTED
- assert_some, assert_none, assert_ok, assert_err -- IMPLEMENTED in stdlib, need tests (STALE)
- 7A.4 NaN behavior -- NOT STARTED
- dbg() -- NOT STARTED
- Compile-time location capture -- NOT STARTED
- compile_error(msg:) -- NOT STARTED
- drop_early -- NOT STARTED
- embed/has_embed -- NOT STARTED
- Most char/byte classification methods -- NOT STARTED (6 char + 4 byte partially done)
- Conversion methods (to_digit, to_ascii_uppercase/lowercase) -- NOT STARTED
- All LLVM-specific test files mentioned (assertion_tests.rs, comparison_tests.rs, etc.) -- DO NOT EXIST
