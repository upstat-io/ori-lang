---
section: "02"
title: "ori_arc Implementation"
status: not-started
reviewed: false
goal: "Implement the locked decisions from Section 01 in ori_arc: add the ArgEscaping variant, verify canonicalize rules fire identically, migrate 8 non-test consumer files, convert ParamContract::may_escape to a derived method, update CHAIN_HEIGHT, extend the lattice test matrix, add the Lean 4 inversion comment to dimensions.rs"
success_criteria:
  - "compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality has 5 variants in order BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown — verified by a rank test"
  - "AimsState::CHAIN_HEIGHT == 16 (was 15) and the test at lattice/tests.rs:2054 passes with the new value"
  - "ParamContract has no stored may_escape field; ParamContract::may_escape() is a method returning self.locality_bound > Locality::BlockLocal"
  - "All 8 non-test consumer files in compiler/ori_arc/ compile after the variant addition (handle the new arm in any exhaustive matches)"
  - "all_locality() helper at lattice/tests.rs:26 returns all 5 variants (fixes pre-existing bug + extends for ArgEscaping)"
  - "Soundness pin test exists asserting ArgEscaping + Unique stays Unique through canonicalize() — Rule 6 does NOT fire"
  - "Lean 4 inversion comment added to dimensions.rs near the Locality enum"
  - "cargo test -p ori_arc green; ./test-all.sh green; ./clippy-all.sh green"
  - "Connects upward to mission criteria: chain ordering (lattice change), CHAIN_HEIGHT update, may_escape LEAK fix, soundness pin"
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
    title: "Migrate 8 non-test consumer files"
    status: not-started
  - id: "02.6"
    title: "Convert ParamContract::may_escape from field to derived method"
    status: not-started
  - id: "02.7"
    title: "Fix incomplete all_locality() helper at lattice/tests.rs:26"
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
**Goal:** Implement the locked decisions from Section 01 inside `ori_arc`. Add the `ArgEscaping` variant, verify the existing canonicalize rules continue to fire identically (Section 01.2 locked: zero changes to rule logic), migrate 8 non-test consumer files to handle the new variant in any exhaustive matches, convert `ParamContract::may_escape` from a stored field to a derived method (fixing the `LEAK:scattered-knowledge`), update `CHAIN_HEIGHT` from 15 to 16, fix the pre-existing bug in `all_locality()` at `lattice/tests.rs:26` while extending it for the new variant, and extend the lattice test matrix (commutativity, associativity, exhaustive enumeration, soundness pin). Finally, add the Lean 4 inversion comment to `dimensions.rs` near the `Locality` enum so future contributors understand why Ori widens where Lean 4 narrows.

**Success Criteria:**

- [ ] `compiler/ori_arc/src/aims/lattice/dimensions.rs::Locality` has 5 variants in order `BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown` — verified by a discriminant rank test in `lattice/tests.rs`
- [ ] `AimsState::CHAIN_HEIGHT == 16` (was 15) and the test at `lattice/tests.rs:2054` passes with the new value
- [ ] `iteration_limit()` formula recomputes correctly (still `CHAIN_HEIGHT * num_variables * num_blocks`, just with the new constant)
- [ ] `ParamContract` has no stored `may_escape` field. `ParamContract::may_escape()` is a method returning `self.locality_bound > Locality::BlockLocal`. Verified by `grep -n 'may_escape:' compiler/ori_arc/src/aims/contract/mod.rs` returning only function-definition lines (not field accesses)
- [ ] All 8 non-test consumer files in `compiler/ori_arc/` compile after the variant addition. `cargo check -p ori_arc` succeeds with zero errors
- [ ] `all_locality()` helper at `lattice/tests.rs:26` returns **all 5** variants (fixes pre-existing bug — currently returns 2 of 4 — AND extends for `ArgEscaping`)
- [ ] **Soundness pin test exists** asserting `AimsState { locality: ArgEscaping, uniqueness: Unique, ... }` survives `canonicalize()` with `uniqueness == Unique` (i.e., Rule 6 does NOT fire on the new variant)
- [ ] Lean 4 inversion comment added to `dimensions.rs` near the `Locality` enum (per Section 01.5 + Codex Step 6B Finding 4)
- [ ] `cargo test -p ori_arc` green
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Connects upward to mission criteria: "Locality enum has 5 variants...", "CHAIN_HEIGHT == 16", "ParamContract::may_escape is no longer a stored field", "Soundness pin"

