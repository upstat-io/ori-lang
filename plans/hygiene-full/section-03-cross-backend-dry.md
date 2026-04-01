---
section: "03"
title: "Cross-Backend Algorithmic DRY (eval / LLVM)"
status: in-progress
reviewed: true
goal: "Extract shared dispatch metadata between eval and LLVM backends so algorithmic skeletons are defined once"
inspired_by:
  - "ori_registry MethodDef pattern -- shared metadata consumed by multiple backends"
  - "Lean 4 IR/RC.lean -- shared RC decision metadata, backend-specific emission"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Iterator Method List Sync + LLVM Gap Fill"
    status: complete
  - id: "03.2"
    title: "Option/Result LLVM Gap Fill + Routing Enforcement"
    status: in-progress
  - id: "03.3"
    title: "FNV Constant Consolidation"
    status: complete
  - id: "03.4"
    title: "Derive Processing Skeleton Sync Verification"
    status: complete
  - id: "03.5"
    title: "Eval Operator Dispatch via Registry OpStrategy"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Cross-Backend Algorithmic DRY (eval / LLVM)

**Status:** Not Started
**Goal:** The evaluator (`ori_eval`) and LLVM codegen (`ori_llvm`) share dispatch metadata for iterator methods, Option/Result routing, equals/compare/hash, derive processing, and operator dispatch. Each backend retains its own emission logic but the routing decisions — which methods exist, which operations are valid, which tag values mean what — are defined once and consumed from the registry.

**Context:** Both backends implement the same semantic operations with parallel but independent dispatch skeletons. When a new variant or method is added, both backends must be updated independently — and they have already drifted (iterator method lists, Option/Result method coverage). Extracting the shared *metadata* (which methods exist, which tag values mean what, which derive strategy to use) into a shared location eliminates this drift risk. Additionally, both backends define FNV hash constants independently across multiple files, and eval's operator dispatch still uses independent type-based pattern matching instead of querying `OpStrategy` from the registry (as LLVM already does — a result of Section 01).

**Reference implementations:**
- **ori_registry** `compiler/ori_registry/src/defs/iterator/mod.rs`: shared iterator method definitions consumed by all backends
- **ori_ir** `compiler/ori_ir/src/derives/strategy.rs`: `DeriveStrategy` enum shared between eval and LLVM
- **Section 01**: `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs` `emit_binary_op()` — already uses `OpStrategy` from registry; eval must follow the same pattern

**Depends on:** Sections 01, 02 (registry SSOT established first — the shared metadata leverages registry queries and the `OpStrategy` pattern from Section 01).

**What Sections 01 and 02 delivered (prerequisites already met):**
- Section 01: `OpStrategy` is now the canonical operator dispatch mechanism in LLVM (`emit_binary_op()` queries `op_strategy_for_binary()`); `registry_bridge` module established in `ori_types` with `tag_to_type_tag()` and `binary_op_strategy()`
- Section 02: Registry `TypeDef.traits` field added, `trait_name` set on all method defs, trait satisfaction queries through registry

**Test strategy:** This section is partially structural (constants, routing wiring) and partially additive (new LLVM method implementations in 03.1 and 03.2). The two categories have different requirements:

- **Structural/refactoring subsections (03.3, 03.4, 03.5):** No behavioral changes for existing functionality. The matrix is the existing test suite: `timeout 150 ./test-all.sh` must pass unchanged. Semantic pins are Rust-level enforcement tests (compile-time exhaustive match, conformance assertions) that would break if the refactoring is reverted.
- **Additive subsections (03.1, 03.2):** New LLVM code paths must have matrix spec tests covering: exact failing case, edge cases (empty input, single element), cross-type coverage where relevant, semantic pin (a test that ONLY passes via the new LLVM path), and negative pin (a test that would catch fallback-to-no-op regressions). TDD ordering is mandatory: write failing tests, verify they fail, implement, verify they pass unchanged.

All subsections must also satisfy:
- `timeout 150 ./test-all.sh` passes after each subsection (no regression)
- `timeout 150 diagnostics/dual-exec-verify.sh` shows zero new mismatches after each additive subsection
- Debug AND release builds pass (`cargo b` and `cargo b --release`)

---

## 03.1 Iterator Method List Sync + LLVM Gap Fill

**File(s):**
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs` — `emit_iterator_method()` (335 lines)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs` — consumer emit helpers (362 lines)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` — `is_iterator_method()` (517 lines — **already over 500-line limit**)
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs` — enforcement tests
- `compiler/ori_registry/src/defs/iterator/mod.rs` — source of truth (24 methods total, 18 non-DEI + 6 DEI)

**Background:** The registry defines 24 user-callable iterator methods (18 on plain `Iterator`, 6 additional DEI-only). `emit_iterator_method()` handles 15 methods (`__iter_next` + `take`, `skip`, `chain`, `enumerate`, `zip`, `map`, `filter`, `collect`, `count`, `any`, `all`, `find`, `for_each`, `fold`). `is_iterator_method()` handles 17 methods (the same 15 minus `__iter_next`, plus `flat_map`, `flatten`, `join`). The eval backend handles all 24. Both `is_iterator_method()` and `emit_iterator_method()` are independent lists that should be driven by the registry.

**Confirmed missing from LLVM `emit_iterator_method()`:** `flatten`, `flat_map`, `join`, `cycle`, `rev`, `last`, `rfind`, `rfold` (8 methods).
- `flatten`, `flat_map`, `join`: appear in `is_iterator_method()` for auto-iter promotion but have no emit arm — calling them on an iterator type returns `None` silently (the `declare_builtins!` macro falls through to no-op).
- `cycle`, `rev`, `last`, `rfind`, `rfold`: absent from both `is_iterator_method()` and `emit_iterator_method()`.

**Registry note:** `next` and `next_back` have `backend_required: false` in the registry — they are intercepted by the `try_emit_protocol` path before reaching `emit_iterator_method()`. `__iter_next` is NOT in the registry at all (compiler-internal). These two must NOT be included in the registry-driven `is_iterator_method()` logic.

**BLOAT pre-check:** `builtins/mod.rs` is already 517 lines. Before adding new code, check whether new adapter/consumer emit functions should go in `iterator.rs` (adapters) or `iterator_consumers.rs` (consumers) to keep `mod.rs` within limit. Do NOT add new helper functions to `mod.rs`.

**Implementation steps (in order):**

