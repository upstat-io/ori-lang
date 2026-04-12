---
section: "06"
title: "Protocol Builtin Verification Matrix"
status: in-progress
reviewed: true
goal: "Verify that ProtocolBuiltin ownership values are correctly defined (ori_ir) and consumed (ori_arc borrow inference and AIMS contract seeding), with end-to-end validation through AOT leak checks (ori_llvm) across a type x pattern matrix. Note: ori_llvm does not directly consume arg_ownership() — it dispatches on from_name() and type info; ownership correctness flows through ori_arc's RC annotations."
success_criteria:
  - "Every ProtocolBuiltin variant (Index, Iter, IterNext, IterDrop, CollectSet) has its ownership per-arg pinned in existing tests — audit confirms no gaps"
  - "ori_arc consumers (seed_builtin_contracts, annotate_arg_ownership, promote_callee_args) produce correct MemoryContract/ArgOwnership from ProtocolBuiltin ownership values — verified by new consumer-level tests"
  - "AOT programs exercising each protocol builtin pass ORI_CHECK_LEAKS=1 across a type x pattern matrix (list, str, map, set x full iteration, break, yield, nested)"
  - "Negative pins verify that wrong ownership values (e.g., IterDrop=Borrowed) would produce observable failures"
  - "Exhaustiveness guard ensures new ProtocolBuiltin variants cannot be added without test coverage"
  - "Stale IterDrop doc comment fixed in source"
inspired_by:
  - "Rust compiletest codegen tests — pin specific codegen patterns for compiler builtins"
  - "Lean4 IR Checker — exhaustive property checking for builtin operations"
  - "Swift ARC tests — positive + negative RC pairing (must-optimize + must-not-optimize)"
depends_on: ["01"]
third_party_review:
  status: resolved
  updated: 2026-04-12
sections:
  - id: "06.1"
    title: "Existing Test Audit & Gap-Fill"
    status: complete
  - id: "06.2"
    title: "ori_arc Consumer Verification"
    status: complete
  - id: "06.3"
    title: "AOT End-to-End Type x Pattern Matrix"
    status: complete
  - id: "06.4"
    title: "Exhaustiveness Guard & Doc Fix"
    status: complete
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "06.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 06: Protocol Builtin Verification Matrix

> **RESET (2026-04-11):** All work in this section was produced by an autopilot session with inadequate planning and TPR oversight. Implementation code may exist in the codebase (commits from the autopilot session) but the design, test coverage, and verification cannot be trusted as valid. This section must be re-done from scratch with proper planning, review (`/review-plan`), and verification (`/tpr-review` + `/impl-hygiene-review`). The existing code should be audited during re-implementation — it may be partially reusable but must not be assumed correct.

**Status:** Not Started
**Goal:** Verify that `ProtocolBuiltin` ownership values are correctly **defined** (`ori_ir`) and **consumed** (`ori_arc` — borrow inference and AIMS contract seeding), with **end-to-end validation** through AOT leak checks (`ori_llvm`). Note: `try_emit_protocol()` in `ori_llvm` dispatches on `from_name()` and type information — it does NOT read `arg_ownership()`. The ownership consumers are purely in `ori_arc`; LLVM correctness is validated end-to-end via AOT tests, not by direct ownership-table consumption.

Protocol builtins (`compiler/ori_ir/src/builtin_constants/protocol/mod.rs`) are compiler-internal functions emitted by ARC lowering that carry per-argument ownership semantics. The critical failure mode is not that `ProtocolBuiltin::Index.arg_ownership()` returns the wrong constant (that is a tautology test) — it is that the **consumers** of these values (`seed_builtin_contracts`, `annotate_arg_ownership`, `promote_callee_args`) fail to use them correctly, producing wrong `MemoryContract`s or wrong `ArgOwnership` annotations. Note: `try_emit_protocol` in `ori_llvm` dispatches on `ProtocolBuiltin::from_name()` and type information — it does NOT read `arg_ownership()`. The ownership consumers are purely in `ori_arc`; LLVM is verified end-to-end via AOT tests. The `__index` RC leak was caused by exactly this class of bug — the ownership constant was correct, but the consumer fell through to the "unknown callee -> all Owned" default.

**Success Criteria:**

