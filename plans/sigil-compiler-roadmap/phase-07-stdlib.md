# Phase 7: Standard Library

**Goal**: Core stdlib modules (moved after Capabilities to allow using capability traits)

> **SPEC**: `spec/11-built-in-functions.md`
> **DESIGN**: `modules/` documentation
> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md` — Integer overflow behavior

---

## 7.1 Type Conversions

- [ ] **Implement**: `int(x)` — spec/11-built-in-functions.md § int
  - [ ] **Rust Tests**: `oric/src/eval/function_val.rs` — int conversion tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`

- [ ] **Implement**: `float(x)` — spec/11-built-in-functions.md § float
  - [ ] **Rust Tests**: `oric/src/eval/function_val.rs` — float conversion tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`

- [ ] **Implement**: `str(x)` — spec/11-built-in-functions.md § str
  - [ ] **Rust Tests**: `oric/src/eval/function_val.rs` — str conversion tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`

- [ ] **Implement**: `byte(x)` — spec/11-built-in-functions.md § byte
  - [ ] **Rust Tests**: `oric/src/eval/function_val.rs` — byte conversion tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`

---

## 7.2 Collection Functions

> **NOTE**: `len` and `is_empty` are being moved from free functions to methods on collections (see 7.12).
> The free function forms are deprecated in favor of `.len()` and `.is_empty()` methods.
> Keep backward compatibility during transition, then remove free functions.

- [ ] **Implement**: `len(x)` — spec/11-built-in-functions.md § len (deprecated, use `.len()`)
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — len function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/collections.ori`

- [ ] **Implement**: `is_empty(x)` — spec/11-built-in-functions.md § is_empty (deprecated, use `.is_empty()`)
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — is_empty function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/collections.ori`

---

## 7.3 Option Functions

- [ ] **Implement**: `is_some(x)` — spec/11-built-in-functions.md § is_some
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `is_none(x)` — spec/11-built-in-functions.md § is_none
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `Option.map` — spec/11-built-in-functions.md § Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `Option.unwrap_or` — spec/11-built-in-functions.md § Option.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `Option.ok_or` — spec/11-built-in-functions.md § Option.ok_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.ok_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `Option.and_then` — spec/11-built-in-functions.md § Option.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

- [ ] **Implement**: `Option.filter` — spec/11-built-in-functions.md § Option.filter
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`

---

## 7.4 Result Functions

- [ ] **Implement**: `is_ok(x)` — spec/11-built-in-functions.md § is_ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `is_err(x)` — spec/11-built-in-functions.md § is_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.map` — spec/11-built-in-functions.md § Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.map_err` — spec/11-built-in-functions.md § Result.map_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.unwrap_or` — spec/11-built-in-functions.md § Result.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.ok` — spec/11-built-in-functions.md § Result.ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.err` — spec/11-built-in-functions.md § Result.err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

- [ ] **Implement**: `Result.and_then` — spec/11-built-in-functions.md § Result.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`

---

## 7.5 Assertions

- [ ] **Implement**: `assert(cond)` — spec/11-built-in-functions.md § assert
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_eq(a, b)` — spec/11-built-in-functions.md § assert_eq
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_eq tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_ne(a, b)` — spec/11-built-in-functions.md § assert_ne
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_ne tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_some(x)` — spec/11-built-in-functions.md § assert_some
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_none(x)` — spec/11-built-in-functions.md § assert_none
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_ok(x)` — spec/11-built-in-functions.md § assert_ok
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

- [ ] **Implement**: `assert_err(x)` — spec/11-built-in-functions.md § assert_err
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`

---

## 7.6 I/O and Other

- [ ] **Implement**: `print(x)` — spec/11-built-in-functions.md § print
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — print tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/io.ori`

- [ ] **Implement**: `compare(a, b)` — spec/11-built-in-functions.md § compare
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — compare tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/compare.ori`

- [ ] **Implement**: `min(a, b)`, `max(a, b)` — spec/11-built-in-functions.md § min/max
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — min/max tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/minmax.ori`

- [ ] **Implement**: `panic(msg)` — spec/11-built-in-functions.md § panic
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — panic tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/panic.ori`

---

## 7.7 std.validate Module

- [ ] **Implement**: `validate(rules, value)` — modules/std.validate/index.md § validate
  - [ ] **Rust Tests**: `library/std/validate.rs` — validate function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/validate.ori`

> **Syntax**: `use std.validate { validate }`
>
> ```ori
> validate(rules: [(cond, "error"), ...], value: val)
> ```
>
> Returns `Result<T, [str]>` — all rules checked, errors accumulated.

---

## 7.8 Collection Methods on `[T]`

> **Design Principle**: Lean core, rich libraries. Data transformation is stdlib, not compiler patterns.

- [ ] **Implement**: `[T].map(f: T -> U) -> [U]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].filter(f: T -> bool) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list fold tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].find(f: T -> bool) -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list find tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].any(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list any tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].all(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list all tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].first() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list first tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].last() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list last tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].take(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list take tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].skip(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list skip tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].reverse() -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list reverse tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].sort() -> [T]` where `T: Comparable` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list sort tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].contains(value: T) -> bool` where `T: Eq` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list contains tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list push tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].concat(other: [T]) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list concat tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