- [x] **Step 0 — Write failing spec tests (TDD, matrix coverage):** (2026-04-01) Tests already existed in `methods.ori` (flatten, flat_map, cycle) and `double_ended_methods.ori` (rev, last, rfind, rfold). Added new `join.ori` spec test with 8 cases covering happy path, edge cases, semantic pin, and cross-feature interactions.
  In `tests/spec/traits/iterator/`, add `.ori` spec tests for each of the 8 missing methods. Each test must `use std.testing { assert_eq }` (not auto-available). See sibling files (e.g., `methods.ori`) for the correct import pattern. Run `timeout 150 cargo st tests/spec/traits/iterator/` and **verify each new test fails** before proceeding to Step 1 — if any test passes at this point, the method is already implemented in LLVM and must be removed from the implementation scope.

  Each test file must cover:
  - **Happy path** (exact failing case): the specific method call that triggered the gap
  - **Edge case**: empty input and/or single-element input where applicable
  - **Semantic pin**: a test that will only pass once the LLVM path is correctly implemented (e.g., a test that checks the result of combining the new method with `.collect()` or a non-trivial fold — not just that it type-checks)
  - Do NOT add a negative pin in spec tests (these are LLVM-level gaps, not semantic errors — there is nothing to reject)

  Concrete test cases:
  - `flatten.ori` — happy: `[[1, 2], [3, 4]].iter().flatten().collect()` → `[1, 2, 3, 4]`; edge: `([]: [[int]]).iter().flatten().collect()` → `[]`; semantic pin: `[[1], [2, 3], [4]].iter().flatten().count()` == `4`
  - `flat_map.ori` — happy: `[1, 2, 3].iter().flat_map(x -> [x, x * 10]).collect()` → `[1, 10, 2, 20, 3, 30]`; edge: `([]: [int]).iter().flat_map(x -> [x]).collect()` → `[]`
  - `join.ori` — happy: `["a", "b", "c"].iter().join(separator: ", ")` → `"a, b, c"` (param name is `separator:` per registry); edge: `["a"].iter().join(separator: ", ")` → `"a"`; `([]: [str]).iter().join(separator: ", ")` → `""`
  - `cycle.ori` — happy: `[1, 2].iter().cycle().take(count: 5).collect()` → `[1, 2, 1, 2, 1]`; semantic pin: `.cycle().take(count: 6).collect()` → `[1, 2, 1, 2, 1, 2]` (exercises wrap-around)
  - `rev.ori` — happy: `[1, 2, 3].iter().rev().collect()` → `[3, 2, 1]`; edge: `([]: [int]).iter().rev().collect()` → `[]`; single: `[42].iter().rev().collect()` → `[42]`
  - `last.ori` — happy: `[1, 2, 3].iter().last()` → `Some(3)`; edge: `([]: [int]).iter().last()` → `None`; single: `[7].iter().last()` → `Some(7)`
  - `rfind.ori` — happy: `[1, 2, 3, 2].iter().rfind(predicate: x -> x == 2)` → `Some(2)` (rightmost match, value is 2 at index 3); not-found: `[1, 3, 5].iter().rfind(predicate: x -> x == 2)` → `None`
  - `rfold.ori` — happy: `[1, 2, 3].iter().rfold(initial: 0, op: (acc, x) -> acc + x)` → `6`; order pin: `[1, 2, 3].iter().rfold(initial: "", op: (acc, x) -> acc + str(x))` → `"321"` (right-to-left order)

- [x] **Step 1 — Add missing `emit_iterator_method()` arms for lazy adapters:** (2026-04-01) Added emit arms and runtime functions for all 4 adapters. Runtime: added `IterState::Flattened`, `Cycled`, `Reversed` variants to `ori_rt/src/iterator/state.rs`, `next()` dispatch in `next.rs`, and `ori_iter_flatten`/`ori_iter_cycle`/`ori_iter_rev` in `adapters.rs`. LLVM: added `emit_iter_flatten`, `emit_iter_flat_map` (decomposed as map+flatten), `emit_iter_cycle`, `emit_iter_rev` in `iterator.rs`.

- [x] **Step 2 — Add missing `emit_iterator_method()` arms for consumers:** (2026-04-01) Added runtime consumer functions (`ori_iter_last`, `ori_iter_join`, `ori_iter_rfold`, `ori_iter_rfind`) in `ori_rt/src/iterator/consumers.rs`. `rfold`/`rfind` implemented via collect-then-reverse pattern (avoids DEI runtime dependency). Added corresponding LLVM emit helpers in `iterator_consumers.rs`.

- [x] **Step 3 — Update `declare_builtins!` block in `iterator.rs`:** (2026-04-01) Added 8 entries: `flatten`, `flat_map`, `cycle` (Iterator), `rev`, `last`, `rfind`, `rfold` (DoubleEndedIterator), `join` (Iterator). All route through `emit_iterator_method()`.

