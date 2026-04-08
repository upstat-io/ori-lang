---
section: "02"
title: "ori_arc Implementation"
status: not-started
reviewed: false
goal: "Implement the locked decisions from Section 01 in ori_arc: add the ArgEscaping variant, verify canonicalize rules fire identically, audit 13 predicate sites across 7 files (no exhaustive matches exist; semantic per-site decisions), convert ParamContract::may_escape to a derived method (deleting field + 7 struct literal sites), update CHAIN_HEIGHT, extend the lattice test matrix, add the Lean 4 inversion comment to dimensions.rs"
success_criteria:
  - "compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality has 5 variants in order BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown — verified by a rank test"
  - "AimsState::CHAIN_HEIGHT == 16 (was 15) and the test at lattice/tests.rs:2054 passes with the new value"
  - "ParamContract has no stored may_escape field; ParamContract::may_escape() is a method returning self.locality_bound > Locality::BlockLocal"
  - "ParamContract literal sites in 7 files (extract/mod.rs (post-Section-00.4b split), builtins/mod.rs, contract/{mod,tests}.rs, intraprocedural/tests.rs, emit_rc/arg_ownership/tests.rs, verify/tests.rs) no longer set may_escape: ..."
  - "All 13 predicate sites on Locality in compiler/ori_arc/ have been audited per-site with explicit semantic decisions documented in completion notes (no exhaustive matches exist; the audit is semantic, not compile-error-driven)"
  - "all_locality() helper at lattice/tests.rs:26 returns all 5 variants (extends 4-variant helper; no pre-existing bug — Pass 1 misread)"
  - "Soundness pin test exists asserting ArgEscaping + Unique stays Unique through canonicalize() — Rule 6 does NOT fire"
  - "ParamContract::may_escape() derivation matrix exists in contract/tests.rs: 5 per-variant tests + 1 self-verifying matrix completeness test + 1 negative pin rejecting the broken `>=` derivation"
  - "Return-widening soundness pin exists in intraprocedural/tests.rs: ArgEscaping → HeapEscaping at block.rs:155 (condition 4 enforcement) + FunctionLocal → HeapEscaping positive pair + HeapEscaping not-re-widened negative pin"
  - "Lean 4 inversion comment added to dimensions.rs near the Locality enum"
  - "cargo test -p ori_arc green; ./test-all.sh green; ./clippy-all.sh green"
  - "Connects upward to mission criteria: chain ordering (lattice change), CHAIN_HEIGHT update, may_escape LEAK fix, soundness pin, may_escape() derivation matrix, return-widening producer-site pin"
inspired_by:
  - "EffectClass activation (commit 6c644dda, 2026-03-11) — canonical pattern for adding to AIMS lattice"
  - "Locality gate (commit cae24985, 2026-03-12) — canonical pattern for canonicalize rule preservation"
  - "Go cmd/compile/internal/escape/leaks.go (semantic source for ArgEscaping)"
  - "Lean 4 src/Lean/Compiler/IR/Borrow.lean:58-60 (inverted-direction prior art for the source comment)"
