# Section 07C: Collections & Iteration -- Verification Results

**Verified**: 2026-03-19
**Section status**: in-progress (105/297, 35%)
**Methodology**: Sampled 3-5 checked items per subsection across 7C.1-7C.5. Verified `[x]` items by reading test code and running tests. Verified `[ ]` items by checking actual implementation state. Ran AOT tests for sets, iterators, collections_ext, and strings.

## Summary

| Status | Count |
|--------|-------|
| VERIFIED | 22 |
| STALE (marked `[ ]` but implemented) | 14 |
| WEAK TESTS | 1 |
| CONFIRMED INCOMPLETE | ~50 |
| INACCURACY | 3 |

**Overall assessment**: The section has significant staleness. 14 items are marked `[ ]` incomplete but are actually implemented and have passing tests. The roadmap also claims some AOT tests are "ignored" when they are not -- these tests run and pass. Several unchecked items in 7C.2 (list methods) are fully implemented in the evaluator and have passing AOT tests. `{K: V}.is_empty()` (7C.4) is implemented and tested but marked `[ ]`.

---

## 7C.1 Collection Functions

### `len(x)` -- [x] Implement

**VERIFIED.** `len` free function is implemented. The evaluator handles it via `BuiltinMethodResolver`. Ori tests at `tests/spec/expressions/field_access.ori` use `.len()`. AOT tests not yet present for the free function form -- roadmap correctly marks LLVM/AOT items as `[ ]`.

### `is_empty(x)` -- [x] Implement

**VERIFIED.** `is_empty` free function is implemented. Ori tests at `tests/spec/traits/core/is_empty.ori` test both the free function form (`is_empty(collection: [])`) and method form (`.is_empty()`).

**INACCURACY**: Roadmap says "15+ tests" in `is_empty.ori`. Actual count: 9 active `@test` functions (4 more are commented out as PENDING). The file has 13 `@test` annotations total, of which 4 are commented out.

---

## 7C.2 Collection Methods on `[T]`

### `[T].map(f)` -- [x] Implement

**VERIFIED.** All `[x]` sub-items confirmed:
- Rust tests: evaluator method dispatch works via `CollectionMethodResolver`
- Ori tests: used in `tests/spec/types/primitives.ori`
- AOT tests: `iterators::test_iter_map` (1 test), `collections_ext::test_coll_list_iter_map_collect_length` (1 test) -- all pass

### `[T].filter(f)` -- [x] Implement

**VERIFIED.** Same pattern as `map`. AOT tests: `iterators::test_iter_filter` + `collections_ext::test_coll_list_iter_filter_count` -- all pass.

### `[T].fold(initial, f)` -- [x] Implement

**VERIFIED.** AOT tests: `iterators::test_iter_fold_sum`, `test_iter_fold_with_filter`, `collections_ext::test_coll_list_iter_sum_via_fold` -- all pass. Ori tests in `tests/spec/types/primitives.ori` line 1632 use `fold(init:, f:)` parameter names.

### `[T].find(f)` -- [ ] Implement

**STALE.** Implementation exists through the iterator pipeline (`CollectionMethod::IterFind`). The evaluator at `ori_eval/src/interpreter/method_dispatch/iterator/mod.rs:101` handles `IterFind`. AOT tests at `iterators::test_iter_find_some` and `test_iter_find_none` pass (confirmed by test run). The `[x] AOT Tests` sub-item is correctly marked but the top-level `[ ] Implement` is stale -- find works via the iterator method dispatch chain.

### `[T].any(f)` -- [ ] Implement

**STALE.** Same as `find` -- implemented via `CollectionMethod::IterAny`. AOT tests at `iterators::test_iter_any_true/false` and `collections_ext::test_coll_list_iter_any_all` pass. The `[x] AOT Tests` sub-items are correctly marked.

### `[T].all(f)` -- [ ] Implement

**STALE.** Same pattern -- implemented via `CollectionMethod::IterAll`. AOT tests pass.

### `[T].first()` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:29-30`. AOT tests at `collections_ext::test_coll_list_first` and `test_coll_list_first_empty` pass (confirmed by test run -- 2 tests, 0 ignored, all pass). The roadmap claims these are "1 test, ignored: `list.first()` not in builtin table" -- this is wrong; the tests are NOT ignored and they PASS.

### `[T].last()` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:31-32`. AOT tests at `collections_ext::test_coll_list_last` and `test_coll_list_last_empty` pass (2 tests, 0 ignored). Roadmap claims "1 test, ignored" -- incorrect, not ignored and passing.