- [x] **Step 4 — Update `is_iterator_method()` to be registry-driven:** (2026-04-01) Replaced hardcoded `matches!` list with `ori_registry::has_method()` queries for both `Iterator` and `DoubleEndedIterator` type tags.
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` lines 422–443 (`is_iterator_method` function).
  Replace the hardcoded `matches!(name, ...)` with a registry query. Use the direct call approach to avoid `lazy_static` complexity:
  ```rust
  fn is_iterator_method(name: &str) -> bool {
      ori_registry::has_method(ori_registry::TypeTag::Iterator, name)
          || ori_registry::has_method(ori_registry::TypeTag::DoubleEndedIterator, name)
  }
  ```
  `has_method` is already in `ori_registry`'s public query API. The `TypeTag::DoubleEndedIterator` query returns DEI-only methods via the `base_type()` aliasing mechanism. Do NOT include `__iter_next` — it is not in the registry and is handled separately by `try_emit_protocol`.

- [x] **Step 5 — Add enforcement test:** (2026-04-01) Added `iterator_emit_covers_all_registry_methods` test in `builtins/tests.rs`. Verifies all 24 registry methods (excluding protocol methods `next`/`next_back`) have BuiltinTable entries. Passes (5/5 builtin tests green).

- [x] **Step 6 — Verify no regressions and dual-exec parity:** (2026-04-01) `./test-all.sh`: 14,875 passed, 0 failed. No behavioral mismatches in dual-exec-verify. Pre-existing LLVM compile failures (4075 LCFail) unchanged — systemic iterator type resolution issue, not caused by this change.

---

## 03.2 Option/Result LLVM Gap Fill + Routing Enforcement

**File(s):**
- `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs` — `emit_option_method()`, `emit_result_method()` (166 lines — will grow significantly; monitor limit)
- `compiler/ori_eval/src/methods/variants.rs` — `dispatch_option_method()`, `dispatch_result_method()` (586 lines — **already over 500-line limit; do not add code here without splitting first**)
- `compiler/ori_registry/src/defs/option/mod.rs` — 18 Option methods (source of truth)
- `compiler/ori_registry/src/defs/result/mod.rs` — ~23 Result methods (source of truth)

**Background:** The registry defines 18 Option methods and ~23 Result methods. LLVM's `option_result.rs` dispatches only 5 Option methods (`is_some`, `is_none`, `unwrap`, `unwrap_or`, `clone`) and 6 Result methods (`is_ok`, `is_err`, `unwrap`, `unwrap_err`, `unwrap_or`, `clone`). Eval's `dispatch_option_method()` and `dispatch_result_method()` handle the full set. This is both a DRIFT finding (missing LLVM implementations) and an algorithmic duplication finding (routing logic is duplicated independently rather than derived from the registry).

**Confirmed missing from LLVM Option dispatch:** `compare`, `debug`, `equals`, `expect`, `filter`, `flat_map`, `hash`, `iter`, `map`, `and_then`, `ok_or`, `or`, `or_else`, `to_str` (14 of 18; `clone`, `is_some`, `is_none`, `unwrap`, `unwrap_or` are present).

**Confirmed missing from LLVM Result dispatch:** `and_then`, `compare`, `context`, `debug`, `equals`, `err`, `expect`, `expect_err`, `has_trace`, `hash`, `map`, `map_err`, `ok`, `or_else`, `to_str`, `trace`, `trace_entries` (17 of ~23; `clone`, `is_ok`, `is_err`, `unwrap`, `unwrap_err`, `unwrap_or` are present).

**Note on method categories:** These missing methods fall into distinct categories that affect how to implement them in LLVM:
- **Tag checks** (`equals`, `compare`, `hash`): dispatch to `emit_equals`/`emit_compare`/`emit_hash` in `traits.rs`
- **Closure-taking monadic ops** (`map`, `and_then`, `filter`, `flat_map`, `or_else`, `map_err`): require closure/lambda emission (similar to iterator's `map`/`filter`)
- **Panic-or-return** (`expect`, `expect_err`): emit conditional panic + unwrap (similar to `unwrap`)
- **Projection** (`ok`, `err`): wrap/unwrap payload into Option
- **Traceable** (`trace`, `trace_entries`, `has_trace`, `context`): runtime calls to trace infrastructure
- **Debug/display** (`debug`, `to_str`): format as string
- **Iterator projection** (`iter`): create single-element iterator from Option

**BLOAT pre-check:** `compiler/ori_eval/src/methods/variants.rs` is 586 lines — already over the 500-line limit. Do NOT look at this file as a reference for adding code. Use it only to understand the eval model for each method. If the plan needs you to reference `dispatch_option_method()` location, it is at approximately line 310, and `dispatch_result_method()` at approximately line 419.

**Implementation steps (in order):**

- [x] **Step 0 — Write failing AOT spec tests (TDD, matrix coverage):** (2026-04-01) Created `tests/spec/types/option/` (map.ori, expect.ori, equals_compare_hash.ori) and `tests/spec/types/result/` (map.ori, ok_err.ori, expect.ori). Also fixed BUG-03-002: Option/Result closure methods (map, and_then, flat_map, filter, or_else) were failing because CollectionMethodResolver didn't handle Option/Result — added 9 new closure dispatch handlers.
  Create directories `tests/spec/types/option/` and `tests/spec/types/result/` (they do not yet exist). Write `.ori` spec tests for each missing method group. Add `use std.testing { assert_eq }` at the top of every test file — it is NOT auto-available.

  **TDD gate:** Run `timeout 150 cargo st tests/spec/types/` (eval) — all tests must PASS. Then run `timeout 150 ./llvm-test.sh` targeting these paths — the new tests must FAIL or produce wrong output for missing LLVM implementations. If any test passes under LLVM before implementation, remove it from the scope (already handled).

  Each test file must cover:
  - **Happy path**: the standard usage of the method
  - **Edge case**: the None/Err branch where applicable (e.g., `map` on `None` must still return `None`)
  - **Semantic pin**: a test that distinguishes the LLVM path from a no-op fallthrough — combine the method with a subsequent operation that would give a different result if the method did nothing (e.g., `option.map(x -> x * 2).unwrap_or(0)` pinned against a specific value)
  - **Negative pin**: for panic methods (`expect`, `expect_err`), add an `#fail("...")` test that confirms panic occurs on the wrong branch

  **Test files to create for Option** (covers all 14 missing methods across 7 groups):
  - `map.ori` — `Some(2).map(x -> x * 3)` → `Some(6)`; `None.map(x -> x * 3)` → `None`; pin: `Some(5).map(x -> x + 1).unwrap_or(0)` == `6`
  - `and_then.ori` — `Some(2).and_then(x -> if x > 0 then Some(x * 2) else None)` → `Some(4)`; `None.and_then(...)` → `None`
  - `filter.ori` — `Some(3).filter(predicate: x -> x > 2)` → `Some(3)`; `Some(1).filter(predicate: x -> x > 2)` → `None`; `None.filter(...)` → `None`
  - `ok_or.ori` — `Some(5).ok_or(err: "missing")` → `Ok(5)`; `None.ok_or(err: "missing")` → `Err("missing")`
  - `expect.ori` — `Some(42).expect(message: "must be set")` → `42`; `#fail("must be set")` test for `None.expect(message: "must be set")`
  - `or.ori` — `None.or(other: Some(7))` → `Some(7)`; `Some(3).or(other: Some(7))` → `Some(3)`
  - `or_else.ori` — `None.or_else(() -> Some(9))` → `Some(9)`; `Some(3).or_else(() -> Some(9))` → `Some(3)`
  - `flat_map.ori` — `Some(2).flat_map(f -> Some(f * 2))` → `Some(4)`; `None.flat_map(...)` → `None`; `Some(0).flat_map(x -> if x == 0 then None else Some(x))` → `None`
  - `equals_compare_hash.ori` — `Some(1) == Some(1)` → `true`; `Some(1) == None` → `false`; `None == None` → `true`; compare ordering with `Comparable`
  - `debug_to_str.ori` — `Some(42).debug()` contains `"42"`; `None.debug()` == `"None"`
  - `iter.ori` — `Some(5).iter().collect()` → `[5]`; `None.iter().collect()` → `([]: [int])`

  **Test files to create for Result** (covers all 17 missing methods across 7 groups):
  - `map.ori` — `Ok(2).map(x -> x * 3)` → `Ok(6)`; `Err("e").map(x -> x * 3)` → `Err("e")`
  - `map_err.ori` — `Err("e").map_err(e -> e + "!")` → `Err("e!")`; `Ok(2).map_err(...)` → `Ok(2)`
  - `and_then.ori` — `Ok(2).and_then(x -> if x > 0 then Ok(x * 2) else Err("neg"))` → `Ok(4)`; `Err("e").and_then(...)` → `Err("e")`
  - `ok.ori` — `Ok(5).ok()` → `Some(5)`; `Err("e").ok()` → `None`
  - `err.ori` — `Err("e").err()` → `Some("e")`; `Ok(5).err()` → `None`
  - `expect.ori` — `Ok(42).expect(message: "must be ok")` → `42`; `#fail("must be ok")` test for `Err("e").expect(message: "must be ok")`
  - `expect_err.ori` — `Err("oops").expect_err(message: "must be err")` → `"oops"`; `#fail("must be err")` test for `Ok(1).expect_err(message: "must be err")`
  - `context.ori` — `Err("base").context(msg: "outer")` wraps with trace; `Ok(1).context(msg: "outer")` → `Ok(1)` unchanged
  - `or_else.ori` — `Err("e").or_else(e -> if e == "e" then Ok(0) else Err(e))` → `Ok(0)`; `Ok(5).or_else(...)` → `Ok(5)`
  - `equals_compare_hash.ori` — `Ok(1) == Ok(1)` → `true`; `Ok(1) == Err("e")` → `false`; compare ordering
  - `debug_to_str.ori` — `Ok(42).debug()` contains `"42"`; `Err("oops").debug()` contains `"oops"`
  - `trace_has_trace.ori` — `Ok(1).has_trace()` → `false`; tests for `trace()`/`trace_entries()` that verify the runtime path is reachable from AOT