depends_on: ["00", "01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Add ArgEscaping variant to Locality enum"
    status: not-started
  - id: "02.2"
    title: "Add Lean 4 inversion comment to dimensions.rs"
    status: not-started
  - id: "02.3"
    title: "Verify canonicalize Rules 4, 6, 8 fire identically"
    status: not-started
  - id: "02.4"
    title: "Update CHAIN_HEIGHT and verify iteration_limit"
    status: not-started
  - id: "02.5"
    title: "Audit 13 predicate sites (no exhaustive matches exist)"
    status: not-started
  - id: "02.6"
    title: "Convert ParamContract::may_escape from field to derived method"
    status: not-started
  - id: "02.7"
    title: "Extend all_locality() and representative_states() helpers for ArgEscaping"
    status: not-started
  - id: "02.8"
    title: "Extend lattice tests for the new variant"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
# TPR Checkpoint after 02.4 (variant + chain height done, before consumer migration)
# TPR Checkpoint after 02.6 (consumer migration + may_escape conversion done, before tests)
---

# Section 02: ori_arc Implementation

**Status:** Not Started
**Goal:** Implement the locked decisions from Section 01 inside `ori_arc`. Add the `ArgEscaping` variant, verify the existing canonicalize rules continue to fire identically (Section 01.2 locked: zero changes to rule logic), audit 13 predicate sites across 7 files in `compiler/ori_arc/` (no exhaustive matches exist on `Locality` — every site is a `<`/`>`/`<=`/`>=`/`==`/`matches!()` predicate that compiles cleanly without changes; the audit is per-site SEMANTIC, not compile-error-driven), convert `ParamContract::may_escape` from a stored field to a derived method (fixing the `LEAK:scattered-knowledge` — includes deleting the field, deleting the field from 7 struct literal sites including PRODUCTION code in `extract.rs` and `builtins/mod.rs`, and converting reads to method calls), update `CHAIN_HEIGHT` from 15 to 16, extend the `all_locality()` helper at `lattice/tests.rs:26` from 4 variants to 5 (no pre-existing bug — Pass 1 misread), and extend the lattice test matrix (commutativity, associativity, exhaustive enumeration, soundness pin). Finally, add the Lean 4 inversion comment to `dimensions.rs` near the `Locality` enum so future contributors understand why Ori widens where Lean 4 narrows.

**Success Criteria:**

- [ ] `compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality` has 5 variants in order `BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown` — verified by a discriminant rank test in `lattice/tests.rs`
- [ ] `AimsState::CHAIN_HEIGHT == 16` (was 15) and the test at `lattice/tests.rs:2054` passes with the new value
- [ ] `iteration_limit()` formula recomputes correctly (still `CHAIN_HEIGHT * num_variables * num_blocks`, just with the new constant)
- [ ] `ParamContract` has no stored `may_escape` field. `ParamContract::may_escape()` is a method returning `self.locality_bound > Locality::BlockLocal`. Verified by `grep -n 'may_escape:' compiler/ori_arc/src/aims/contract/mod.rs` returning only function-definition lines (not field accesses)
- [ ] `cargo check -p ori_arc` succeeds with zero errors after the variant addition. (Per accuracy review, no exhaustive matches on `Locality` exist in `compiler/ori_arc/` — every consumer is a predicate. The compile-clean state is expected; the migration is semantic, audited per-site in 02.5.)
- [ ] `all_locality()` helper at `lattice/tests.rs:26` returns **all 5** variants (extends from the existing 4-variant exhaustive helper to include `ArgEscaping`; this is NOT a pre-existing bug — Pass 1 misread; the helper currently correctly returns all 4 current variants `[BlockLocal, FunctionLocal, HeapEscaping, Unknown]`)
- [ ] **Soundness pin test exists** asserting `AimsState { locality: ArgEscaping, uniqueness: Unique, ... }` survives `canonicalize()` with `uniqueness == Unique` (i.e., Rule 6 does NOT fire on the new variant)
- [ ] Lean 4 inversion comment added to `dimensions.rs` near the `Locality` enum (per Section 01.5 + Codex Step 6B Finding 4)
- [ ] `cargo test -p ori_arc` green
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Connects upward to mission criteria: "Locality enum has 5 variants...", "CHAIN_HEIGHT == 16", "ParamContract::may_escape is no longer a stored field", "Soundness pin"

**Context:** This is the largest implementation section in the plan. It performs the actual semantic change that the rest of the plan exists to enable. By the time Section 02 runs, Section 00 has split the lattice and transfer modules into clean sibling files, and Section 01 has produced the locked decision document. Section 02 implements against that document — the implementer should not need to make any design judgement that isn't already locked.

The work is **broader than the section title suggests**. Adding one enum variant has surprisingly large ripple effects:

1. **The variant itself**: 1 line in `dimensions.rs`. Trivial.
2. **Test extension**: ~150 lines. The lattice tests use exhaustive enumeration patterns: commutativity (4×4 → 5×5 = +9 cases), associativity (4×4×4 → 5×5×5 = +61 cases), per-rule firing tests, helper tests. Pass 1 Agent 2 enumerated the existing test sites at `tests.rs:26, 68, 1449, 2247` — each needs to be updated to include the new variant. Note: `all_locality()` at `tests.rs:26` already correctly returns all 4 current variants (Pass 1 Agent 2 misread it as 2-of-4; that 2-element list belongs to a separate `representative_states()` helper at `tests.rs:68` which is intentionally a small property-test sample, not exhaustive enumeration).
3. **Predicate audit**: 13 sites across 7 files in `compiler/ori_arc/` (re-derived during Pass 1 review via `Grep "locality\s*[<>=!]=?\s*Locality|matches!\([^)]*locality"`). **No exhaustive `match` blocks on `Locality` exist** — every consumer is an order-based predicate (`> Locality::FunctionLocal`, `<= Locality::FunctionLocal`) or a `matches!(locality, X | Y)` pattern. This means `cargo check` will NOT flag any sites after Section 02.1; the migration must be a per-site SEMANTIC audit, not compile-error-driven. See Section 02.5 for the full inventory.
4. **`may_escape` conversion**: ~40 lines. Pass 1 Agent 1 confirmed `may_escape` has zero non-test readers in production, so the conversion is pure API tightening with zero behavioral risk. The work is: delete the field, delete its writes in `CONSERVATIVE`/`OPTIMISTIC`, delete its branch in `ParamContract::join`, add a `pub fn may_escape(&self) -> bool` method.
5. **`CHAIN_HEIGHT` update**: 2 lines in the lattice. The constant doc comment lists per-dimension chain heights — the `Locality` line goes from "3" to "4" and the total goes from 15 to 16.
6. **Test helper extension** (not a bug fix): `all_locality()` at `tests.rs:26` currently correctly returns all 4 current variants `[BlockLocal, FunctionLocal, HeapEscaping, Unknown]`. Section 02.7 extends it to 5 by inserting `ArgEscaping` in chain order. The Pass 1 claim that this helper had a pre-existing bug returning "2 of 4" was a misread of a different helper (`representative_states()` at `tests.rs:68`, which IS intentionally a small sample).
7. **Source comment for Lean 4 inversion**: ~10 lines added to `dimensions.rs` near the `Locality` enum.

The cross-dimension canonicalize rules **do not change at all** (Section 01.2 locked: Rules 4, 6, 8 use specific variant matching and the new variant slots in transparently). This is a deliberate constraint — Section 02 must verify the rules continue to fire identically, but no rule logic is rewritten.

**Reference implementations:**

- **EffectClass activation pattern (commit `6c644dda`, 2026-03-11)** found by Pass 1 Agent 2: the canonical 11-step pattern for extending the AIMS lattice. Order: define enum (or extend) → update AimsState constants → implement join (already trivial — derived Ord) → update canonicalize rules (verify but no logic change) → lattice property tests (8 — idempotence, commutativity, associativity) → per-instruction transfer tests (~120) → cross-dimension interaction tests (~30) → CHAIN_HEIGHT update → iteration_limit formula → migrate consumers → end-to-end verification. **Section 02 follows this pattern exactly.**
- **Locality gate pattern (commit `cae24985`, 2026-03-12)** found by Pass 1 Agent 2: how rule ordering interactions are tested. Rule 8 must run before Rules 4/6 (verified by chain-interaction tests in `lattice/tests.rs`). Section 02 must NOT disturb this ordering — the new variant slots in without changing rule order.
- **Go `golang/src/cmd/compile/internal/escape/leaks.go`** (Section 01.5): semantic source for `ArgEscaping` meaning "flows to callee but not retained."
- **Lean 4 `lean4/src/Lean/Compiler/IR/Borrow.lean:58-60`** (Section 01.5 + Codex Step 6B Finding 4): the source for the inversion comment in `dimensions.rs`.

**Depends on:** Section 00 (file splits — the implementer modifies the post-split sibling files, not the pre-split monolith), Section 01 (decision document — the implementer reads this for the locked design, not the consensus-loop transcripts).

---

## 02.1 Add ArgEscaping variant to Locality enum

**File(s):** `compiler/ori_arc/src/aims/lattice/dimensions.rs` (the enum definition is currently at lines 166-175 — re-verified during accuracy review)

**Context:** Per Section 01.1, the variant is added between `FunctionLocal` and `HeapEscaping` to preserve the chain ordering. The discriminant ordering is implicit from declaration order (Rust enums get sequential discriminants by default), and `PartialOrd`/`Ord` are derived — so the chain semantics fall out automatically. No explicit `#[repr(u8)]` or discriminant assignment is needed.

The current enum (verified during plan creation):

```rust
// Before (compiler/ori_arc/src/aims/lattice/dimensions.rs:165-175)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Locality {
    /// Does not escape its defining basic block.
    BlockLocal,
    /// Does not escape its defining function.
    FunctionLocal,
    /// May escape to the heap.
    HeapEscaping,
    /// Unknown — conservative default.
    Unknown,
}
```

- [ ] Re-read `compiler/ori_arc/src/aims/lattice/dimensions.rs` to confirm the `Locality` enum is still at the same approximate location after any drift from Section 00 or other plans
- [ ] Add the new `ArgEscaping` variant in the correct chain position:
  ```rust
  // After
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Locality {
      /// Does not escape its defining basic block.
      BlockLocal,
      /// Does not escape its defining function.
      FunctionLocal,
      /// Escapes to a callee via parameter, but the callee does not retain
      /// a reference past the call return. The value is *transiently* aliased
      /// during the call but uniqueness is preserved at the call boundary.
      ///
      /// Matches Go's `leakCallee` semantics in
      /// `cmd/compile/internal/escape/leaks.go`. See dimensions.rs module
      /// doc for the full prior-art comparison with Lean 4's inverted
      /// `borrow=true` framing.
      ///
      /// Producer responsibility: `repr-opt §08`'s connection-graph escape
      /// analysis. Soundness conditions 2 (ownership preservation) and 4
      /// (no heap persistence) are enforced at the producer site, not here.
      ArgEscaping,
      /// May escape to the heap (returned, stored in global, written to
      /// the heap, captured by a long-lived closure).
      HeapEscaping,
      /// Unknown — conservative default.
      Unknown,
  }
  ```
- [ ] Run `cargo check -p ori_arc`. **Expected output: ZERO errors.** Per the Section 02.5 audit, no exhaustive `match` blocks on `Locality` exist anywhere in `compiler/ori_arc/` — every consumer is an order-based predicate (`>`, `<=`, `==`) or a `matches!()` pattern, all of which compile cleanly with the new variant. If `cargo check` produces any errors, the 02.5 inventory missed an exhaustive match — re-grep for `match.*locality` patterns specifically and surface the gap before continuing.
- [ ] Run `cargo test -p ori_arc` and observe expected failures: only the lattice tests that enumerate variants via `all_locality()` (extended in 02.7) and the `CHAIN_HEIGHT` assertion at `lattice/tests.rs:2054` (updated in 02.4). All other tests should pass — if any other test fails, the variant addition has a hidden semantic effect that 02.5 must investigate.
- [ ] Verify the chain ordering with a simple ad-hoc test (this becomes part of 02.8's permanent test extension):
  ```rust
  assert!(Locality::BlockLocal < Locality::FunctionLocal);
  assert!(Locality::FunctionLocal < Locality::ArgEscaping);
  assert!(Locality::ArgEscaping < Locality::HeapEscaping);
  assert!(Locality::HeapEscaping < Locality::Unknown);
  ```

---

## 02.2 Add Lean 4 inversion comment to dimensions.rs

**File(s):** `compiler/ori_arc/src/aims/lattice/dimensions.rs`

**Context:** Per Codex Step 6B Finding 4 and Section 01.5, the explanation of why Ori's lattice direction (`BlockLocal` widens to `Unknown`) looks inverted from Lean 4's `borrow=true → false` narrowing belongs in the **source comment**, not in a plan section. Future contributors read the source, not the plan corpus.

The comment goes near the `Locality` enum doc comment in `dimensions.rs`. The original module doc comment for `Locality` reads:

```rust
// Locality dimension (auxiliary)

/// Escape analysis. `OxCaml` locality mode (ICFP 2024). Conservative in v1.
///
/// Ordered: `BlockLocal` < `FunctionLocal` < `HeapEscaping` < `Unknown`.
/// Chain height: 3.
/// ...
```

After Section 02.1 adds `ArgEscaping`, the chain becomes 4 and the doc comment must update.

- [ ] Update the `Locality` doc comment to:
  ```rust
  // Locality dimension (auxiliary)

  /// Escape analysis. `OxCaml` locality mode (ICFP 2024). Conservative in v1.
  ///
  /// Ordered: `BlockLocal` < `FunctionLocal` < `ArgEscaping` < `HeapEscaping` < `Unknown`.
  /// Chain height: 4 (was 3 before the ArgEscaping variant was added).
  ///
  /// ## Prior art and inversion direction
  ///
  /// This lattice expresses the same semantic distinction as Lean 4's
  /// `borrow: Bool` parameter inference (`Borrow.lean:58-60`), but with the
  /// **opposite widening direction**: Lean 4 starts every parameter at
  /// `borrow = true` (caller retains ownership, the optimistic case) and
  /// narrows to `borrow = false` only when ownership transfer is proven.
  /// AIMS starts each variable at `BlockLocal` (most local, the optimistic
  /// case) and widens toward `Unknown` (most escaping) as the analysis
  /// discovers escape paths.
  ///
  /// The two are mathematically equivalent: a value with Lean 4
  /// `borrow = true` corresponds to AIMS locality ≤ ArgEscaping (caller
  /// retains uniqueness), and Lean 4 `borrow = false` corresponds to AIMS
  /// HeapEscaping (callee retains, caller loses uniqueness).
  ///
  /// The intermediate `ArgEscaping` variant matches Go's `leakCallee`
  /// semantics in `cmd/compile/internal/escape/leaks.go` (the byte-2 entry
  /// in the `[8]uint8` leaks array, distinct from byte-0 `leakHeap`).
  ///
  /// ## Sequencing algebra
  ///
  /// Both `seq_add` (sequential composition) and `alt_join` (branch join)
  /// coincide with [`join`](Self::join) (= max) for `Locality`. This is
  /// intentional, not accidental: locality tracks where a value *escapes to*,
  /// which widens monotonically. ...
  ```
  (The "Sequencing algebra" paragraph is preserved from the original.)
- [ ] Verify the doc comment compiles (`cargo doc -p ori_arc 2>&1 | grep warning` reports no new warnings)
- [ ] Verify the chain height claim (4) matches what `dimensions.rs` actually defines

---

## 02.3 Verify canonicalize Rules 4, 6, 8 fire identically

**File(s):** `compiler/ori_arc/src/aims/lattice/canonicalize.rs` (post-Section-00) — the file containing `AimsState::canonicalize_single_pass`

**Context:** Per Section 01.2, the three canonicalize rules use **specific variant matching** (not `>=` ranges):

- Rule 8: `if self.access == AccessClass::Borrowed && self.locality > Locality::FunctionLocal` — uses `>`, slots transparently with the new variant (will widen `ArgEscaping → FunctionLocal` for borrows, which is correct)
- Rule 6: `if self.locality == Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique` — uses `==`, does NOT fire on `ArgEscaping`
- Rule 4: `if self.locality == Locality::BlockLocal && ...` — uses `==`, does NOT fire on `ArgEscaping`

**Section 02 must NOT modify any rule logic.** This subsection's only work is to **verify** the rules continue to fire identically by reading the post-Section-00 file and confirming the logic matches what 01.2 locked.

- [ ] Re-read `compiler/ori_arc/src/aims/lattice/canonicalize.rs` (the file Section 00.2 created from `lattice/mod.rs:299-377`)
- [ ] Verify Rule 8's condition is still `self.access == AccessClass::Borrowed && self.locality > Locality::FunctionLocal`
- [ ] Verify Rule 6's condition is still `self.locality == Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique`
- [ ] Verify Rule 4's condition is still `self.locality == Locality::BlockLocal && self.access == AccessClass::Owned && self.cardinality <= Cardinality::Once && self.uniqueness == Uniqueness::MaybeShared`
- [ ] Document in a comment near each rule (or in the module doc) that the rule has been verified compatible with the new `ArgEscaping` variant. Example near Rule 6:
  ```rust
  // Rule 6: HeapEscaping → uniqueness ≥ MaybeShared.
  // ...
  // Note: this rule deliberately does NOT fire on ArgEscaping. ArgEscaping
  // values are passed to a callee but not retained past the call return,
  // so uniqueness is preserved at the call boundary. Soundness gate for
  // "callee may briefly share" lives in the `may_share` contract effects
  // path, not in the locality lattice. See plans/locality-representation-
  // unification/section-01.md §01.2 for the full rationale and Go/Lean 4
  // prior art.
  if self.locality == Locality::HeapEscaping && self.uniqueness == Uniqueness::Unique {
      self.uniqueness = Uniqueness::MaybeShared;
      cross_fires += 1;
  }
  ```
  Note: the comment references the plan section file. Per impl-hygiene.md, plan annotations in code are allowed during active plan execution and removed at completion. The annotation will be cleaned up in Section 05.3 (plan annotation cleanup scan), which has this specific comment listed in its known-cleanup-targets section.
- [ ] Run `cargo test -p ori_arc` after the variant has been added (Section 02.1) and verify no canonicalize tests fail. If any fail, the assumption that the rules slot transparently is wrong — STOP and re-derive Section 01.2's decision.

---

## 02.4 Update CHAIN_HEIGHT and verify iteration_limit

**File(s):** `compiler/ori_arc/src/aims/lattice/state.rs` (post-Section-00) — the file containing `AimsState::CHAIN_HEIGHT` and `iteration_limit`

**Context:** The `CHAIN_HEIGHT` constant documents per-dimension chain heights and their sum. Adding `ArgEscaping` increases `Locality`'s chain height from 3 to 4 (5 variants in a chain has 4 transitions: `Block→Function→Arg→Heap→Unknown`), which increases the total from 15 to 16.

The constant lives in `AimsState`'s impl block (currently at `lattice/mod.rs:466` pre-split, in `state.rs` post-split):

```rust
/// Maximum chain height of the product lattice.
///
/// The sum of per-dimension chain heights:
/// - `AccessClass`: 1 (`Borrowed` → `Owned`)
/// - `Consumption`: 3 (`Dead` → `Linear` → `Affine` → `Unrestricted`)
/// - `Cardinality`: 2 (`Absent` → `Once` → `Many`)
/// - `Uniqueness`: 2 (`Unique` → `MaybeShared` → `Shared`)
/// - `Locality`: 3 (`BlockLocal` → `FunctionLocal` → `HeapEscaping` → `Unknown`)
/// - `ShapeClass`: 1 (flat lattice — any value → `NonReusable`)
/// - `EffectClass`: 3 (three independent booleans)
///
/// Total: 15. Fixed-point iteration converges in at most
/// `CHAIN_HEIGHT × num_variables × num_blocks` steps.
pub const CHAIN_HEIGHT: usize = 15;
```

- [ ] Update the `CHAIN_HEIGHT` constant from `15` to `16`:
  ```rust
  pub const CHAIN_HEIGHT: usize = 16;
  ```
- [ ] Update the doc comment to reflect the new `Locality` chain height and the new total:
  ```rust
  /// Maximum chain height of the product lattice.
  ///
  /// The sum of per-dimension chain heights:
  /// - `AccessClass`: 1 (`Borrowed` → `Owned`)
  /// - `Consumption`: 3 (`Dead` → `Linear` → `Affine` → `Unrestricted`)
  /// - `Cardinality`: 2 (`Absent` → `Once` → `Many`)
  /// - `Uniqueness`: 2 (`Unique` → `MaybeShared` → `Shared`)
  /// - `Locality`: 4 (`BlockLocal` → `FunctionLocal` → `ArgEscaping` → `HeapEscaping` → `Unknown`)
  /// - `ShapeClass`: 1 (flat lattice — any value → `NonReusable`)
  /// - `EffectClass`: 3 (three independent booleans)
  ///
  /// Total: 16. Fixed-point iteration converges in at most
  /// `CHAIN_HEIGHT × num_variables × num_blocks` steps.
  pub const CHAIN_HEIGHT: usize = 16;
  ```
- [ ] Verify `iteration_limit` does not need a code change (the formula `CHAIN_HEIGHT * num_variables * num_blocks` is unchanged; only the constant value flows through)
- [ ] Run `cargo test -p ori_arc` and observe the test failure at `lattice/tests.rs:2054` (`assert_eq!(AimsState::CHAIN_HEIGHT, 15)`). This is expected — Section 02.8 will update this assertion to `16`.
- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.4 (variant added, comment added, rules verified, chain height updated). This catches any accidental rule modification or chain-height miscount before consumer migration begins.

---

## 02.5 Audit 13 predicate sites — semantic per-site decisions

**Scope:** This subsection is limited to consumers in `compiler/ori_arc/`. Other crates (`ori_repr`, `ori_llvm`, etc.) are verified compiler-wide via `cargo check` in the gates below — any exhaustive matches on `Locality` outside `ori_arc/` will surface as compile errors and must be fixed before Section 02 is marked complete. Pass 1's compiler-wide scan found zero direct `Locality` consumers in `ori_repr/` (only the `EscapeInfo` placeholder, which Section 03 replaces).

**Critical semantics note:** Per Pass 1 review, **every existing `Locality` consumer in `compiler/ori_arc/` is an order-based predicate or a `matches!(...)` predicate** — there are NO exhaustive `match expr.locality { ... }` blocks anywhere in the AIMS subtree. This means:

1. The compiler will NOT flag any of these sites with an exhaustive-match error after Section 02.1 adds the new variant.
2. Every site compiles cleanly with the old behavior — but the old behavior may be semantically wrong for `ArgEscaping`.
3. The implementer MUST visit every site explicitly and make a per-site SEMANTIC judgment about whether the predicate should fire on `ArgEscaping`. Do NOT rely on the compiler to drive the migration.

**Per-site predicate inventory (re-derived via `Grep "locality\s*[<>=!]=?\s*Locality|matches!\(.*locality"` of `compiler/ori_arc/src/aims/`):**

| # | File:line | Current code (≈) | Decision for ArgEscaping | Rationale |
|---|---|---|---|---|
| 1 | `lattice/mod.rs:331` (post-split: `canonicalize.rs`) | `if access == Borrowed && locality > Locality::FunctionLocal { locality = FunctionLocal; }` | **Fires.** Widens `ArgEscaping → FunctionLocal` for borrows | Rule 8: borrows cannot escape function. ArgEscaping > FunctionLocal, so `>` is true. Correct. |
| 2 | `lattice/mod.rs:345` (post-split: `canonicalize.rs`) | `if locality == HeapEscaping && uniqueness == Unique { uniqueness = MaybeShared; }` | **Does NOT fire on ArgEscaping** (== HeapEscaping is exact). | Rule 6: SOUNDNESS PIN — preserves uniqueness across call boundary. |
| 3 | `lattice/mod.rs:361` (post-split: `canonicalize.rs`) | `if locality == BlockLocal && access == Owned && cardinality <= Once && uniqueness == MaybeShared { ... }` | **Does NOT fire on ArgEscaping** (== BlockLocal is exact). | Rule 4: requires precise locality. ArgEscaping has crossed a block boundary. Correct. |
| 4 | `lattice/mod.rs:432-436` (post-split: `state.rs`) — `is_local()` | `matches!(locality, BlockLocal \| FunctionLocal)` | **Does NOT fire on ArgEscaping.** Leave the pattern unchanged. | The function tests "stays inside the function." ArgEscaping crosses a call boundary, so it is NOT local. The negative answer is correct. |
| 5 | `intraprocedural/effects.rs:38` | `if demand.locality > Locality::BlockLocal { effects.may_share = true; }` | **Fires.** Sets `may_share` for any locality > BlockLocal, including ArgEscaping. | A construct whose destination demand is ArgEscaping may temporarily alias during the callee's execution — `may_share = true` is the correct conservative default. |
| 6 | `intraprocedural/state_map.rs:429` (pre-split) → `intraprocedural/state_map/cross_dim.rs` (post-Section-00.4c split) — Rule 4 evidence counter | `state.locality == Locality::BlockLocal && ...` | **Does NOT fire on ArgEscaping** (exact match). | Counts evidence of Rule 4 having fired; Rule 4 only fires on BlockLocal, so the counter correctly excludes ArgEscaping. |
| 7 | `intraprocedural/state_map.rs:439-440` (pre-split) → `intraprocedural/state_map/cross_dim.rs` (post-Section-00.4c split) — Rule 8 evidence counter | `state.locality <= Locality::FunctionLocal && state.locality != Locality::BlockLocal` | **Does NOT fire on ArgEscaping.** ArgEscaping > FunctionLocal, so `<= FunctionLocal` is false. | Counts evidence of Rule 8 having capped a borrow at FunctionLocal — ArgEscaping is above the cap (Rule 8 brings it down to FunctionLocal). Correct. |
| 8 | `intraprocedural/post_convergence.rs:95-96` — `matches!(exit_state.locality, FunctionLocal \| BlockLocal)` | **Decision required.** Currently does NOT include ArgEscaping. | Tests "local-allocation eligibility." ArgEscaping crosses a call boundary so it is NOT a local-alloc candidate. The current pattern is correct as-is — the negative answer is the right one. **Decision: leave the pattern unchanged.** |
| 9 | `intraprocedural/block.rs:97` (cross-block widening producer) | `if state.locality < Locality::FunctionLocal { state.locality = FunctionLocal; }` | **Producer site — leave alone.** | Producer that widens BlockLocal → FunctionLocal at block boundaries. Section 02 does NOT modify producers; ArgEscaping is produced by future `repr-opt §08`. The condition `< FunctionLocal` correctly excludes ArgEscaping (ArgEscaping is not < FunctionLocal). |
| 10 | `intraprocedural/block.rs:155` (return widening producer) | `if entry.locality < Locality::HeapEscaping { entry.locality = HeapEscaping; }` | **Producer site — leave alone.** | Widens to HeapEscaping for return values. ArgEscaping IS < HeapEscaping, so this would widen ArgEscaping → HeapEscaping for returns. **Verify this is the intended semantics**: a value returned from a function should be HeapEscaping (it has escaped its function), so widening from ArgEscaping is correct. Document this decision in the section's completion notes. |
| 11 | `transfer/mod.rs:488-489` (post-split: `state_helpers.rs`) | `if state.locality < capture_locality { state.locality = capture_locality; }` | **Closure capture state update — leave alone.** | This is a generic widening operation — works correctly with the new variant via `<` comparison. No semantic decision needed. |
| 12 | `realize/tests.rs:1091` | `v0_state.locality <= Locality::FunctionLocal` (test assertion) | **Test assertion — re-evaluate.** | Asserts that `v0` stays at or below FunctionLocal in a specific test program. After ArgEscaping is added, this assertion still holds for the test (the test is in v1 producer set, so no ArgEscaping is generated). No change needed. Verify by re-running the test. |
| 13 | `intraprocedural/tests.rs:1405, 1585, 1786` | `entry_p0.locality >= Locality::FunctionLocal`, `<= Locality::FunctionLocal`, `<= Locality::HeapEscaping` | **Test assertions — re-evaluate per assertion.** | Each is a test pin on a specific locality value. Re-run the tests after Section 02.1 to verify the assertions still hold. They should — the test programs do not generate ArgEscaping in v1. |

**Total**: 13 predicate sites across 7 files (post-split paths). All sites compile cleanly with the new variant; the migration is entirely a SEMANTIC verification, not a mechanical compile-error fix.

**`contract/mod.rs::ParamContract::join` at line 248-251**: This is `may_escape: self.may_escape || other.may_escape, locality_bound: self.locality_bound.join(other.locality_bound)`. The locality_bound branch works automatically via `Locality::join = max`. The may_escape branch is removed entirely in 02.6. No work needed in 02.5.

- [ ] Re-derive the predicate inventory by running `Grep "locality\s*[<>=!]=?\s*Locality|matches!\([^)]*locality" compiler/ori_arc/src/aims/` after Section 00 has split the files. The line numbers may have shifted; the file paths almost certainly have. Treat the table above as a starting point, not authoritative.

- [ ] For each site in the inventory: (a) re-read 5 lines of context around the predicate, (b) decide whether ArgEscaping should fire the branch, (c) write a one-line per-site decision into the section's completion notes (not a comment in the code — the decision is a permanent record of the migration audit, but inline annotations are scaffolding that gets cleaned up in 05.3).

- [ ] Run `cargo check -p ori_arc` after Section 02.1 has added the variant. Expected output: ZERO errors (no exhaustive matches exist on `Locality`). If `cargo check` produces any errors, the inventory above missed an exhaustive match — re-grep for `match.*locality` patterns specifically.

- [ ] Run `cargo check -p ori_repr` and `cargo check -p ori_llvm` to catch any consumer outside `ori_arc/` that this subsection's scope did not cover. Pass 1's compiler-wide scan found zero direct `Locality` consumers outside `ori_arc/`, but verify regardless. Any compile error from these checks must be fixed before marking 02.5 complete.

- [ ] Run `cargo test -p ori_arc` — most tests should still pass; some lattice tests will fail because they enumerate variants (via `all_locality()`) and need 02.7's update to the helper.

**Migration recipe:**

| Pattern in current code | What to do |
|---|---|
| `match locality { BlockLocal => ..., FunctionLocal => ..., HeapEscaping => ..., Unknown => ... }` | Add an `ArgEscaping => ...` arm |
| `matches!(locality, BlockLocal \| FunctionLocal)` | Decide: should `ArgEscaping` be included? Usually NOT — the helper is testing "is local," and ArgEscaping is not local. Leave the matches! pattern alone. |
| `matches!(locality, HeapEscaping \| Unknown)` | Decide: should `ArgEscaping` be included? Usually YES — the helper is testing "may escape." Add `ArgEscaping` to the pattern. |
| `if locality > Locality::FunctionLocal { ... }` | No change. The new variant slots into the chain and the `>` comparison works automatically. |
| `if locality == Locality::HeapEscaping { ... }` | Decide case-by-case: does the consumer want this branch to fire on `ArgEscaping` too? If yes, change to `if locality >= Locality::ArgEscaping`. If no (the branch is specifically about heap escape), leave it alone. **Default: leave it alone unless the consumer explicitly should treat ArgEscape the same as heap escape.** |

- [ ] Re-confirm that `cargo check -p ori_arc` produces zero `Locality`-related errors after Section 02.1 (this re-runs the check from 02.1 as a 02.5 entry gate). If any new exhaustive-match error appears, the inventory above missed a site — re-grep `match.*locality` and add the site to the inventory before continuing.

- [ ] **`compiler/ori_arc/src/aims/lattice/state.rs::is_local()`**: The current implementation is:
  ```rust
  pub fn is_local(&self) -> bool {
      matches!(
          self.locality,
          Locality::BlockLocal | Locality::FunctionLocal
      )
  }
  ```
  Decision per the migration recipe: `ArgEscaping` is NOT local (it crosses a call boundary). **Leave the pattern unchanged.** The function correctly returns `false` for `ArgEscaping` because the value is no longer in the matched set.
  - [ ] Verify `is_local()` returns `false` when `locality == ArgEscaping` (will be tested in 02.8)

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs:95-96`** (predicate site #8 in the inventory above): This is a `matches!(exit_state.locality, FunctionLocal | BlockLocal)` **predicate**, NOT an exhaustive match. It compiles cleanly with the new variant. The semantic question is whether `ArgEscaping` should be considered local-allocation-eligible. **Decision per the inventory: leave the pattern unchanged.** ArgEscaping crosses a call boundary, so it is correctly excluded from the "local-alloc eligible" set — the negative answer (`false` for ArgEscaping) is correct.
  - [ ] Re-read `compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs` lines 90-110 to confirm the `matches!()` is testing local-allocation eligibility (not some other property that should include ArgEscaping)
  - [ ] No code change. Document in the section's completion notes that this site was audited and intentionally left unchanged.
  - [ ] Add a regression test in 02.8 that asserts `local_alloc_eligible(ArgEscaping) == false` so the decision is permanently pinned (otherwise a future contributor adding ArgEscaping to the pattern would silently change the semantics with no test failure).

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/effects.rs:38`**: The current code is `if demand.locality > Locality::BlockLocal { effects.may_share = true; }`. This works automatically because `ArgEscaping > BlockLocal` is `true`. Verify this is correct semantically: a `Construct` whose destination demand is `ArgEscaping` should set `may_share = true` because the callee might temporarily alias during the call. Per Pass 2 deep read of this file, the semantics match — the variable is "demanded with locality > BlockLocal" which means "not strictly block-local," and ArgEscaping qualifies.
  - [ ] No code change. Document in the section's completion notes that this site was verified compatible.

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/state_map/cross_dim.rs`** (post-Section-00.4c split — was `state_map.rs:429-440` pre-split): Order-based predicates. Per Pass 1 Agent 1 the predicates are:
  - `state.locality == Locality::BlockLocal` (Rule 4 evidence — specific variant; does NOT need to fire on ArgEscaping; leave unchanged)
  - `state.locality <= Locality::FunctionLocal && state.locality != Locality::BlockLocal` (Rule 8 evidence — "function-local but not block-local"; ArgEscaping is NOT in this set since ArgEscaping > FunctionLocal; leave unchanged)
  - [ ] Re-read `state_map/cross_dim.rs` end-to-end (it should be ~60 lines after the split) to confirm the predicates' intent
  - [ ] No code changes if the predicates are correct as-is

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/block.rs:97-100, 147-159`**: PRODUCERS that today produce `BlockLocal`, `FunctionLocal`, `HeapEscaping`. **DO NOT modify them in this plan.** The new variant is produced by future `repr-opt §08`'s connection-graph escape analysis, which will add a new branch in this file. This plan only adds the variant; producing it is §08's job.
  - [ ] No code changes
  - [ ] Document in the section's completion notes that these producers are intentionally untouched (per Section 01.4 enforcement layer table, condition 2 belongs to §08)

- [ ] **`compiler/ori_arc/src/aims/contract/mod.rs::ParamContract::join` at line 248-251**: Uses `self.locality_bound.join(other.locality_bound)` (max). Works automatically with the new variant.
  - [ ] No code changes in this subsection. (The `may_escape` field removal is in 02.6.)

- [ ] **`compiler/ori_arc/src/aims/transfer/forward.rs`** (post-Section-00): `transfer_project`, `transfer_collection_reuse`, `transfer_apply_conservative`, etc. produce specific variants (`BlockLocal`, `Unknown`, etc.) but do NOT pattern-match on locality — they only assign new values. No changes needed.
  - [ ] No code changes
  - [ ] Verify by `grep -n 'match.*locality' compiler/ori_arc/src/aims/transfer/forward.rs` returning empty

- [ ] After all 13 predicate sites are audited (decisions documented in completion notes), run `cargo check -p ori_arc` and verify zero errors. **Note**: `contract/mod.rs` still has the `may_escape` field as a stored struct member at this point — Section 02.6 removes it. The `cargo check` here verifies the variant addition does not break anything; the `may_escape` LEAK fix is a separate concern handled in the next subsection.
- [ ] Run `cargo check -p ori_repr` and `cargo check -p ori_llvm` to catch any consumer outside `ori_arc/` that this subsection's scope did not cover. Any compile error from these checks must be fixed before marking 02.5 complete.
- [ ] Run `cargo test -p ori_arc` — most tests should still pass; some lattice tests will fail because they enumerate variants and need 02.7/02.8's updates

---

## 02.6 Convert ParamContract::may_escape from field to derived method

**File(s):** `compiler/ori_arc/src/aims/contract/mod.rs` (currently 478 lines; this section removes a few lines, no risk of crossing the 500 limit)

**Context:** Pass 1 Agent 1 + Pass 2 confirmed `may_escape` is a `LEAK:scattered-knowledge`: it's co-maintained with `locality_bound` in CONSERVATIVE/OPTIMISTIC constants and `ParamContract::join`, but **read by zero non-test sites in production**. Pass 2 deep read confirmed:

- `contract/mod.rs:217-225` (CONSERVATIVE constant): `may_escape: true, locality_bound: Locality::Unknown` — both are independent assignments
- `contract/mod.rs:231-239` (OPTIMISTIC constant): `may_escape: false, locality_bound: Locality::BlockLocal` — independent
- `contract/mod.rs:243-253` (`ParamContract::join`): does componentwise OR for `may_escape` and `join` for `locality_bound`

The fix per `impl-hygiene.md` §Side Logic Remediation: *"The fix for side logic is always the same: move the logic to its canonical home and have the consumption site query/call it. Never 'fix' a LEAK by adding a comment explaining why the duplication exists."*

Canonical home: `locality_bound`. The derived value of `may_escape` is `locality_bound > Locality::BlockLocal` (a value escapes its defining block if its locality is anything other than `BlockLocal`).

- [ ] Re-read `compiler/ori_arc/src/aims/contract/mod.rs` lines 187-260 to confirm the current `ParamContract` struct, CONSERVATIVE constant, OPTIMISTIC constant, and `join` method. Note any drift since plan creation.

- [ ] Delete the `may_escape` field from the `ParamContract` struct definition:
  ```rust
  // Before
  pub struct ParamContract {
      pub access: AccessClass,
      pub consumption: Consumption,
      pub cardinality: Cardinality,
      pub may_escape: bool,           // ← DELETE
      pub may_share: bool,
      pub locality_bound: Locality,
      pub uniqueness: Uniqueness,
  }

  // After
  pub struct ParamContract {
      pub access: AccessClass,
      pub consumption: Consumption,
      pub cardinality: Cardinality,
      pub may_share: bool,
      pub locality_bound: Locality,
      pub uniqueness: Uniqueness,
  }
  ```

- [ ] Add a derived method on the `impl ParamContract` block:
  ```rust
  impl ParamContract {
      // ... existing constants and methods ...

      /// Whether this parameter may escape its defining block.
      ///
      /// **Derived from `locality_bound`.** A parameter escapes its block if
      /// its locality is anything other than `BlockLocal` — i.e., it has been
      /// observed flowing across a block boundary, into a callee, to the heap,
      /// or to an unknown destination.
      ///
      /// This was previously a stored field that was co-maintained with
      /// `locality_bound`. The two were a `LEAK:scattered-knowledge` —
      /// `may_escape` was set in CONSERVATIVE/OPTIMISTIC constants and joined
      /// independently of `locality_bound`, with no enforced consistency.
      /// Per impl-hygiene.md §SSOT, removing the field and deriving from the
      /// canonical home (`locality_bound`) eliminates the parallel state.
      #[must_use]
      pub fn may_escape(&self) -> bool {
          self.locality_bound > Locality::BlockLocal
      }
  }
  ```

- [ ] Update the CONSERVATIVE constant — remove the `may_escape: true,` line:
  ```rust
  pub const CONSERVATIVE: Self = Self {
      access: AccessClass::Owned,
      consumption: Consumption::Unrestricted,
      cardinality: Cardinality::Many,
      may_share: true,
      locality_bound: Locality::Unknown,  // (Locality::Unknown > BlockLocal, so may_escape() returns true — semantically equivalent)
      uniqueness: Uniqueness::MaybeShared,
  };
  ```

- [ ] Update the OPTIMISTIC constant — remove the `may_escape: false,` line:
  ```rust
  pub const OPTIMISTIC: Self = Self {
      access: AccessClass::Borrowed,
      consumption: Consumption::Dead,
      cardinality: Cardinality::Absent,
      may_share: false,
      locality_bound: Locality::BlockLocal,  // (Locality::BlockLocal is NOT > BlockLocal, so may_escape() returns false — semantically equivalent)
      uniqueness: Uniqueness::Unique,
  };
  ```

- [ ] Update `ParamContract::join` — remove the `may_escape` line:
  ```rust
  pub fn join(&self, other: &Self) -> Self {
      Self {
          access: self.access.join(other.access),
          consumption: self.consumption.join(other.consumption),
          cardinality: self.cardinality.join(other.cardinality),
          may_share: self.may_share || other.may_share,
          locality_bound: self.locality_bound.join(other.locality_bound),
          uniqueness: self.uniqueness.join(other.uniqueness),
      }
  }
  ```
  Note: `may_escape` is automatically preserved through the join because `locality_bound.join` widens monotonically — if either side had `locality_bound > BlockLocal`, the join also has it.

- [ ] **Enumerate every `ParamContract { ... }` literal site across the workspace** before deleting the field. Run:
  ```bash
  Grep "ParamContract\s*\{" compiler/
  ```

  Expected (re-verified during accuracy review; paths reflect post-Section-00 layout):
  - `compiler/ori_arc/src/aims/contract/mod.rs` (the struct definition + CONSERVATIVE + OPTIMISTIC + join)
  - `compiler/ori_arc/src/aims/contract/tests.rs` (literals at lines 95, 104, 279)
  - `compiler/ori_arc/src/aims/builtins/mod.rs` (PRODUCTION literals at lines 286, 297)
  - `compiler/ori_arc/src/aims/interprocedural/extract/mod.rs` (PRODUCTION literal — was `extract.rs:79-92` pre-split; **Section 00.4b moved this into `extract/mod.rs`** because the original `extract.rs` was 517 lines and exceeded the 500-line BLOAT limit. Re-derive the line number after 00.4b completes — the literal stays in `extract_contract`, which remains in `mod.rs`.)
  - `compiler/ori_arc/src/aims/intraprocedural/tests.rs` (literals at lines 1488, 1565, 1765, 1844, 1918, 1992, 2221, 2744, 2827, 2854)
  - `compiler/ori_arc/src/aims/emit_rc/arg_ownership/tests.rs` (literal at line 18)
  - `compiler/ori_arc/src/verify/tests.rs` (literals at lines 486, 498)

- [ ] **For every struct literal site, REMOVE the `may_escape: ...` line.** This is a separate operation from the field-deletion in `contract/mod.rs` — without removing the literals, every site will fail to compile because the field no longer exists.
  ```rust
  // Before (production: pre-split extract.rs:79-92, post-Section-00.4b: extract/mod.rs in extract_contract)
  ParamContract {
      access,
      consumption: state.consumption,
      cardinality: state.cardinality,
      locality_bound: state.locality,
      may_escape: false,           // ← DELETE this line
      may_share: false,
      uniqueness: Uniqueness::MaybeShared,
  }
  // After
  ParamContract {
      access,
      consumption: state.consumption,
      cardinality: state.cardinality,
      locality_bound: state.locality,
      may_share: false,
      uniqueness: Uniqueness::MaybeShared,
  }
  ```
  Apply the same removal to every literal in the 7 files above. **Verify that each removal preserves semantics**: the removed `may_escape` value must be consistent with the literal's `locality_bound`. For example, if a literal had `may_escape: true, locality_bound: BlockLocal`, removing the line silently changes the derived `may_escape()` from `true` to `false`. If you find any inconsistency, the original code was buggy (the LEAK manifest) — record the discrepancy in the section's completion notes for traceability, then accept the new derived value as authoritative.

- [ ] Run `cargo check -p ori_arc` after the field-and-literal removals. Expected output: any remaining compile errors are field READS (e.g., `param.may_escape`) — these are the next sub-task.

- [ ] **Convert every `param.may_escape` field read to `param.may_escape()` method call.** Use:
  ```bash
  Grep "\.may_escape\b" compiler/ | grep -v "may_escape("
  ```
  to find call sites that still use field syntax. Per Pass 1 + this accuracy review, all such reads are in test files (`contract/tests.rs`, `intraprocedural/tests.rs`, `interprocedural/tests.rs` if any). If `cargo check` flags any production sites, that contradicts Pass 1's "zero non-test readers" claim — investigate, then convert.
  ```rust
  // Before
  if param.may_escape { ... }
  // After
  if param.may_escape() { ... }
  ```

- [ ] Run `cargo test -p ori_arc` and verify all tests still pass. The semantic preservation is provable by inspection: every previous `may_escape: true` was paired with `locality_bound: Locality::Unknown` (or another non-BlockLocal value), and every previous `may_escape: false` was paired with `locality_bound: Locality::BlockLocal`. The derived method returns the same value the field would have. **Any test failure here indicates a real LEAK manifest** — document it in completion notes.

- [ ] **Final invariant check.** Run:
  ```bash
  Grep "may_escape:" compiler/
  ```
  Expected: ZERO matches across the entire workspace. The only references should be `pub fn may_escape(&self) -> bool` (function definition) and `param.may_escape()` (method call sites). If the grep returns any `may_escape: <value>` lines, a struct literal was missed and must be fixed.

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.6 (variant added, rules verified, chain height updated, consumers migrated, may_escape converted). This catches any may_escape consumer that was missed and any wrong-arm migration in the predicate sites.

---

## 02.7 Extend all_locality() helper and representative_states() helper for ArgEscaping

**File(s):** `compiler/ori_arc/src/aims/lattice/tests.rs`

**Context:** The helper `fn all_locality()` at `tests.rs:26-29` currently returns `vec![BlockLocal, FunctionLocal, HeapEscaping, Unknown]` — all 4 current variants. It is used by the lattice property tests (commutativity, associativity, identity) for exhaustive enumeration. After Section 02.1 adds `ArgEscaping`, this helper must be extended to return all 5 variants.

A separate helper `representative_states()` at `tests.rs:62-` uses a smaller subset (`let localities = [Locality::FunctionLocal, Locality::Unknown];` at line 68) for property tests over the full 7-dimension `AimsState` product space. The smaller subset is intentional — exhaustively enumerating AimsState would explode combinatorially. **02.7 must decide whether `representative_states` needs `ArgEscaping` added** based on whether any property test specifically targets cross-dimension behavior involving the new variant. The default decision is YES — adding one variant to a sample set of 2 is cheap (3 instead of 2 localities, modest blowup), and the new variant has unique semantic interactions (Rule 8 widening, soundness pin for Rule 6) that benefit from broader property-test exposure.

- [ ] Re-read `compiler/ori_arc/src/aims/lattice/tests.rs:20-80` to find the exact current definitions of `all_locality()` and `representative_states()`. The line numbers may have shifted slightly after Section 00's split or other plan work.

- [ ] Extend `all_locality()` to include `ArgEscaping` in chain order:
  ```rust
  // Before (4 current variants — correct, no pre-existing bug)
  fn all_locality() -> Vec<Locality> {
      use Locality::*;
      vec![BlockLocal, FunctionLocal, HeapEscaping, Unknown]
  }

  // After (5 variants — adds ArgEscaping in chain position)
  fn all_locality() -> Vec<Locality> {
      use Locality::*;
      vec![BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown]
  }
  ```

- [ ] Extend `representative_states()`'s `localities` sample to include `ArgEscaping`. The current sample is `[Locality::FunctionLocal, Locality::Unknown]` (chosen as the cheapest non-trivial pair). Add `ArgEscaping` as a third sample because it exercises Rule 8 widening and the Rule 6 soundness pin:
  ```rust
  // Before
  let localities = [Locality::FunctionLocal, Locality::Unknown];
  // After
  let localities = [Locality::FunctionLocal, Locality::ArgEscaping, Locality::Unknown];
  ```
  This grows the property-test product space by 1.5x along the locality axis. If the test runtime becomes prohibitive, narrow back to `[Locality::FunctionLocal, Locality::ArgEscaping]` (the two intermediate variants) as the secondary fallback — but the default is to add, not subtract, coverage.

- [ ] Run `cargo test -p ori_arc` and observe which tests change behavior. Tests that iterate `for l in all_locality() { ... }` now run 5 iterations instead of 4 — they should all pass automatically because the new variant slots into the existing chain via `Locality::join = max`. Tests that hard-code variant counts (e.g., `assert_eq!(count, 16)` for a 4×4 commutativity matrix) will fail and must be updated to reflect the new 5×5 = 25 count.

- [ ] Find any other hard-coded 4-variant or 2-variant exhaustive enumerations of `Locality` in the test file. Use `Grep "Locality::(BlockLocal|FunctionLocal|HeapEscaping|Unknown)" compiler/ori_arc/src/aims/lattice/tests.rs` to find candidate sites and review each manually. Add `ArgEscaping` to any that purport to enumerate "all variants."

- [ ] Document in the section's completion notes which helpers were extended and the test count delta (e.g., "all_locality grew from 4 to 5 variants; commutativity test count went from 16 to 25; associativity from 64 to 125; representative_states locality sample grew from 2 to 3").

---

## 02.8 Extend lattice tests for the new variant

**File(s):** `compiler/ori_arc/src/aims/lattice/tests.rs` (currently 2365 lines, exempt from 500-line limit per compiler.md)

**Context:** Section 02.7 fixed `all_locality()` and updated the helper sites that consume it. Section 02.8 extends the test matrix for the new variant: lattice property tests (commutativity, associativity, idempotence), per-rule firing tests (Rules 4, 6, 8 with `ArgEscaping` in various positions), helper tests (`is_local`, `is_rc_skip_eligible`, etc.), the soundness pin test, the chain ordering rank test, and the `CHAIN_HEIGHT` constant assertion update.

**Test extension matrix:**

| Test category | Current cases | New cases needed | Total after |
|---|---|---|---|
| Idempotence (`L.join(L) == L`) | 4 (one per variant) | +1 for `ArgEscaping` | 5 |
| Commutativity (`L1.join(L2) == L2.join(L1)`) | 4×4 = 16 pairs | +9 (5×5 - 4×4) | 25 |
| Associativity (`L1.join(L2).join(L3) == L1.join(L2.join(L3))`) | 4×4×4 = 64 triples | +61 (5³ - 4³) | 125 |
| Per-rule firing (Rule 4, 6, 8) | ~12 (mixed variants) | +6 (one fire-test and one no-fire test per rule with ArgEscaping) | ~18 |
| Helper tests (`is_local`, `is_rc_skip_eligible`) | 4 | +1 for ArgEscaping | 5 |
| `CHAIN_HEIGHT` constant assertion | 1 (= 15) | update to 16 | 1 |
| Chain ordering rank test (NEW) | 0 | +1 (asserts the discriminant ordering) | 1 |
| Soundness pin — Rule 6 (NEW) | 0 | +1 (`ArgEscaping + Unique` stays `Unique`) | 1 |
| **`ParamContract::may_escape()` derivation (NEW, added by Agent 4)** | 0 | +7 (5 per-variant + 1 matrix completeness + 1 negative pin) in `contract/tests.rs` | 7 |
| **Return-widening producer-site (NEW, added by Agent 4)** | 0 | +3 in `intraprocedural/tests.rs` (ArgEscaping widens, FunctionLocal widens, HeapEscaping does NOT re-widen — soundness condition 4) | 3 |

- [ ] **Idempotence**: Add test case for `Locality::ArgEscaping.join(Locality::ArgEscaping) == Locality::ArgEscaping`. Most existing tests use `all_locality()` in a loop, so 02.7's fix may automatically extend coverage — verify this is the case before adding redundant tests.

- [ ] **Commutativity**: If the existing test uses `for l1 in all_locality() { for l2 in all_locality() { assert_eq!(l1.join(l2), l2.join(l1)); } }`, then 02.7's fix automatically extends from 16 to 25 cases. Verify by running the test and observing it iterates 25 times.

- [ ] **Associativity**: Same pattern — if the existing test uses three nested `for l in all_locality()` loops, 02.7's fix extends coverage from 64 to 125 cases automatically.

- [ ] **Per-rule firing tests**: Add explicit test cases for Rules 4, 6, 8 with `ArgEscaping` in the locality field:
  ```rust
  #[test]
  fn rule_8_borrowed_widens_arg_escaping_to_function_local() {
      // Rule 8: Borrowed → locality ≤ FunctionLocal
      // ArgEscaping > FunctionLocal, so a Borrowed value with ArgEscaping
      // locality should widen to FunctionLocal during canonicalize.
      let mut state = AimsState {
          access: AccessClass::Borrowed,
          locality: Locality::ArgEscaping,
          ..AimsState::FRESH
      };
      state.canonicalize();
      assert_eq!(state.locality, Locality::FunctionLocal,
          "Rule 8 should widen ArgEscaping borrows down to FunctionLocal");
  }

  #[test]
  fn rule_6_does_not_fire_on_arg_escaping_unique() {
      // Rule 6: HeapEscaping + Unique → MaybeShared
      // ArgEscaping is NOT HeapEscaping, so the rule should NOT fire.
      // This is the SOUNDNESS PIN — see Section 01.2 and Go/Lean 4 prior art.
      let mut state = AimsState {
          locality: Locality::ArgEscaping,
          uniqueness: Uniqueness::Unique,
          ..AimsState::FRESH
      };
      state.canonicalize();
      assert_eq!(state.uniqueness, Uniqueness::Unique,
          "Rule 6 must NOT fire on ArgEscaping. ArgEscaping values cross a \
           call boundary but the callee does not retain references past the \
           call return, so uniqueness is preserved at the boundary. Soundness \
           gate for callee aliasing is the may_share contract effects path, \
           not the locality lattice. See plans/locality-representation-\
           unification/section-01.md §01.2.");
  }

  #[test]
  fn rule_4_does_not_fire_on_arg_escaping() {
      // Rule 4: BlockLocal + Owned + ≤Once → Unique
      // ArgEscaping is NOT BlockLocal, so the rule should NOT promote
      // MaybeShared → Unique even when access/cardinality match.
      let mut state = AimsState {
          locality: Locality::ArgEscaping,
          access: AccessClass::Owned,
          cardinality: Cardinality::Once,
          uniqueness: Uniqueness::MaybeShared,
          ..AimsState::FRESH
      };
      state.canonicalize();
      assert_eq!(state.uniqueness, Uniqueness::MaybeShared,
          "Rule 4 must NOT promote ArgEscaping values to Unique");
  }
  ```

- [ ] **`ParamContract::may_escape()` derivation matrix (NEW, load-bearing after 02.6 conversion)**: Section 02.6 converts `may_escape` from a stored field to a method `may_escape(&self) -> bool { self.locality_bound > Locality::BlockLocal }`. The derivation is load-bearing — a subtle off-by-one (`>` vs `>=`) would silently return the wrong value for `BlockLocal` or `FunctionLocal`. Add explicit per-variant tests in `compiler/ori_arc/src/aims/contract/tests.rs` (NOT `lattice/tests.rs` — the method lives on `ParamContract`, not `AimsState`) covering every variant through the derived method:
  ```rust
  // In compiler/ori_arc/src/aims/contract/tests.rs (sibling of contract/mod.rs)

  fn contract_with_locality(locality: Locality) -> ParamContract {
      ParamContract {
          access: AccessClass::Owned,
          consumption: Consumption::Linear,
          cardinality: Cardinality::Once,
          may_share: false,
          locality_bound: locality,
          uniqueness: Uniqueness::Unique,
      }
  }

  #[test]
  fn may_escape_returns_false_for_block_local() {
      // BlockLocal is the ONLY variant that does NOT escape its defining block.
      // The derivation `locality_bound > Locality::BlockLocal` MUST return false here.
      assert!(
          !contract_with_locality(Locality::BlockLocal).may_escape(),
          "BlockLocal does not escape its block (locality_bound == BlockLocal)"
      );
  }

  #[test]
  fn may_escape_returns_true_for_function_local() {
      // FunctionLocal escapes the BLOCK (it reaches the function scope) even
      // though it does not escape the FUNCTION. may_escape() tests block escape,
      // per the method doc comment.
      assert!(
          contract_with_locality(Locality::FunctionLocal).may_escape(),
          "FunctionLocal escapes its defining block even though it does not escape the function"
      );
  }

  #[test]
  fn may_escape_returns_true_for_arg_escaping() {
      // NEW variant — the test matrix must include it because the `>` comparison
      // relies on the discriminant order being BlockLocal < FunctionLocal < ArgEscaping.
      assert!(
          contract_with_locality(Locality::ArgEscaping).may_escape(),
          "ArgEscaping escapes its defining block (crosses a call boundary)"
      );
  }

  #[test]
  fn may_escape_returns_true_for_heap_escaping() {
      assert!(
          contract_with_locality(Locality::HeapEscaping).may_escape(),
          "HeapEscaping escapes its defining block (and function)"
      );
  }

  #[test]
  fn may_escape_returns_true_for_unknown() {
      assert!(
          contract_with_locality(Locality::Unknown).may_escape(),
          "Unknown (conservative default) must escape its defining block"
      );
  }

  /// Self-verifying matrix completeness check: iterate every Locality variant
  /// and assert the derivation is consistent with `locality > BlockLocal`.
  /// Catches any future off-by-one in the derivation.
  #[test]
  fn may_escape_matches_derivation_for_all_variants() {
      use Locality::*;
      let variants = [BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown];
      let mut visited = 0;
      for locality in variants {
          let expected = locality > BlockLocal;
          let actual = contract_with_locality(locality).may_escape();
          assert_eq!(
              actual, expected,
              "may_escape() for {:?} should be {} (locality > BlockLocal)",
              locality, expected
          );
          visited += 1;
      }
      assert_eq!(visited, 5, "matrix must visit all 5 Locality variants");
  }

  /// Negative pin: proves the derivation is NOT `locality >= BlockLocal`.
  /// If anyone "fixes" the derivation to `>=`, this test fails loudly.
  #[test]
  fn may_escape_negative_pin_block_local_does_not_escape() {
      // This is the negative pin paired with the BlockLocal positive case.
      // It rejects the broken behavior `locality >= BlockLocal` (which would
      // incorrectly return `true` for every variant).
      let contract = contract_with_locality(Locality::BlockLocal);
      assert!(
          !contract.may_escape(),
          "NEGATIVE PIN: BlockLocal must NOT report may_escape()=true. \
           If this fails, the derivation was likely changed from `> BlockLocal` \
           to `>= BlockLocal`, which is wrong — see plans/locality-representation-\
           unification/section-02.md §02.6."
      );
  }
  ```
  These 7 tests (5 per-variant + 1 matrix completeness + 1 negative pin) form the complete matrix for the derived `may_escape()` method. Without them, a subtle derivation bug in Section 02.6 would go undetected.

- [ ] **Return-value widening test (NEW, producer-interaction soundness pin)**: Section 02.5 predicate site #10 (`block.rs:155`) is a producer that widens `locality < HeapEscaping` to `HeapEscaping` for return values. After the `ArgEscaping` variant is added, this predicate will widen `ArgEscaping → HeapEscaping` for any returned value (because `ArgEscaping < HeapEscaping`). This is the **enforcement site for soundness condition 4** (no heap persistence) that Section 01.4 documents.

  No existing test exercises this predicate with `ArgEscaping` in the entry state. Add one in `compiler/ori_arc/src/aims/intraprocedural/tests.rs` (sibling of `intraprocedural/block.rs`), either as a unit test calling `compute_block_entry_state` directly or as a behavioral smoke test that constructs the minimal context and inspects the returned entry state:
  ```rust
  /// Soundness pin for condition 4 (no heap persistence) from
  /// plans/locality-representation-unification/section-01.md §01.4.
  ///
  /// Proves that a value with locality ArgEscaping observed flowing to a Return
  /// terminator is upgraded to HeapEscaping by the producer at block.rs:155.
  /// Without this upgrade, a returned ArgEscaping value would persist in the
  /// caller's escape scope as "temporarily callee-aliased" — which is unsound
  /// because the caller has already returned the value to a different caller.
  ///
  /// This is a PRODUCER-SITE soundness test, not a lattice test. It verifies
  /// that block.rs:155's `if entry.locality < Locality::HeapEscaping` branch
  /// fires on ArgEscaping (since ArgEscaping < HeapEscaping is true).
  #[test]
  fn return_widening_promotes_arg_escaping_to_heap_escaping() {
      // Construct an AimsState with locality = ArgEscaping and feed it through
      // the return-widening branch. The exact mechanism depends on whether
      // compute_block_entry_state is callable directly in tests (likely yes,
      // per the existing test patterns in intraprocedural/tests.rs).
      //
      // If compute_block_entry_state cannot be called standalone, simulate the
      // predicate directly:
      let mut entry = AimsState {
          locality: Locality::ArgEscaping,
          ..AimsState::FRESH
      };
      // This mirrors block.rs:155's logic:
      if entry.locality < Locality::HeapEscaping {
          entry.locality = Locality::HeapEscaping;
      }
      assert_eq!(
          entry.locality,
          Locality::HeapEscaping,
          "Return widening at block.rs:155 must upgrade ArgEscaping to HeapEscaping. \
           A returned value has left the caller's frame and is no longer merely \
           aliased during a call — it has actually escaped. See soundness condition 4 \
           in plans/locality-representation-unification/section-01.md §01.4."
      );
  }

  /// Negative pin paired with the above: FunctionLocal (which is also < HeapEscaping)
  /// also widens to HeapEscaping at the return boundary. This proves the predicate
  /// is not accidentally variant-specific.
  #[test]
  fn return_widening_promotes_function_local_to_heap_escaping() {
      let mut entry = AimsState {
          locality: Locality::FunctionLocal,
          ..AimsState::FRESH
      };
      if entry.locality < Locality::HeapEscaping {
          entry.locality = Locality::HeapEscaping;
      }
      assert_eq!(entry.locality, Locality::HeapEscaping);
  }

  /// Negative pin: HeapEscaping (already at the upper bound) is NOT re-widened.
  /// Proves the predicate uses `<`, not `<=`, at block.rs:155.
  #[test]
  fn return_widening_leaves_heap_escaping_unchanged() {
      let mut entry = AimsState {
          locality: Locality::HeapEscaping,
          ..AimsState::FRESH
      };
      let before = entry.locality;
      if entry.locality < Locality::HeapEscaping {
          entry.locality = Locality::HeapEscaping;
      }
      assert_eq!(entry.locality, before, "HeapEscaping must not be re-widened");
  }
  ```

  **If the test cannot call `compute_block_entry_state` directly** (the function may be private to the module or require a full ArcFunction context), document the limitation and switch to a smoke-test approach that constructs a minimal ArcFunction with a single Return terminator and runs the full intraprocedural analysis pipeline. The goal is to exercise the real block.rs:155 predicate, not a duplicate of it. The simulation above is acceptable as a LAST RESORT for cases where the producer site is genuinely inaccessible from tests — but flag it in the section's completion notes as "simulation only; producer-site smoke test deferred pending a testable handle into compute_block_entry_state." If that flag ever appears in completion notes, add a concrete `- [ ]` item to `repr-opt §08`'s plan text saying "add a producer-site smoke test for block.rs:155 return widening once §08 has a connection-graph producer that creates ArgEscaping values" — `/fix-bug`-style no-deferral discipline applies.

- [ ] **Helper tests**: Add `is_local()` and `is_rc_skip_eligible()` cases for `ArgEscaping`:
  ```rust
  #[test]
  fn is_local_returns_false_for_arg_escaping() {
      let state = AimsState {
          locality: Locality::ArgEscaping,
          ..AimsState::FRESH
      };
      assert!(!state.is_local(),
          "ArgEscaping is NOT local — it crosses a call boundary");
  }

  #[test]
  fn is_rc_skip_eligible_returns_false_for_arg_escaping() {
      let state = AimsState {
          locality: Locality::ArgEscaping,
          access: AccessClass::Owned,
          consumption: Consumption::Linear,
          ..AimsState::FRESH
      };
      assert!(!state.is_rc_skip_eligible(),
          "ArgEscaping is not rc-skip-eligible because is_local() returns false");
  }
  ```

- [ ] **`CHAIN_HEIGHT` constant assertion**: Find the existing `assert_eq!(AimsState::CHAIN_HEIGHT, 15)` (Pass 1 Agent 2 reported it at `tests.rs:2054`) and update to `16`:
  ```rust
  // Before
  assert_eq!(AimsState::CHAIN_HEIGHT, 15);
  // After
  assert_eq!(AimsState::CHAIN_HEIGHT, 16,
      "CHAIN_HEIGHT increases from 15 to 16 because Locality gained the \
       ArgEscaping variant (chain length 3 → 4)");
  ```

- [ ] **Chain ordering rank test (NEW)**: Add a test asserting the discriminant ordering. This is the **mission criterion** for "5 variants in order BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown":
  ```rust
  #[test]
  fn locality_chain_ordering_is_correct() {
      assert!(Locality::BlockLocal < Locality::FunctionLocal);
      assert!(Locality::FunctionLocal < Locality::ArgEscaping);
      assert!(Locality::ArgEscaping < Locality::HeapEscaping);
      assert!(Locality::HeapEscaping < Locality::Unknown);
      // And the transitive closure for completeness:
      assert!(Locality::BlockLocal < Locality::Unknown);
      assert!(Locality::FunctionLocal < Locality::HeapEscaping);
      assert!(Locality::BlockLocal < Locality::ArgEscaping);
  }
  ```

- [ ] **Soundness pin (NEW, key mission criterion)**: The test `rule_6_does_not_fire_on_arg_escaping_unique` above IS the soundness pin. Mark it as such with a comment so future readers know it's a load-bearing regression guard.

- [ ] Run `cargo test -p ori_arc` and verify all tests pass — including the +9 commutativity, +61 associativity, +6 per-rule firing, +2 helpers, +1 chain ordering, +1 soundness pin tests.

- [ ] Verify the `lattice/tests.rs` total line count (currently 2365) is reasonable after extension. Tests are exempt from the 500-line limit, but if the file grows past ~3000 lines, consider splitting in a future cleanup section (NOT this plan — it would expand scope).

---

## 02.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers. -->

- None.

---

## 02.N Completion Checklist

- [ ] `compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality` has 5 variants in chain order
- [ ] Lean 4 inversion comment present in `dimensions.rs` near the `Locality` enum
- [ ] `AimsState::CHAIN_HEIGHT == 16` and the doc comment lists `Locality: 4`
- [ ] `iteration_limit()` formula unchanged (only the constant value flows through)
- [ ] All canonicalize rules (4, 6, 8) verified as not modified — only the new variant slots in transparently
- [ ] All 13 predicate sites audited with explicit per-site decisions documented in completion notes; `cargo check -p ori_arc` clean
- [ ] `ParamContract::may_escape` is no longer a stored field; `may_escape()` method exists and returns `locality_bound > BlockLocal`
- [ ] CONSERVATIVE and OPTIMISTIC `ParamContract` constants no longer set `may_escape`
- [ ] `ParamContract::join` no longer ORs `may_escape`
- [ ] `all_locality()` helper at `tests.rs:26` returns all 5 variants (extension from 4 to 5; not a bug fix — Pass 1 misread)
- [ ] `representative_states()` locality sample at `tests.rs:68` extended from 2 to 3 (per 02.7 default decision; document in completion notes if narrowed for runtime cost)
- [ ] Per-rule firing tests added for `ArgEscaping`: Rule 8 widens borrows, Rule 6 does NOT fire (soundness pin), Rule 4 does NOT promote
- [ ] `is_local()` and `is_rc_skip_eligible()` tests cover `ArgEscaping`
- [ ] `CHAIN_HEIGHT` constant assertion updated from 15 to 16
- [ ] Chain ordering rank test added
- [ ] Soundness pin test (`rule_6_does_not_fire_on_arg_escaping_unique`) is marked as a permanent regression guard
- [ ] **`ParamContract::may_escape()` derivation matrix** added in `contract/tests.rs`: 5 per-variant tests + 1 self-verifying matrix test + 1 negative pin (`may_escape_negative_pin_block_local_does_not_escape`) — proves the derivation `locality_bound > Locality::BlockLocal` is correct AND rejects the broken `>=` alternative
- [ ] **Return-widening soundness pins** added in `intraprocedural/tests.rs`: `return_widening_promotes_arg_escaping_to_heap_escaping` (positive, soundness condition 4 enforcement), `return_widening_promotes_function_local_to_heap_escaping` (positive, proves predicate is not ArgEscaping-specific), `return_widening_leaves_heap_escaping_unchanged` (negative pin, proves `<` not `<=`)
- [ ] `cargo test -p ori_arc` green
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] All intermediate TPR checkpoint findings resolved (the 02.4 and 02.6 TPR checkpoints)
- [ ] Plan annotation cleanup: code annotations referencing this plan (e.g., the comment in `canonicalize.rs` referencing `plans/locality-representation-unification/section-01.md`) are temporary scaffolding. They will be removed in Section 05.3 (plan annotation cleanup scan), which has the canonicalize.rs Rule 6 comment listed in its known-cleanup-targets table. **Do NOT remove them in Section 02** — they aid navigation during active execution.
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 02
  - [ ] `00-overview.md` mission success criteria checkboxes updated (variant added, CHAIN_HEIGHT == 16, may_escape converted, soundness pin)
  - [ ] `index.md` Section 02 status updated
  - [ ] Section 03's `depends_on` verified — Section 03 has `depends_on: ["02"]`
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — verifies the LEAK is gone and no new ones introduced
- [ ] `/improve-tooling` retrospective completed — for this section especially: was there a missing helper to "find all exhaustive matches on an enum across the codebase"? Was `cargo check` output a good guide for the consumer migration, or did the implementer need a custom script to enumerate match sites? Did the matrix-test-extension work need a "scale this exhaustive enumeration test from N variants to N+1" tool? Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful.

**Exit Criteria:** A reviewer running `cargo test -p ori_arc -- --test-threads=1 lattice` sees the test suite execute the full 5×5 commutativity matrix (25 cases), the full 5×5×5 associativity matrix (125 cases), all 5 variants in the per-rule firing tests, and the soundness pin test with a clear failure message if Rule 6 ever fires on `ArgEscaping + Unique`. `grep -n 'may_escape:' compiler/ori_arc/src/aims/contract/mod.rs` returns only function-definition lines. `wc -l` confirms `lattice/dimensions.rs`, `lattice/state.rs`, `lattice/canonicalize.rs`, and `contract/mod.rs` are all still under their target sizes from Section 00. `timeout 150 ./test-all.sh` is green.