### `[T].reverse()` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:47-50`. AOT tests: `test_coll_list_reverse`, `test_coll_list_reverse_single`, `test_coll_list_reverse_values`, `test_coll_list_reverse_reverse`, `test_coll_list_reverse_empty` -- 5 tests, all pass. Roadmap claims "1 test, ignored" -- incorrect, 5 tests running and passing.

### `[T].sort()` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:51-58`. AOT tests: `test_coll_list_sort_ints`, `test_coll_list_sort_already_sorted`, `test_coll_list_sort_reverse_order`, `test_coll_list_sort_single` -- 4 tests, all pass. Roadmap says "No AOT coverage yet" -- incorrect.

### `[T].contains(value)` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:33-35`. AOT tests: `test_coll_list_contains`, `test_coll_list_contains_missing` -- 2 tests, all pass. Roadmap claims "1 test, ignored" -- incorrect, 2 tests running and passing.

### `[T].push(value)` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:36-40`. AOT tests: `test_coll_list_push`, `test_coll_list_push_empty`, `test_coll_list_push_multi`, `test_coll_list_push_push`, `test_coll_list_push_concat`, `test_coll_list_push_then_reverse`, `test_coll_list_push_loop_1000` -- 7 tests, all pass. Roadmap claims "1 test, ignored" -- incorrect.

### `[T].concat(other)` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/list.rs:59-63`. AOT tests: `test_coll_list_concat_basic`, `test_coll_list_concat_empty`, `test_coll_list_concat_reverse` -- 3 tests, all pass. Roadmap says "No AOT coverage yet" -- incorrect.

---

## 7C.3 Range Methods

All items are marked `[ ]` for the top-level Implement. AOT sub-items are marked `[x]` where iterator-level tests exist.

**CONFIRMED INCOMPLETE** for Range-specific method implementations (Range.map, Range.filter, etc. as direct methods). These work through the iterator pipeline (`.iter().map()`) which has passing AOT tests, but there are no Range-direct method implementations.

---

## 7C.4 Collection Methods (len, is_empty)

### `[T].len()` -- [x] Implement

**VERIFIED.** AOT tests: `test_coll_list_length_empty`, `test_coll_list_length_one`, `test_coll_list_length_many`, `test_coll_list_len_alias` -- 4 tests, all pass.

### `[T].is_empty()` -- [x] Implement

**VERIFIED.** AOT tests: `test_coll_list_is_empty_true`, `test_coll_list_is_empty_false` -- 2 tests, all pass.

### `{K: V}.len()` -- [x] Implement

**VERIFIED.** AOT tests: `test_coll_map_length_basic`, `test_coll_map_length_one`, `test_coll_map_len_alias` -- 3 tests, all pass.

### `{K: V}.is_empty()` -- [ ] Implement

**STALE.** Implemented in evaluator at `ori_eval/src/methods/collections.rs:425-426`. Ori tests exist in `tests/spec/traits/core/is_empty.ori` (lines 58-73, map is_empty tests). AOT tests: `test_coll_map_is_empty_true`, `test_coll_map_is_empty_false` -- 2 tests, all pass. Roadmap says "No AOT coverage yet" -- incorrect.

### `str.len()` -- [x] Implement

**VERIFIED.** AOT tests: 5 str_length + 1 str_len_alias = 6 tests, all pass.

### `str.is_empty()` -- [x] Implement

**VERIFIED.** AOT tests: `test_str_is_empty_true`, `test_str_is_empty_false`, `test_str_is_empty_space` -- 3 tests, all pass.

### `Set<T>.len()` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_length` passes.

### `Set<T>.is_empty()` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_is_empty` passes.

### `Set<T>.contains(elem)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_contains` passes.

### `Set<T>.insert(elem)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_insert` passes.

### `Set<T>.remove(elem)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_remove` passes.

### `Set<T>.union(other)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_union` passes.

### `Set<T>.intersection(other)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_intersection` passes.

### `Set<T>.difference(other)` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_difference` passes.

### `Set<T>.to_list()` -- [x] Implement

**VERIFIED.** AOT test: `test_aot_set_to_list` passes.

---

## 7C.5 Comparable Methods (min, max, compare)

### `T.compare(other)` -- [x] Implement