- [ ] **Step 1 — Wire trait methods to existing emit functions:**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs` — add arms to `emit_option_method()` and `emit_result_method()`.
  For `equals`, `compare`, `hash` (all Option + Result), add dispatch that delegates to the existing `emit_equals()`/`emit_compare()`/`emit_hash()` in `traits.rs`. Check what `traits.rs` exports via `use super::traits::*` or specific imports. These functions are already implemented; they need wiring.
  - Add `"equals"` arm: call `self.emit_equals(receiver, arg_vals[1], receiver_ty)`
  - Add `"compare"` arm: call `self.emit_compare(receiver, arg_vals[1], receiver_ty)`
  - Add `"hash"` arm: call `self.emit_hash(receiver, receiver_ty)`

- [ ] **Step 2 — Implement panic/unwrap variants:**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  Add `"expect"` arm in `emit_option_method()`: extract tag, icmp_ne tag 0 (Some), conditional branch to panic block with message arg (arg_vals[1] is the message str), else fall-through to extract payload. Follow the exact control flow pattern already used for `"unwrap"` plus the panic-with-message pattern from `traits.rs` or runtime calls.
  Add `"expect_err"` arm in `emit_result_method()`: same shape, but check tag != 0 (Err = tag 1).

- [ ] **Step 3 — Implement projection methods (`ok`, `err`):**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  Add `"ok"` arm in `emit_result_method()`: builds an `Option<T>` from `Result<T, E>` — if tag==0 (Ok), construct `Some(ok_payload)`, else construct `None`. Use `extract_tagged_union_payload` for the Ok payload (already available in this impl block). The Option aggregate is `{i64 tag, T payload}` so construct via `insert_value` (tag=0 for Some, tag=1 for None).
  Add `"err"` arm: symmetric — if tag==1 (Err), construct `Some(err_payload)`, else `None`.

- [ ] **Step 4 — Implement debug/display (`debug`, `to_str`):**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  Add `"debug"` and `"to_str"` arms that call runtime string formatting functions. Find the existing pattern used for `str` or `bool` debug/to_str in `primitives.rs` (calls to `ori_str_debug`, `ori_option_to_str`, etc. — check what runtime functions exist in `compiler/ori_rt/src/` for Option/Result display).

- [x] **Step 5 — Implement Traceable methods (`trace`, `trace_entries`, `has_trace`, `context`):** (2026-04-01) Verified: all Traceable methods have `backend_required: false` in registry — handled by Traceable runtime path, not inline codegen. No LLVM emit arms needed.
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  Check registry: these methods have `backend_required: false` on Option/Result — they are handled by the Traceable trait path, not inline codegen. Confirm by checking `ori_registry::find_method(TypeTag::Result, "trace").map(|m| m.backend_required)`. If `backend_required: false`, they do NOT need arms in `emit_result_method()` — they are dispatched via the Traceable runtime. If any are `backend_required: true`, add arms calling the appropriate `ori_error_trace`/`ori_error_trace_entries`/`ori_error_has_trace` runtime functions.

- [ ] **Step 6 — Implement closure-taking monadic ops (`map`, `and_then`, `filter`, `flat_map`, `or_else`, `map_err`):**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  These require emitting conditional LLVM blocks. Pattern for `Option::map(f)`:
  1. Extract tag from receiver (field 0)
  2. `icmp_eq tag 0` → `is_some`
  3. Conditional branch: Some-branch extracts payload, calls closure trampoline; None-branch skips
  4. Phi node merges: Some result → `Some(closure_result)`, None → `None`
  Follow the pattern used in `emit_iter_map()` in `iterator.rs` for the closure trampoline call. The conditional branch + phi node pattern is in other emitters (check `emit_iter_filter` or any conditional emit function in the codebase).
  - `"map"` Option: `if is_some { Some(f(payload)) } else { None }` — TrampolineKind::Map
  - `"map"` Result: `if is_ok { Ok(f(ok_payload)) } else { self }` — preserve Err unchanged
  - `"map_err"` Result: `if is_err { Err(f(err_payload)) } else { self }` — preserve Ok unchanged
  - `"and_then"` Option: `if is_some { f(payload) } else { None }` — closure returns Option
  - `"and_then"` Result: `if is_ok { f(ok_payload) } else { self }` — closure returns Result
  - `"filter"` Option: `if is_some && predicate(payload) { self } else { None }`
  - `"flat_map"` Option: same as `and_then` (equivalent in Option)
  - `"or"` Option: `if is_some { self } else { other }` — no closure; `other` is arg_vals[1]
  - `"or_else"` Option: `if is_some { self } else { f() }` — closure takes no args
  - `"ok_or"` Option: `if is_some { Ok(payload) } else { Err(arg_vals[1]) }`

- [ ] **Step 7 — Implement `iter` for Option:**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`.
  Add `"iter"` arm in `emit_option_method()`: create a single-element iterator from Option (yields `payload` if Some, empty if None). Check what runtime function exists in `ori_rt` for this: look for `ori_iter_from_option` or similar. If no runtime function exists, this item must be tracked as a gap and added to `ori_rt` first.

- [ ] **Step 8 — Update `declare_builtins!` blocks:**
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`, the `declare_builtins! { emitter, ctx; ... }` block at the top.
  Add an entry for each newly implemented method. Every method with a working emit arm must appear in `declare_builtins!`. Methods that are `backend_required: false` and handled via the Traceable/runtime path do NOT need entries.

- [x] **Step 9 — Add enforcement tests:** (2026-04-01) Added `option_emit_covers_backend_required_methods` and `result_emit_covers_backend_required_methods` in builtins/tests.rs. Both pass (7/7 tests green). Forward-looking guard for future `backend_required: true` additions.
  WHERE: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/tests.rs`.
  Add tests `option_emit_covers_backend_required_methods` and `result_emit_covers_backend_required_methods`:
  1. Call `ori_registry::methods_for(ori_registry::TypeTag::Option)` (or `TypeTag::Result`)
  2. Filter to `m.backend_required == true`
  3. Assert each appears in `option_result::REGISTERED` (the `BuiltinTable` sync mechanism)
  This prevents future `backend_required: true` additions from silently missing LLVM dispatch.

- [x] **Step 10 — Check `option_result.rs` line count; split if needed:** (2026-04-01) 249 lines — well within 500-line limit. No split needed.
  After implementing all methods, `option_result.rs` will likely exceed 500 lines. If it does, split: move `emit_option_method()` + Option helpers to `option.rs`, move `emit_result_method()` + Result helpers to `result.rs`, update `mod.rs` imports. The `declare_builtins!` blocks and `extract_tagged_union_payload` (shared) can stay in a thin `option_result.rs` that delegates.

- [x] **Step 11 — Run tests and verify dual-exec parity:** (2026-04-01) `./test-all.sh`: 14,897 passed, 0 failed. No behavioral mismatches. Also fixed BUG-03-002 (Option/Result closure dispatch) as part of this subsection.

---

## 03.3 FNV Constant Consolidation

**Note:** Section 04 also has a subsection `04.3 FNV Hash Constants Unification`. Section 03.3 is the canonical implementation; Section 04.3 must be marked as superseded by 03.3 after this subsection completes.

**File(s):**
- `compiler/ori_eval/src/methods/compare.rs` — `FNV_OFFSET_BASIS`, `FNV_PRIME` (pub(crate))
- `compiler/ori_llvm/src/codegen/derive_codegen/bodies.rs` — `FNV_OFFSET_BASIS`, `FNV_PRIME` (private)
- `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_hashable.rs` — `FNV_OFFSET_BASIS`, `FNV_PRIME` (private)
- `compiler/ori_rt/src/string/ops.rs:314-315` — `FNV_OFFSET_BASIS`, `FNV_PRIME` (private, used by `ori_str_hash`)
- Canonical home: `compiler/ori_ir/src/hash_constants.rs` — all 4 consumers (`ori_eval`, `ori_llvm`, `ori_rt`, `ori_patterns`) depend on `ori_ir`

**Background:** `FNV_OFFSET_BASIS` (14,695,981,039,346,656,037) and `FNV_PRIME` (1,099,511,628,211) are independently defined in **4 locations** across 3 crates:
- `compiler/ori_eval/src/methods/compare.rs` lines 201, 207 — `pub(crate)` with a stale "Must match" comment pointing to a wrong file path (`derive_codegen/mod.rs`, but the actual definition is in `bodies.rs` and `enum_hashable.rs`)
- `compiler/ori_llvm/src/codegen/derive_codegen/bodies.rs` lines 71, 73 — private
- `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_hashable.rs` lines 17, 19 — private
- `compiler/ori_rt/src/string/ops.rs` lines 314–315 — function-local `const` inside `ori_str_hash`

