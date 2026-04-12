---
section: "06"
title: "Protocol Builtin Verification Matrix"
status: in-progress
reviewed: true
goal: "Pin every ProtocolBuiltin variant x argument position x ownership value in a test matrix, with RC balance verification through LLVM codegen audit for each protocol — ensuring that protocol builtin ownership changes never silently break RC correctness"
success_criteria:
  - "Every ProtocolBuiltin variant (Index, Iter, IterNext, IterDrop, CollectSet) has its ownership per-arg pinned in a test"
  - "LLVM codegen audit (ORI_AUDIT_CODEGEN=1) verifies RC balance for programs exercising each protocol builtin"
  - "An exhaustiveness test iterates ProtocolBuiltin::ALL and verifies every variant has test coverage"
  - "Ownership changes to any protocol builtin cause test failure (semantic pin)"
  - "New ProtocolBuiltin variants cannot be added without updating the test matrix (compile-time or test-time enforcement)"
inspired_by:
  - "Rust compiletest codegen tests — pin specific codegen patterns for compiler builtins"
  - "Lean4 IR Checker — exhaustive property checking for builtin operations"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Ownership Pin Matrix Tests"
    status: complete
  - id: "06.2"
    title: "RC Balance Codegen Audit Tests"
    status: complete
  - id: "06.3"
    title: "Exhaustiveness Guard"
    status: complete
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "06.N"
    title: "Completion Checklist"
    status: complete
---

# Section 06: Protocol Builtin Verification Matrix

**Status:** Complete
**Goal:** Pin every `ProtocolBuiltin` variant x argument position x ownership value in a test matrix, with RC balance verification through LLVM codegen audit for each protocol builtin. Protocol builtins (`ori_ir/src/builtin_constants/protocol/mod.rs`) are compiler-internal functions emitted by ARC lowering that carry per-argument ownership semantics. A wrong ownership value on a protocol builtin causes silent RC leaks or double-frees — the `__index` RC leak was caused by exactly this class of bug (unknown callee -> all Owned fallthrough). This section ensures that ownership changes to protocol builtins are caught by tests, not by users.

**Success Criteria:**

- [x] Every `ProtocolBuiltin` variant has per-arg ownership pinned — satisfies mission criterion: "Protocol builtin ownership pinned"
- [x] RC balance verified through LLVM codegen audit for each builtin — satisfies mission criterion: "Protocol builtin ownership pinned"
- [x] Exhaustiveness guard prevents unmatched new variants — satisfies mission criterion: "Protocol builtin ownership pinned"

**Context:** The `ProtocolBuiltin` enum has 5 variants: `Index`, `Iter`, `IterNext`, `IterDrop`, `CollectSet`. Each has a fixed `arg_count()` and `arg_ownership()` that determines how borrow inference treats its arguments. The existing tests in `protocol/tests.rs` (79 lines) cover existence, `from_name()`, and ownership per variant. The existing AIMS builtin tests in `ori_arc/src/aims/builtins/tests.rs` (98 lines) cover seed contract computation. What's missing: (1) RC balance verification through the full LLVM codegen pipeline for each protocol builtin, and (2) a test that will fail if a new `ProtocolBuiltin` variant is added without test coverage.

This is NOT a Cartesian product test — each argument position has exactly ONE expected ownership value (Owned or Borrowed). The matrix is small and fixed:

| Variant | Arg 0 | Arg 1 | Intercepted |
|---------|-------|-------|-------------|
| `Index` | Borrowed | Borrowed | Yes |
| `Iter` | Borrowed | — | No |
| `IterNext` | Owned | Borrowed | Yes |
| `IterDrop` | Owned | — | No |
| `CollectSet` | Owned | — | Yes |

**Reference implementations:**
- **Rust** `tests/codegen/intrinsics/` — pins codegen patterns for compiler intrinsics.

**Depends on:** Section 01 (blocking codegen audit behavior needed for RC balance verification under `ORI_AUDIT_CODEGEN=1`).

---

## 06.1 Ownership Pin Matrix Tests

**File(s):** `compiler/ori_ir/src/builtin_constants/protocol/tests.rs`

Extend the existing protocol tests to pin every ownership value explicitly. These are semantic pin tests — they exist to FAIL if someone changes the ownership of a protocol builtin argument without updating all downstream consumers.