**Context:** This is the largest implementation section in the plan. It performs the actual semantic change that the rest of the plan exists to enable. By the time Section 02 runs, Section 00 has split the lattice and transfer modules into clean sibling files, and Section 01 has produced the locked decision document. Section 02 implements against that document — the implementer should not need to make any design judgement that isn't already locked.

The work is **broader than the section title suggests**. Adding one enum variant has surprisingly large ripple effects:

1. **The variant itself**: 1 line in `dimensions.rs`. Trivial.
2. **Test extension**: ~150 lines. The lattice tests use exhaustive enumeration patterns: commutativity (4×4 → 5×5 = +9 cases), associativity (4×4×4 → 5×5×5 = +61 cases), per-rule firing tests, helper tests. Pass 1 Agent 2 enumerated the existing test sites at `tests.rs:26, 68, 1449, 2247` — each needs to be updated to include the new variant.
3. **Consumer migration**: 8 non-test files in `compiler/ori_arc/` (per Pass 1 Agent 1's count). Most consumers use order-based predicates (`> Locality::FunctionLocal`, `<= Locality::FunctionLocal`) which work transparently with the new variant. A few use exhaustive matches (e.g., `is_local()` at `lattice/state.rs` post-Section-00) that need a new arm.
4. **`may_escape` conversion**: ~40 lines. Pass 1 Agent 1 confirmed `may_escape` has zero non-test readers in production, so the conversion is pure API tightening with zero behavioral risk. The work is: delete the field, delete its writes in `CONSERVATIVE`/`OPTIMISTIC`, delete its branch in `ParamContract::join`, add a `pub fn may_escape(&self) -> bool` method.
5. **`CHAIN_HEIGHT` update**: 2 lines in the lattice. The constant doc comment lists per-dimension chain heights — the `Locality` line goes from "3" to "4" and the total goes from 15 to 16.
6. **Pre-existing test bug fix**: `all_locality()` at `tests.rs:26` currently returns only `[FunctionLocal, Unknown]` (2 of 4 current variants — pre-existing gap). Section 02.7 fixes this incidentally while extending it to all 5 post-`ArgEscaping`.
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

**File(s):** `compiler/ori_arc/src/aims/lattice/dimensions.rs` (the enum definition is currently at lines 165-176)

**Context:** Per Section 01.1, the variant is added between `FunctionLocal` and `HeapEscaping` to preserve the chain ordering. The discriminant ordering is implicit from declaration order (Rust enums get sequential discriminants by default), and `PartialOrd`/`Ord` are derived — so the chain semantics fall out automatically. No explicit `#[repr(u8)]` or discriminant assignment is needed.

The current enum (verified during plan creation):

```rust
// Before (compiler/ori_arc/src/aims/lattice/dimensions.rs:165-176)
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
- [ ] Run `cargo check -p ori_arc` and observe the resulting compile errors. Most should be in exhaustive matches (`is_local`, possibly `iteration_limit` doc comment, etc.) — these are 02.5's work. The variant addition itself succeeds.
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

## 02.5 Migrate 8 non-test consumer files

**Scope:** This subsection is limited to consumers in `compiler/ori_arc/`. Other crates (`ori_repr`, `ori_llvm`, etc.) are verified compiler-wide via `cargo check` in the gates below — any exhaustive matches on `Locality` outside `ori_arc/` will surface as compile errors and must be fixed before Section 02 is marked complete. Pass 1 Agent 1's compiler-wide scan found zero direct `Locality` consumers in `ori_repr/` (only the EscapeInfo placeholder, which Section 03 replaces).

**File(s):** Per Pass 1 Agent 1 + Pass 2 deep read, the 8 non-test files in `compiler/ori_arc/` that consume `Locality` and need attention are (post-Section-00 paths):

1. `compiler/ori_arc/src/aims/lattice/state.rs` — `is_local()` exhaustive match (formerly `lattice/mod.rs:432-436`)
2. `compiler/ori_arc/src/aims/lattice/canonicalize.rs` — Rules 4, 6, 8 (already verified in 02.3, no change)
3. `compiler/ori_arc/src/aims/intraprocedural/block.rs` — cross-block widening at lines 92-100 and return widening at lines 147-159 (PRODUCERS that today produce `BlockLocal/FunctionLocal/HeapEscaping`; do NOT modify them in this plan — the new variant is produced by future `repr-opt §08`)
4. `compiler/ori_arc/src/aims/intraprocedural/effects.rs` — order predicate at line 38 (`demand.locality > Locality::BlockLocal`) — works automatically, no change
5. `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` — order predicates at lines 429, 439-440 — work automatically, no change
6. `compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs` — exhaustive match at lines 95-96 (`exit_state.locality` matched against `FunctionLocal | BlockLocal`) — needs new arm
7. `compiler/ori_arc/src/aims/transfer/forward.rs` (post-Section-00) — `transfer_collection_reuse` and other producers — no change in v1 (those produce specific variants, not pattern-match on locality)
8. `compiler/ori_arc/src/aims/contract/mod.rs` — `ParamContract::join` at line 248-251 — works automatically (uses `Locality::join = max`)

**Context:** Most consumers are **order-based predicates** that work transparently because the new variant slots into the existing chain. The few consumers that **exhaustively match** on `Locality` need new arms for `ArgEscaping`. After Section 02.1 adds the variant, `cargo check -p ori_arc` will flag every exhaustive match that needs an update — use the compiler errors to drive the migration.

**Migration recipe:**

| Pattern in current code | What to do |
|---|---|
| `match locality { BlockLocal => ..., FunctionLocal => ..., HeapEscaping => ..., Unknown => ... }` | Add an `ArgEscaping => ...` arm |
| `matches!(locality, BlockLocal \| FunctionLocal)` | Decide: should `ArgEscaping` be included? Usually NOT — the helper is testing "is local," and ArgEscaping is not local. Leave the matches! pattern alone. |
| `matches!(locality, HeapEscaping \| Unknown)` | Decide: should `ArgEscaping` be included? Usually YES — the helper is testing "may escape." Add `ArgEscaping` to the pattern. |
| `if locality > Locality::FunctionLocal { ... }` | No change. The new variant slots into the chain and the `>` comparison works automatically. |
| `if locality == Locality::HeapEscaping { ... }` | Decide case-by-case: does the consumer want this branch to fire on `ArgEscaping` too? If yes, change to `if locality >= Locality::ArgEscaping`. If no (the branch is specifically about heap escape), leave it alone. **Default: leave it alone unless the consumer explicitly should treat ArgEscape the same as heap escape.** |

- [ ] Run `cargo check -p ori_arc` after Section 02.1 has added the variant. Capture the list of exhaustive-match warnings/errors.

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

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs:95-96`**: Exhaustive match needs a new arm. Re-read the file to see the surrounding context, then add the `ArgEscaping` arm. The semantics depend on what the surrounding code does with the match — preserve the conservative default (whatever `HeapEscaping | Unknown` does today).
  - [ ] Re-read `compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs` lines 90-110
  - [ ] Add the `ArgEscaping` arm with the same handling as `HeapEscaping` (the conservative default — values that have escaped beyond function-local treatment)
  - [ ] Verify `cargo check -p ori_arc` no longer flags this match site

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/effects.rs:38`**: The current code is `if demand.locality > Locality::BlockLocal { effects.may_share = true; }`. This works automatically because `ArgEscaping > BlockLocal` is `true`. Verify this is correct semantically: a `Construct` whose destination demand is `ArgEscaping` should set `may_share = true` because the callee might temporarily alias during the call. Per Pass 2 deep read of this file, the semantics match — the variable is "demanded with locality > BlockLocal" which means "not strictly block-local," and ArgEscaping qualifies.
  - [ ] No code change. Document in the section's completion notes that this site was verified compatible.

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/state_map.rs:429-440`**: Order-based predicates. Per Pass 1 Agent 1 the predicates are:
  - Line 429: `state.locality == Locality::BlockLocal` (specific variant — does NOT need to fire on ArgEscaping; leave unchanged)
  - Lines 439-440: `state.locality <= Locality::FunctionLocal && state.locality != Locality::BlockLocal` (this is the "function-local but not block-local" case; ArgEscaping is NOT in this set since ArgEscaping > FunctionLocal; leave unchanged)
  - [ ] Re-read state_map.rs lines 425-450 to confirm the predicates' intent
  - [ ] No code changes if the predicates are correct as-is

- [ ] **`compiler/ori_arc/src/aims/intraprocedural/block.rs:97-100, 147-159`**: PRODUCERS that today produce `BlockLocal`, `FunctionLocal`, `HeapEscaping`. **DO NOT modify them in this plan.** The new variant is produced by future `repr-opt §08`'s connection-graph escape analysis, which will add a new branch in this file. This plan only adds the variant; producing it is §08's job.
  - [ ] No code changes
  - [ ] Document in the section's completion notes that these producers are intentionally untouched (per Section 01.4 enforcement layer table, condition 2 belongs to §08)

- [ ] **`compiler/ori_arc/src/aims/contract/mod.rs::ParamContract::join` at line 248-251**: Uses `self.locality_bound.join(other.locality_bound)` (max). Works automatically with the new variant.
  - [ ] No code changes in this subsection. (The `may_escape` field removal is in 02.6.)

- [ ] **`compiler/ori_arc/src/aims/transfer/forward.rs`** (post-Section-00): `transfer_project`, `transfer_collection_reuse`, `transfer_apply_conservative`, etc. produce specific variants (`BlockLocal`, `Unknown`, etc.) but do NOT pattern-match on locality — they only assign new values. No changes needed.
  - [ ] No code changes
  - [ ] Verify by `grep -n 'match.*locality' compiler/ori_arc/src/aims/transfer/forward.rs` returning empty

- [ ] After all 8 files are migrated (or verified compatible), run `cargo check -p ori_arc` and verify zero errors. **Note**: `contract/mod.rs` still has the `may_escape` field as a stored struct member at this point — Section 02.6 removes it. The `cargo check` here verifies the variant addition does not break anything; the `may_escape` LEAK fix is a separate concern handled in the next subsection.
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

- [ ] Run `cargo check -p ori_arc` and observe compile errors at sites that read `param.may_escape` as a field. Convert each to `param.may_escape()` method call:
  ```rust
  // Before
  if param.may_escape { ... }
  // After
  if param.may_escape() { ... }
  ```
  Per Pass 1 Agent 1, the only such sites are in test files (`contract/tests.rs`, `intraprocedural/tests.rs`, `interprocedural/tests.rs`). Update each to use the method form.

- [ ] Run `cargo test -p ori_arc` and verify all tests still pass. The semantic preservation is provable by inspection: every previous `may_escape: true` was paired with `locality_bound: Locality::Unknown` (or another non-BlockLocal value), and every previous `may_escape: false` was paired with `locality_bound: Locality::BlockLocal`. The derived method returns the same value the field would have.

- [ ] Verify `grep -n 'may_escape:' compiler/ori_arc/src/aims/contract/mod.rs` returns only function-definition lines (e.g., `pub fn may_escape(&self) -> bool`), not field accesses or assignments.

- [ ] **TPR checkpoint** — `/tpr-review` covering 02.1–02.6 (variant added, rules verified, chain height updated, consumers migrated, may_escape converted). This catches any may_escape consumer that was missed and any wrong-arm migration in the exhaustive matches.

---

## 02.7 Fix incomplete all_locality() helper at lattice/tests.rs:26

**File(s):** `compiler/ori_arc/src/aims/lattice/tests.rs`

**Context:** Pass 1 Agent 2 found a **pre-existing test bug**: the helper `fn all_locality()` at `tests.rs:26` currently returns `[Locality::FunctionLocal, Locality::Unknown]` — only **2 of 4** existing variants. The helper is used by exhaustive enumeration tests (commutativity, associativity), so its incompleteness means the existing test matrix is missing coverage for `Locality::BlockLocal` and `Locality::HeapEscaping`. This is a pre-existing bug, not something introduced by this plan.

Section 02.7 fixes the helper while extending it for `ArgEscaping`. The fix is incidental to the extension (you have to touch the helper anyway to add the new variant), so it's absorbed into Section 02 per impl-hygiene.md §Technical Debt: *"Fix when you find it. If it can't be fixed in the current change, add an entry to the active plan."*

- [ ] Re-read `compiler/ori_arc/src/aims/lattice/tests.rs:20-40` to find the exact current definition of `all_locality()`. Pass 1 reported it as line 26 but the count may have shifted slightly.

- [ ] Update `all_locality()` to return all 5 variants in chain order:
  ```rust
  // Before (pre-existing bug — returns 2 of 4 current variants)
  fn all_locality() -> [Locality; 2] {
      [Locality::FunctionLocal, Locality::Unknown]
  }

  // After (fixes pre-existing bug + extends for ArgEscaping)
  fn all_locality() -> [Locality; 5] {
      [
          Locality::BlockLocal,
          Locality::FunctionLocal,
          Locality::ArgEscaping,
          Locality::HeapEscaping,
          Locality::Unknown,
      ]
  }
  ```

- [ ] Run `cargo test -p ori_arc` and observe which tests now fail because `all_locality()` returns 5 variants where they expected 2. These are the test extension sites that need 02.8's work.

- [ ] Find any other 4-variant or 2-variant exhaustive enumerations of `Locality` in the test file (Pass 1 Agent 2 reported lines 68, 1449, 2247 as candidates) and update them to include all 5 variants. Use `grep -n 'Locality::' compiler/ori_arc/src/aims/lattice/tests.rs | grep -E 'BlockLocal|FunctionLocal|HeapEscaping|Unknown'` to find candidate sites.

- [ ] Document the pre-existing bug fix in the section's completion notes: *"Section 02.7 fixed a pre-existing bug in `all_locality()` (was returning 2 of 4 variants) while extending it to all 5 variants for `ArgEscaping` coverage."*

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
| Soundness pin (NEW) | 0 | +1 (`ArgEscaping + Unique` stays `Unique`) | 1 |

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
- [ ] All 8 non-test consumer files compile after the migration
- [ ] `ParamContract::may_escape` is no longer a stored field; `may_escape()` method exists and returns `locality_bound > BlockLocal`
- [ ] CONSERVATIVE and OPTIMISTIC `ParamContract` constants no longer set `may_escape`
- [ ] `ParamContract::join` no longer ORs `may_escape`
- [ ] `all_locality()` helper at `tests.rs:26` returns all 5 variants
- [ ] Pre-existing bug fix documented in completion notes
- [ ] Per-rule firing tests added for `ArgEscaping`: Rule 8 widens borrows, Rule 6 does NOT fire (soundness pin), Rule 4 does NOT promote
- [ ] `is_local()` and `is_rc_skip_eligible()` tests cover `ArgEscaping`
- [ ] `CHAIN_HEIGHT` constant assertion updated from 15 to 16
- [ ] Chain ordering rank test added
- [ ] Soundness pin test (`rule_6_does_not_fire_on_arg_escaping_unique`) is marked as a permanent regression guard
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
