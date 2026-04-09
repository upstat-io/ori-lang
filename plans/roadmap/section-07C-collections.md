---
section: 7C
title: Collections & Iteration
status: in-progress
reviewed: true
last_verified: "2026-03-29"
verification_summary: "26 verified, 36 stale checkboxes fixed, 8 NEEDS TESTS, 5 missing items added, #skip budget violation noted"
tier: 2
goal: Collection methods, iterator traits, and Debug trait
spec:
  - spec/annex-c-built-in-functions.md
sections:
  - id: "7C.1"
    title: Collection Functions
    status: done
  - id: "7C.2"
    title: Collection Methods on [T]
    status: in-progress
    notes: "5 missing list methods added (pop/insert/remove/slice/updated); 8 methods NEEDS TESTS (spec tests)"
  - id: "7C.3"
    title: Range Methods
    status: in-progress
    notes: "Direct Range methods not implemented; functionality works through .iter() pipeline"
  - id: "7C.4"
    title: Collection Methods (len, is_empty)
    status: done
  - id: "7C.5"
    title: Comparable Methods (min, max, compare)
    status: in-progress
    notes: "min/max as generic Comparable methods not implemented; compare verified for interpreter"
  - id: "7C.6"
    title: Iterator Traits
    status: in-progress
    notes: "Core traits verified; DoubleEndedIterator and join() added as tracked items; 19 extended methods not started"
  - id: "7C.7"
    title: Debug Trait
    status: in-progress
    notes: "Interpreter verified; no LLVM codegen for Debug yet; sum type derive test missing"
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