- [x] Add per-variant ownership pin tests. The existing `tests.rs` may already have some of these — extend to cover every arg position explicitly with clear pin semantics:
  ```rust
  use super::*;

  /// Semantic pin: Index takes two Borrowed args.
  /// Changing this would cause RC ops on index receiver/key where none should exist.
  #[test]
  fn pin_index_ownership_both_borrowed() {
      let ownership = ProtocolBuiltin::Index.arg_ownership();
      assert_eq!(ownership.len(), 2);
      assert_eq!(ownership[0], ProtocolArgOwnership::Borrowed,
          "Index arg 0 (receiver) must be Borrowed — receiver is not consumed");
      assert_eq!(ownership[1], ProtocolArgOwnership::Borrowed,
          "Index arg 1 (key) must be Borrowed — key is not consumed");
  }

  /// Semantic pin: Iter takes one Borrowed arg.
  /// Changing this caused the `@main(args: [str])` ABI mismatch.
  #[test]
  fn pin_iter_ownership_borrowed() {
      let ownership = ProtocolBuiltin::Iter.arg_ownership();
      assert_eq!(ownership.len(), 1);
      assert_eq!(ownership[0], ProtocolArgOwnership::Borrowed,
          "Iter arg 0 (collection) must be Borrowed — iterator borrows, doesn't consume");
  }

  /// Semantic pin: IterNext takes Owned iterator, Borrowed type marker.
  #[test]
  fn pin_iter_next_ownership_owned_borrowed() {
      let ownership = ProtocolBuiltin::IterNext.arg_ownership();
      assert_eq!(ownership.len(), 2);
      assert_eq!(ownership[0], ProtocolArgOwnership::Owned,
          "IterNext arg 0 (iterator) must be Owned — iterator is consumed per step");
      assert_eq!(ownership[1], ProtocolArgOwnership::Borrowed,
          "IterNext arg 1 (type marker) must be Borrowed — marker is not consumed");
  }

  /// Semantic pin: IterDrop takes one Owned arg.
  #[test]
  fn pin_iter_drop_ownership_owned() {
      let ownership = ProtocolBuiltin::IterDrop.arg_ownership();
      assert_eq!(ownership.len(), 1);
      assert_eq!(ownership[0], ProtocolArgOwnership::Owned,
          "IterDrop arg 0 (iterator) must be Owned — iterator is consumed by cleanup");
  }

  /// Semantic pin: CollectSet takes one Owned arg.
  #[test]
  fn pin_collect_set_ownership_owned() {
      let ownership = ProtocolBuiltin::CollectSet.arg_ownership();
      assert_eq!(ownership.len(), 1);
      assert_eq!(ownership[0], ProtocolArgOwnership::Owned,
          "CollectSet arg 0 (iterator) must be Owned — iterator consumed during collection");
  }
  ```

- [x] Pin `is_intercepted()` behavior — the distinction between intercepted (emitted inline by LLVM emitter) and non-intercepted (real function calls) is load-bearing for codegen:
  ```rust
  #[test]
  fn pin_intercepted_status() {
      assert!(ProtocolBuiltin::Index.is_intercepted());
      assert!(!ProtocolBuiltin::Iter.is_intercepted());
      assert!(ProtocolBuiltin::IterNext.is_intercepted());
      assert!(!ProtocolBuiltin::IterDrop.is_intercepted());
      assert!(ProtocolBuiltin::CollectSet.is_intercepted());
  }
  ```

- [x] Pin `arg_count()` values:
  ```rust
  #[test]
  fn pin_arg_counts() {
      assert_eq!(ProtocolBuiltin::Index.arg_count(), 2);
      assert_eq!(ProtocolBuiltin::Iter.arg_count(), 1);
      assert_eq!(ProtocolBuiltin::IterNext.arg_count(), 2);
      assert_eq!(ProtocolBuiltin::IterDrop.arg_count(), 1);
      assert_eq!(ProtocolBuiltin::CollectSet.arg_count(), 1);
  }
  ```

- [x] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 06.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type.

---

## 06.2 RC Balance Codegen Audit Tests

**File(s):** `compiler/ori_llvm/tests/aot/protocol_builtins.rs` (new), `compiler/ori_llvm/tests/aot/mod.rs`

