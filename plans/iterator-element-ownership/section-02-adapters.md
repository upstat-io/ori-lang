---
section: "02"
title: "Adapters: Decrement on Discard"
status: not-started
reviewed: false
goal: "Adapters that discard elements (filter, skip) call elem_dec_fn; adapters that pass through or produce inherit correct ownership"
success_criteria:
  - "filter adapter calls elem_dec_fn on predicate-rejected elements"
  - "skip adapter calls elem_dec_fn on skipped elements"
  - "cycle adapter correctly handles buffered element RC"
  - "rev adapter correctly handles collected element RC"
  - "map, take, chain, enumerate, zip, flatten pass through ownership unchanged"
sections:
  - id: "02.1"
    title: "Filter: Dec rejected elements"
    status: not-started
  - id: "02.2"
    title: "Skip: Dec skipped elements"
    status: not-started
  - id: "02.3"
    title: "Cycle and Rev: Storage ownership"
    status: not-started
  - id: "02.4"
    title: "Pass-through adapters: Verify"
    status: not-started
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
---

# 02 Adapters: Decrement on Discard

With Section 01 complete, every yielded element is owned (+1 RC). Adapters that **discard** elements (filter rejections, skip's N elements) must now dec them to prevent leaks. Adapters that **pass through** elements (take, chain, enumerate) already have correct ownership — the element flows from source to consumer unchanged. Adapters that **produce new** elements (map, flat_map) already yield owned values.

## 02.1 Filter: Dec Rejected Elements

**File:** `compiler/ori_rt/src/iterator/next.rs` — `next_filtered` (lines 157-172)

Currently, rejected elements are read into a scratch buffer and silently discarded when the loop re-advances. With owned elements, these must be dec'd.

- [ ] Add `elem_dec_fn: Option<extern "C" fn(*mut u8)>` to `IterState::Filtered` variant in `state.rs`
- [ ] Update `ori_iter_filter` in `adapters.rs` to accept and store `elem_dec_fn`
- [ ] In `next_filtered`: after predicate returns false, call `elem_dec_fn(scratch.as_mut_ptr())` on the rejected element
- [ ] **TDD:** AOT test — `[1,2,3,4,5].iter().filter(x -> x % 2 == 0).collect()` with str elements, verify zero leaks
- [ ] Codegen update (Section 04): `emit_iter_filter` must pass `get_or_generate_elem_dec_fn(elem_ty)`

- [ ] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.2 Skip: Dec Skipped Elements

**File:** `compiler/ori_rt/src/iterator/next.rs` — `next_skip` (lines 191-206)

Same pattern as filter. Skipped elements are read into scratch and discarded.

- [ ] Add `elem_dec_fn: Option<extern "C" fn(*mut u8)>` to `IterState::SkipN` variant in `state.rs`
- [ ] Update `ori_iter_skip` in `adapters.rs` to accept and store `elem_dec_fn`
- [ ] In `next_skip`: after each skipped element, call `elem_dec_fn(scratch.as_mut_ptr())`
- [ ] **TDD:** AOT test — `["a","b","c","d"].iter().skip(count: 2).join(separator: ",")` with str elements, verify zero leaks
- [ ] Codegen update (Section 04): `emit_iter_skip` must pass `get_or_generate_elem_dec_fn(elem_ty)`

- [ ] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.3 Cycle and Rev: Storage Ownership

**Files:** `adapters.rs` (ori_iter_cycle, ori_iter_rev), `next.rs` (next_cycled, next_reversed)

These adapters eagerly collect elements into a `Vec<u8>` buffer. With owned elements:

- [ ] **cycle**: First pass stores owned elements into buffer (they were yielded as +1 from source). Buffer now owns them. On replay passes, the buffer yields copies — these need inc (same as list yields). Add `elem_inc_fn` to `IterState::Cycled`.
- [ ] **rev**: Collects all elements into `Vec<u8>`, drops source. Elements in the Vec are owned. When yielding backward, the bytes are copied out — but since the Vec will be freed, no additional inc needed (transfer semantics). Verify the Vec cleanup path calls `elem_dec_fn` for remaining elements if iteration terminates early (break/take).
- [ ] **TDD:** AOT tests for cycle + rev with str elements under leak check

- [ ] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.4 Pass-Through Adapters: Verify

**Adapters:** take, chain, enumerate, zip, flatten, flat_map

These adapters do not discard elements — they pass them through from source to consumer. With owned elements from Section 01, the ownership flows naturally.

- [ ] **take**: passes through first N elements, then stops. Source's remaining elements are cleaned up by the source's Drop (which already handles RC). No change needed. Verify.
- [ ] **chain**: passes through all elements from `first` then `second`. Ownership flows from each source. No change needed. Verify.
- [ ] **enumerate**: wraps element in `(index, element)` tuple. The element's ownership transfers into the tuple. No change needed. Verify.
- [ ] **zip**: concatenates bytes from two sources. Both sources yield owned elements. The zip output tuple owns both. No change needed. Verify.
- [ ] **flatten**: yields elements from nested iterators. Inner iterators yield owned elements. No change needed. Verify.
- [ ] **flat_map**: composition of map + flatten. Both already handle ownership. No change needed. Verify.

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting Section 03:
  - [ ] All tasks above are `[x]` and behavior verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any detected temp files.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] Filter and skip adapters correctly dec discarded elements
- [ ] Cycle correctly handles first-pass storage and replay inc
- [ ] Rev correctly handles collected element cleanup
- [ ] All pass-through adapters verified (no change needed)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