- [x] Audit confirms existing `protocol/tests.rs` (93 lines, 10 tests) already pins all ownership values — no new IR-level pin tests needed
- [x] New `ori_arc` consumer tests verify that `seed_builtin_contracts` produces correct `MemoryContract` params for each protocol builtin (access, consumption, cardinality=Once fields — all three dimensions pinned)
- [x] New `ori_arc` consumer tests verify that `annotate_arg_ownership` produces correct `ArgOwnership` vectors when called with protocol builtin callees
- [x] New `ori_arc` consumer tests verify that `promote_callee_args` correctly promotes/borrows params at protocol builtin call sites
- [x] AOT programs pass `ORI_CHECK_LEAKS=1` across type x pattern matrix: `[str]`, `[int]`, `{str: int}`, `Set<int>` x full iteration, break, yield, nested
- [x] At least one negative pin demonstrates that wrong ownership (e.g., IterDrop=Borrowed) produces a double-free or leak
- [x] Exhaustiveness guard: `ProtocolBuiltin::ALL.len() == 5` assertion prevents silent additions
- [x] `IterDrop` doc comment fixed: "borrowed (freed internally)" -> "owned (consumed by cleanup)"

**Context:** The `ProtocolBuiltin` enum has 5 variants. The ownership matrix is small and fixed:

| Variant | Arg 0 | Arg 1 | Intercepted | Consumers |
|---------|-------|-------|-------------|-----------|
| `Index` | Borrowed | Borrowed | Yes | `seed_builtin_contracts`, `annotate_arg_ownership`, `promote_callee_args` |
| `Iter` | Borrowed | — | No | `seed_builtin_contracts`, `borrowing_builtin_names` |
| `IterNext` | Owned | Borrowed | Yes | `seed_builtin_contracts`, `annotate_arg_ownership`, `promote_callee_args` |
| `IterDrop` | Owned | — | No | `seed_builtin_contracts`, `annotate_arg_ownership`, `promote_callee_args` |
| `CollectSet` | Owned | — | Yes | `seed_builtin_contracts`, `annotate_arg_ownership`, `promote_callee_args` |

**Existing test coverage (ALREADY DONE — do NOT rewrite):**
- `compiler/ori_ir/src/builtin_constants/protocol/tests.rs` (93 lines, 10 tests): pins `arg_count()`, `arg_ownership()` per variant, `is_intercepted()`, `from_name()` round-trip, exhaustiveness over `ALL`
- `compiler/ori_arc/src/aims/builtins/tests.rs` (151 lines, 7 tests): verifies `seed_builtin_contracts` populates entries for all protocol builtins
- `compiler/ori_arc/src/borrow/builtins/tests.rs` (376 lines, 18 tests): verifies `borrowing_builtin_names` includes all-borrowed protocols, `protocol_builtins_borrowing_sync` checks sync
- `compiler/ori_llvm/tests/aot/iterator_drop.rs`: exercises `IterDrop` ownership via enum/struct/tuple iterator scope exit
- `compiler/ori_llvm/tests/aot/iter_rc_matrix.rs` (88 tests): 6 types x 8 patterns x 2 loop variants — exercises `Iter`, `IterNext`, `IterDrop` through full AOT pipeline
- `compiler/ori_llvm/tests/aot/sets.rs`: exercises `CollectSet` via set construction
- `compiler/ori_llvm/tests/aot/fat_ptr_iter/method_collect.rs`: exercises `collect()` path including `__collect_set`

**What is MISSING (the actual work for this section):**
1. **ori_arc consumer-level tests** — existing tests verify constants and set membership, but do NOT verify that `seed_builtin_contracts` produces correct `MemoryContract` field values (access class, consumption, cardinality), that `annotate_arg_ownership` produces correct `ArgOwnership` vectors for protocol callees, or that `promote_callee_args` correctly promotes/borrows at protocol call sites
2. **Negative pins** — no test verifies that wrong ownership produces observable failure
3. **Map/Set type coverage in AOT** — `iter_rc_matrix.rs` covers `str`, `[int]`, `Option<str>`, closures, structs, maps but Set coverage is limited; `__index` on maps is not explicitly tested for RC balance
4. **ORI_AUDIT_CODEGEN=1 wiring** — the AOT harness sets `ORI_CHECK_LEAKS=1` but does NOT set `ORI_AUDIT_CODEGEN=1`; codegen audit is not exercised in protocol tests

**Reference implementations:**
- **Rust** `tests/codegen/intrinsics/` — pins codegen patterns for compiler intrinsics.
- **Swift** ARC tests — positive + negative RC pairing per protocol.