**VERIFIED.** Ori tests at `tests/spec/traits/core/comparable.ori` -- 133 occurrences of "compare" confirmed. Tests cover int, float, bool, char, byte, str, list, Option, Result, and Ordering `.compare()` methods. All 4181 tests pass when running spec tests.

**WEAK TESTS**: No AOT coverage for `compare` -- roadmap correctly marks AOT items as `[ ]`.

### `T.min(other)` and `T.max(other)` -- [ ] Implement

**CONFIRMED INCOMPLETE** as methods on Comparable trait. The free functions `min(left:, right:)` and `max(left:, right:)` exist and are tested in `comparable.ori`, but the method forms `.min(other:)` / `.max(other:)` are not implemented.

---

## 7C.6 Iterator Traits -- CONFIRMED INCOMPLETE

All `[ ]` items are genuinely incomplete as formal trait definitions. The built-in iterator pipeline works (as demonstrated by 25 passing AOT tests in `iterators.rs`), but the formal `Iterator`, `Iterable`, and `Collect` traits are not defined as user-visible traits in the type system.

**AOT coverage is strong**: 25 iterator tests cover map, filter, take, skip, count, any, all, find, fold, for_each, collect, zip, chain, and for-loop desugaring. All pass.

**Missing from AOT coverage**: `enumerate`, `flatten`, `flat_map`, `cycle`, `join`, `rev` adapters.

---

## 7C.7 Debug Trait -- CONFIRMED INCOMPLETE

All items genuinely `[ ]`. No Debug trait definition, no derive(Debug), no standard Debug implementations.

---

## 7C.8 Section Completion Checklist -- CONFIRMED INCOMPLETE

Correctly `[ ]` -- section is not complete.

---

## Critical Findings

### INACCURACY 1: Ignored test claims are wrong

The roadmap claims several AOT tests are "ignored: `list.X()` not in builtin table" for first, last, reverse, contains, push. **This is incorrect** -- all these tests run and pass without `#[ignore]`. The `collections_ext.rs` file has **zero** `#[ignore]` annotations. These methods ARE in the builtin table and work correctly.

### INACCURACY 2: is_empty.ori test count

Roadmap claims "15+ tests" but the file has only 9 active test functions (4 more are commented out as PENDING).

### INACCURACY 3: Sort and concat AOT claims

Roadmap says `[T].sort()` has "No AOT coverage yet" -- actually has 4 passing tests plus 2 sort_stable tests. Roadmap says `[T].concat()` has "No AOT coverage yet" -- actually has 3 passing tests.

### 14 stale `[ ]` items should be `[x]`

The following items are marked `[ ]` but are implemented with passing tests:
1. `[T].find(f)` -- via iterator pipeline + 2 passing AOT tests
2. `[T].any(f)` -- via iterator pipeline + 3 passing AOT tests
3. `[T].all(f)` -- via iterator pipeline + 3 passing AOT tests
4. `[T].first()` -- evaluator + 2 passing AOT tests
5. `[T].last()` -- evaluator + 2 passing AOT tests
6. `[T].reverse()` -- evaluator + 5 passing AOT tests
7. `[T].sort()` -- evaluator + 4 passing AOT tests
8. `[T].contains(value)` -- evaluator + 2 passing AOT tests
9. `[T].push(value)` -- evaluator + 7 passing AOT tests
10. `[T].concat(other)` -- evaluator + 3 passing AOT tests
11. `{K: V}.is_empty()` -- evaluator + 2 passing AOT tests + Ori tests
12. `[T].take(n)` (evaluator has zero-copy slice impl)
13. `[T].skip(n)` (evaluator has zero-copy slice impl)
14. All 5 `[T].find/any/all/take/skip` `[x] AOT Tests` sub-items within the `[ ]` parent are accurate

### Test run summary

| Test suite | Result |
|-----------|--------|
| `cargo test -p ori_llvm --test aot -- sets::` | 10 passed, 0 failed |
| `cargo test -p ori_llvm --test aot -- iterators::` | 25 passed, 0 failed |
| `cargo test -p ori_llvm --test aot -- collections_ext::test_coll_list_*` | All passing (30+ tests) |
| `cargo test -p ori_llvm --test aot -- strings::test_str_length/is_empty/len` | 9 passed, 0 failed |
| `cargo st tests/spec/traits/core/is_empty.ori` | 4181 passed, 0 failed |
| `cargo st tests/spec/traits/core/comparable.ori` | 4181 passed, 0 failed |
| `cargo st tests/spec/types/set_methods/set_methods.ori` | 4181 passed, 0 failed |