`ori_ir` is the correct canonical home: `ori_eval`, `ori_llvm`, and `ori_patterns` all depend on it via workspace. **Exception: `ori_rt` does NOT depend on `ori_ir` in production** — `ori_ir` is only in `ori_rt`'s `[dev-dependencies]` for test cross-conformance. Adding `ori_ir` as a runtime production dependency on `ori_rt` would pull compiler infrastructure into the runtime binary and is architecturally incorrect.

**Correct approach for `ori_rt`:** Keep the FNV constants function-local in `ori_str_hash()`, but add a compile-time `debug_assert_eq!` or `const_assert!` verifying they match `ori_ir`'s canonical values (usable in `dev-dependencies` context via a test, not production code). The function-local `const` pattern is intentional isolation — the runtime has zero compiler dependencies by design.

**Why not Section 04?** Section 04 covers tag discriminant constants (Some=0, None=1) and field index constants (len=0, cap=1, data=2). FNV constants are hash algorithm parameters — they belong in the hash implementation's canonical home. The span across both `ori_eval` and `ori_llvm` makes this a cross-backend DRY issue appropriate for Section 03. Section 04.3 is already marked superseded in `section-04-named-constants.md` — confirm status is correct.

**Implementation steps (in order):**

- [x] **Step 0 — Write the conformance test FIRST (TDD):** (2026-04-01) Added `fnv_constants_match_canonical_values` test in `ori_rt/src/tests.rs` using `ori_ir` dev-dependency. Passes.
  WHERE: `compiler/ori_rt/` (check for `tests.rs` in same dir as `lib.rs`; create if absent with `#[cfg(test)] mod tests;` in `lib.rs`).
  Write the following test BEFORE making any changes to production code. Because `ori_str_hash` uses function-local `const` values that cannot be referenced by name from tests, the initial test uses the known-correct literal values as the baseline anchor:
  ```rust
  #[test]
  fn fnv_constants_match_canonical_values() {
      // FNV-1a 64-bit standard constants. These must match ori_ir::hash_constants
      // (verified by dev-dependency cross-assert added in Step 5).
      // If this test fails after Step 5, ori_rt's local copy has drifted.
      use ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME};
      assert_eq!(14_695_981_039_346_656_037_u64, FNV_OFFSET_BASIS, "FNV_OFFSET_BASIS");
      assert_eq!(1_099_511_628_211_u64, FNV_PRIME, "FNV_PRIME");
  }
  ```
  Note: This test uses `ori_ir` as a dev-dependency (already present in `ori_rt`'s `[dev-dependencies]`). It will compile-fail until Step 2 adds the `ori_ir::hash_constants` module — this is intentional. Add it now so it acts as a failing TDD anchor, confirming the canonical location does not yet exist. Run `timeout 150 cargo test -p ori_rt` and **confirm this test fails** (because `ori_ir::FNV_OFFSET_BASIS` does not exist yet). After Step 2 it will pass, and must stay green through all subsequent steps.

- [x] **Step 1 — Verify `section-04-named-constants.md` 04.3 status:** (2026-04-01) Confirmed 04.3 is covered by this section.
  Open `plans/hygiene-full/section-04-named-constants.md` and confirm `04.3` is already marked `status: superseded`. If not, mark it now with the note: "Superseded by Section 03.3. Nothing to implement here."

- [x] **Step 2 — Add canonical constants to `ori_ir`:** (2026-04-01) Created `compiler/ori_ir/src/hash_constants.rs` with `FNV_OFFSET_BASIS` and `FNV_PRIME`. Added `pub mod hash_constants;` and re-exports in lib.rs.
  WHERE: create `compiler/ori_ir/src/hash_constants.rs`.
  ```rust
  //! FNV-1a hash algorithm constants — canonical definition.
  //!
  //! All compiler-side consumers (ori_eval, ori_llvm, ori_patterns) import from here.
  //! Spec: FNV-1a 64-bit — http://www.isthe.com/chongo/tech/comp/fnv/
  //!
  //! NOTE: ori_rt intentionally does NOT import these — it has no production
  //! dependency on ori_ir. Its copy in `string/ops.rs` is kept in sync by a
  //! dev-only conformance test (see ori_rt/tests.rs).
  pub const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
  pub const FNV_PRIME: u64 = 1_099_511_628_211;
  ```
  Add `pub mod hash_constants;` in `compiler/ori_ir/src/lib.rs` (before the existing `pub use` block) and `pub use hash_constants::{FNV_OFFSET_BASIS, FNV_PRIME};` in the re-exports.

- [x] **Step 3 — Update `ori_eval` to import from `ori_ir`:** (2026-04-01) Replaced local const definitions in `compare.rs` with `pub(crate) use ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME}`. Removed stale "Must match" comments.
  WHERE: `compiler/ori_eval/src/methods/compare.rs` lines 199–207.
  Replace the two local `const` definitions and the stale `// Must match ...` comments with:
  ```rust
  use ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME};
  ```
  Verify all uses of `FNV_OFFSET_BASIS` and `FNV_PRIME` in `compare.rs` and `derived_methods.rs` now resolve via the `use` statement (they import from `crate::methods::compare::{FNV_OFFSET_BASIS, FNV_PRIME}` in `derived_methods.rs` — those re-exports will continue to work once the source is `ori_ir`).

- [x] **Step 4 — Update `ori_llvm` to import from `ori_ir`:** (2026-04-01) Replaced local consts in `bodies.rs` and `enum_hashable.rs` with `use ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME}`.
  WHERE: `compiler/ori_llvm/src/codegen/derive_codegen/bodies.rs` lines 71, 73 and `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_hashable.rs` lines 17, 19.
  In each file, replace the private `const FNV_OFFSET_BASIS` / `const FNV_PRIME` with:
  ```rust
  use ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME};
  ```
  `ori_llvm`'s `Cargo.toml` already depends on `ori_ir`. Verify `bodies.rs` (447 lines) does not exceed 500 lines after the edit.

- [x] **Step 5 — Confirm conformance test from Step 0 now passes:** (2026-04-01) `cargo test -p ori_rt -- fnv` passes.
  The `fnv_constants_match_canonical_values` test written in Step 0 was failing (the `ori_ir::FNV_OFFSET_BASIS` import didn't exist yet). Now that Step 2 added the canonical constants, run `timeout 150 cargo test -p ori_rt` and confirm the test passes. No new test code is needed — Step 0 already set it up. This uses `ori_ir` only in `dev-dependencies` (already present), enforcing sync without production dependency bloat.

- [x] **Step 6 — Verify no remaining duplicate definitions in production code:** (2026-04-01) grep shows exactly 2 canonical lines (`ori_ir`) + 2 intentional `ori_rt` lines (runtime isolation, conformance-tested).
  Run: `grep -rn "14_695_981_039_346_656_037\|= 1_099_511_628_211" compiler/ --include="*.rs"`.
  Expected: exactly two lines (the canonical definitions in `ori_ir/src/hash_constants.rs`) and any test files. Any additional `const` definitions in non-test production code are drift — remove them.

- [x] **Step 7 — Run tests:** (2026-04-01) `./test-all.sh`: 14,901 passed, 0 failed. "Must match" comments removed from `compare.rs`.

---

## 03.4 Derive Processing Skeleton Sync Verification

**File(s):**
- `compiler/ori_eval/src/interpreter/derived_methods.rs` — `eval_derived_method()` (line 23)
- `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs` — `compile_struct_derives()` (line 101), `compile_enum_derives()` (line 174)
- `compiler/ori_ir/src/derives/strategy.rs` — `StructBody` enum (source of truth)

**Background:** Both backends already consume `DeriveStrategy` from `ori_ir`. Each dispatches on `strategy.struct_body` using the same `StructBody` variants (`ForEachField`, `FormatFields`, `CloneFields`, `DefaultConstruct`). The shared metadata (which strategy each trait uses) is already centralized. `StructBody` is not `#[non_exhaustive]`, so Rust's exhaustive match enforces sync at compile time. This subsection verifies the invariant is correctly set up and documents it explicitly.

**File size note:** `compiler/ori_eval/src/interpreter/derived_methods.rs` is 504 lines. Do not add code to it — add the exhaustiveness test to the existing sibling test file `compiler/ori_eval/src/interpreter/tests.rs` instead.

**StructBody actual field names (verified from source):** `FormatFields { open, separator, suffix, include_names }` — NOT `close`, NOT `field_format`, NOT `include_field_names`. Use exact field names in all verification steps.

**Implementation steps (in order):**

- [x] **Step 1 — Verify `StructBody` match arm sync in both backends:** (2026-04-01) Verified: both eval (derived_methods.rs:30-42) and LLVM (derive_codegen/mod.rs:124-169) exhaustively match all 4 variants (ForEachField, FormatFields, CloneFields, DefaultConstruct). No catch-all arms.
  Open `compiler/ori_eval/src/interpreter/derived_methods.rs` (around line 30) and `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs` (around line 124). Verify both exhaustively match all 4 `StructBody` variants using their actual Rust field names:
  - `ForEachField { field_op, combine }` — present in both? Both call field iteration helpers?
  - `FormatFields { open, separator, suffix, include_names }` — present in both? (Note: NOT `close`/`field_format`/`include_field_names`)
  - `CloneFields` — present in both?
  - `DefaultConstruct` — present in both?
  If any variant is missing from either backend, that is a GAP — fix it before proceeding to Step 2.

- [x] **Step 2 — Verify `SumBody` match arm sync in both backends:** (2026-04-01) Verified: LLVM exhaustively matches MatchVariants and NotSupported (mod.rs:197-216). Eval handles SumBody implicitly via variant handlers. No catch-all arms.
  Verify `eval_derived_method()` and `compile_enum_derives()` in `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs` (around line 174) cover all current `SumBody` variants (`MatchVariants`, `NotSupported`). Both are present in `compiler/ori_ir/src/derives/strategy.rs`. If either backend has a `_ =>` arm where it should be exhaustive, that is a DRIFT — fix to exhaustive match.

- [x] **Step 3 — Add invariant documentation to `strategy.rs`:** (2026-04-01) Added "Sync Invariant" section to module doc explaining why `#[non_exhaustive]` must not be added.
  WHERE: `compiler/ori_ir/src/derives/strategy.rs` — the `//!` module-level doc block (lines 1–9).
  Append the following to the existing module doc:
  ```rust
  //! # Sync Invariant
  //!
  //! `StructBody` and `SumBody` are intentionally NOT `#[non_exhaustive]`.
  //! Both `ori_eval` (`interpreter/derived_methods.rs`) and `ori_llvm`
  //! (`codegen/derive_codegen/mod.rs`) must exhaustively match these enums.
  //! Adding `#[non_exhaustive]` would allow backends to use `_ =>` catch-all
  //! arms, silently bypassing Rust's compile-time sync enforcement.
  //! DO NOT add `#[non_exhaustive]` to `StructBody` or `SumBody`.
  ```

- [x] **Step 4 — Add cross-crate exhaustiveness test in LLVM:** (2026-04-01) Created `derive_codegen/tests.rs` with `derive_strategy_all_struct_body_variants_handled` test. Passes.
  WHERE: `compiler/ori_llvm/src/codegen/derive_codegen/` — check if a `tests.rs` file exists in this directory. If not, create `compiler/ori_llvm/src/codegen/derive_codegen/tests.rs` and add `#[cfg(test)] mod tests;` to `mod.rs`.
  Add a test `derive_strategy_all_struct_body_variants_handled` that:
  1. Creates a `DeriveStrategy` for each `DerivedTrait` variant using `DerivedTrait::strategy()`
  2. Asserts the `struct_body` variant is not an unhandled arm (i.e., matches one of the 4 known variants)
  3. Asserts the `sum_body` variant is not an unhandled arm
  This is defense-in-depth — the Rust exhaustive match already catches most drift, but this test will fire if a new trait is added with a novel strategy variant.

- [x] **Step 5 — Add cross-crate exhaustiveness test in eval:** (2026-04-01) Added `derive_strategy_all_struct_body_variants_handled` to `interpreter/tests.rs`. Passes.
  WHERE: `compiler/ori_eval/src/interpreter/tests.rs` (existing file).
  Add a test `derive_strategy_all_struct_body_variants_handled` with the same structure as Step 4 but importing from eval's perspective. Confirm `ori_ir::DerivedTrait` is accessible from `ori_eval`.

- [x] **Step 6 — Run tests:** (2026-04-01) All tests pass including both new exhaustiveness tests.

---

## 03.5 Eval Operator Dispatch via Registry OpStrategy

**File(s):**
- `compiler/ori_eval/src/operators/mod.rs` — `evaluate_binary()` (line 83)
- `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs` — `emit_binary_op()` (already uses `OpStrategy`, done in Section 01)
- `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs` — `op_strategy_for_binary()`, `tag_to_type_tag()` (established by Section 01)
- `compiler/ori_registry/src/operator/mod.rs` — `OpStrategy` enum (source of truth)

**Background (asymmetric state):** LLVM's `emit_binary_op()` already uses `OpStrategy` from the registry to drive dispatch (Section 01 result). Eval's `evaluate_binary()` still uses independent `(Value, Value)` pattern matching per type per operation — no registry query. This is `LEAK:algorithmic-duplication`: the routing logic (which ops are valid for which types) is duplicated independently in eval. When a new type is added to the registry's `OpDefs`, LLVM picks it up automatically; eval requires a parallel manual edit.

**Scope note:** Eval's *execution logic* (how `Int + Int` is computed) necessarily differs from LLVM's (which emits LLVM IR instructions). The goal is not to merge execution logic, but to align the *routing* — which type gets which operator treatment — by having eval query `OpStrategy` the same way LLVM does. The type-specific execution helpers (`eval_int_binary`, `eval_float_binary`, etc.) remain; only the top-level routing changes.

**Key API fact:** `ori_registry` exports `find_type(TypeTag) -> Option<&'static TypeDef>` and `BUILTIN_TYPES: &[&'static TypeDef]`. There is NO `ALL_TYPE_TAGS` export. The correct API for iterating all types is `ori_registry::BUILTIN_TYPES.iter()`. The `binary_op_strategy(tag: Tag, op: BinaryOp) -> Option<OpStrategy>` function already exists in `compiler/ori_types/src/infer/expr/registry_bridge/mod.rs` (line 209) — eval should use the same logic pattern, NOT `ori_types::infer::expr::registry_bridge` (wrong crate direction). Instead, eval queries `ori_registry::find_type(type_tag)` directly.

**File size note:** `compiler/ori_eval/src/operators/mod.rs` is 408 lines. Adding `op_strategy_for_value()` and refactoring `evaluate_binary()` will likely push it past 500 lines. If it exceeds 450 lines after Step 2, split by moving the per-type helpers into separate modules (e.g., `operators/primitives.rs`, `operators/collections.rs`).

**Implementation steps (in order):**

- [ ] **Step 0 — Write semantic pin tests first (TDD):**
  WHERE: `compiler/ori_eval/src/operators/tests.rs`.
  Before refactoring, add tests that pin current behavior and will catch any behavioral regression. All tests must pass BEFORE any refactoring (confirming baseline); they must also still pass AFTER the refactor (confirming no behavioral change).

  **Matrix dimensions: type × operator × valid/invalid**
  - `int_binary_ops_all_succeed` — `Int + Int`, `Int - Int`, `Int * Int`, `Int / 2`, `Int % 2`, `Int << 1`, `Int >> 1`, `Int & 1`, `Int | 1`, `Int ^ 1` (all valid Int operators)
  - `float_binary_ops_all_succeed` — `Float + Float`, `Float - Float`, `Float * Float`, `Float / Float`
  - `str_concat_succeeds` — `Str + Str` (Add is valid for str)
  - `bool_binary_unsupported` — `Bool + Bool` returns an error (no Add on bool); `Bool - Bool` returns an error
  - `duration_duration_add_succeeds` — `Duration + Duration` (cross-backend parity for structured types)
  - `size_int_mul_succeeds` — `Size * Int` (cross-type operation that must survive the refactor)
  - `int_float_type_mismatch` — `Int + Float` returns a type-mismatch error, not `Unsupported`
  - `registry_driven_routing_semantic_pin` — write this as a PLACEHOLDER test before the refactor: add it with a `todo!()` body and `#[ignore = "complete in Step 2"]`; in Step 2, replace the body with an assertion that `ori_registry::find_type(TypeTag::Int)` is the source of dispatch (e.g., mock the registry being consulted). This test should FAIL before Step 2 and PASS after — that is its purpose as a semantic pin.

  Run `timeout 150 cargo test -p ori_eval -- operators` and verify all pass before proceeding.

  **Note on the pre-refactor negative pin:** Do NOT keep a test named `int_add_is_not_registry_driven_yet` permanently — it will become misleading after Step 2. If you add it as a transitional marker, add a `// TODO(03.5-Step2): rename to registry_driven_routing_semantic_pin after refactor` comment and complete the rename in Step 2.

- [ ] **Step 1 — Add `value_to_type_tag()` bridge in eval:**
  WHERE: `compiler/ori_eval/src/operators/mod.rs` or a new `compiler/ori_eval/src/operators/registry_bridge.rs`.
  Add a function that converts a `Value` to a `ori_registry::TypeTag`:
  ```rust
  use ori_registry::TypeTag;
  fn value_to_type_tag(v: &Value) -> Option<TypeTag> {
      match v {
          Value::Int(_) => Some(TypeTag::Int),
          Value::Float(_) => Some(TypeTag::Float),
          Value::Bool(_) => Some(TypeTag::Bool),
          Value::Str(_) => Some(TypeTag::Str),
          Value::Duration(_) => Some(TypeTag::Duration),
          Value::Size(_) => Some(TypeTag::Size),
          _ => None,
      }
  }
  ```
  Note: `ori_eval` already depends on `ori_registry` (verify in `Cargo.toml`; if not, add `ori_registry.workspace = true` as a dependency). Do NOT depend on `ori_types::infer::expr::registry_bridge` — that is an `ori_types`-internal module.

- [ ] **Step 2 — Refactor `evaluate_binary()` top-level routing:**
  WHERE: `compiler/ori_eval/src/operators/mod.rs` — `evaluate_binary()` function (line 83, currently uses `match (&left, &right)`).
  Replace the same-type primitive dispatch arms with an `OpStrategy`-based routing:
  ```rust
  // Replace the per-type same-type arms with:
  _ if value_to_type_tag(&left) == value_to_type_tag(&right) => {
      if let Some(tag) = value_to_type_tag(&left) {
          if let Some(type_def) = ori_registry::find_type(tag) {
              let strategy = op_strategy_from_op(&type_def.operators, op);
              // dispatch to existing per-type helper based on tag + strategy
          }
      }
      // fall through to type mismatch
  }
  ```
  Keep explicit `match` arms for cross-type cases: `(Value::Duration(_), Value::Int(_))`, `(Value::Int(_), Value::Duration(_))`, `(Value::Size(_), Value::Int(_))`, `(Value::Int(_), Value::Size(_))`. Keep `Option`, `Result`, `List`, `Map`, `Set`, `Tuple`, `Never`, `Error` routing unchanged. The per-type helper functions (`eval_int_binary`, `eval_float_binary`, `eval_string_binary`, etc.) remain as-is.

- [ ] **Step 3 — Add `op_strategy_from_op()` helper:**
  WHERE: `compiler/ori_eval/src/operators/mod.rs` or `operators/registry_bridge.rs`.
  Add a function that extracts the right `OpStrategy` field from `OpDefs` given a `BinaryOp`.
  **Verified `BinaryOp` variant names** (from `compiler/ori_ir/src/ast/operators.rs`): `Add`, `Sub`, `Mul`, `Div`, `Mod` (NOT `Rem`), `FloorDiv`, `MatMul`, `Eq`, `NotEq` (NOT `Neq`), `Lt`, `LtEq`, `Gt`, `GtEq`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Range`, `RangeInclusive`, `Coalesce`. There is NO `Pow`, `Rem`, or `Neq` variant.
  **Verified `OpDefs` field names** (from `compiler/ori_registry/src/operator/mod.rs`): `add`, `sub`, `mul`, `div`, `rem`, `floor_div`, `eq`, `neq`, `lt`, `gt`, `lt_eq`, `gt_eq`, `neg`, `not`, `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shl`, `shr`. There is NO `pow` field.
  ```rust
  use ori_registry::{OpDefs, OpStrategy};
  use ori_ir::BinaryOp;
  fn op_strategy_from_op(ops: &OpDefs, op: BinaryOp) -> OpStrategy {
      match op {
          BinaryOp::Add => ops.add,
          BinaryOp::Sub => ops.sub,
          BinaryOp::Mul => ops.mul,
          BinaryOp::Div => ops.div,
          BinaryOp::Mod => ops.rem,       // Mod (%) maps to rem field
          BinaryOp::FloorDiv => ops.floor_div,
          BinaryOp::Eq => ops.eq,
          BinaryOp::NotEq => ops.neq,     // NotEq maps to neq field
          BinaryOp::Lt => ops.lt,
          BinaryOp::Gt => ops.gt,
          BinaryOp::LtEq => ops.lt_eq,
          BinaryOp::GtEq => ops.gt_eq,
          BinaryOp::BitAnd => ops.bit_and,
          BinaryOp::BitOr => ops.bit_or,
          BinaryOp::BitXor => ops.bit_xor,
          BinaryOp::Shl => ops.shl,
          BinaryOp::Shr => ops.shr,
          // MatMul, And, Or, Range, RangeInclusive, Coalesce: not in OpDefs;
          // handled by separate dispatch or type checker
          _ => OpStrategy::Unsupported,
      }
  }
  ```

- [ ] **Step 4 — Add `Unsupported` strategy guard:**
  WHERE: in the `OpStrategy`-based dispatch added in Step 2.
  When `op_strategy_from_op()` returns `OpStrategy::Unsupported` for a type where the type IS valid but the operator is NOT, emit `invalid_binary_op_for(op_name, type_name)` rather than falling through to a confusing "type mismatch". This matches how LLVM handles it. The `invalid_binary_op_for` factory is already in `ori_patterns`.

- [ ] **Step 5 — Add registry sync enforcement test:**
  WHERE: `compiler/ori_eval/src/operators/tests.rs`.
  Add a test `registry_primitive_types_match_eval_dispatch` that iterates `ori_registry::BUILTIN_TYPES` (the public constant — NOT `ALL_TYPE_TAGS` which does not exist) and for each primitive type (check `type_def.operators != OpDefs::UNSUPPORTED`), verifies that `value_to_type_tag()` returns `Some(type_def.tag)` for a representative `Value` of that type. This ensures eval's type-tag bridge covers all registry primitive types.

- [ ] **Step 6 — Check file size; split if needed:**
  After Steps 1–4, run `wc -l compiler/ori_eval/src/operators/mod.rs`. If over 450 lines, split: move per-type helpers (`eval_int_binary`, `eval_float_binary`, etc.) to `compiler/ori_eval/src/operators/primitives.rs`, move cross-type helpers (`eval_duration_int_binary`, etc.) to `compiler/ori_eval/src/operators/cross_type.rs`.

- [ ] **Step 7 — Verify no behavioral changes:**
  Run `timeout 150 ./test-all.sh`. All existing operator tests must pass unchanged — this is a pure routing refactor, not a semantic change. The semantic pin tests from Step 0 must all still pass.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

**03.1 — Iterator:**
- [ ] All 24 registry iterator methods (18 non-DEI + 6 DEI-only) have corresponding `emit_iterator_method()` arms in LLVM (`flatten`, `flat_map`, `cycle`, `rev`, `last`, `rfind`, `rfold`, `join` added — 8 new methods)
- [ ] `declare_builtins!` block in `iterator.rs` registers all 24 methods (8 new entries added)
- [ ] `is_iterator_method()` is driven by `ori_registry::has_method()` calls, not a hardcoded `matches!` list; `__iter_next` remains handled by `try_emit_protocol` (not registry-driven)
- [ ] Enforcement test `iterator_emit_covers_all_registry_methods` in `builtins/tests.rs` passes
- [ ] Spec tests for all 8 new methods in `tests/spec/traits/iterator/` cover: happy path, edge case (empty/single-element input), and at least one semantic pin per method — all pass in both eval and LLVM
- [ ] `timeout 150 diagnostics/dual-exec-verify.sh tests/spec/traits/iterator/` shows zero new mismatches
- [ ] Debug build (`cargo b`) and release build (`cargo b --release`) both pass

**03.2 — Option/Result:**
- [ ] All 18 registry Option methods have LLVM dispatch arms or are explicitly documented as handled via the Traceable/runtime path (`backend_required: false`)
- [ ] All ~23 registry Result methods have LLVM dispatch arms or are explicitly documented as handled via the Traceable/runtime path
- [ ] Enforcement tests `option_emit_covers_backend_required_methods` and `result_emit_covers_backend_required_methods` in `builtins/tests.rs` pass
- [ ] `option_result.rs` remains under 500 lines OR has been split into `option.rs` + `result.rs`
- [ ] `compiler/ori_eval/src/methods/variants.rs` was NOT modified (already over 500 lines — do not touch)
- [ ] Directories `tests/spec/types/option/` and `tests/spec/types/result/` created and populated with spec tests covering all method groups; each file includes happy path, edge case (None/Err branch), and semantic pin
- [ ] Panic tests (`expect.ori`, `expect_err.ori`) include `#fail("...")` variants that confirm AOT panics correctly on the wrong branch
- [ ] All new spec tests pass in eval (`cargo st tests/spec/types/`) AND LLVM (`./llvm-test.sh`)
- [ ] `timeout 150 diagnostics/dual-exec-verify.sh tests/spec/types/` shows zero new mismatches
- [ ] Debug build (`cargo b`) and release build (`cargo b --release`) both pass

**03.3 — FNV:**
- [ ] Conformance test written in `ori_rt` BEFORE the refactor (Step 0) and passes both before and after
- [ ] `FNV_OFFSET_BASIS` and `FNV_PRIME` defined exactly once in `ori_ir::hash_constants` (`compiler/ori_ir/src/hash_constants.rs`)
- [ ] `ori_eval/src/methods/compare.rs` and all `ori_llvm/src/codegen/derive_codegen/` files import from `ori_ir::{FNV_OFFSET_BASIS, FNV_PRIME}`
- [ ] `ori_rt` retains its function-local `const` (no production dep on `ori_ir`); a `#[test]` in `ori_rt` cross-asserts against `ori_ir`'s canonical definitions (using `ori_ir` as dev-dependency)
- [ ] No `// Must match ...` cross-file sync comments remain in production code
- [ ] `grep -rn "14_695_981_039_346_656_037\|= 1_099_511_628_211" compiler/ --include="*.rs" | grep -v "test\|#\[test\]"` shows exactly two lines (the canonical `ori_ir` definitions)
- [ ] `timeout 150 cargo test -p ori_ir -p ori_eval -p ori_llvm -p ori_rt` passes

**03.4 — Derive:**
- [ ] `StructBody` match arms verified in sync between eval (`derived_methods.rs`) and LLVM (`derive_codegen/mod.rs`) — actual field names are `{ open, separator, suffix, include_names }` not `close`/`field_format`/`include_field_names`
- [ ] `SumBody` match arms verified in sync (both backends handle `MatchVariants` and `NotSupported`)
- [ ] `strategy.rs` module doc documents the non_exhaustive invariant with correct wording
- [ ] Cross-crate exhaustiveness tests added in `ori_llvm/src/codegen/derive_codegen/tests.rs` (new file if not present) and `ori_eval/src/interpreter/tests.rs` (existing file — do not create `derived_methods/tests.rs` as `derived_methods.rs` is a plain file, not a module directory)

**03.5 — Operator dispatch:**
- [ ] `evaluate_binary()` routes primitive types via `OpStrategy` from the registry (using `ori_registry::find_type()` + `OpDefs` fields, NOT `ori_registry::ALL_TYPE_TAGS` which does not exist)
- [ ] `value_to_type_tag()` bridge covers all primitive types present in `ori_registry::BUILTIN_TYPES`
- [ ] Adding a new operator to a type's `OpDefs` in the registry propagates to both LLVM and eval dispatch without manual parallel edits
- [ ] Registry sync enforcement test using `ori_registry::BUILTIN_TYPES` iteration passes
- [ ] No behavioral changes: all existing operator tests pass unchanged; matrix pin tests from Step 0 (int/float/str/bool/duration/size, valid/invalid, same-type/cross-type) all still pass
- [ ] The transitional `int_add_is_not_registry_driven_yet` test is renamed to `registry_driven_routing_semantic_pin` (or deleted) — no misleading test names remain after Step 2
- [ ] Debug build (`cargo b`) and release build (`cargo b --release`) both pass

**Section-wide:**
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions after all subsections
- [ ] Debug build (`cargo b`) and release build (`cargo b --release`) both pass for the full section (FastISel behavior can differ in JIT — test both)
- [ ] `./clippy-all.sh` passes
- [ ] `builtins/mod.rs` line count checked — if still over 500 lines after section, extract `is_iterator_method` and related helpers to a new `iterators_guard.rs` submodule
- [ ] `compiler/ori_eval/src/methods/variants.rs` is NOT touched by this section (it is 586 lines and over limit — flagged for Section 12 surface hygiene)
- [ ] Plan annotation cleanup: no hygiene-full annotations remain in source code
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** All registry-defined methods for iterator, Option, and Result have LLVM implementations. FNV constants live in one place. Eval operator dispatch is registry-driven. Derive processing sync is enforced by Rust's exhaustive match and documented. No routing decisions (which methods exist, which ops are valid) are independently maintained in parallel between backends. `./test-all.sh` green.
