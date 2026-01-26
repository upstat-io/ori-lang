# Phase 7: Standard Library

**Goal**: Core stdlib modules (moved after Capabilities to allow using capability traits)

> **SPEC**: `spec/11-built-in-functions.md`
> **DESIGN**: `modules/` documentation
> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md` — Integer overflow behavior

---

## 7.1 Type Conversions

- [ ] **Implement**: `int(x)` — spec/11-built-in-functions.md § int
  - [ ] **Rust Tests**: `sigilc/src/eval/function_val.rs` — int conversion tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/conversions.si`

- [ ] **Implement**: `float(x)` — spec/11-built-in-functions.md § float
  - [ ] **Rust Tests**: `sigilc/src/eval/function_val.rs` — float conversion tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/conversions.si`

- [ ] **Implement**: `str(x)` — spec/11-built-in-functions.md § str
  - [ ] **Rust Tests**: `sigilc/src/eval/function_val.rs` — str conversion tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/conversions.si`

- [ ] **Implement**: `byte(x)` — spec/11-built-in-functions.md § byte
  - [ ] **Rust Tests**: `sigilc/src/eval/function_val.rs` — byte conversion tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/conversions.si`

---

## 7.2 Collection Functions

> **NOTE**: `len` and `is_empty` are being moved from free functions to methods on collections (see 7.12).
> The free function forms are deprecated in favor of `.len()` and `.is_empty()` methods.
> Keep backward compatibility during transition, then remove free functions.

- [ ] **Implement**: `len(x)` — spec/11-built-in-functions.md § len (deprecated, use `.len()`)
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — len function tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/collections.si`

- [ ] **Implement**: `is_empty(x)` — spec/11-built-in-functions.md § is_empty (deprecated, use `.is_empty()`)
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — is_empty function tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/collections.si`

---

## 7.3 Option Functions

- [ ] **Implement**: `is_some(x)` — spec/11-built-in-functions.md § is_some
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — is_some tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `is_none(x)` — spec/11-built-in-functions.md § is_none
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — is_none tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `Option.map` — spec/11-built-in-functions.md § Option.map
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Option.map tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `Option.unwrap_or` — spec/11-built-in-functions.md § Option.unwrap_or
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Option.unwrap_or tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `Option.ok_or` — spec/11-built-in-functions.md § Option.ok_or
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Option.ok_or tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `Option.and_then` — spec/11-built-in-functions.md § Option.and_then
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Option.and_then tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

- [ ] **Implement**: `Option.filter` — spec/11-built-in-functions.md § Option.filter
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Option.filter tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/option.si`

---

## 7.4 Result Functions

- [ ] **Implement**: `is_ok(x)` — spec/11-built-in-functions.md § is_ok
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — is_ok tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `is_err(x)` — spec/11-built-in-functions.md § is_err
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — is_err tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.map` — spec/11-built-in-functions.md § Result.map
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.map tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.map_err` — spec/11-built-in-functions.md § Result.map_err
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.map_err tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.unwrap_or` — spec/11-built-in-functions.md § Result.unwrap_or
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.unwrap_or tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.ok` — spec/11-built-in-functions.md § Result.ok
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.ok tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.err` — spec/11-built-in-functions.md § Result.err
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.err tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

- [ ] **Implement**: `Result.and_then` — spec/11-built-in-functions.md § Result.and_then
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.and_then tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/result.si`

---

## 7.5 Assertions

- [ ] **Implement**: `assert(cond)` — spec/11-built-in-functions.md § assert
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_eq(a, b)` — spec/11-built-in-functions.md § assert_eq
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_eq tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_ne(a, b)` — spec/11-built-in-functions.md § assert_ne
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_ne tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_some(x)` — spec/11-built-in-functions.md § assert_some
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_some tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_none(x)` — spec/11-built-in-functions.md § assert_none
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_none tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_ok(x)` — spec/11-built-in-functions.md § assert_ok
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_ok tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

- [ ] **Implement**: `assert_err(x)` — spec/11-built-in-functions.md § assert_err
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — assert_err tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/assertions.si`

---

## 7.6 I/O and Other

- [ ] **Implement**: `print(x)` — spec/11-built-in-functions.md § print
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — print tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/io.si`

- [ ] **Implement**: `compare(a, b)` — spec/11-built-in-functions.md § compare
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — compare tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/compare.si`

- [ ] **Implement**: `min(a, b)`, `max(a, b)` — spec/11-built-in-functions.md § min/max
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — min/max tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/minmax.si`

- [ ] **Implement**: `panic(msg)` — spec/11-built-in-functions.md § panic
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — panic tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/panic.si`

---

## 7.7 std.validate Module

- [ ] **Implement**: `validate(rules, value)` — modules/std.validate/index.md § validate
  - [ ] **Rust Tests**: `library/std/validate.rs` — validate function tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/validate.si`

> **Syntax**: `use std.validate { validate }`
>
> ```sigil
> validate(rules: [(cond, "error"), ...], value: val)
> ```
>
> Returns `Result<T, [str]>` — all rules checked, errors accumulated.

---

## 7.8 Collection Methods on `[T]`

> **Design Principle**: Lean core, rich libraries. Data transformation is stdlib, not compiler patterns.

- [ ] **Implement**: `[T].map(f: T -> U) -> [U]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list map tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].filter(f: T -> bool) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list filter tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list fold tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].find(f: T -> bool) -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list find tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].any(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list any tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].all(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list all tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].first() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list first tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].last() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list last tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].take(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list take tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].skip(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list skip tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].reverse() -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list reverse tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].sort() -> [T]` where `T: Comparable` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list sort tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].contains(value: T) -> bool` where `T: Eq` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list contains tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list push tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].concat(other: [T]) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list concat tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