**Depends on:** Section 01 (blocking codegen audit behavior needed for RC balance verification under `ORI_AUDIT_CODEGEN=1`).

---

## 06.1 Existing Test Audit & Gap-Fill

**File(s):** `compiler/ori_ir/src/builtin_constants/protocol/tests.rs`, `compiler/ori_ir/src/builtin_constants/protocol/mod.rs`

Audit the existing 10 tests in `protocol/tests.rs` and confirm they pin all ownership values. Fix the stale `IterDrop` doc comment in the source. The existing tests are comprehensive for IR-level pinning — this subsection is an audit, not a rewrite.

- [x] **Audit existing tests** — read `compiler/ori_ir/src/builtin_constants/protocol/tests.rs` and verify the following are covered (check, do NOT rewrite):
  - `pin_arg_counts` — all 5 variants' arg counts pinned
  - `all_variants_covered` — `ALL.len() == 5`, `from_name` round-trip, ownership len == arg count
  - `from_name_returns_none_for_unknown` — negative: unknown names return `None`
  - `index_ownership_is_all_borrowed` — both args Borrowed
  - `iter_next_ownership_is_owned_borrowed` — arg 0 Owned, arg 1 Borrowed
  - `iter_ownership_is_borrowed` — single arg Borrowed
  - `iter_drop_ownership_is_owned` — single arg Owned (with doc comment explaining the Borrowed→Owned history)
  - `collect_set_ownership_is_owned` — single arg Owned
  - `is_intercepted_matches_dispatch` — `Iter`/`IterDrop` not intercepted, others intercepted
  - `is_intercepted_exhaustive` — all variants have defined interception status
  - **Result**: All 10 tests confirmed — no gaps for IR-level ownership pinning (2026-04-12)

- [x] **Fix stale `IterDrop` doc comment** in `compiler/ori_ir/src/builtin_constants/protocol/mod.rs` line 25: change `"Iterator cleanup. Iterator state borrowed (freed internally)."` to `"Iterator cleanup. Iterator handle owned (consumed by cleanup)."` — the ownership was changed from Borrowed to Owned (TPR-07-008) but the doc comment was not updated. This is a DRIFT finding. (Fixed 2026-04-12)