Verify RC balance through the full LLVM codegen pipeline for programs exercising each protocol builtin. These tests use `ORI_AUDIT_CODEGEN=1` (from Section 01's verifier gates) to check that the emitted LLVM IR has balanced RC operations.

- [x] Create Ori test programs that exercise each protocol builtin. Each program must be minimal but must trigger the specific protocol path:

  - **Index** (`__index`): `let xs = [1, 2, 3]; let x = xs[0]`
  - **Iter** (`iter`): `for x in [1, 2, 3] do print(msg: x.to_str())`
  - **IterNext** (`__iter_next`): implicit in `for` loops — same test as Iter exercises both
  - **IterDrop** (`ori_iter_drop`): `for x in [1, 2, 3] do { if x == 2 then break }` (early exit triggers explicit drop)
  - **CollectSet** (`__collect_set`): requires a `Set` collection from an iterator (e.g., `for x in [1, 2, 3] yield x` collected into a `Set`)

- [x] For each test program, compile via AOT and run the codegen audit:
  ```rust
  #[test]
  fn protocol_index_rc_balance() {
      // Compile the Index-exercising Ori program through the full pipeline
      // with ORI_AUDIT_CODEGEN=1 active.
      // Verify: zero audit findings related to RC balance.
  }
  ```

- [x] Run each compiled binary with `ORI_CHECK_LEAKS=1` and verify zero leaks:
  ```rust
  #[test]
  fn protocol_iter_drop_no_leaks() {
      // Compile + run the IterDrop-exercising program
      // with ORI_CHECK_LEAKS=1.
      // Verify: leak checker reports zero leaks.
  }
  ```

- [x] **TPR checkpoint** — `/tpr-review` covering 06.1–06.2 implementation work

- [x] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 06.1's close-out, scoped to 06.2's debugging journey.

---

## 06.3 Exhaustiveness Guard

**File(s):** `compiler/ori_ir/src/builtin_constants/protocol/tests.rs`

Add a test that iterates `ProtocolBuiltin::ALL` and verifies every variant has test coverage. This guard ensures that adding a new `ProtocolBuiltin` variant without updating the test matrix causes an immediate test failure.

- [x] Add an exhaustiveness test that checks `ALL` matches the expected count:
  ```rust
  #[test]
  fn exhaustiveness_all_variants_covered() {
      // If this test fails, a new ProtocolBuiltin variant was added
      // without updating the test matrix above.
      assert_eq!(
          ProtocolBuiltin::ALL.len(),
          5,
          "ProtocolBuiltin::ALL has {} variants but test matrix expects 5. \
           Update the ownership pin tests and RC balance tests for the new variant.",
          ProtocolBuiltin::ALL.len()
      );
  }
  ```

- [x] Add an exhaustiveness test that verifies every variant in `ALL` has a name and ownership:
  ```rust
  #[test]
  fn exhaustiveness_all_variants_have_name_and_ownership() {
      for builtin in ProtocolBuiltin::ALL {
          // name() must not panic
          let name = builtin.name();
          assert!(!name.is_empty(), "variant {builtin:?} has empty name");

          // arg_ownership() length must match arg_count()
          let ownership = builtin.arg_ownership();
          assert_eq!(
              ownership.len(),
              builtin.arg_count(),
              "variant {builtin:?} ({name}): arg_ownership length {} != arg_count {}",
              ownership.len(),
              builtin.arg_count(),
          );

          // from_name round-trip
          assert_eq!(
              ProtocolBuiltin::from_name(name),
              Some(*builtin),
              "from_name round-trip failed for {builtin:?}"
          );
      }
  }
  ```

- [x] Add a compile-time enforcement consideration: the `match` in `arg_ownership()` already covers all variants exhaustively (Rust enforces this). The test-time guard above is defense-in-depth for the cases where a variant is added to the enum and to the match but NOT to the test assertions.

- [x] **Subsection close-out (06.3)** — MANDATORY before starting 06.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 06.1's close-out, scoped to 06.3's debugging journey.

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

- None.

---

## 06.N Completion Checklist

- [x] Every `ProtocolBuiltin` variant has per-arg ownership pinned in dedicated test functions
- [x] `is_intercepted()` behavior pinned for all variants
- [x] `arg_count()` values pinned for all variants
- [x] RC balance verified via codegen audit for Index, Iter, IterNext, IterDrop, CollectSet
- [x] Leak check passed for programs exercising each protocol builtin
- [x] Exhaustiveness guard: adding a new variant without tests fails immediately
- [x] `from_name()` round-trip verified for all variants
- [x] No regressions: `timeout 150 ./test-all.sh` green
- [x] `timeout 150 ./clippy-all.sh` green
- [x] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 06` returns 0 annotations
- [x] All intermediate TPR checkpoint findings resolved
- [x] **Plan sync** — update plan metadata:
  - [x] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [x] `00-overview.md` Quick Reference updated
  - [x] `index.md` section status updated
- [x] `/tpr-review` passed (final, full-section)
- [x] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [x] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** Every `ProtocolBuiltin` variant has its per-argument ownership semantics pinned in test assertions. RC balance is verified through the full LLVM codegen pipeline for programs exercising each protocol builtin. An exhaustiveness guard ensures new variants cannot be added without test coverage. `ORI_CHECK_LEAKS=1` reports zero leaks for all protocol builtin test programs. `timeout 150 ./test-all.sh` passes with all new tests included.
