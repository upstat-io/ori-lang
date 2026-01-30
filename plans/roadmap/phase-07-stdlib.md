# Phase 7: Standard Library

**Goal**: Core stdlib modules (moved after Capabilities to allow using capability traits)

> **SPEC**: `spec/11-built-in-functions.md`
> **DESIGN**: `modules/` documentation
> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md` — Integer overflow behavior

---

## 7.1 Type Conversions

> **PROPOSAL**: `proposals/drafts/as-conversion-proposal.md`
>
> Type conversions use `as`/`as?` syntax instead of `int()`, `float()`, etc.
> This removes the special-case exception for positional arguments.

- [ ] **Implement**: `As<T>` trait — infallible conversions
  - [ ] **Rust Tests**: `oric/src/typeck/traits/as_trait.rs` — As trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for As trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As trait codegen

- [ ] **Implement**: `TryAs<T>` trait — fallible conversions returning `Option<T>`
  - [ ] **Rust Tests**: `oric/src/typeck/traits/try_as_trait.rs` — TryAs trait tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for TryAs trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs trait codegen

- [ ] **Implement**: `x as T` syntax — desugars to `As<T>.as(self: x)`
  - [ ] **Rust Tests**: `oric/src/eval/as_conversion.rs` — as syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as syntax codegen

- [ ] **Implement**: `x as? T` syntax — desugars to `TryAs<T>.try_as(self: x)`
  - [ ] **Rust Tests**: `oric/src/eval/as_conversion.rs` — as? syntax tests
  - [ ] **Ori Tests**: `tests/spec/expressions/as_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for as? syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as? syntax codegen