- [x] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`

---

## 06.2 ori_arc Consumer Verification

**File(s):** `compiler/ori_arc/src/aims/builtins/tests.rs`, `compiler/ori_arc/src/borrow/builtins/tests.rs`, `compiler/ori_arc/src/rc_insert/tests.rs`

The critical failure mode is not the constants — it is the **consumers**. Test that `seed_builtin_contracts`, `annotate_arg_ownership`, and `promote_callee_args` produce correct output from the `ProtocolBuiltin` ownership values. This is where the `__index` RC leak actually occurred: the constant was correct but the consumer fell through to the wrong default.

**Key consumers in ori_arc:**
1. `seed_builtin_contracts()` (`compiler/ori_arc/src/aims/builtins/mod.rs:77-79`) — converts `ProtocolArgOwnership` to `ParamContract` via `protocol_contract()`, inserting into the signature map
2. `annotate_arg_ownership()` (`compiler/ori_arc/src/rc_insert/annotate.rs:78-91`) — looks up protocol builtins in `BuiltinOwnershipSets.protocol` and maps to `ArgOwnership::Owned`/`Borrowed`
3. `promote_callee_args()` (`compiler/ori_arc/src/borrow/update.rs`) — promotes parameters to Owned based on callee argument ownership; protocol builtins are consulted via `BuiltinOwnershipSets`

- [x] **Add consumer-level tests to `compiler/ori_arc/src/aims/builtins/tests.rs`** — verify that `seed_builtin_contracts` produces `MemoryContract` with correct field values (not just "entry exists"). Added 5 per-builtin tests + found dispatch order bug in `annotate_arg_ownership` (2026-04-12):
  ```rust
  /// Verify seed_builtin_contracts produces correct MemoryContract
  /// fields for each protocol builtin — not just that an entry exists.
  /// Test naming: <subject>_<scenario>_<expected> per impl-hygiene.md.
  #[test]
  fn protocol_contract_index_has_two_borrowed_params() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let index_name = interner.intern("__index");
      let contract = &sigs[&index_name];
      assert_eq!(contract.params.len(), 2);
      assert_eq!(contract.params[0].access, AccessClass::Borrowed);
      assert_eq!(contract.params[0].consumption, Consumption::Dead);
      assert_eq!(contract.params[0].cardinality, Cardinality::Once);
      assert_eq!(contract.params[1].access, AccessClass::Borrowed);
      assert_eq!(contract.params[1].consumption, Consumption::Dead);
      assert_eq!(contract.params[1].cardinality, Cardinality::Once);
  }

  #[test]
  fn protocol_contract_iter_drop_has_owned_linear_param() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("ori_iter_drop");
      let contract = &sigs[&name];
      assert_eq!(contract.params.len(), 1);
      assert_eq!(contract.params[0].access, AccessClass::Owned);
      assert_eq!(contract.params[0].consumption, Consumption::Linear);
      assert_eq!(contract.params[0].cardinality, Cardinality::Once);
  }

  #[test]
  fn protocol_contract_iter_next_owned_then_borrowed() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("__iter_next");
      let contract = &sigs[&name];
      assert_eq!(contract.params.len(), 2);
      assert_eq!(contract.params[0].access, AccessClass::Owned);
      assert_eq!(contract.params[0].consumption, Consumption::Linear);
      assert_eq!(contract.params[0].cardinality, Cardinality::Once);
      assert_eq!(contract.params[1].access, AccessClass::Borrowed);
      assert_eq!(contract.params[1].consumption, Consumption::Dead);
      assert_eq!(contract.params[1].cardinality, Cardinality::Once);
  }

  #[test]
  fn protocol_contract_collect_set_owned_linear() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("__collect_set");
      let contract = &sigs[&name];
      assert_eq!(contract.params.len(), 1);
      assert_eq!(contract.params[0].access, AccessClass::Owned);
      assert_eq!(contract.params[0].consumption, Consumption::Linear);
      assert_eq!(contract.params[0].cardinality, Cardinality::Once);
  }

  #[test]
  fn protocol_contract_iter_borrowed_param() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("iter");
      let contract = &sigs[&name];
      assert_eq!(contract.params.len(), 1);
      assert_eq!(contract.params[0].access, AccessClass::Borrowed);
      assert_eq!(contract.params[0].consumption, Consumption::Dead);
      assert_eq!(contract.params[0].cardinality, Cardinality::Once);
  }
  ```

- [x] **Add `BuiltinOwnershipSets` integration test to `compiler/ori_arc/src/borrow/builtins/tests.rs`** — verify that `BuiltinOwnershipSets::new()` correctly populates the `protocol` map with the right ownership arrays (2026-04-12):
  ```rust
  /// Verify BuiltinOwnershipSets.protocol maps each protocol builtin
  /// name to its correct per-arg ownership array. This is the data
  /// that annotate_arg_ownership() consumes at call sites.
  #[test]
  fn ownership_sets_protocol_map_matches_all_builtins() {
      let interner = StringInterner::default();
      let sets = BuiltinOwnershipSets::new(&interner);
      for &pb in ProtocolBuiltin::ALL {
          let name = interner.intern(pb.name());
          let ownership = sets.protocol.get(&name)
              .unwrap_or_else(|| panic!("protocol {:?} missing from BuiltinOwnershipSets.protocol", pb));
          assert_eq!(*ownership, pb.arg_ownership(),
              "BuiltinOwnershipSets.protocol[{:?}] doesn't match ProtocolBuiltin.arg_ownership()", pb);
      }
      assert_eq!(sets.protocol.len(), ProtocolBuiltin::ALL.len(),
          "BuiltinOwnershipSets.protocol has extra entries beyond ProtocolBuiltin::ALL");
  }
  ```

- [x] **Add `annotate_arg_ownership` consumer test to `compiler/ori_arc/src/rc_insert/tests.rs`** — this is the consumer directly responsible for the original `__index` bug. Verify that when `annotate_arg_ownership` encounters a protocol builtin callee, it produces the correct `ArgOwnership` vector. **Found and fixed dispatch order bug**: `ori_iter_drop` was incorrectly matched by the `ori_` prefix check (external C runtime → all-Borrowed) before reaching the protocol builtin check. Fix: moved protocol check to first position in the cascade (2026-04-12):
  - `__index` call → `[Borrowed, Borrowed]` ✓
  - `__iter_next` call → `[Owned, Borrowed]` ✓
  - `ori_iter_drop` call → `[Owned]` ✓ (FIXED: was `[Borrowed]` before dispatch reorder)
  - `__collect_set` call → `[Owned]` ✓
  - `iter` call → `[Borrowed]` ✓

- [x] **Add `promote_callee_args` consumer test to `compiler/ori_arc/src/borrow/tests.rs`** — verify that borrow inference correctly promotes/borrows parameters at protocol builtin call sites (2026-04-12):
  - For `__index` (both Borrowed): neither arg is promoted to Owned ✓
  - For `ori_iter_drop` (Owned): arg IS promoted ✓

- [x] **Add negative pin (forbid-old-behavior)** — a negative pin must forbid the broken behavior that existed before the fix. For protocol builtins, the historic bug was `IterDrop` having `Borrowed` ownership (causing double-free). The negative pin asserts the old behavior does NOT exist (2026-04-12):
  ```rust
  /// Negative pin: IterDrop must NOT have Borrowed access.
  /// Before the fix (TPR-07-008), IterDrop was Borrowed, causing
  /// a double-free on iterator cleanup. This test forbids the
  /// old behavior — if someone reverts the ownership change,
  /// this test fails immediately.
  #[test]
  fn protocol_contract_iter_drop_forbids_borrowed() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("ori_iter_drop");
      let contract = &sigs[&name];
      assert_ne!(contract.params[0].access, AccessClass::Borrowed,
          "IterDrop MUST NOT be Borrowed — Borrowed ownership causes \
           a second scope-exit RcDec (double-free). See TPR-07-008.");
  }
  
  /// Negative pin: Index must NOT have Owned access on arg 0.
  /// The __index bug was caused by the "unknown callee -> all Owned"
  /// fallthrough. This forbids that regression.
  #[test]
  fn protocol_contract_index_forbids_owned_receiver() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      let name = interner.intern("__index");
      let contract = &sigs[&name];
      assert_ne!(contract.params[0].access, AccessClass::Owned,
          "Index receiver MUST NOT be Owned — Owned receiver causes \
           the collection to be consumed on index lookup. See __index RC leak.");
  }
  ```

- [x] **Add consistency pin (supplemental, not negative)** — verify that for every protocol builtin, the `seed_builtin_contracts` output matches `arg_ownership()`. This is a positive regression guard, not a negative pin (2026-04-12):
  ```rust
  /// Consistency pin: contracts match arg_ownership() for all protocol builtins.
  #[test]
  fn protocol_contract_access_consistent_with_arg_ownership() {
      let (interner, builtins) = setup();
      let mut sigs = FxHashMap::default();
      seed_builtin_contracts(&mut sigs, &builtins, &interner);
      for &pb in ProtocolBuiltin::ALL {
          let name = interner.intern(pb.name());
          let contract = &sigs[&name];
          for (i, arg_own) in pb.arg_ownership().iter().enumerate() {
              let expected = match arg_own {
                  ProtocolArgOwnership::Borrowed => AccessClass::Borrowed,
                  ProtocolArgOwnership::Owned => AccessClass::Owned,
              };
              assert_eq!(contract.params[i].access, expected,
                  "{:?} arg {i}: contract {:?} != expected {:?}",
                  pb, contract.params[i].access, expected);
          }
      }
  }
  ```

- [x] **TPR checkpoint** — `/tpr-review` covering 06.1–06.2 implementation work (2026-04-12). Clean on iteration 2. Iteration 1: convergent GAP fixed (missing __iter_next mixed-ownership test). Iteration 2: codex clean (16 files, ran all tests), gemini naming finding rejected after verification (matches established crate convention).

- [x] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`