- [x] **Implement**: `len(x)` — spec/annex-c-built-in-functions.md § len (deprecated, use `.len()`) [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator builtin — len function tests
  - [x] **Ori Tests**: Used extensively in test suite via `.len()` method
  - [x] **LLVM Support**: Free function delegates to method form; method `.len()` has full LLVM codegen (see 7C.4) (verified 2026-03-29)
  - [x] **AOT Tests**: Method form `.len()` covered in 7C.4 with passing AOT tests (verified 2026-03-29)

- [x] **Implement**: `is_empty(x)` — spec/annex-c-built-in-functions.md § is_empty (deprecated, use `.is_empty()`) [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator builtin — is_empty function tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 10 test functions (verified 2026-03-29)
  - [x] **LLVM Support**: Free function delegates to method form; method `.is_empty()` has full LLVM codegen (see 7C.4) (verified 2026-03-29)
  - [x] **AOT Tests**: Method form `.is_empty()` covered in 7C.4 with passing AOT tests (verified 2026-03-29)

---

## 7C.2 Collection Methods on `[T]`

> **Design Principle**: Lean core, rich libraries. Data transformation is stdlib, not compiler patterns.

- [x] **Implement**: `[T].map(f: T -> U) -> [U]` — modules/prelude.md § List [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — list map tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_map; also used in `tests/spec/types/primitives.ori`, `tests/spec/lexical/keywords.ori` (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_map` (iterator-level `.map()`, 1 test); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_map_collect_length` (1 test)

- [x] **Implement**: `[T].filter(f: T -> bool) -> [T]` — modules/prelude.md § List [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — list filter tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_filter; `tests/spec/lexical/operators.ori` — `list.filter(predicate:)` (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_filter` (iterator-level `.filter()`, 1 test); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_filter_count` (1 test)

- [x] **Implement**: `[T].fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § List [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — list fold tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_fold; `tests/spec/lexical/keywords.ori` — fold usage (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_fold_sum`, `test_iter_fold_with_filter` (iterator-level `.fold()`, 2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_sum_via_fold` (1 test)

- [x] **Implement**: `[T].find(f: T -> bool) -> Option<T>` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_find, iter_find_none (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_find_some`, `test_iter_find_none` (iterator-level `.find()`, 2 tests)

- [x] **Implement**: `[T].any(f: T -> bool) -> bool` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_any_true, iter_any_false, iter_empty_any (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_any_true`, `test_iter_any_false` (iterator-level `.any()`, 2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_any_all` (1 test)

- [x] **Implement**: `[T].all(f: T -> bool) -> bool` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_all_true, iter_all_false, iter_empty_all (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_all_true`, `test_iter_all_false` (iterator-level `.all()`, 2 tests); `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_iter_any_all` (1 test)

- [x] **Implement**: `[T].first() -> Option<T>` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.first()`; need `tests/spec/collections/list/first.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 2 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_first`, `test_coll_list_first_empty` (2 tests, passing)

- [x] **Implement**: `[T].last() -> Option<T>` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.last()`; need `tests/spec/collections/list/last.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 2 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_last`, `test_coll_list_last_empty` (2 tests, passing)

- [x] **Implement**: `[T].take(n: int) -> [T]` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_take, iter_take_excess (with edge case) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_take` (iterator-level take adapter, 1 test)

- [x] **Implement**: `[T].skip(n: int) -> [T]` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — iter_skip, iter_skip_excess (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_skip` (iterator-level skip adapter, 1 test)

- [x] **Implement**: `[T].reverse() -> [T]` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.reverse()`; need `tests/spec/collections/list/reverse.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 6 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_reverse` + 5 variants (6 tests, passing)

- [x] **Implement**: `[T].sort() -> [T]` where `T: Comparable` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.sort()`; need `tests/spec/collections/list/sort.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 7 passing AOT tests including COW (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_sort_ints` + 6 variants including COW (7 tests, passing) [done] (verified 2026-03-28)

- [x] **Implement**: `[T].contains(value: T) -> bool` where `T: Eq` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.contains()`; need `tests/spec/collections/list/contains.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 2 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_contains`, `test_coll_list_contains_missing` (2 tests, passing)

- [x] **Implement**: `[T].push(value: T) -> [T]` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.push()`; need `tests/spec/collections/list/push.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 7 passing AOT tests including COW (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_push` + 6 variants including COW (7 tests, passing)

- [x] **Implement**: `[T].concat(other: [T]) -> [T]` — modules/prelude.md § List [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via AOT tests
  - [ ] **Ori Tests**: NEEDS TESTS — no Ori spec test exists for `.concat()`; need `tests/spec/collections/list/concat.ori`
  - [x] **LLVM Support**: Works via AOT codegen; 6 passing AOT tests including COW (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_concat_basic` + 5 variants including COW (6 tests, passing) [done] (verified 2026-03-28)

> **GAP (identified 2026-03-29)**: The following list methods from the prelude spec are not tracked in this section. They may need to be added or confirmed as covered elsewhere.

- [ ] **Implement**: `[T].pop() -> (Option<T>, [T])` — modules/prelude.md § List
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `[T].insert(index: int, value: T) -> [T]` — modules/prelude.md § List
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `[T].remove(index: int) -> [T]` — modules/prelude.md § List
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `[T].slice(start: int, end: int) -> [T]` — modules/prelude.md § List
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `[T].updated(key: int, value: T) -> [T]` — modules/prelude.md § List
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7C.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7C.3 Range Methods

> **NOTE (2026-03-29)**: Direct `Range.map()` / `Range.filter()` / etc. methods do not exist. This functionality works through the `.iter()` pipeline (e.g., `(0..10).iter().map(f).collect()`). These items are correctly `[ ]` — consider whether direct Range methods are needed or if iterator pipeline is sufficient.

- [ ] **Implement**: `Range.map(f: T -> U) -> [U]` — modules/prelude.md § Range — WEAK TESTS: works through `.iter()` pipeline but no direct method
  - [ ] **Ori Tests**: NEEDS TESTS (spec tests through `.iter().map()` exist in `methods.ori`)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_map` tests map on list iterator; `test_chained_map_filter_take` tests map on range-derived iterator

- [ ] **Implement**: `Range.filter(f: T -> bool) -> [T]` — modules/prelude.md § Range — WEAK TESTS: works through `.iter()` pipeline but no direct method
  - [ ] **Ori Tests**: NEEDS TESTS (spec tests through `.iter().filter()` exist in `methods.ori`)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_filter` tests filter on list iterator; `test_chained_map_filter_take` tests filter on range-derived iterator

- [ ] **Implement**: `Range.fold(initial: U, f: (U, T) -> U) -> U` — modules/prelude.md § Range — WEAK TESTS: works through `.iter()` pipeline but no direct method
  - [ ] **Ori Tests**: NEEDS TESTS (spec tests through `.iter().fold()` exist in `methods.ori`)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_iter_fold_sum`, `test_iter_fold_with_filter` test fold on range-derived iterators via chaining

- [ ] **Implement**: `Range.collect() -> [T]` — modules/prelude.md § Range — works through `.iter().collect()` but no direct method
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` has `range_iter_collect` test via `.iter().collect()`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_range_iter_collect` (range `.iter().collect()` to list, 1 test)

- [ ] **Implement**: `Range.contains(value: T) -> bool` — modules/prelude.md § Range
  - [ ] **Ori Tests**: NEEDS TESTS
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7C.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7C.4 Collection Methods (len, is_empty)

> Move from free functions to methods on collections.

- [x] **Implement**: `[T].len() -> int` — modules/prelude.md § List [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — list len tests
  - [x] **Ori Tests**: `tests/spec/expressions/field_access.ori`, `tests/spec/lexical/delimiters.ori` (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen; 4 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_length_empty` + 2 variants, `test_coll_list_len_alias` (4 tests, 0 ignored)

- [x] **Implement**: `[T].is_empty() -> bool` — modules/prelude.md § List [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — list is_empty tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 15+ tests (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen; 2 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_list_is_empty_true`, `test_coll_list_is_empty_false` (2 tests, 0 ignored)

- [x] **Implement**: `{K: V}.len() -> int` — modules/prelude.md § Map [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — map len tests
  - [x] **Ori Tests**: `tests/spec/lexical/delimiters.ori` — `map.len()` tests (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen; 3 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_map_length_basic`, `test_coll_map_length_one`, `test_coll_map_len_alias` (3 tests, 0 ignored)

- [x] **Implement**: `{K: V}.is_empty() -> bool` — modules/prelude.md § Map [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — tested via spec tests and AOT tests (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 2 map is_empty tests [done] (verified 2026-03-28)
  - [x] **LLVM Support**: Works via AOT codegen; 2 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/collections_ext.rs` — `test_coll_map_is_empty_true`, `test_coll_map_is_empty_false` (2 tests, passing) [done] (verified 2026-03-28)

- [x] **Implement**: `str.len() -> int` — modules/prelude.md § str [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — str len tests
  - [x] **Ori Tests**: `tests/spec/expressions/field_access.ori` — `"hello".len()` (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen; 6 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/strings.rs` — `test_str_length_basic` + 4 variants, `test_str_len_alias` (6 tests, 0 ignored)

- [x] **Implement**: `str.is_empty() -> bool` — modules/prelude.md § str [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — str is_empty tests
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — `"".is_empty()`, `!"hello".is_empty()` (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen; 3 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/strings.rs` — `test_str_is_empty_true`, `test_str_is_empty_false`, `test_str_is_empty_space` (3 tests, 0 ignored)

- [x] **Implement**: `Set<T>.len() -> int` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/methods/helpers/mod.rs` — set len in `TYPECK_BUILTIN_METHODS`
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set len tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_length()` (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_length` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.is_empty() -> bool` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/methods/helpers/mod.rs` — set is_empty in `TYPECK_BUILTIN_METHODS`
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set is_empty tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_is_empty()` (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_is_empty` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.contains(elem) -> bool` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set contains dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set contains tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_contains()` via `ori_set_contains` runtime (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_contains` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.insert(elem) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set insert dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set insert tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_insert()` via `ori_set_insert` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_insert` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.remove(elem) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set remove dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set remove tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_remove()` via `ori_set_remove` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_remove` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.union(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set union dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set union tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_union()` via `ori_set_union` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_union` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.intersection(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set intersection dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set intersection tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_intersection()` via `ori_set_intersection` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_intersection` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.difference(other) -> Set<T>` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set difference dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set difference tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_difference()` via `ori_set_difference` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_difference` (passing) (2026-03-01) (verified 2026-03-29)

- [x] **Implement**: `Set<T>.to_list() -> [T]` — modules/prelude.md § Set [done] (2026-02-25) (verified 2026-03-29)
  - [x] **Evaluator**: `ori_eval/src/methods/collections.rs` — set to_list dispatch
  - [x] **Ori Tests**: `tests/spec/types/set_methods/set_methods.ori` — set to_list tests (verified 2026-03-29)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/builtins/collections.rs` — `emit_set_to_list()` via `ori_set_to_list` runtime (sret) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — `test_aot_set_to_list` (passing) (2026-03-01) (verified 2026-03-29)

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

- [x] **Implement**: `T.compare(other: T) -> Ordering` where `T: Comparable` — modules/prelude.md § Comparable [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — compare method tests
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — 133 test occurrences (verified 2026-03-29)
  - [ ] **LLVM Support**: No LLVM codegen for compare method yet
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7C.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7C.6 Iterator Traits

> **PROPOSAL**: `proposals/approved/iterator-traits-proposal.md`
>
> Formalize iteration with traits, enabling user types in `for` loops and generic iteration.

- [x] **Implement**: `Iterator` trait (protocol implemented, formal trait definition pending) [done] (verified 2026-03-28) (verified 2026-03-29)
  ```ori
  trait Iterator {
      type Item
      @next (mut self) -> Option<Self.Item>
  }
  ```
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/iterator.ori` — core iterator protocol (.iter(), .next(), fused behavior, 7+ tests); 13 dedicated spec test files total [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; AOT tests in `ori_llvm/tests/aot/iterators.rs` demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — 25 tests covering iterator pipeline (verified 2026-03-29)

- [x] **Implement**: `Iterable` trait (protocol implemented, formal trait definition pending) [done] (verified 2026-03-28) (verified 2026-03-29)
  ```ori
  trait Iterable {
      type Item
      @iter (self) -> impl Iterator where Item == Self.Item
  }
  ```
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/for_loop.ori` — for-loop desugaring for list, range, str, set, map, Option [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via for-loop codegen; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_for_over_iterator`, `test_for_over_range_iterator` (verified 2026-03-29)

- [x] **Implement**: `Collect` trait (collect-to-list works, formal trait definition pending) [done] (verified 2026-03-28) (verified 2026-03-29)
  ```ori
  trait Collect<T> {
      @from_iter (iter: impl Iterator where Item == T) -> Self
  }
  ```
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/collect.ori` — collect_default_list [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Collect-to-list (`ori_iter_collect`) and collect-to-set (`ori_iter_collect_set`) both implemented (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_list_iter_collect`, `test_iter_chain_collect` (verified 2026-03-29)
  > NOTE: `tests/spec/traits/iterator/collect_set.ori` has 6 `#skip` markers (budget is 3 per tests.md) — type-directed collect to Set not fully implemented in evaluator

- [x] **Implement**: Standard `Iterable` implementations [done] (verified 2026-03-28) (verified 2026-03-29)
  - `impl<T> [T]: Iterable` — list iteration
  - `impl<K, V> {K: V}: Iterable` — map iteration (yields tuples)
  - `impl<T> Set<T>: Iterable` — set iteration
  - `impl str: Iterable` — character iteration
  - `impl Range<int>: Iterable` — range iteration
  - `impl<T> Option<T>: Iterable` — zero/one element
  - [x] **Ori Tests**: `tests/spec/traits/iterator/for_loop.ori` — all iterable types tested (list, range, str, set, map, Option) [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via AOT codegen for list, range, map, str iterables (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — list `.iter()` (2 tests), range `.iter()` (2 tests); `ori_llvm/tests/aot/collections_ext.rs` — map `.iter()` (2 tests), string `.iter()` via `ori_llvm/tests/aot/strings.rs` (3 tests) (verified 2026-03-29)

- [x] **Implement**: Standard `Collect` implementations (collect-to-list works; collect-to-set LLVM only) [done] (verified 2026-03-28) (verified 2026-03-29)
  - `impl<T> Collect<T> for [T]` — collect to list
  - `impl<T> Collect<T> for Set<T>` — collect to set (LLVM `__collect_set` implemented 2026-03-01)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/collect.ori` — collect_default_list [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: LLVM codegen for standard Collect implementations — collect-to-list (`ori_iter_collect`) and collect-to-set (`ori_iter_collect_set` via `__collect_set` intercept) both implemented (2026-03-01) (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_list_iter_collect`, `test_iter_chain_collect` (collect to list via `.iter().collect()`) (verified 2026-03-29)

- [x] **Implement**: `for` loop desugaring to `.iter()` and `.next()` [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/for_loop.ori` — 16+ tests covering all iterable types [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via for-loop codegen; AOT tests demonstrate LLVM codegen (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — `test_for_over_iterator`, `test_for_over_range_iterator` (for-loop over `.iter()` on lists and ranges) (verified 2026-03-29)

- [x] **Implement**: Iterator extension methods [done] (verified 2026-03-28) (verified 2026-03-29)
  - `map`, `filter`, `fold`, `find`, `collect`, `count`
  - `any`, `all`, `take`, `skip`, `enumerate`, `zip`, `chain`
  - Also: `flatten`, `flat_map`, `cycle`, `for_each`, `join`
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — 50+ tests covering all methods with exact value checks and edge cases [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **LLVM Support**: Works via iterator pipeline; 25 passing AOT tests (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/iterators.rs` — 25 tests (0 ignored) covering map, filter, take, skip, count, any, all, find, fold, for_each, collect, zip, chain adapters/consumers via built-in iterator pipeline (verified 2026-03-29)

- [ ] **Implement**: Extended Iterator methods (19 new methods)
  **Proposal**: `proposals/approved/iterator-extended-methods-proposal.md` [approved] (2026-03-22)
  - [ ] **Phase 1 — Reductions**: `max`, `min`, `max_by`, `min_by`, `max_by_key`, `min_by_key`, `sum`, `sum_by`, `product`, `reduce`
    - [ ] Registry: Add 10 `MethodDef` entries to `ori_registry/src/defs/iterator/mod.rs`
    - [ ] Type checker: Method resolution in `ori_types`
    - [ ] Evaluator: Consumer dispatch in `ori_eval/method_dispatch/iterator/consumers.rs`
    - [ ] **Ori Tests**: `tests/spec/traits/iterator/reductions.ori`
  - [ ] **Phase 2 — Adapters**: `filter_map`, `take_while`, `skip_while`
    - [ ] Registry: Add 3 `MethodDef` entries
    - [ ] Evaluator: New `IteratorValue` variants (`FilterMap`, `TakeWhile`, `SkipWhile`)
    - [ ] Evaluator: `next()` dispatch in `ori_eval/method_dispatch/iterator/next.rs`
    - [ ] **Ori Tests**: `tests/spec/traits/iterator/adapters_ext.ori`
  - [ ] **Phase 3 — Index/Partition/Inspect**: `step_by`, `inspect`, `position`, `nth`, `partition`, `rposition`
    - [ ] Registry: Add 6 `MethodDef` entries
    - [ ] Evaluator: New `IteratorValue` variants (`StepBy`, `Inspect`)
    - [ ] Evaluator: Consumer dispatch for `position`, `nth`, `partition`, `rposition`
    - [ ] **Ori Tests**: `tests/spec/traits/iterator/index_partition.ori`
  - [ ] **Phase 4 — LLVM Codegen**: AOT support for all 19 new methods
    - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/iterators_ext.rs`

> **GAP (identified 2026-03-29)**: The following iterator features have tests but are not tracked in this section.

- [x] **Implement**: `DoubleEndedIterator` trait — modules/prelude.md § DoubleEndedIterator (verified 2026-03-29)
  - Methods: `.rev`, `.last`, `.rfind`, `.rfold`
  - [x] **Ori Tests**: `tests/spec/traits/iterator/double_ended.ori`, `double_ended_methods.ori`, `double_ended_gating.ori` — tests exist and pass (verified 2026-03-29)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Iterator `.join(separator:)` method (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/debug/join.ori` — tests exist and pass (verified 2026-03-29)
  - [ ] **AOT Tests**: No AOT coverage yet

> **QUALITY NOTE (2026-03-29)**: No negative tests (`#compile_fail`) exist anywhere in this section. Missing coverage for: calling `.sort()` on non-Comparable type, calling `.contains()` with wrong element type, using `Set<T>` where T is not Hashable, collecting to wrong type.

- [ ] **Subsection close-out (7C.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7C.7 Debug Trait

> **PROPOSAL**: `proposals/drafts/debug-trait-proposal.md`
>
> Developer-facing structural output, separate from user-facing `Printable`.

- [x] **Implement**: `Debug` trait [done] (verified 2026-03-28) (verified 2026-03-29)
  ```ori
  trait Debug {
      @debug (self) -> str
  }
  ```
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/debug/definition.ori` — Debug trait definition and debug vs printable distinction [done] (verified 2026-03-28) (verified 2026-03-29)
  - [ ] **LLVM Support**: No LLVM codegen for Debug trait yet
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `#[derive(Debug)]` for structs and sum types [done] (verified 2026-03-28) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator — tested via spec test suite (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/traits/debug/derive.ori` — derived debug on structs [done] (verified 2026-03-28) (verified 2026-03-29)
  > NOTE: NEEDS TESTS — only struct derive tested, no sum type derive test in derive.ori
  - [ ] **LLVM Support**: No LLVM codegen for derive(Debug) yet
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Standard `Debug` implementations [done] (verified 2026-03-28) (verified 2026-03-29)
  - All primitives: `int`, `float`, `bool`, `str`, `char`, `byte`, `void`
  - Collections: `[T]`, `{K: V}`, `Set<T>` (require `T: Debug`)
  - `Option<T>`, `Result<T, E>` (require inner types `Debug`)
  - Tuples (require element types `Debug`)
  - [x] **Ori Tests**: `tests/spec/traits/debug/primitives.ori`, `tests/spec/traits/debug/collections.ori`, `tests/spec/traits/debug/wrappers.ori`, `tests/spec/traits/debug/tuples.ori` — all types covered [done] (verified 2026-03-28) (verified 2026-03-29)
  - [ ] **LLVM Support**: No LLVM codegen for standard Debug implementations yet
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: String escaping in Debug output [done] (verified 2026-03-28) (verified 2026-03-29)
  - `"hello".debug()` → `"\"hello\""`
  - `'\n'.debug()` → `"'\\n'"`
  - [x] **Ori Tests**: `tests/spec/traits/debug/escape.ori` — 12+ tests: newline, tab, cr, backslash, quote escaping [done] (verified 2026-03-28) (verified 2026-03-29)
  - [ ] **LLVM Support**: No LLVM codegen for Debug string escaping yet
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (7C.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7C.8 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Collections and iteration working correctly

- [ ] **Subsection close-out (7C.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7C.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7C.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