---

## 7.9 Range Methods

- [ ] **Implement**: `Range.map(f: T -> U) -> [U]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`

- [ ] **Implement**: `Range.filter(f: T -> bool) -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`

- [ ] **Implement**: `Range.fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.fold tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`

- [ ] **Implement**: `Range.collect() -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.collect tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`

- [ ] **Implement**: `Range.contains(value: T) -> bool` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.contains tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`

---

## 7.10 std.resilience Module

- [ ] **Implement**: `retry(operation, attempts, backoff)` — modules/std.resilience/index.md § retry
  - [ ] **Rust Tests**: `library/std/resilience.rs` — retry function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`

- [ ] **Implement**: `exponential(base: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § exponential
  - [ ] **Rust Tests**: `library/std/resilience.rs` — exponential backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`

- [ ] **Implement**: `linear(delay: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § linear
  - [ ] **Rust Tests**: `library/std/resilience.rs` — linear backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`

---

## 7.11 std.math Module — Overflow-Safe Arithmetic

> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md`

Default integer arithmetic panics on overflow. These functions provide explicit alternatives.

### 7.11.1 Saturating Arithmetic

Clamps result to type bounds on overflow:

- [ ] **Implement**: `saturating_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`

- [ ] **Implement**: `saturating_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`

- [ ] **Implement**: `saturating_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`

- [ ] **Implement**: Byte variants (`saturating_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte saturating tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`

### 7.11.2 Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

- [ ] **Implement**: `wrapping_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`

- [ ] **Implement**: `wrapping_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`

- [ ] **Implement**: `wrapping_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`

- [ ] **Implement**: Byte variants (`wrapping_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte wrapping tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`

### 7.11.3 Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

- [ ] **Implement**: `checked_add(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`

- [ ] **Implement**: `checked_sub(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`

- [ ] **Implement**: `checked_mul(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`

- [ ] **Implement**: Byte variants (`checked_add(a: byte, b: byte) -> Option<byte>`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte checked tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`

### 7.11.4 Type Bounds Constants

- [ ] **Implement**: `int.min`, `int.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — type constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`

- [ ] **Implement**: `byte.min`, `byte.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — byte constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`

### 7.11.5 Default Overflow Behavior

- [ ] **Implement**: Arithmetic operators panic on overflow
  - [ ] Addition, subtraction, multiplication emit overflow checks
  - [ ] Division by zero and `int.min / -1` panic
  - [ ] Consistent behavior in debug and release builds
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — overflow panic tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/overflow_panic.ori`

- [ ] **Implement**: Compile-time constant overflow is a compile error
  - [ ] `$big = int.max + 1` → ERROR: constant overflow
  - [ ] **Rust Tests**: `oric/src/typeck/checker/const_eval.rs` — constant overflow tests
  - [ ] **Ori Tests**: `tests/compile-fail/constant_overflow.ori`

---

## 7.12 Collection Methods (len, is_empty)

> Move from free functions to methods on collections.

- [ ] **Implement**: `[T].len() -> int` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `[T].is_empty() -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`

- [ ] **Implement**: `{K: V}.len() -> int` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — map len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/map_methods.ori`

- [ ] **Implement**: `{K: V}.is_empty() -> bool` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — map is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/map_methods.ori`

- [ ] **Implement**: `str.len() -> int` — modules/prelude.md § str
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — str len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/str_methods.ori`

- [ ] **Implement**: `str.is_empty() -> bool` — modules/prelude.md § str
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — str is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/str_methods.ori`

- [ ] **Implement**: `Set<T>.len() -> int` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — set len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/set_methods.ori`

- [ ] **Implement**: `Set<T>.is_empty() -> bool` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — set is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/set_methods.ori`

---

## 7.13 Comparable Methods (min, max, compare)

> Move from free functions to methods on Comparable trait.

- [ ] **Implement**: `T.min(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — min method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`

- [ ] **Implement**: `T.max(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — max method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`

- [ ] **Implement**: `T.compare(other: T) -> Ordering` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — compare method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`

---

## 7.14 std.testing Module

> Move testing assertions from built-ins to std.testing.

- [ ] **Implement**: `assert_eq(actual, expected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_eq tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_ne(actual, unexpected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ne tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_some(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_none(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_ok(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_err(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_panics(expr)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

- [ ] **Implement**: `assert_panics_with(expr, message)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics_with tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`

---

## 7.15 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: Basic programs can use stdlib