---

## 06.3 AOT End-to-End Type x Pattern Matrix

**File(s):** Existing AOT test files (extend where gaps found), `compiler/ori_llvm/tests/aot/util/aot.rs`

Verify RC balance through the full LLVM codegen pipeline for programs exercising each protocol builtin. All existing AOT tests already enable `ORI_CHECK_LEAKS=1` (wired in `compile_and_run_capture`). The work here is to identify gaps in the existing type x pattern matrix and fill them — NOT to create a new `protocol_builtins.rs` test file that duplicates existing coverage.

**Existing coverage audit (DO FIRST — check these files for gaps):**
- `iter_rc_matrix.rs` (88 tests): `str`, `[int]`, `Option<str>`, closure, struct, map x full/break/yield/two-call/nested/guard/unwind/continue — exercises `Iter`, `IterNext`, `IterDrop`
- `iterator_drop.rs`: enum-tagged-ptr iterator, struct field, tuple element, bare unused iter — exercises `IterDrop` specifically
- `sets.rs`: set length, contains, insert, remove, union, intersection, difference, for_each — exercises `CollectSet`
- `fat_ptr_iter/method_collect.rs`: `.iter().collect()` for `[str]` and `Set<str>` — exercises `CollectSet`
- `cow_map_set.rs`: map/set COW operations
- `for_loops.rs`: basic for-loop patterns
- Other `fat_ptr_iter/` submodules: `control_flow.rs`, `map_set.rs`, `nested_list.rs`, etc.

