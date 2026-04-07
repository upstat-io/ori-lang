---
section: "03"
title: "ori_repr EscapeInfo Storage + Query Plumbing"
status: not-started
reviewed: false
goal: "Replace the ori_repr::escape::EscapeInfo placeholder ZST with the concrete per-function FxHashMap<ArcVarId, Locality> storage shape locked in Section 01.3, and replace the hardcoded `let result = true;` body of ReprPlan::escapes() at plan/query.rs:111 with a lookup mirroring the rc_strategy() pattern at plan/query.rs:124-134"
success_criteria:
  - "compiler/ori_repr/src/escape/mod.rs no longer contains the `pub struct EscapeInfo;` ZST. It defines a concrete struct with a `var_escape: FxHashMap<ArcVarId, Locality>` field"
  - "EscapeInfo has 4 public methods: escape_scope, escapes, is_non_escaping, join_escape_scope"
  - "join_escape_scope is monotone — it never narrows (verified by a test)"
  - "compiler/ori_repr/src/plan/query.rs::ReprPlan::escapes() body no longer hardcodes `let result = true;`. It consults `escape_info` for the given function and variable"
  - "The new escapes() body returns true (the same as the hardcode) for any (func, var) pair not present in escape_info — behavioral parity with the previous hardcode for unanalyzed code"
  - "EscapeInfo round-trips through ori_arc::Locality cleanly (cross-crate behavioral test passes)"
  - "cargo test -p ori_repr green; cargo test -p ori_arc green (no regression from cross-crate import)"
  - "Connects upward to mission criteria: EscapeInfo placeholder removed, escapes() body replaced"
inspired_by:
  - "compiler/ori_repr/src/plan/query.rs::ReprPlan::rc_strategy at line 124-134 (the canonical pattern this section mirrors)"
  - "compiler/ori_repr/src/plan.rs:21 (existing ori_arc::ArcVarId import — proves cross-crate import is fine)"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Replace EscapeInfo placeholder with concrete storage"
    status: not-started
  - id: "03.2"
    title: "Replace ReprPlan::escapes() body to consult escape_info"
    status: not-started
  - id: "03.3"
    title: "Add EscapeInfo unit tests"
    status: not-started
  - id: "03.4"
    title: "Add cross-crate behavioral test (round-trip through ori_arc::Locality)"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: ori_repr EscapeInfo Storage + Query Plumbing

**Status:** Not Started
**Goal:** Replace the 12-line `EscapeInfo` placeholder ZST in `compiler/ori_repr/src/escape/mod.rs` with the concrete `FxHashMap<ArcVarId, Locality>` storage shape locked in Section 01.3, and replace the hardcoded `let result = true;` body of `ReprPlan::escapes()` at `plan/query.rs:111` with a real lookup mirroring the `rc_strategy()` pattern at `plan/query.rs:124-134`. This section is the **plumbing layer** that future `repr-opt §08` will populate.

**Success Criteria:**

- [ ] `compiler/ori_repr/src/escape/mod.rs` no longer contains `pub struct EscapeInfo;` (the placeholder ZST). It defines a concrete struct with a `var_escape: FxHashMap<ArcVarId, Locality>` field
- [ ] `EscapeInfo` has 4 public methods: `escape_scope`, `escapes`, `is_non_escaping`, `join_escape_scope`
- [ ] `join_escape_scope` is **monotone** — it never narrows. Verified by a test that calls `join_escape_scope(var, HeapEscaping)` then `join_escape_scope(var, BlockLocal)` and asserts the final value is `HeapEscaping`, not `BlockLocal`
- [ ] `compiler/ori_repr/src/plan/query.rs::ReprPlan::escapes()` body no longer hardcodes `let result = true;`. It consults `escape_info.get(&func).map(|info| info.escapes(var)).unwrap_or(true)` (or equivalent)
- [ ] **Behavioral parity with the hardcode** for the unanalyzed case: `escapes()` returns `true` for any `(func, var)` pair where the function has not yet been analyzed by `repr-opt §08`. Verified by a test that calls `escapes()` on an empty `ReprPlan` and asserts the result is `true`
- [ ] **Behavioral departure from the hardcode** for the analyzed case: `escapes()` returns the actual computed value when `escape_info` contains a relevant entry. Verified by a test that constructs an `EscapeInfo` with `var → BlockLocal` and asserts `escapes()` returns `false`
- [ ] EscapeInfo round-trips through `ori_arc::Locality` cleanly (cross-crate behavioral test in 03.4)
- [ ] `cargo test -p ori_repr` green
- [ ] `cargo test -p ori_arc` green (no regression from the new `EscapeInfo` consuming `Locality`)
- [ ] `cargo check -p ori_repr` and `cargo check -p ori_llvm` green (any downstream crate that re-exports or consumes `EscapeInfo` still compiles)
- [ ] Connects upward to mission criteria: "EscapeInfo placeholder no longer exists", "escapes() body replaced"