- [ ] **Implement**: Standard `As` implementations
  - `impl As<float> for int` — widening (infallible)
  - `impl As<str> for int` — formatting (infallible)
  - `impl As<str> for float` — formatting (infallible)
  - `impl As<str> for bool` — "true"/"false" (infallible)
  - `impl As<int> for char` — codepoint (infallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard As implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — As implementations codegen

- [ ] **Implement**: Standard `TryAs` implementations
  - `impl TryAs<int> for str` — parsing (fallible)
  - `impl TryAs<float> for str` — parsing (fallible)
  - `impl TryAs<byte> for int` — range check (fallible)
  - `impl TryAs<char> for int` — valid codepoint check (fallible)
  - [ ] **Ori Tests**: `tests/spec/stdlib/try_as_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard TryAs implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — TryAs implementations codegen

- [ ] **Implement**: Compile-time enforcement — `as` only for infallible conversions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_conversion.rs` — enforcement tests
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible.ori`
  - [ ] **LLVM Support**: LLVM codegen for as conversion enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — as enforcement codegen

- [ ] **Implement**: Float truncation methods (not `as`)
  - `float.truncate() -> int` — toward zero
  - `float.round() -> int` — nearest
  - `float.floor() -> int` — toward negative infinity
  - `float.ceil() -> int` — toward positive infinity
  - [ ] **Ori Tests**: `tests/spec/stdlib/float_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for float truncation methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — float truncation codegen

- [ ] **Remove**: `int()`, `float()`, `str()`, `byte()` function syntax
  - These are replaced by `as`/`as?` syntax
  - No migration period needed if implementing fresh
  - [ ] **LLVM Support**: LLVM codegen removal of legacy conversion functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/conversion_tests.rs` — verify legacy functions removed

---

## 7.2 Collection Functions

> **NOTE**: `len` and `is_empty` are being moved from free functions to methods on collections (see 7.12).
> The free function forms are deprecated in favor of `.len()` and `.is_empty()` methods.
> Keep backward compatibility during transition, then remove free functions.

- [ ] **Implement**: `len(x)` — spec/11-built-in-functions.md § len (deprecated, use `.len()`)
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — len function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for len function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — len function codegen

- [ ] **Implement**: `is_empty(x)` — spec/11-built-in-functions.md § is_empty (deprecated, use `.is_empty()`)
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — is_empty function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_empty function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — is_empty function codegen

---

## 7.3 Option Functions

- [ ] **Implement**: `is_some(x)` — spec/11-built-in-functions.md § is_some
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_some codegen

- [ ] **Implement**: `is_none(x)` — spec/11-built-in-functions.md § is_none
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_none codegen

- [ ] **Implement**: `Option.map` — spec/11-built-in-functions.md § Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.map codegen

- [ ] **Implement**: `Option.unwrap_or` — spec/11-built-in-functions.md § Option.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.unwrap_or codegen

- [ ] **Implement**: `Option.ok_or` — spec/11-built-in-functions.md § Option.ok_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.ok_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.ok_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.ok_or codegen

- [ ] **Implement**: `Option.and_then` — spec/11-built-in-functions.md § Option.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.and_then codegen

- [ ] **Implement**: `Option.filter` — spec/11-built-in-functions.md § Option.filter
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.filter codegen

---

## 7.4 Result Functions

- [ ] **Implement**: `is_ok(x)` — spec/11-built-in-functions.md § is_ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_ok codegen

- [ ] **Implement**: `is_err(x)` — spec/11-built-in-functions.md § is_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_err codegen

- [ ] **Implement**: `Result.map` — spec/11-built-in-functions.md § Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map codegen

- [ ] **Implement**: `Result.map_err` — spec/11-built-in-functions.md § Result.map_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map_err codegen

- [ ] **Implement**: `Result.unwrap_or` — spec/11-built-in-functions.md § Result.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.unwrap_or codegen

- [ ] **Implement**: `Result.ok` — spec/11-built-in-functions.md § Result.ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.ok codegen

- [ ] **Implement**: `Result.err` — spec/11-built-in-functions.md § Result.err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.err codegen

- [ ] **Implement**: `Result.and_then` — spec/11-built-in-functions.md § Result.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.and_then codegen

---

## 7.5 Assertions

- [ ] **Implement**: `assert(cond)` — spec/11-built-in-functions.md § assert
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert codegen

- [ ] **Implement**: `assert_eq(a, b)` — spec/11-built-in-functions.md § assert_eq
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_eq tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_eq codegen

- [ ] **Implement**: `assert_ne(a, b)` — spec/11-built-in-functions.md § assert_ne
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_ne tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_ne
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_ne codegen

- [ ] **Implement**: `assert_some(x)` — spec/11-built-in-functions.md § assert_some
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_some codegen

- [ ] **Implement**: `assert_none(x)` — spec/11-built-in-functions.md § assert_none
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_none codegen

- [ ] **Implement**: `assert_ok(x)` — spec/11-built-in-functions.md § assert_ok
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_ok codegen

- [ ] **Implement**: `assert_err(x)` — spec/11-built-in-functions.md § assert_err
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — assert_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/assertions.ori`
  - [ ] **LLVM Support**: LLVM codegen for assert_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/assertion_tests.rs` — assert_err codegen

---

## 7.6 I/O and Other

- [ ] **Implement**: `print(x)` — spec/11-built-in-functions.md § print
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — print tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/io.ori`
  - [ ] **LLVM Support**: LLVM codegen for print
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/io_tests.rs` — print codegen

- [ ] **Implement**: `compare(a, b)` — spec/11-built-in-functions.md § compare
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — compare tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/compare.ori`
  - [ ] **LLVM Support**: LLVM codegen for compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — compare codegen

- [ ] **Implement**: `min(a, b)`, `max(a, b)` — spec/11-built-in-functions.md § min/max
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — min/max tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/minmax.ori`
  - [ ] **LLVM Support**: LLVM codegen for min/max
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — min/max codegen

- [ ] **Implement**: `panic(msg)` — spec/11-built-in-functions.md § panic
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — panic tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — panic codegen

---

## 7.7 std.validate Module

- [ ] **Implement**: `validate(rules, value)` — modules/std.validate/index.md § validate
  - [ ] **Rust Tests**: `library/std/validate.rs` — validate function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/validate.ori`
  - [ ] **LLVM Support**: LLVM codegen for validate
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/validate_tests.rs` — validate codegen

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
  - [ ] **LLVM Support**: LLVM codegen for list map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list map codegen

- [ ] **Implement**: `[T].filter(f: T -> bool) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list filter codegen

- [ ] **Implement**: `[T].fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list fold tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list fold
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list fold codegen

- [ ] **Implement**: `[T].find(f: T -> bool) -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list find tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list find
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list find codegen

- [ ] **Implement**: `[T].any(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list any tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list any
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list any codegen

- [ ] **Implement**: `[T].all(f: T -> bool) -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list all tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list all
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list all codegen

- [ ] **Implement**: `[T].first() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list first tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list first
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list first codegen

- [ ] **Implement**: `[T].last() -> Option<T>` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list last tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list last
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list last codegen

- [ ] **Implement**: `[T].take(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list take tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list take
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list take codegen

- [ ] **Implement**: `[T].skip(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list skip tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list skip
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list skip codegen

- [ ] **Implement**: `[T].reverse() -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list reverse tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list reverse
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list reverse codegen

- [ ] **Implement**: `[T].sort() -> [T]` where `T: Comparable` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list sort tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list sort
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list sort codegen

- [ ] **Implement**: `[T].contains(value: T) -> bool` where `T: Eq` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list contains tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list contains
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list contains codegen

- [ ] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list push tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list push
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list push codegen

- [ ] **Implement**: `[T].concat(other: [T]) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list concat tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list concat
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list concat codegen

---

## 7.9 Range Methods

- [ ] **Implement**: `Range.map(f: T -> U) -> [U]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.map codegen

- [ ] **Implement**: `Range.filter(f: T -> bool) -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.filter codegen

- [ ] **Implement**: `Range.fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.fold tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.fold
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.fold codegen

- [ ] **Implement**: `Range.collect() -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.collect tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.collect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.collect codegen

- [ ] **Implement**: `Range.contains(value: T) -> bool` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.contains tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.contains
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.contains codegen

---

## 7.10 std.resilience Module

- [ ] **Implement**: `retry(operation, attempts, backoff)` — modules/std.resilience/index.md § retry
  - [ ] **Rust Tests**: `library/std/resilience.rs` — retry function tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for retry
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — retry codegen

- [ ] **Implement**: `exponential(base: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § exponential
  - [ ] **Rust Tests**: `library/std/resilience.rs` — exponential backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for exponential backoff
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — exponential backoff codegen

- [ ] **Implement**: `linear(delay: Duration) -> BackoffStrategy` — modules/std.resilience/index.md § linear
  - [ ] **Rust Tests**: `library/std/resilience.rs` — linear backoff tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/resilience.ori`
  - [ ] **LLVM Support**: LLVM codegen for linear backoff
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/resilience_tests.rs` — linear backoff codegen

---

## 7.11 std.math Module — Overflow-Safe Arithmetic

> **PROPOSAL**: `proposals/approved/overflow-behavior-proposal.md`

Default integer arithmetic panics on overflow. These functions provide explicit alternatives.

### 7.11.1 Saturating Arithmetic

Clamps result to type bounds on overflow:

- [ ] **Implement**: `saturating_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_add codegen

- [ ] **Implement**: `saturating_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_sub codegen

- [ ] **Implement**: `saturating_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — saturating_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for saturating_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — saturating_mul codegen

- [ ] **Implement**: Byte variants (`saturating_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte saturating tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_saturating.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte saturating arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte saturating codegen

### 7.11.2 Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

- [ ] **Implement**: `wrapping_add(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_add codegen

- [ ] **Implement**: `wrapping_sub(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_sub codegen

- [ ] **Implement**: `wrapping_mul(a: int, b: int) -> int`
  - [ ] **Rust Tests**: `library/std/math.rs` — wrapping_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for wrapping_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — wrapping_mul codegen

- [ ] **Implement**: Byte variants (`wrapping_add(a: byte, b: byte) -> byte`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte wrapping tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_wrapping.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte wrapping arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte wrapping codegen

### 7.11.3 Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

- [ ] **Implement**: `checked_add(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_add tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_add
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_add codegen

- [ ] **Implement**: `checked_sub(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_sub tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_sub
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_sub codegen

- [ ] **Implement**: `checked_mul(a: int, b: int) -> Option<int>`
  - [ ] **Rust Tests**: `library/std/math.rs` — checked_mul tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for checked_mul
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — checked_mul codegen

- [ ] **Implement**: Byte variants (`checked_add(a: byte, b: byte) -> Option<byte>`, etc.)
  - [ ] **Rust Tests**: `library/std/math.rs` — byte checked tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/math_checked.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte checked arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte checked codegen

### 7.11.4 Type Bounds Constants

- [ ] **Implement**: `int.min`, `int.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — type constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`
  - [ ] **LLVM Support**: LLVM codegen for int.min/max constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — int constants codegen

- [ ] **Implement**: `byte.min`, `byte.max` constants
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — byte constants tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/type_bounds.ori`
  - [ ] **LLVM Support**: LLVM codegen for byte.min/max constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — byte constants codegen

### 7.11.5 Default Overflow Behavior

- [ ] **Implement**: Arithmetic operators panic on overflow
  - [ ] Addition, subtraction, multiplication emit overflow checks
  - [ ] Division by zero and `int.min / -1` panic
  - [ ] Consistent behavior in debug and release builds
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — overflow panic tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/overflow_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for overflow panic behavior
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — overflow panic codegen

- [ ] **Implement**: Compile-time constant overflow is a compile error
  - [ ] `$big = int.max + 1` → ERROR: constant overflow
  - [ ] **Rust Tests**: `oric/src/typeck/checker/const_eval.rs` — constant overflow tests
  - [ ] **Ori Tests**: `tests/compile-fail/constant_overflow.ori`
  - [ ] **LLVM Support**: LLVM codegen for compile-time overflow errors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/math_tests.rs` — constant overflow codegen

---

## 7.12 Collection Methods (len, is_empty)

> Move from free functions to methods on collections.

- [ ] **Implement**: `[T].len() -> int` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list len method codegen

- [ ] **Implement**: `[T].is_empty() -> bool` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list is_empty method codegen

- [ ] **Implement**: `{K: V}.len() -> int` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — map len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/map_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for map len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — map len method codegen

- [ ] **Implement**: `{K: V}.is_empty() -> bool` — modules/prelude.md § Map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — map is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/map_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for map is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — map is_empty method codegen

- [ ] **Implement**: `str.len() -> int` — modules/prelude.md § str
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — str len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/str_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for str len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/string_tests.rs` — str len method codegen

- [ ] **Implement**: `str.is_empty() -> bool` — modules/prelude.md § str
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — str is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/str_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for str is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/string_tests.rs` — str is_empty method codegen

- [ ] **Implement**: `Set<T>.len() -> int` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — set len tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/set_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for set len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — set len method codegen

- [ ] **Implement**: `Set<T>.is_empty() -> bool` — modules/prelude.md § Set
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — set is_empty tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/set_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for set is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — set is_empty method codegen

---

## 7.13 Comparable Methods (min, max, compare)

> Move from free functions to methods on Comparable trait.

- [ ] **Implement**: `T.min(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — min method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for min method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — min method codegen

- [ ] **Implement**: `T.max(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — max method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for max method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — max method codegen

- [ ] **Implement**: `T.compare(other: T) -> Ordering` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — compare method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for compare method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — compare method codegen

---

## 7.14 std.testing Module

> Move testing assertions from built-ins to std.testing.

- [ ] **Implement**: `assert_eq(actual, expected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_eq tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_eq codegen

- [ ] **Implement**: `assert_ne(actual, unexpected)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ne tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_ne
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_ne codegen

- [ ] **Implement**: `assert_some(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_some codegen

- [ ] **Implement**: `assert_none(option)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_none codegen

- [ ] **Implement**: `assert_ok(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_ok codegen

- [ ] **Implement**: `assert_err(result)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_err codegen

- [ ] **Implement**: `assert_panics(expr)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_panics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_panics codegen

- [ ] **Implement**: `assert_panics_with(expr, message)` — modules/std.testing/index.md
  - [ ] **Rust Tests**: `library/std/testing.rs` — assert_panics_with tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/testing.ori`
  - [ ] **LLVM Support**: LLVM codegen for std.testing assert_panics_with
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/testing_tests.rs` — assert_panics_with codegen

---

## 7.15 Iterator Traits

> **PROPOSAL**: `proposals/drafts/iterator-traits-proposal.md`
>
> Formalize iteration with traits, enabling user types in `for` loops and generic iteration.

- [ ] **Implement**: `Iterator` trait
  ```ori
  trait Iterator {
      type Item
      @next (mut self) -> Option<Self.Item>
  }
  ```
  - [ ] **Rust Tests**: `oric/src/typeck/traits/iterator.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/iterator.ori`
  - [ ] **LLVM Support**: LLVM codegen for Iterator trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterator trait codegen

- [ ] **Implement**: `Iterable` trait
  ```ori
  trait Iterable {
      type Item
      @iter (self) -> impl Iterator where Item == Self.Item
  }
  ```
  - [ ] **Rust Tests**: `oric/src/typeck/traits/iterable.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/iterable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Iterable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterable trait codegen

- [ ] **Implement**: `Collect` trait
  ```ori
  trait Collect<T> {
      @from_iter (iter: impl Iterator where Item == T) -> Self
  }
  ```
  - [ ] **Rust Tests**: `oric/src/typeck/traits/collect.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/collect.ori`
  - [ ] **LLVM Support**: LLVM codegen for Collect trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Collect trait codegen

- [ ] **Implement**: Standard `Iterable` implementations
  - `impl<T> Iterable for [T]` — list iteration
  - `impl<K, V> Iterable for {K: V}` — map iteration (yields tuples)
  - `impl<T> Iterable for Set<T>` — set iteration
  - `impl Iterable for str` — character iteration
  - `impl Iterable for Range<int>` — range iteration
  - `impl<T> Iterable for Option<T>` — zero/one element
  - [ ] **Ori Tests**: `tests/spec/stdlib/iterable_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard Iterable implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterable implementations codegen

- [ ] **Implement**: Standard `Collect` implementations
  - `impl<T> Collect<T> for [T]` — collect to list
  - `impl<T> Collect<T> for Set<T>` — collect to set
  - [ ] **Ori Tests**: `tests/spec/stdlib/collect_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard Collect implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Collect implementations codegen

- [ ] **Implement**: `for` loop desugaring to `.iter()` and `.next()`
  - [ ] **Rust Tests**: `oric/src/eval/for_loop.rs`
  - [ ] **Ori Tests**: `tests/spec/control/for_iterator.ori`
  - [ ] **LLVM Support**: LLVM codegen for for loop desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — for loop desugaring codegen

- [ ] **Implement**: Iterator extension methods
  - `map`, `filter`, `fold`, `find`, `collect`, `count`
  - `any`, `all`, `take`, `skip`, `enumerate`, `zip`, `chain`
  - [ ] **Ori Tests**: `tests/spec/stdlib/iterator_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for iterator extension methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — iterator extension methods codegen

---

## 7.16 Debug Trait

> **PROPOSAL**: `proposals/drafts/debug-trait-proposal.md`
>
> Developer-facing structural output, separate from user-facing `Printable`.

- [ ] **Implement**: `Debug` trait
  ```ori
  trait Debug {
      @debug (self) -> str
  }
  ```
  - [ ] **Rust Tests**: `oric/src/typeck/traits/debug.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/debug.ori`
  - [ ] **LLVM Support**: LLVM codegen for Debug trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug trait codegen

- [ ] **Implement**: `#[derive(Debug)]` for structs and sum types
  - [ ] **Rust Tests**: `oric/src/typeck/derives/debug.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/debug_derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Debug)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — derive(Debug) codegen

- [ ] **Implement**: Standard `Debug` implementations
  - All primitives: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`
  - Collections: `[T]`, `{K: V}`, `Set<T>` (require `T: Debug`)
  - `Option<T>`, `Result<T, E>` (require inner types `Debug`)
  - Tuples (require element types `Debug`)
  - [ ] **Ori Tests**: `tests/spec/stdlib/debug_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard Debug implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug implementations codegen

- [ ] **Implement**: String escaping in Debug output
  - `"hello".debug()` → `"\"hello\""`
  - `'\n'.debug()` → `"'\\n'"`
  - [ ] **Ori Tests**: `tests/spec/stdlib/debug_escaping.ori`
  - [ ] **LLVM Support**: LLVM codegen for Debug string escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug escaping codegen

---

## 7.17 Developer Functions

> **PROPOSAL**: `proposals/drafts/developer-functions-proposal.md`
>
> Convenience functions for development: placeholders and debugging.

- [ ] **Implement**: `todo()` and `todo(reason: str)` → `Never`
  - Panics with "not yet implemented" and location
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — todo tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/todo.ori`
  - [ ] **LLVM Support**: LLVM codegen for todo
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — todo codegen

- [ ] **Implement**: `unreachable()` and `unreachable(reason: str)` → `Never`
  - Panics with "unreachable code reached" and location
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — unreachable tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/unreachable.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — unreachable codegen

- [ ] **Implement**: `dbg(value: T)` and `dbg(value: T, label: str)` → `T`
  - Requires `T: Debug`
  - Prints `[file:line] label = <debug>` to stderr
  - Returns value unchanged
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — dbg tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/dbg.ori`
  - [ ] **LLVM Support**: LLVM codegen for dbg
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — dbg codegen

- [ ] **Implement**: Location capture for `todo`, `unreachable`, `dbg`
  - Compiler passes call-site location implicitly
  - [ ] **Rust Tests**: `oric/src/eval/location.rs`
  - [ ] **LLVM Support**: LLVM codegen for location capture
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/developer_tests.rs` — location capture codegen

---

## 7.18 std.time Module

**Proposal**: `proposals/approved/stdlib-time-api-proposal.md`

Date/time types, formatting, parsing, arithmetic, and timezone handling.

### 7.18.1 Core Types

- [ ] **Implement**: `Instant` type — UTC timestamp (nanoseconds since Unix epoch)
  - `Instant.now()`, `from_unix_secs()`, `from_unix_millis()`, `to_unix_secs()`, `to_unix_millis()`
  - `add()`, `sub()`, `diff()` for Duration arithmetic
  - Implements `Comparable`
  - [ ] **Rust Tests**: `library/std/time/instant.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/instant.ori`
  - [ ] **LLVM Support**: LLVM codegen for Instant
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Instant codegen

- [ ] **Implement**: `DateTime` type — date and time in a specific timezone
  - `now()`, `now_utc()`, `from_instant()`, `from_parts()`
  - `to_instant()`, `to_timezone()`, `to_utc()`, `to_local()`
  - `date()`, `time()`, `weekday()` component accessors
  - `add()`, `add_days()`, `add_months()`, `add_years()` arithmetic
  - [ ] **Rust Tests**: `library/std/time/datetime.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/datetime.ori`
  - [ ] **LLVM Support**: LLVM codegen for DateTime
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — DateTime codegen

- [ ] **Implement**: `Date` type — date only (no time component)
  - `today()`, `new()`
  - `weekday()`, `day_of_year()`, `is_leap_year()`, `days_in_month()`
  - `add_days()`, `add_months()`, `add_years()`, `diff_days()`
  - [ ] **Rust Tests**: `library/std/time/date.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/date.ori`
  - [ ] **LLVM Support**: LLVM codegen for Date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Date codegen

- [ ] **Implement**: `Time` type — time of day only (no date component)
  - `now()`, `new()`, `midnight()`, `noon()`
  - `to_seconds()`, `to_millis()`
  - [ ] **Rust Tests**: `library/std/time/time.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/time.ori`
  - [ ] **LLVM Support**: LLVM codegen for Time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Time codegen

- [ ] **Implement**: `Timezone` type — timezone info (opaque)
  - `utc()`, `local()`, `from_name()`, `from_offset()`, `fixed()`
  - `name()`, `offset_at()`
  - [ ] **Rust Tests**: `library/std/time/timezone.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/timezone.ori`
  - [ ] **LLVM Support**: LLVM codegen for Timezone
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Timezone codegen

- [ ] **Implement**: `Weekday` sum type — `Monday | Tuesday | ... | Sunday`
  - `is_weekend()`, `next()`, `prev()`, `all()`
  - [ ] **Rust Tests**: `library/std/time/weekday.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/weekday.ori`
  - [ ] **LLVM Support**: LLVM codegen for Weekday
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Weekday codegen

### 7.18.2 Duration Extension Methods

> **Note:** These are extension methods requiring `use std.time { Duration }`.

- [ ] **Implement**: Duration construction methods
  - `from_nanos()`, `from_micros()`, `from_millis()`, `from_secs()`, `from_mins()`, `from_hours()`, `from_days()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration construction codegen

- [ ] **Implement**: Duration extraction methods
  - `to_nanos()`, `to_micros()`, `to_millis()`, `to_secs()`, `to_mins()`, `to_hours()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration extraction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration extraction codegen

- [ ] **Implement**: Duration component methods
  - `hours_part()`, `minutes_part()`, `seconds_part()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration components
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration components codegen

- [ ] **Implement**: Duration arithmetic and checks
  - `add()`, `sub()`, `mul()`, `div()`
  - `is_zero()`, `is_negative()`
  - [ ] **Rust Tests**: `library/std/time/duration.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for Duration arithmetic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — Duration arithmetic codegen

### 7.18.3 Formatting

- [ ] **Implement**: `format(dt, pattern)` — DateTime formatting with pattern specifiers
  - Pattern specifiers: `YYYY`, `YY`, `MM`, `M`, `DD`, `D`, `HH`, `H`, `hh`, `h`, `mm`, `ss`, `SSS`, `a`, `E`, `EEEE`, `MMM`, `MMMM`, `Z`, `ZZ`, `z`
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format codegen

- [ ] **Implement**: `format_date(d, pattern)` — Date-only formatting
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format_date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format_date codegen

- [ ] **Implement**: `format_time(t, pattern)` — Time-only formatting
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/format.ori`
  - [ ] **LLVM Support**: LLVM codegen for format_time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — format_time codegen

- [ ] **Implement**: ISO 8601 formatting
  - `to_iso8601(dt)`, `to_iso8601_date(d)`, `to_iso8601_time(t)`
  - [ ] **Rust Tests**: `library/std/time/format.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/iso8601.ori`
  - [ ] **LLVM Support**: LLVM codegen for ISO 8601 formatting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — ISO 8601 formatting codegen

### 7.18.4 Parsing

- [ ] **Implement**: `parse(source, pattern, tz)` — DateTime parsing with optional timezone
  - `tz` parameter defaults to UTC for patterns without timezone info
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse codegen

- [ ] **Implement**: `parse_date(source, pattern)` — Date-only parsing
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_date
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse_date codegen

- [ ] **Implement**: `parse_time(source, pattern)` — Time-only parsing
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_time
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — parse_time codegen

- [ ] **Implement**: ISO 8601 parsing
  - `from_iso8601(source)`, `from_iso8601_date(source)`
  - [ ] **Rust Tests**: `library/std/time/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/iso8601.ori`
  - [ ] **LLVM Support**: LLVM codegen for ISO 8601 parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — ISO 8601 parsing codegen

### 7.18.5 Error Type

- [ ] **Implement**: `TimeError` and `TimeErrorKind`
  - `InvalidDate`, `InvalidTime`, `InvalidTimezone`, `ParseError`, `Overflow`
  - [ ] **Rust Tests**: `library/std/time/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for TimeError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — TimeError codegen

### 7.18.6 Clock Capability

- [ ] **Implement**: `Clock` trait update
  - `now() -> Instant`, `local_timezone() -> Timezone`
  - [ ] **Rust Tests**: `oric/src/capabilities/clock.rs`
  - [ ] **Ori Tests**: `tests/spec/capabilities/clock.ori`
  - [ ] **LLVM Support**: LLVM codegen for Clock capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` — Clock codegen

- [ ] **Implement**: `MockClock` for testing
  - `MockClock.new(now)` constructor
  - `advance(by)` with interior mutability
  - [ ] **Rust Tests**: `library/std/time/mock.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/time/mock_clock.ori`
  - [ ] **LLVM Support**: LLVM codegen for MockClock
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/time_tests.rs` — MockClock codegen

---

## 7.19 std.json Module

**Proposal**: `proposals/approved/stdlib-json-api-proposal.md`

JSON parsing, serialization, and manipulation.

### 7.19.1 Core Types

- [ ] **Implement**: `JsonValue` sum type
  - `Null | Bool(bool) | Number(float) | String(str) | Array([JsonValue]) | Object({str: JsonValue})`
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonValue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonValue codegen

- [ ] **Implement**: `JsonError` and `JsonErrorKind` types
  - `ParseError | TypeError | MissingField | UnknownField | ValueError`
  - Fields: `kind`, `message`, `path`, `position`
  - [ ] **Rust Tests**: `library/std/json/error.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonError codegen

- [ ] **Implement**: `Json` trait
  - `@to_json (self) -> JsonValue`
  - `@from_json (json: JsonValue) -> Result<Self, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/trait.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/trait.ori`
  - [ ] **LLVM Support**: LLVM codegen for Json trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — Json trait codegen

### 7.19.2 Parsing API

- [ ] **Implement**: `parse(source: str) -> Result<JsonValue, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — parse codegen

- [ ] **Implement**: `parse_as<T: Json>(source: str) -> Result<T, JsonError>`
  - [ ] **Rust Tests**: `library/std/json/parse.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/parse.ori`
  - [ ] **LLVM Support**: LLVM codegen for parse_as
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — parse_as codegen

### 7.19.3 Serialization API

- [ ] **Implement**: `stringify(value: JsonValue) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for stringify
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — stringify codegen

- [ ] **Implement**: `stringify_pretty(value: JsonValue, indent: int = 2) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for stringify_pretty
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — stringify_pretty codegen

- [ ] **Implement**: `to_json_string<T: Json>(value: T) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for to_json_string
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — to_json_string codegen

- [ ] **Implement**: `to_json_string_pretty<T: Json>(value: T, indent: int = 2) -> str`
  - [ ] **Rust Tests**: `library/std/json/stringify.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/stringify.ori`
  - [ ] **LLVM Support**: LLVM codegen for to_json_string_pretty
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — to_json_string_pretty codegen

### 7.19.4 JsonValue Methods

- [ ] **Implement**: Type check methods
  - `is_null()`, `is_bool()`, `is_number()`, `is_string()`, `is_array()`, `is_object()`
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for type check methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — type check methods codegen

- [ ] **Implement**: Safe extraction methods
  - `as_bool()`, `as_number()`, `as_int()`, `as_string()`, `as_array()`, `as_object()`
  - `as_int()` returns `Some` only for exact integers within int range (no truncation)
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for extraction methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — extraction methods codegen

- [ ] **Implement**: Indexing methods
  - `get(key: str)` for objects, `get_index(index: int)` for arrays
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/value_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for indexing methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — indexing methods codegen

- [ ] **Implement**: Path access method
  - `at(path: str)` — dot notation with array index support (`"users[0].name"`)
  - [ ] **Rust Tests**: `library/std/json/value.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/path_access.ori`
  - [ ] **LLVM Support**: LLVM codegen for path access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — path access codegen

### 7.19.5 Derive Macro

- [ ] **Implement**: `#derive(Json)` for structs
  - Generate `to_json` and `from_json` implementations
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_struct.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Json) structs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — derive(Json) struct codegen

- [ ] **Implement**: `#derive(Json)` for sum types
  - Simple variants serialize as strings, payload variants as objects
  - Support `#json(tag: "type", content: "data")` for tagged unions
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_enum.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Json) enums
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — derive(Json) enum codegen

- [ ] **Implement**: Field attributes for `#derive(Json)`
  - `#json(rename: "name")` — different JSON field name
  - `#json(skip)` — exclude from serialization
  - `#json(default: value)` — default if field missing
  - `#json(flatten)` — merge nested object into parent (compile error on conflicts)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/json.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/derive_attrs.ori`
  - [ ] **LLVM Support**: LLVM codegen for field attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — field attributes codegen

### 7.19.6 Standard Type Implementations

- [ ] **Implement**: Primitive Json implementations
  - `bool`, `int`, `float`, `str`
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_primitive.ori`
  - [ ] **LLVM Support**: LLVM codegen for primitive Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — primitive impls codegen

- [ ] **Implement**: Collection Json implementations
  - `[T]` (array), `{str: V}` (object), `Set<T>` (array), `Option<T>` (null or value), `(A, B)` (array)
  - Non-string map keys serialize as strings
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for collection Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — collection impls codegen

- [ ] **Implement**: Built-in type Json implementations
  - `Duration` → ISO 8601 duration string (`"PT1H30M"`)
  - `Size` → integer bytes
  - [ ] **Rust Tests**: `library/std/json/impls.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/impls_builtin.ori`
  - [ ] **LLVM Support**: LLVM codegen for built-in Json impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — built-in impls codegen

### 7.19.7 Streaming API

- [ ] **Implement**: `JsonParser` type with Iterator trait
  - `new(source: str)` constructor
  - Implements `Iterator` and `Iterable` with `Item = JsonEvent`
  - [ ] **Rust Tests**: `library/std/json/stream.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/streaming.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonParser
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonParser codegen

- [ ] **Implement**: `JsonEvent` sum type
  - `StartObject | EndObject | StartArray | EndArray | Key(str) | Value(JsonValue)`
  - [ ] **Rust Tests**: `library/std/json/stream.rs`
  - [ ] **Ori Tests**: `tests/spec/stdlib/json/streaming.ori`
  - [ ] **LLVM Support**: LLVM codegen for JsonEvent
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/json_tests.rs` — JsonEvent codegen

---

## 7.20 Float NaN Behavior

> **Decision**: NaN comparisons panic (no proposal needed — behavioral decision)
>
> Fits Ori's "bugs should be caught" philosophy (same as integer overflow).

- [ ] **Implement**: NaN comparison panics
  - `NaN == NaN` → PANIC
  - `NaN < x` → PANIC
  - `NaN > x` → PANIC
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — NaN comparison tests
  - [ ] **Ori Tests**: `tests/spec/types/float_nan.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN comparison panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN comparison panic codegen

- [ ] **Implement**: NaN-producing operations don't panic (only comparisons)
  - `0.0 / 0.0` → NaN (allowed)
  - Using NaN in arithmetic → NaN (allowed)
  - Comparing NaN → PANIC
  - [ ] **Ori Tests**: `tests/spec/types/float_nan_ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for NaN-producing operations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/float_tests.rs` — NaN operations codegen

---

## 7.20 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all`
- [ ] **LLVM Support**: All LLVM codegen tests pass
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/` — full stdlib LLVM test coverage

**Exit Criteria**: Basic programs can use stdlib