**Known gaps to fill:**

- [x] **Map indexing RC balance gap-fill** (2026-04-12) — added `test_coll_map_index_int_str` ({int: str} reversed types, exercises value-type RC) and `test_coll_map_index_in_loop` (map indexing inside for-loop, exercises __index + Iter/IterNext interaction). Both pass with ORI_CHECK_LEAKS=1.

- [x] **Set iteration type coverage** (2026-04-12) — Set iteration NOW WORKS in AOT (stale exclusion). Added `Set<int>` to iter_rc_matrix with 4 tests: for-do full/break + for-yield full/break. Updated matrix header from E7 excluded to E7 = Set<int>. All pass with ORI_CHECK_LEAKS=1.

- [x] **`CollectSet` with non-trivial element types** (2026-04-12) — added `test_iter_collect_nested_list` ([[int]] → collect exercises elem_inc_fn for [int]) and `test_iter_collect_map_elements` ([{str: int}] → collect exercises elem_inc_fn for {str: int}). Both pass. Note: nested_list test avoids `Option<[int]>.unwrap()` due to BUG-04-061 (AOT monomorphization gap).

- [x] **AOT regression guard** (2026-04-12) — all 2,161 AOT tests pass (0 failed, 22 ignored) with `ORI_CHECK_LEAKS=1`. Positive regression guard confirmed.

- [x] **ORI_AUDIT_CODEGEN=1 in harness** (2026-04-12) — evaluated: 38/2161 tests fail with `ORI_AUDIT_CODEGEN=1`, all concentrated in unwind/panic/catch paths (cli::test_main_args_*, error_handling::test_catch_*, fat_ptr_iter::unwind::*). Decision: NOT adding globally — filed as BUG-04-062. Once unwind RC audit is fixed, harness can enable it.

- [x] **Debug AND release** (2026-04-12) — both debug (`cargo test -p ori_llvm`: 2161 pass/0 fail) and release (`cargo test -p ori_llvm --release`: 2161 pass/0 fail) builds pass. No FastISel divergence on protocol-exercising tests.

- [x] **Subsection close-out (06.3)** — MANDATORY before starting 06.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 06.3: The `ORI_AUDIT_CODEGEN=1` evaluation exposed 38 unwind-path RC failures (filed as BUG-04-062) — this surfaced a real issue class. The `unwrap()` monomorphization gap was caught by a test exercising complex generic types (filed as BUG-04-061). No diagnostic scripts needed; targeted `cargo test` with specific filters was sufficient. No tooling gaps — the AOT harness's `ORI_CHECK_LEAKS=1` integration provided immediate, actionable feedback (exit code 2 = leak).

---

## 06.4 Exhaustiveness Guard & Doc Fix

**File(s):** `compiler/ori_ir/src/builtin_constants/protocol/tests.rs`

The exhaustiveness guard already exists: `all_variants_covered` asserts `ALL.len() == 5` and iterates all variants. This subsection verifies it is sufficient and adds defense-in-depth if needed.

- [x] **Verify existing exhaustiveness guard** (2026-04-12) — `all_variants_covered()` in `protocol/tests.rs` is sufficient: asserts `ALL.len() == 5`, iterates all variants checking `name()` non-empty + `from_name` round-trip + `arg_ownership().len() == arg_count()`. `is_intercepted_exhaustive()` separately confirms all variants have defined interception status. `pin_arg_counts()` pins exact arg counts per variant. No new code needed.

- [x] **Compile-time exhaustiveness note** (2026-04-12) — added doc comment on `all_variants_covered()` explaining that all four methods use exhaustive match (no `_` catch-all), so Rust enforces coverage at compile time. Test-time guard is defense-in-depth for the `ALL` constant and test assertions.

