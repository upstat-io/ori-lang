---
section: 7C
title: Collections & Iteration
status: in-progress
reviewed: false
tier: 2
goal: Collection methods, iterator traits, and Debug trait
spec:
  - spec/annex-c-built-in-functions.md
sections:
  - id: "7C.1"
    title: Collection Functions
    status: in-progress
  - id: "7C.2"
    title: Collection Methods on [T]
    status: in-progress
  - id: "7C.3"
    title: Range Methods
    status: in-progress
  - id: "7C.4"
    title: Collection Methods (len, is_empty)
    status: in-progress
  - id: "7C.5"
    title: Comparable Methods (min, max, compare)
    status: in-progress
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

> **SPEC**: `spec/annex-c-built-in-functions.md`
> **DESIGN**: `modules/prelude.md`

---

## 7C.1 Collection Functions

> **NOTE**: `len` and `is_empty` are being moved from free functions to methods on collections (see 7C.4).
> The free function forms are deprecated in favor of `.len()` and `.is_empty()` methods.
> Keep backward compatibility during transition, then remove free functions.

- [x] **Implement**: `len(x)` — spec/annex-c-built-in-functions.md § len (deprecated, use `.len()`) [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — len function tests
  - [x] **Ori Tests**: Used extensively in test suite via `.len()` method
  - [ ] **LLVM Support**: LLVM codegen for len function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — len function codegen
  - [ ] **AOT Tests**: No AOT coverage yet (free function form; method form covered in 7C.4)

- [x] **Implement**: `is_empty(x)` — spec/annex-c-built-in-functions.md § is_empty (deprecated, use `.is_empty()`) [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — is_empty function tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 9 active tests (4 commented out as PENDING)
  - [ ] **LLVM Support**: LLVM codegen for is_empty function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — is_empty function codegen
  - [ ] **AOT Tests**: No AOT coverage yet (free function form; method form covered in 7C.4)

---

## 7C.2 Collection Methods on `[T]`

> **Design Principle**: Lean core, rich libraries. Data transformation is stdlib, not compiler patterns.

- [x] **Implement**: `[T].map(f: T -> U) -> [U]` — modules/prelude.md § List [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — list map tests
  - [x] **Ori Tests**: Used in `tests/spec/types/primitives.ori`, `tests/spec/lexical/keywords.ori`
  - [ ] **LLVM Support**: LLVM codegen for list map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list map codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_map` (iterator-level `.map()`, 1 test); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_map_collect_length` (1 test)

- [x] **Implement**: `[T].filter(f: T -> bool) -> [T]` — modules/prelude.md § List [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — list filter tests
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori` — `list.filter(predicate: x -> x % 2 == 0)`
  - [ ] **LLVM Support**: LLVM codegen for list filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list filter codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_filter` (iterator-level `.filter()`, 1 test); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_filter_count` (1 test)

- [x] **Implement**: `[T].fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § List [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — list fold tests
  - [x] **Ori Tests**: `tests/spec/lexical/keywords.ori` — `[1,2,3].fold(initial: 0, combine: (a,b) -> a + b)`
  - [ ] **LLVM Support**: LLVM codegen for list fold
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list fold codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_fold_sum`, `test_iter_fold_with_filter` (iterator-level `.fold()`, 2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_sum_via_fold` (1 test)

- [x] **Implement**: `[T].find(f: T -> bool) -> Option<T>` — modules/prelude.md § List [done — via iterator pipeline]
  - Implemented via `CollectionMethod::IterFind` in evaluator
  - [ ] **LLVM Support**: LLVM codegen for list find
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_find_some`, `test_iter_find_none` (2 tests, all pass)

- [x] **Implement**: `[T].any(f: T -> bool) -> bool` — modules/prelude.md § List [done — via iterator pipeline]
  - Implemented via `CollectionMethod::IterAny` in evaluator
  - [ ] **LLVM Support**: LLVM codegen for list any
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_any_true`, `test_iter_any_false` (2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_any_all` (1 test) — all pass

- [x] **Implement**: `[T].all(f: T -> bool) -> bool` — modules/prelude.md § List [done — via iterator pipeline]
  - Implemented via `CollectionMethod::IterAll` in evaluator
  - [ ] **LLVM Support**: LLVM codegen for list all
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_all_true`, `test_iter_all_false` (2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_any_all` (1 test) — all pass

- [x] **Implement**: `[T].first() -> Option<T>` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list first
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_first`, `test_coll_list_first_empty` (2 tests, all pass)

- [x] **Implement**: `[T].last() -> Option<T>` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list last
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_last`, `test_coll_list_last_empty` (2 tests, all pass)

- [ ] **Implement**: `[T].take(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list take tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list take
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list take codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_take` (iterator-level take adapter, 1 test)

- [ ] **Implement**: `[T].skip(n: int) -> [T]` — modules/prelude.md § List
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — list skip tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/list_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for list skip
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list skip codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_skip` (iterator-level skip adapter, 1 test)

- [x] **Implement**: `[T].reverse() -> [T]` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list reverse
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_reverse`, `test_coll_list_reverse_single`, `test_coll_list_reverse_values`, `test_coll_list_reverse_reverse`, `test_coll_list_reverse_empty` (5 tests, all pass)

- [x] **Implement**: `[T].sort() -> [T]` where `T: Comparable` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list sort
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_sort_ints`, `test_coll_list_sort_already_sorted`, `test_coll_list_sort_reverse_order`, `test_coll_list_sort_single` (4 tests, all pass)

- [x] **Implement**: `[T].contains(value: T) -> bool` where `T: Eq` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list contains
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_contains`, `test_coll_list_contains_missing` (2 tests, all pass)

- [x] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list push
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_push`, `test_coll_list_push_empty`, `test_coll_list_push_multi`, `test_coll_list_push_push`, `test_coll_list_push_concat`, `test_coll_list_push_then_reverse`, `test_coll_list_push_loop_1000` (7 tests, all pass)

- [x] **Implement**: `[T].concat(other: [T]) -> [T]` — modules/prelude.md § List [done]
  - Implemented in evaluator at `methods/list.rs`
  - [ ] **LLVM Support**: LLVM codegen for list concat
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_concat_basic`, `test_coll_list_concat_empty`, `test_coll_list_concat_reverse` (3 tests, all pass)

---

## 7C.3 Range Methods

- [ ] **Implement**: `Range.map(f: T -> U) -> [U]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.map codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_map` tests map on list iterator; `test_chained_map_filter_take` tests map on range-derived iterator

- [ ] **Implement**: `Range.filter(f: T -> bool) -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.filter codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_filter` tests filter on list iterator; `test_chained_map_filter_take` tests filter on range-derived iterator

- [ ] **Implement**: `Range.fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.fold tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.fold
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.fold codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_fold_sum`, `test_iter_fold_with_filter` test fold on range-derived iterators via chaining

- [ ] **Implement**: `Range.collect() -> [T]` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.collect tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.collect
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.collect codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_range_iter_collect` (range `.iter().collect()` to list, 1 test)

- [ ] **Implement**: `Range.contains(value: T) -> bool` — modules/prelude.md § Range
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Range.contains tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/range_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range.contains
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — Range.contains codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7C.4 Collection Methods (len, is_empty)

> Move from free functions to methods on collections.

- [x] **Implement**: `[T].len() -> int` — modules/prelude.md § List [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — list len tests
  - [x] **Ori Tests**: `tests/spec/expressions/field_access.ori`, `tests/spec/lexical/delimiters.ori`
  - [ ] **LLVM Support**: LLVM codegen for list len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list len method codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_length_empty` + 2 variants, `test_coll_list_len_alias` (4 tests, 0 ignored)

- [x] **Implement**: `[T].is_empty() -> bool` — modules/prelude.md § List [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — list is_empty tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 9 active tests (4 commented out as PENDING)
  - [ ] **LLVM Support**: LLVM codegen for list is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list is_empty method codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_is_empty_true`, `test_coll_list_is_empty_false` (2 tests, 0 ignored)

- [x] **Implement**: `{K: V}.len() -> int` — modules/prelude.md § Map [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — map len tests
  - [x] **Ori Tests**: `tests/spec/lexical/delimiters.ori` — `map.len()` tests
  - [ ] **LLVM Support**: LLVM codegen for map len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — map len method codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_map_length_basic`, `test_coll_map_length_one`, `test_coll_map_len_alias` (3 tests, 0 ignored)

- [x] **Implement**: `{K: V}.is_empty() -> bool` — modules/prelude.md § Map [done]
  - Implemented in evaluator at `methods/collections.rs`
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — map is_empty tests
  - [ ] **LLVM Support**: LLVM codegen for map is_empty method
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_map_is_empty_true`, `test_coll_map_is_empty_false` (2 tests, all pass)

- [x] **Implement**: `str.len() -> int` — modules/prelude.md § str [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — str len tests
  - [x] **Ori Tests**: `tests/spec/expressions/field_access.ori` — `"hello".len()`
  - [ ] **LLVM Support**: LLVM codegen for str len method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/string_tests.rs` — str len method codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/strings.rs` — `test_str_length_basic` + 4 variants, `test_str_len_alias` (6 tests, 0 ignored)

- [x] **Implement**: `str.is_empty() -> bool` — modules/prelude.md § str [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — str is_empty tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — `"".is_empty()`, `!"hello".is_empty()`
  - [ ] **LLVM Support**: LLVM codegen for str is_empty method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/string_tests.rs` — str is_empty method codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/strings.rs` — `test_str_is_empty_true`, `test_str_is_empty_false`, `test_str_is_empty_space` (3 tests, 0 ignored)

- [x] **Implement**: `Set<T>.len() -> int` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Rust Tests**: `ori_eval/src/methods/helpers/mod.rs` — set len in `TYPECK_BUILTIN_METHODS`
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set len tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_length()`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_length` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.is_empty() -> bool` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Rust Tests**: `ori_eval/src/methods/helpers/mod.rs` — set is_empty in `TYPECK_BUILTIN_METHODS`
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set is_empty tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_is_empty()`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_is_empty` (passing — `check_collect_to_set` now unifies element type with expected) (2026-03-01)

- [x] **Implement**: `Set<T>.contains(elem) -> bool` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set contains dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set contains tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_contains()` via `ori_set_contains` runtime
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_contains` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.insert(elem) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set insert dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set insert tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_insert()` via `ori_set_insert` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_insert` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.remove(elem) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set remove dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set remove tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_remove()` via `ori_set_remove` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_remove` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.union(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set union dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set union tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_union()` via `ori_set_union` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_union` (passing — lexer fix: reserved-future keywords suppressed in method position) (2026-03-01)

- [x] **Implement**: `Set<T>.intersection(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set intersection dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set intersection tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_intersection()` via `ori_set_intersection` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_intersection` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.difference(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set difference dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set difference tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_difference()` via `ori_set_difference` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_difference` (passing) (2026-03-01)

- [x] **Implement**: `Set<T>.to_list() -> [T]` — modules/prelude.md § Set [done] (2026-02-25)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set to_list dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set to_list tests
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_to_list()` via `ori_set_to_list` runtime (sret)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_to_list` (passing) (2026-03-01)

---

## 7C.5 Comparable Methods (min, max, compare)

> Move from free functions to methods on Comparable trait.

- [ ] **Implement**: `T.min(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — min method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for min method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — min method codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `T.max(other: T) -> T` where `T: Comparable` — modules/prelude.md § Comparable
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — max method tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for max method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — max method codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `T.compare(other: T) -> Ordering` where `T: Comparable` — modules/prelude.md § Comparable [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — compare method tests
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — 133 test occurrences
  - [ ] **LLVM Support**: LLVM codegen for compare method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparison_tests.rs` — compare method codegen
  - [ ] **AOT Tests**: No AOT coverage yet

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
  - [ ] **Rust Tests**: `ori_types/src/check/traits/iterator.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/iterator.ori`
  - [ ] **LLVM Support**: LLVM codegen for Iterator trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterator trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet (formal Iterator trait definition)

- [ ] **Implement**: `Iterable` trait
  ```ori
  trait Iterable {
      type Item
      @iter (self) -> impl Iterator where Item == Self.Item
  }
  ```
  - [ ] **Rust Tests**: `ori_types/src/check/traits/iterable.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/iterable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Iterable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterable trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet (formal Iterable trait definition)

- [ ] **Implement**: `Collect` trait
  ```ori
  trait Collect<T> {
      @from_iter (iter: impl Iterator where Item == T) -> Self
  }
  ```
  - [ ] **Rust Tests**: `ori_types/src/check/traits/collect.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/collect.ori`
  - [ ] **LLVM Support**: LLVM codegen for Collect trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Collect trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet (formal Collect trait definition)

- [ ] **Implement**: Standard `Iterable` implementations
  - `impl<T> [T]: Iterable` — list iteration
  - `impl<K, V> {K: V}: Iterable` — map iteration (yields tuples)
  - `impl<T> Set<T>: Iterable` — set iteration
  - `impl str: Iterable` — character iteration
  - `impl Range<int>: Iterable` — range iteration
  - `impl<T> Option<T>: Iterable` — zero/one element
  - [ ] **Ori Tests**: `tests/spec/stdlib/iterable_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard Iterable implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Iterable implementations codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — list `.iter()` (2 tests), range `.iter()` (2 tests); `ori_llvm/tests/aot/collections_ext.rs` — map `.iter()` (2 tests), string `.iter()` via `ori_llvm/tests/aot/strings.rs` (3 tests)

- [ ] **Implement**: Standard `Collect` implementations
  - `impl<T> Collect<T> for [T]` — collect to list
  - `impl<T> Collect<T> for Set<T>` — collect to set (LLVM `__collect_set` implemented 2026-03-01)
  - [ ] **Ori Tests**: `tests/spec/stdlib/collect_impls.ori`
  - [x] **LLVM Support**: LLVM codegen for standard Collect implementations — collect-to-list (`ori_iter_collect`) and collect-to-set (`ori_iter_collect_set` via `__collect_set` intercept) both implemented (2026-03-01)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — Collect implementations codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_list_iter_collect`, `test_iter_chain_collect` (collect to list via `.iter().collect()`)

- [ ] **Implement**: `for` loop desugaring to `.iter()` and `.next()`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/for_loop.rs`
  - [ ] **Ori Tests**: `tests/spec/control/for_iterator.ori`
  - [ ] **LLVM Support**: LLVM codegen for for loop desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — for loop desugaring codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_for_over_iterator`, `test_for_over_range_iterator` (for-loop over `.iter()` on lists and ranges)

- [ ] **Implement**: Iterator extension methods
  - `map`, `filter`, `fold`, `find`, `collect`, `count`
  - `any`, `all`, `take`, `skip`, `enumerate`, `zip`, `chain`
  - [ ] **Ori Tests**: `tests/spec/stdlib/iterator_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for iterator extension methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs` — iterator extension methods codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — 25 tests (0 ignored) covering map, filter, take, skip, count, any, all, find, fold, for_each, collect, zip, chain adapters/consumers via built-in iterator pipeline

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
  - [ ] **Rust Tests**: `ori_types/src/check/traits/debug.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/debug.ori`
  - [ ] **LLVM Support**: LLVM codegen for Debug trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `#[derive(Debug)]` for structs and sum types
  - [ ] **Rust Tests**: `ori_types/src/check/derives/debug.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/debug_derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive(Debug)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — derive(Debug) codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Standard `Debug` implementations
  - All primitives: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`
  - Collections: `[T]`, `{K: V}`, `Set<T>` (require `T: Debug`)
  - `Option<T>`, `Result<T, E>` (require inner types `Debug`)
  - Tuples (require element types `Debug`)
  - [ ] **Ori Tests**: `tests/spec/stdlib/debug_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for standard Debug implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug implementations codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: String escaping in Debug output
  - `"hello".debug()` → `"\"hello\""`
  - `'\n'.debug()` → `"'\\n'"`
  - [ ] **Ori Tests**: `tests/spec/stdlib/debug_escaping.ori`
  - [ ] **LLVM Support**: LLVM codegen for Debug string escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs` — Debug escaping codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7C.8 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Collections and iteration working correctly