---

## 7.9 Range Methods

- [ ] **Implement**: `Range.map(f: T -> U) -> [U]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Range.map tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/range_methods.si`

- [ ] **Implement**: `Range.filter(f: T -> bool) -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Range.filter tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/range_methods.si`

- [ ] **Implement**: `Range.fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Range.fold tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/range_methods.si`

- [ ] **Implement**: `Range.collect() -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Range.collect tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/range_methods.si`

- [ ] **Implement**: `Range.contains(value: T) -> bool` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Range.contains tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/range_methods.si`

---

## 7.10 std.resilience Module

- [ ] **Implement**: `retry(operation, attempts, backoff)` — modules/std.resilience/index.md § retry
  - [ ] **Rust Tests**: `library/std/resilience.rs` — retry function tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/resilience.si`

- [ ] **Implement**: `exponential(base: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § exponential
  - [ ] **Rust Tests**: `library/std/resilience.rs` — exponential backoff tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/resilience.si`

- [ ] **Implement**: `linear(delay: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § linear
  - [ ] **Rust Tests**: `library/std/resilience.rs` — linear backoff tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/resilience.si`

---

## 7.11 std.math Module — Overflow-Safe Arithmetic

> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md`

Default integer arithmetic panics on overflow. These functions provide explicit alternatives.

### 7.11.1 Saturating Arithmetic

Clamps result to type bounds on overflow:

- [ ] **Implement**: `saturating_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_add tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_saturating.si`

- [ ] **Implement**: `saturating_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_sub tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_saturating.si`

- [ ] **Implement**: `saturating_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_mul tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_saturating.si`

- [ ] **Implement**: Byte variants (`saturating_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte saturating tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_saturating.si`

### 7.11.2 Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

- [ ] **Implement**: `wrapping_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_add tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_wrapping.si`

- [ ] **Implement**: `wrapping_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_sub tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_wrapping.si`

- [ ] **Implement**: `wrapping_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_mul tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_wrapping.si`

- [ ] **Implement**: Byte variants (`wrapping_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte wrapping tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_wrapping.si`

### 7.11.3 Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

- [ ] **Implement**: `checked_add(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_add tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_checked.si`

- [ ] **Implement**: `checked_sub(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_sub tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_checked.si`

- [ ] **Implement**: `checked_mul(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_mul tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_checked.si`

- [ ] **Implement**: Byte variants (`checked_add(a: byte, b: byte) -> Option<byte>`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte checked tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/math_checked.si`

### 7.11.4 Type Bounds Constants

- [ ] **Implement**: `int.min`, `int.max` constants
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — type constants tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/type_bounds.si`

- [ ] **Implement**: `byte.min`, `byte.max` constants
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — byte constants tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/type_bounds.si`

### 7.11.5 Default Overflow Behavior

- [ ] **Implement**: Arithmetic operators panic on overflow
  - [ ] Addition, subtraction, multiplication emit overflow checks
  - [ ] Division by zero and `int.min / -1` panic
  - [ ] Consistent behavior in debug and release builds
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/binary.rs` — overflow panic tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/overflow_panic.si`

- [ ] **Implement**: Compile-time constant overflow is a compile error
  - [ ] `$big = int.max + 1` → ERROR: constant overflow
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/const_eval.rs` — constant overflow tests
  - [ ] **Sigil Tests**: `tests/compile-fail/constant_overflow.si`

---

## 7.12 Collection Methods (len, is_empty)

> Move from free functions to methods on collections.

- [ ] **Implement**: `[T].len() -> int` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list len tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `[T].is_empty() -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — list is_empty tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/list_methods.si`

- [ ] **Implement**: `{K: V}.len() -> int` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — map len tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/map_methods.si`

- [ ] **Implement**: `{K: V}.is_empty() -> bool` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — map is_empty tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/map_methods.si`

- [ ] **Implement**: `str.len() -> int` — modules/prelude.md § str
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — str len tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/str_methods.si`

- [ ] **Implement**: `str.is_empty() -> bool` — modules/prelude.md § str
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — str is_empty tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/str_methods.si`

- [ ] **Implement**: `Set<T>.len() -> int` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — set len tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/set_methods.si`

- [ ] **Implement**: `Set<T>.is_empty() -> bool` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — set is_empty tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/set_methods.si`

---

## 7.13 Comparable Methods (min, max, compare)

> Move from free functions to methods on Comparable trait.

- [ ] **Implement**: `T.min(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — min method tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/comparable.si`

- [ ] **Implement**: `T.max(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — max method tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/comparable.si`

- [ ] **Implement**: `T.compare(other: T) -> Ordering` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — compare method tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/comparable.si`

---

## 7.14 std.testing Module

> Move testing assertions from built-ins to std.testing.

- [ ] **Implement**: `assert_eq(actual, expected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_eq tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_ne(actual, unexpected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ne tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_some(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_some tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_none(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_none tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_ok(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ok tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_err(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_err tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_panics(expr)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

- [ ] **Implement**: `assert_panics_with(expr, message)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics_with tests
  - [ ] **Sigil Tests**: `tests/spec/stdlib/testing.si`

---

## 7.15 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: Basic programs can use stdlib