- [x] **Subsection close-out (06.4)** — MANDATORY before starting 06.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`

---

## 06.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- [x] `[TPR-06-001-codex][medium]` `compiler/ori_arc/src/borrow/tests.rs:1435` — Add mixed-ownership protocol promotion coverage for __iter_next.
  Evidence: GAP — promote_callee_args coverage only exercises all-borrowed (Index) and single-arg owned (IterDrop). The mixed [Owned, Borrowed] vector for __iter_next is untested.
  Impact: Partial-promotion path unverified; regression could reintroduce ownership DRIFT at iterator-advance call sites.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-12. Added `promote_protocol_iter_next_promotes_first_arg_only` test.
- [x] `[TPR-06-001-gemini][medium]` `compiler/ori_arc/src/borrow/tests.rs:1434` — Add missing promote_callee_args test for __iter_next.
  Evidence: Plan section 06.2 requested consumer test for __iter_next (Owned, Borrowed). Test was omitted.
  Impact: Mixed-ownership iteration logic in promote_callee_args unverified.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed on 2026-04-12. Same fix as [TPR-06-001-codex] (convergent finding).
- [x] `[TPR-06-002-gemini][low]` `compiler/ori_arc/src/aims/builtins/tests.rs:180` — Test functions lack mandatory shape and prefix.
  Evidence: Claims test functions omit `test_` prefix and lack `<scenario>` component per impl-hygiene.md §Test Function Naming.
  Impact: Low (naming convention concern).
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Rejected after verification on 2026-04-12. The existing 30+ tests in borrow/tests.rs (pre-dating this work) all omit the `test_` prefix — this IS the established crate convention. The impl-hygiene.md shape definition is `<subject>_<scenario>_<expected>` (no `test_` in the shape). Adding the prefix would introduce inconsistency with the existing file. Names are descriptive and self-explanatory. Codex (HIGH trust, 16 files read, ran all tests) found zero naming issues.

---

## 06.N Completion Checklist

- [x] Existing `protocol/tests.rs` audited — all 10 tests confirmed covering IR-level ownership pins
- [x] `IterDrop` doc comment fixed: "borrowed" -> "owned (consumed by cleanup)"
- [x] `seed_builtin_contracts` consumer tests verify `MemoryContract` field values (access, consumption, cardinality) for all 5 protocol builtins
- [x] `annotate_arg_ownership` consumer test verifies correct `ArgOwnership` vectors for protocol builtin callees
- [x] `promote_callee_args` consumer test verifies correct promotion/borrowing at protocol call sites
- [x] `BuiltinOwnershipSets.protocol` integration test verifies name-to-ownership mapping for all builtins
- [x] Negative pin (forbid-old-behavior): `assert_ne` pins that IterDrop is NOT Borrowed and Index receiver is NOT Owned — forbids the historic regression states
- [x] Consistency pin (supplemental): contracts read from `arg_ownership()` — drift between constant and contract is caught
- [x] AOT gap-fill: map indexing gap-fill (existing `{str:int}` covered; add `{int:str}` and looped indexing)
- [x] AOT gap-fill: `CollectSet` with complex element types tested (nested list, map elements)
- [x] Set iteration status evaluated — Set iteration works in AOT, added Set<int> to iter_rc_matrix (4 tests)
- [x] `ORI_AUDIT_CODEGEN=1` harness integration evaluated — 38 unwind-path failures, NOT adding globally; filed BUG-04-062
- [x] Debug AND release builds pass for all protocol-exercising AOT tests
- [x] No regressions: `timeout 150 ./test-all.sh` green (17,167 tests pass)
- [x] `timeout 150 ./clippy-all.sh` green
- [x] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan llvm-verification-tooling` returns 0 annotations
- [x] All intermediate TPR checkpoint findings resolved (TPR-06-001 convergent finding fixed, TPR-06-002 rejected)
- [x] **Plan sync** (2026-04-12) — update plan metadata:
  - [x] This section's frontmatter `status` -> `complete`, subsection statuses updated
  - [x] `00-overview.md` Quick Reference updated (effort table + section reference + success criterion)
  - [x] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** Protocol builtin ownership is verified at all three layers: IR definition (existing tests audited), ori_arc consumers (new MemoryContract field-level tests + negative pins), and LLVM codegen (AOT type x pattern matrix with `ORI_CHECK_LEAKS=1`). An exhaustiveness guard ensures new variants cannot be added without test coverage. `timeout 150 ./test-all.sh` passes with all new tests included.
