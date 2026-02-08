---
section: 7C
title: Collections & Iteration
status: not-started
tier: 2
goal: Collection methods, iterator traits, and Debug trait
spec:
  - spec/11-built-in-functions.md
sections:
  - id: "7C.1"
    title: Collection Functions
    status: not-started
  - id: "7C.2"
    title: Collection Methods on [T]
    status: not-started
  - id: "7C.3"
    title: Range Methods
    status: not-started
  - id: "7C.4"
    title: Collection Methods (len, is_empty)
    status: not-started
  - id: "7C.5"
    title: Comparable Methods (min, max, compare)
    status: not-started
  - id: "7C.6"
    title: Iterator Traits
    status: not-started
  - id: "7C.7"
    title: Debug Trait
    status: not-started
  - id: "7C.8"
    title: Section Completion Checklist
    status: not-started
---

# Section 7C: Collections & Iteration

**Goal**: Collection methods, iterator traits, and Debug trait

> **SPEC**: `spec/11-built-in-functions.md`
> **DESIGN**: `modules/prelude.md`

---

## 7C.1 Collection Functions

> **NOTE**: `len` and `is_empty` are being moved from free functions to methods on collections (see 7C.4).
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

## 7C.2 Collection Methods on `[T]`

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

## 7C.3 Range Methods

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

## 7C.4 Collection Methods (len, is_empty)

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

## 7C.5 Comparable Methods (min, max, compare)

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

## 7C.6 Iterator Traits

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

## 7C.7 Debug Trait

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

## 7C.8 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Collections and iteration working correctly