**Context:** Phase 2 research found that the existing `EscapeInfo` is a 12-line placeholder unit struct (`pub struct EscapeInfo;`) and the `ReprPlan::escapes()` query body hardcodes `let result = true;`. Both are deliberately stubbed for future `repr-opt §08` to populate. This plan provides the storage shape and the query plumbing; §08 implements the analysis that fills the storage.

The work is small (~100 lines net) but it has **two cross-crate boundaries** that require care:

1. **`ori_repr` → `ori_arc` import**: The new `EscapeInfo` imports `ori_arc::aims::lattice::Locality`. This is a **new** cross-crate import path, but the dependency direction (`ori_repr → ori_arc`) already exists per `compiler/ori_repr/src/plan.rs:21` (which imports `ori_arc::ArcVarId`). No new dependency edge is added; only a new symbol is imported through the existing edge.
2. **Behavioral parity for unanalyzed code**: The current `escapes()` hardcode returns `true` for everything. After the replacement, it must continue to return `true` for any `(func, var)` pair that `repr-opt §08` has not analyzed (which is currently *all* pairs, since §08 doesn't exist yet). The default value of `EscapeInfo::escape_scope` (returning `Locality::Unknown` for unset variables) plus `EscapeInfo::escapes`'s `> Locality::FunctionLocal` predicate gives the right answer (`Unknown > FunctionLocal` is `true`), but the implementer must verify this behavioral parity with a test rather than reason from inspection alone.

**Reference implementations:**

- **`compiler/ori_repr/src/plan/query.rs::ReprPlan::rc_strategy` at lines 124-134**: the **canonical pattern** this section mirrors. It reads from a per-key map (`rc_strategies`), falls back to a sensible default (`Atomic { width: I64 }`), and emits a tracing event. The new `escapes()` body should look structurally identical, just reading from `escape_info` instead of `rc_strategies` and falling back to `true`.
- **`compiler/ori_repr/src/plan.rs:21`**: existing `use ori_arc::ArcVarId;` line. Proves cross-crate import from `ori_repr` to `ori_arc` is permitted by the existing dependency topology and not a new architectural decision.
- **Section 01.3 of this plan**: the locked `EscapeInfo` API design. Section 03 implements that exact code block.

**Depends on:** Section 02 (the `Locality` enum must have the `ArgEscaping` variant and the new comment in `dimensions.rs` before this section imports it; otherwise `EscapeInfo`'s storage type couldn't represent the new variant).

---

## 03.1 Replace EscapeInfo placeholder with concrete storage

**File(s):** `compiler/ori_repr/src/escape/mod.rs` (currently 12 lines, ZST placeholder)

**Context:** The current file is exactly:

```rust
//! Escape analysis types.
//!
//! **Placeholder** — exports `EscapeInfo` so that
//! `ReprPlan::escape_info` compiles. Replaced when escape analysis
//! is implemented with the full connection graph and escape state framework.

/// Placeholder for per-function escape analysis information.
///
/// Replaced by the full `EscapeInfo` type when escape analysis is implemented.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EscapeInfo;
```

Section 03.1 replaces it with the locked design from Section 01.3.

- [ ] Re-read `compiler/ori_repr/src/escape/mod.rs` to confirm it is still the 12-line placeholder. If it has been touched since plan creation, re-derive the replacement plan against the current content.

- [ ] Replace the entire file with the concrete `EscapeInfo` per Section 01.3:
  ```rust
  //! Per-function escape analysis storage.
  //!
  //! Replaces the placeholder `EscapeInfo` ZST with concrete per-function
  //! escape facts. Populated by repr-opt §08's connection-graph escape
  //! analysis. Consumed by `ReprPlan::escapes()`, repr-opt §09's sharing
  //! bound analysis, and repr-opt §10's thread-locality analysis (which
  //! routes through `RcStrategy::NonAtomic`, NOT a parallel ThreadLocality
  //! enum — see plans/repr-opt/00-overview.md:192).
  //!
  //! ## Design
  //!
  //! The storage is a per-function `FxHashMap<ArcVarId, Locality>` where
  //! the value is the unified `ori_arc::aims::lattice::Locality` ordered
  //! lattice (BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping <
  //! Unknown). Variables not present in the map default to `Unknown`
  //! (conservative — assumed to escape).
  //!
  //! The writer (`join_escape_scope`) is monotone: it widens but never
  //! narrows. This prevents accidental loss of precision in the ordered
  //! lattice from a producer that incorrectly tries to "downgrade" a
  //! variable's escape scope.

  use ori_arc::aims::lattice::Locality;
  use ori_arc::ir::ArcVarId;
  use rustc_hash::FxHashMap;

  /// Per-function escape analysis information.
  ///
  /// Maps each variable in a function to its computed escape scope.
  /// Populated by `repr-opt §08`'s connection-graph escape analysis.
  /// Consumed by `ReprPlan::escapes()`, `repr-opt §09`'s sharing bound
  /// analysis, and `repr-opt §10`'s thread-locality analysis.
  ///
  /// Variables not present default to `Locality::Unknown` (conservative).
  #[derive(Debug, Clone, Default, PartialEq, Eq)]
  pub struct EscapeInfo {
      var_escape: FxHashMap<ArcVarId, Locality>,
  }

  impl EscapeInfo {
      /// Query the escape scope of a variable.
      ///
      /// Returns `Locality::Unknown` if the variable is not present in the
      /// map (the conservative default — variables not yet analyzed are
      /// assumed to escape).
      #[must_use]
      pub fn escape_scope(&self, var: ArcVarId) -> Locality {
          self.var_escape.get(&var).copied().unwrap_or(Locality::Unknown)
      }

      /// Whether the variable escapes its function.
      ///
      /// True iff the variable's locality is `> FunctionLocal` — i.e., it
      /// has been observed flowing across a call boundary, to the heap,
      /// or to an unknown destination. Used by `ReprPlan::escapes()`.
      #[must_use]
      pub fn escapes(&self, var: ArcVarId) -> bool {
          self.escape_scope(var) > Locality::FunctionLocal
      }

      /// Whether the variable is provably non-escaping.
      ///
      /// True iff the variable's locality is `<= FunctionLocal` — i.e., it
      /// is known to stay within the defining function. Convenience for
      /// `repr-opt §09`'s `SharingBound::Unique` check.
      #[must_use]
      pub fn is_non_escaping(&self, var: ArcVarId) -> bool {
          self.escape_scope(var) <= Locality::FunctionLocal
      }

      /// Monotone widening: only widens, never narrows.
      ///
      /// The escape lattice is ordered (BlockLocal < FunctionLocal <
      /// ArgEscaping < HeapEscaping < Unknown), and the analysis discovers
      /// escape paths monotonically. This writer prevents a buggy producer
      /// from accidentally narrowing a variable's escape scope (which would
      /// be unsound — once a value is observed escaping, it cannot un-escape).
      ///
      /// If `var` is not yet in the map, it is inserted at the join of
      /// `BlockLocal` (the implicit starting point) and `scope`. If `var`
      /// is already in the map, the entry is replaced with the join of the
      /// old and new values.
      pub fn join_escape_scope(&mut self, var: ArcVarId, scope: Locality) {
          let entry = self.var_escape.entry(var).or_insert(Locality::BlockLocal);
          *entry = (*entry).join(scope);
      }
  }
  ```

- [ ] Verify the new file imports compile. Run `cargo check -p ori_repr` and confirm zero errors. If the `ori_arc::aims::lattice::Locality` path has changed due to Section 00's split, update the import accordingly. Per Section 00, the post-split path is still `ori_arc::aims::lattice::Locality` because `lattice/mod.rs` re-exports `state::AimsState` and the dimension enums from `dimensions.rs`.

- [ ] Verify the existing `ReprPlan::escape_info: FxHashMap<Name, EscapeInfo>` field at `compiler/ori_repr/src/plan.rs:91` still compiles. The `EscapeInfo` type changed shape but not name, so the field declaration is unchanged.

- [ ] Run `cargo build -p ori_repr` and verify zero errors and zero new warnings.

---

## 03.2 Replace ReprPlan::escapes() body to consult escape_info

**File(s):** `compiler/ori_repr/src/plan/query.rs` (the `escapes()` method body is currently at lines 105-114, with the actual hardcoded line at line 111)

**Context:** The current body is:

```rust
/// Check if a variable escapes its function scope.
///
/// Returns `true` by default — safe (never stack-promotes when unsure).
#[must_use]
pub fn escapes(&self, func: ori_ir::Name, var: ori_arc::ArcVarId) -> bool {
    // Until escape analysis populates escape_info, assume everything escapes.
    let result = true;
    tracing::trace!(?func, ?var, escapes = result, "escapes query");
    result
}
```

The replacement mirrors the `rc_strategy()` pattern at lines 124-134 of the same file:

```rust
/// Get the RC strategy for a type.
#[must_use]
pub fn rc_strategy(&self, idx: ori_types::Idx) -> RcStrategy {
    let strategy = self
        .rc_strategies
        .get(&idx)
        .copied()
        .unwrap_or(RcStrategy::Atomic {
            width: IntWidth::I64,
        });
    tracing::trace!(?idx, ?strategy, "rc_strategy query");
    strategy
}
```

The new `escapes()` body follows the same shape: read from a per-key map, fall back to a sensible default, emit a tracing event.

- [ ] Re-read `compiler/ori_repr/src/plan/query.rs` lines 100-140 to confirm both the old `escapes()` body and the `rc_strategy()` pattern are still in place. If either has changed, adapt the replacement.

- [ ] Replace the `escapes()` body:
  ```rust
  /// Check if a variable escapes its function scope.
  ///
  /// Reads from `self.escape_info`. If the function has not yet been
  /// analyzed by `repr-opt §08`, returns `true` as the conservative default
  /// (matching the previous hardcoded behavior for unanalyzed code). Once
  /// `§08` populates `escape_info`, this query returns the actual computed
  /// answer.
  ///
  /// Mirrors the pattern of `rc_strategy()` at line 124 of this file:
  /// per-key map lookup with sensible default and tracing.
  #[must_use]
  pub fn escapes(&self, func: ori_ir::Name, var: ori_arc::ArcVarId) -> bool {
      let result = self
          .escape_info
          .get(&func)
          .map(|info| info.escapes(var))
          .unwrap_or(true);
      tracing::trace!(?func, ?var, escapes = result, "escapes query");
      result
  }
  ```

- [ ] Verify the replacement compiles. Run `cargo check -p ori_repr` and confirm zero errors.

- [ ] Verify behavioral parity for the unanalyzed case. The previous hardcoded body always returned `true`. The new body returns `self.escape_info.get(&func)... .unwrap_or(true)`. For any `func` where `escape_info` does not contain an entry (i.e., `repr-opt §08` has not analyzed it), the lookup returns `None`, the `.unwrap_or(true)` fires, and the return value is `true`. **Behavioral parity holds for the empty-analysis case.**

- [ ] Verify behavioral departure for the analyzed case. The previous body could not return `false` — every query returned `true`. The new body returns `info.escapes(var)` when the function has been analyzed, which can be `false` if the variable is provably non-escaping. **This is the desired behavioral departure** — it's the entire point of replacing the hardcode.

- [ ] Run `cargo test -p ori_repr` and observe whether the existing test at `tests.rs:1574-1578` (the only current caller of `escapes()`, per Pass 1 Agent 1) still passes. The test currently asserts whatever it asserted with the hardcoded `true` — if it asserted `escapes() == true`, it should still pass for an empty `ReprPlan` (default escape_info is empty, fallback returns `true`).

---

## 03.3 Add EscapeInfo unit tests

**File(s):** `compiler/ori_repr/src/escape/tests.rs` (NEW — the placeholder file does not have a sibling tests file)

**Context:** The new `EscapeInfo` has 4 public methods (`escape_scope`, `escapes`, `is_non_escaping`, `join_escape_scope`) and a default value behavior. Each method needs a test, plus a test for the monotone-widening guarantee on `join_escape_scope` (which is the most subtle invariant).

Per `compiler.md` §Testing: *"Tests in sibling `tests.rs` files (not inline)."* Section 03.3 creates `escape/tests.rs` as a sibling and declares it from `escape/mod.rs`.

- [ ] Add the `mod tests;` declaration to `escape/mod.rs` (at the end of the file):
  ```rust
  #[cfg(test)]
  mod tests;
  ```

- [ ] Create `compiler/ori_repr/src/escape/tests.rs` with tests for each method:
  ```rust
  //! Tests for `EscapeInfo` storage and queries.
  //!
  //! Verifies the monotone-widening invariant on `join_escape_scope` and
  //! the behavioral parity of `escapes()` with the conservative default.

  use super::*;
  use ori_arc::ir::ArcVarId;

  fn var(id: u32) -> ArcVarId {
      ArcVarId::from_raw(id)
  }

  // Default behavior

  #[test]
  fn default_escape_scope_is_unknown() {
      let info = EscapeInfo::default();
      assert_eq!(info.escape_scope(var(0)), Locality::Unknown);
  }

  #[test]
  fn default_escapes_returns_true() {
      let info = EscapeInfo::default();
      assert!(info.escapes(var(0)),
          "Default (no escape info) should conservatively assume escape");
  }

  #[test]
  fn default_is_non_escaping_returns_false() {
      let info = EscapeInfo::default();
      assert!(!info.is_non_escaping(var(0)),
          "Default (no escape info) should NOT be considered non-escaping");
  }

  // Set-and-query behavior

  #[test]
  fn escape_scope_returns_set_value() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::FunctionLocal);
      assert_eq!(info.escape_scope(var(0)), Locality::FunctionLocal);
  }

  #[test]
  fn escapes_returns_false_for_function_local() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::FunctionLocal);
      assert!(!info.escapes(var(0)),
          "FunctionLocal does not escape (locality not > FunctionLocal)");
  }

  #[test]
  fn escapes_returns_false_for_block_local() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::BlockLocal);
      assert!(!info.escapes(var(0)));
  }

  #[test]
  fn escapes_returns_true_for_arg_escaping() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::ArgEscaping);
      assert!(info.escapes(var(0)),
          "ArgEscaping is > FunctionLocal, so escapes() returns true");
  }

  #[test]
  fn escapes_returns_true_for_heap_escaping() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::HeapEscaping);
      assert!(info.escapes(var(0)));
  }

  #[test]
  fn is_non_escaping_returns_true_for_block_local() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::BlockLocal);
      assert!(info.is_non_escaping(var(0)));
  }

  #[test]
  fn is_non_escaping_returns_true_for_function_local() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::FunctionLocal);
      assert!(info.is_non_escaping(var(0)));
  }

  #[test]
  fn is_non_escaping_returns_false_for_arg_escaping() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::ArgEscaping);
      assert!(!info.is_non_escaping(var(0)),
          "ArgEscaping is > FunctionLocal, so is_non_escaping() returns false");
  }

  // Monotone widening invariant — the load-bearing test

  #[test]
  fn join_escape_scope_widens_but_never_narrows() {
      let mut info = EscapeInfo::default();
      // First write: HeapEscaping (the wider value)
      info.join_escape_scope(var(0), Locality::HeapEscaping);
      assert_eq!(info.escape_scope(var(0)), Locality::HeapEscaping);
      // Second write: BlockLocal (a narrower value)
      info.join_escape_scope(var(0), Locality::BlockLocal);
      // The narrower write must NOT clobber the wider one — monotone invariant
      assert_eq!(info.escape_scope(var(0)), Locality::HeapEscaping,
          "join_escape_scope must NEVER narrow. A buggy producer trying to \
           narrow HeapEscaping → BlockLocal is silently widened back to \
           HeapEscaping (the join of the two).");
  }

  #[test]
  fn join_escape_scope_widens_block_local_to_function_local() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::BlockLocal);
      info.join_escape_scope(var(0), Locality::FunctionLocal);
      assert_eq!(info.escape_scope(var(0)), Locality::FunctionLocal);
  }

  #[test]
  fn join_escape_scope_widens_to_unknown_for_unknown_writes() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::Unknown);
      assert_eq!(info.escape_scope(var(0)), Locality::Unknown);
      // Unknown is the top of the chain — nothing widens it further.
      info.join_escape_scope(var(0), Locality::HeapEscaping);
      assert_eq!(info.escape_scope(var(0)), Locality::Unknown);
  }

  // Multiple variables

  #[test]
  fn distinct_variables_are_independent() {
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var(0), Locality::BlockLocal);
      info.join_escape_scope(var(1), Locality::HeapEscaping);
      assert_eq!(info.escape_scope(var(0)), Locality::BlockLocal);
      assert_eq!(info.escape_scope(var(1)), Locality::HeapEscaping);
      // var(2) is unset → Unknown
      assert_eq!(info.escape_scope(var(2)), Locality::Unknown);
  }
  ```

- [ ] Run `cargo test -p ori_repr -- escape::tests` and verify all tests pass. The monotone-widening test (`join_escape_scope_widens_but_never_narrows`) is the load-bearing one — if it fails, the locked design from Section 01.3 has been mis-implemented.

---

## 03.4 Add cross-crate behavioral test (round-trip through ori_arc::Locality)

**File(s):** `compiler/ori_repr/src/escape/tests.rs` (extend the file from 03.3)

**Context:** The cross-crate boundary between `ori_repr::EscapeInfo` and `ori_arc::Locality` is the new architectural seam this section creates. A behavioral test that constructs an `EscapeInfo`, populates it with each `Locality` variant, and queries it back proves the seam works end-to-end and that no variant is lost in translation.

This is a small test but it's explicitly named in the mission success criteria: *"EscapeInfo round-trips through ori_arc::Locality cleanly."*

- [ ] Add a round-trip test to `compiler/ori_repr/src/escape/tests.rs`:
  ```rust
  /// Round-trip test: every Locality variant survives storage and retrieval
  /// through EscapeInfo. Validates the cross-crate boundary between
  /// ori_repr and ori_arc.
  #[test]
  fn round_trip_all_locality_variants() {
      let mut info = EscapeInfo::default();

      info.join_escape_scope(var(0), Locality::BlockLocal);
      info.join_escape_scope(var(1), Locality::FunctionLocal);
      info.join_escape_scope(var(2), Locality::ArgEscaping);
      info.join_escape_scope(var(3), Locality::HeapEscaping);
      info.join_escape_scope(var(4), Locality::Unknown);

      assert_eq!(info.escape_scope(var(0)), Locality::BlockLocal);
      assert_eq!(info.escape_scope(var(1)), Locality::FunctionLocal);
      assert_eq!(info.escape_scope(var(2)), Locality::ArgEscaping);
      assert_eq!(info.escape_scope(var(3)), Locality::HeapEscaping);
      assert_eq!(info.escape_scope(var(4)), Locality::Unknown);

      // Each variant produces the right answer for the boolean helpers
      assert!(!info.escapes(var(0))); // BlockLocal: not escaping
      assert!(!info.escapes(var(1))); // FunctionLocal: not escaping
      assert!(info.escapes(var(2)));  // ArgEscaping: escapes
      assert!(info.escapes(var(3)));  // HeapEscaping: escapes
      assert!(info.escapes(var(4)));  // Unknown: escapes

      assert!(info.is_non_escaping(var(0)));
      assert!(info.is_non_escaping(var(1)));
      assert!(!info.is_non_escaping(var(2)));
      assert!(!info.is_non_escaping(var(3)));
      assert!(!info.is_non_escaping(var(4)));
  }
  ```

- [ ] Add a behavioral parity test for `ReprPlan::escapes()` in the appropriate file (`compiler/ori_repr/src/tests.rs`, where the existing `escapes()` test at line 1574-1578 lives):
  ```rust
  #[test]
  fn repr_plan_escapes_returns_true_for_unanalyzed_function() {
      // Behavioral parity with the previous hardcoded `let result = true;`.
      // For any (func, var) pair where escape_info does not contain an entry,
      // escapes() must continue to return true (the conservative default).
      let plan = ReprPlan::default();
      let func = ori_ir::Name::new("test_func");
      let var = ori_arc::ArcVarId::from_raw(0);
      assert!(plan.escapes(func, var),
          "Empty ReprPlan must return escapes()=true (conservative default)");
  }

  #[test]
  fn repr_plan_escapes_returns_false_for_analyzed_non_escaping_var() {
      // Behavioral departure from the hardcode: when escape_info IS populated,
      // escapes() returns the actual computed answer, not the conservative default.
      let mut plan = ReprPlan::default();
      let func = ori_ir::Name::new("test_func");
      let var = ori_arc::ArcVarId::from_raw(0);
      let mut info = EscapeInfo::default();
      info.join_escape_scope(var, ori_arc::aims::lattice::Locality::BlockLocal);
      plan.set_escape_info(func, info);
      assert!(!plan.escapes(func, var),
          "Populated ReprPlan must return escapes()=false for non-escaping vars");
  }
  ```

- [ ] Verify the existing test at `compiler/ori_repr/src/tests.rs:1574-1578` (which Pass 1 Agent 1 noted asserts `assert_eq!(std::mem::size_of::<EscapeInfo>(), 0)`) is **deleted or updated**. After Section 03.1 the size is no longer 0 (it's the size of an `FxHashMap`), so the size assertion would fail. The right move is to **delete** the size assertion — it was a placeholder check, not a real invariant.
  - [ ] Find the line `assert_eq!(std::mem::size_of::<EscapeInfo>(), 0)` (approximately `tests.rs:308` per Pass 1 Agent 1) and delete it
  - [ ] Replace it with the new behavioral parity tests above (added to the same file or to `escape/tests.rs`, whichever is more natural)

- [ ] Run `cargo test -p ori_repr` and verify all tests pass — the new EscapeInfo unit tests, the new round-trip test, the new behavioral parity tests, and all preexisting `ori_repr` tests.

- [ ] Run `cargo test -p ori_arc` and verify nothing regresses (the cross-crate import shouldn't affect `ori_arc`'s tests, but verify regardless).

---

## 03.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers. -->

- None.

---

## 03.N Completion Checklist

- [ ] `compiler/ori_repr/src/escape/mod.rs` no longer contains `pub struct EscapeInfo;` ZST. It defines a struct with `var_escape: FxHashMap<ArcVarId, Locality>` and 4 public methods
- [ ] `EscapeInfo` derives `Debug, Clone, Default, PartialEq, Eq` (no `Hash` because `FxHashMap` is not `Hash`)
- [ ] `escape_scope`, `escapes`, `is_non_escaping`, `join_escape_scope` are all public, all `#[must_use]` where applicable
- [ ] `join_escape_scope` is monotone — verified by the `join_escape_scope_widens_but_never_narrows` test
- [ ] `compiler/ori_repr/src/escape/tests.rs` exists with at least 14 unit tests covering all 4 methods plus the round-trip
- [ ] `compiler/ori_repr/src/plan/query.rs::ReprPlan::escapes()` body no longer hardcodes `let result = true;`. It consults `escape_info`.
- [ ] Behavioral parity test passes: `repr_plan_escapes_returns_true_for_unanalyzed_function`
- [ ] Behavioral departure test passes: `repr_plan_escapes_returns_false_for_analyzed_non_escaping_var`
- [ ] The deleted `assert_eq!(size_of::<EscapeInfo>(), 0)` placeholder check is gone (or replaced with a comment explaining why it was removed)
- [ ] `cargo test -p ori_repr` green
- [ ] `cargo test -p ori_arc` green (no regression from cross-crate import)
- [ ] `cargo check -p ori_llvm` green (downstream crate that may consume ReprPlan still compiles)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 03
  - [ ] `00-overview.md` mission success criteria checkboxes updated (EscapeInfo placeholder removed, escapes() body replaced)
  - [ ] `index.md` Section 03 status updated
  - [ ] Section 04's `depends_on` verified — Section 04 depends on `["03"]`
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — for this section: was there a missing helper to "find all callers of a public method across crates" (the implementer needed to grep for `.escapes(` callers to verify behavioral parity)? Was there a missing diagnostic for "show the diff between cargo doc output before/after a public API change"? Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`.

**Exit Criteria:** `wc -l compiler/ori_repr/src/escape/mod.rs` shows the file is approximately 80-100 lines (was 12, has grown to hold the concrete struct + 4 methods + extensive doc comments). `wc -l compiler/ori_repr/src/escape/tests.rs` shows the new test file exists with 14+ test functions. `cargo test -p ori_repr` passes the full suite. The behavioral parity tests prove that any consumer of `ReprPlan::escapes()` continues to see `true` for unanalyzed functions and the actual computed answer for analyzed ones — no consumer change is needed in this section because the conservative default holds.
