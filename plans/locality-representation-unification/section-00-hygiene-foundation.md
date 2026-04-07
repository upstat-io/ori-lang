---
section: "00"
title: "Hygiene Foundation"
status: not-started
reviewed: true
goal: "Restructure the AIMS lattice and transfer modules to live below the 450-line proactive-split threshold and eliminate 17+ stale Section 09.x plan annotations from 5 AIMS files, with zero semantic change to any analysis behavior"
success_criteria:
  - "compiler/ori_arc/src/aims/lattice/mod.rs is ≤80 lines (becomes a dispatch hub: module doc + mod declarations + re-exports only)"
  - "Every sibling file under compiler/ori_arc/src/aims/lattice/ is ≤300 lines"
  - "compiler/ori_arc/src/aims/transfer/mod.rs is ≤80 lines (becomes a dispatch hub)"
  - "Every sibling file under compiler/ori_arc/src/aims/transfer/ is ≤250 lines"
  - "`bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale Section 09 references in compiler/ori_arc/src/aims/"
  - "`grep -rn 'Section 09\\.' compiler/ori_arc/src/aims/` returns only spec references (`Spec: Clause N.M`) if any, never plan-section navigation"
  - "`./test-all.sh` green — zero behavioral regressions (this section makes no semantic changes)"
  - "`./clippy-all.sh` green"
  - "`cargo test -p ori_arc` green"
inspired_by:
  - "Rust rustc_codegen_llvm dispatch-hub mod.rs pattern"
  - "compiler.md §File Layout: 'lib.rs is an index... mod.rs dispatches'"
  - "impl-hygiene.md §File Organization: 'Proactive split: split at ~450 lines'"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "00.1"
    title: "Define seams for lattice/mod.rs split"
    status: not-started
  - id: "00.2"
    title: "Execute lattice/mod.rs split"
    status: not-started
  - id: "00.3"
    title: "Define seams for transfer/mod.rs split"
    status: not-started
  - id: "00.4"
    title: "Execute transfer/mod.rs split"
    status: not-started
  - id: "00.5"
    title: "Rewrite stale Section 09.x annotations in lattice files"
    status: not-started
  - id: "00.6"
    title: "Rewrite stale annotations in 4 other AIMS files"
    status: not-started
  - id: "00.7"
    title: "Safety net: /add-bug for any residual stale annotations"
    status: not-started
  - id: "00.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "00.N"
    title: "Completion Checklist"
    status: not-started
# TPR Checkpoint after 00.4 (file splits done, before annotation work begins)
---

# Section 00: Hygiene Foundation

**Status:** Not Started
**Goal:** Restructure the AIMS lattice and transfer modules to live below the 450-line proactive-split threshold and eliminate 17+ stale `Section 09.x` plan annotations from 5 AIMS files, with **zero semantic change** to any analysis behavior. Every test passing before this section must pass after, byte-identically. The section creates the substrate that Sections 02 and 05 will write into.

**Success Criteria:**

- [ ] `compiler/ori_arc/src/aims/lattice/mod.rs` is ≤80 lines (becomes a dispatch hub: module doc + `mod` declarations + `pub use` re-exports only) — verified by `wc -l`
- [ ] Every sibling file under `compiler/ori_arc/src/aims/lattice/` is ≤300 lines — verified by `wc -l compiler/ori_arc/src/aims/lattice/*.rs | awk '{if ($1 > 300 && $2 != "tests.rs") print $0}'` returning empty
- [ ] `compiler/ori_arc/src/aims/transfer/mod.rs` is ≤80 lines (becomes a dispatch hub) — verified by `wc -l`
- [ ] Every sibling file under `compiler/ori_arc/src/aims/transfer/` is ≤250 lines — verified the same way
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale `Section 09` references in `compiler/ori_arc/src/aims/` files
- [ ] `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns only spec references (`Spec: Clause N.M`) if any, never plan-section navigation pointers
- [ ] `./test-all.sh` green — zero behavioral regressions (no semantic changes were made)
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] Connects upward to mission criteria: "lattice/mod.rs ≤450 lines", "transfer/mod.rs ≤450 lines", "no stale Section 09.x annotations in AIMS files"

**Context:** Phase 2 research surfaced two foundational obstacles for the rest of this plan. First, `compiler/ori_arc/src/aims/lattice/mod.rs` is 552 lines and `compiler/ori_arc/src/aims/transfer/mod.rs` is 524 lines — both over the 500-line limit from `compiler.md`. The moment Section 02 adds `Locality::ArgEscaping` and the test extensions, both files would exceed the limit further. Per `impl-hygiene.md` §File Organization: *"Proactive split: split at ~450 lines if you know more code is coming. Don't wait until over the limit."*

Second, `compiler/ori_arc/src/aims/lattice/mod.rs` contains 17 references to `Section 09.2`, `Section 09.3`, and `Section 09.5` as plan-navigation annotations (lines 8, 9, 10, 35, 37, 40, 120, 180, 224, 280, 283, 284, 286, 324, 336, 350, 357, 370, 420). The phrase "Effect Activation" (which the annotations cite as the originating plan section) appears in 5 source files in `compiler/ori_arc/src/aims/` but in **zero** plan files anywhere in `plans/` or `plans/completed/`. The originating plan was either renamed during a restructuring, deleted without cleanup, or never tracked formally. Per `impl-hygiene.md` §Comments: *"Plan annotations are temporary scaffolding... They MUST be removed when the plan completes... Stale annotations from completed plans are hygiene violations (DRIFT category)."*

The two problems are tightly coupled to the rest of the plan's scope: Section 02 will write new code into `lattice/mod.rs` and `transfer/mod.rs`, and the annotations live in the same files. Doing the hygiene work as a separate prerequisite plan would create an awkward intermediate state where the splits exist but haven't been used. Codex Step 6B Finding 1 confirmed: absorb into Section 0 of this plan. Codex Step 6B Finding 2 added: do the annotation rewrites in Section 0 too, not in verification — keeping all "no semantic change" hygiene work in a single coherent phase prevents semantic and structural cleanup from getting tangled.

**Reference implementations:**

- **compiler.md** `§File Layout`: *"`lib.rs` is an index... `mod.rs` dispatches: routes to submodules, holds shared private items. Leaf files implement: actual logic lives here."* The current `lattice/mod.rs` and `transfer/mod.rs` violate this — they both implement logic AND act as module roots.
- **impl-hygiene.md** `§File Organization`: *"500-line limit: source files (excluding tests); exceeding = BLOAT finding. Proactive split: split at ~450 lines if you know more code is coming."*
- **impl-hygiene.md** `§Comments`: *"Plan annotations are temporary scaffolding... They MUST be removed when the plan completes... Stale annotations from completed plans are hygiene violations (DRIFT category). Run `.claude/skills/impl-hygiene-review/plan-annotations.sh` to scan."*
- **`compiler/ori_arc/src/aims/lattice/dimensions.rs`** (281 lines): an example of a properly-extracted submodule under `lattice/`. The split work in this section creates 3-4 more sibling files matching this pattern.
- **`compiler/ori_arc/src/aims/contract/context.rs`** (113 lines): another properly-extracted submodule (`ContextBehavior`, `ContextRegion`). The pattern is `mod context; pub use context::{...};` in `mod.rs` plus a narrow sibling file with the implementation.
- **EffectClass activation pattern (commit `6c644dda`, 2026-03-11)** referenced by Pass 1 Agent 2: when the AIMS lattice last gained a new dimension, the work was preceded by hygiene cleanup. Section 00 follows the same precedent.

**Depends on:** Nothing. This is the first section of the plan and must complete before any other section runs. Sections 01-05 all depend on this section transitively.

---

## 00.1 Define seams for lattice/mod.rs split

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs` (currently 552 lines), `compiler/ori_arc/src/aims/lattice/` (target directory for new sibling files)

**Context:** Before executing the split, the architectural seams must be chosen — codex Step 6B emphasized "define the target seams up front so the split is architectural, not mechanical." A bad split (e.g., chunking by line count) creates files that violate the single-responsibility principle and have to be re-split later. A good split aligns with the natural type/concern boundaries already present in the file.

The current `lattice/mod.rs` has these natural sections:

| Lines | Content | Natural sibling |
|---|---|---|
| 1-34 | Module doc + imports + `dimensions` re-exports | (stays in `mod.rs`) |
| 35-66 | `CanonicalizeFeedback` struct + impl | extract to `canonicalize.rs` (with the rules) |
| 68-97 | `SizeClass` newtype + impl | extract to its own `size_class.rs` (auxiliary type, not part of state) |
| 99-189 | `AimsState` struct + constants (TOP, BOTTOM, SCALAR, FRESH) | extract to `state.rs` |
| 200-216 | `AimsState::join` | extract to `state.rs` (stays with the struct) |
| 218-377 | `canonicalize`, `canonicalize_with_feedback`, `canonicalize_single_pass` | extract to `canonicalize.rs` |
| 379-451 | Helpers: `is_rc_needed`, `needs_cow_check`, `is_reuse_candidate`, `is_rc_skip_eligible`, `is_local`, `from_arc_class` | extract to `state.rs` (predicate methods on AimsState) |
| 453-479 | `CHAIN_HEIGHT` constant + `iteration_limit` | extract to `state.rs` (constants on AimsState) |
| 482-552 | `BorrowSource` enum + impl | extract to `borrow_source.rs` (self-contained side-table type) |

Target sibling structure after the split:

```
compiler/ori_arc/src/aims/lattice/
├── mod.rs           # ~50 lines: module doc + mod declarations + pub use re-exports
├── dimensions.rs    # 281 lines (existing, unchanged)
├── state.rs         # NEW ~250 lines: AimsState struct, constants, join, helpers, CHAIN_HEIGHT, iteration_limit
├── canonicalize.rs  # NEW ~200 lines: canonicalize_*, CanonicalizeFeedback, all rules
├── borrow_source.rs # NEW ~70 lines: BorrowSource enum + impl
├── size_class.rs    # NEW ~30 lines: SizeClass newtype
└── tests.rs         # 2365 lines (existing, exempt from 500-line limit per compiler.md)
```

- [ ] Verify the seam list above matches what is in the current `lattice/mod.rs` by re-reading lines 1-552. The line ranges may have shifted slightly during plan creation; treat them as approximate, not authoritative
- [ ] Confirm with the implementer (or via re-read) that the natural type boundaries match the proposed seams. If the file has been touched since plan creation, re-derive the seams from the current source
- [ ] Document any deviation from the proposed seams in the section's `Context` paragraph for traceability
- [ ] Ensure the proposed `mod.rs` will be ≤80 lines after the split (re-exports + module doc only)

---

## 00.2 Execute lattice/mod.rs split

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`, plus four NEW files: `lattice/state.rs`, `lattice/canonicalize.rs`, `lattice/borrow_source.rs`, `lattice/size_class.rs`

**Context:** Mechanical-but-careful execution of the seams from 00.1. The work is structurally simple but requires careful attention to: (a) preserving every test that currently passes, (b) preserving every visibility annotation (`pub`, `pub(crate)`, `pub(super)`), (c) preserving every doc comment exactly, (d) preserving every derive, (e) updating all imports across the codebase to point to the new sibling files where the public API path changes.

- [ ] Create `compiler/ori_arc/src/aims/lattice/state.rs` containing:
  - `AimsState` struct definition (currently `lattice/mod.rs:99-122`)
  - `AimsState::TOP`, `BOTTOM`, `SCALAR`, `FRESH` constants (currently `lattice/mod.rs:124-189`)
  - `AimsState::join` (currently `lattice/mod.rs:200-216`)
  - All `AimsState` predicate methods: `is_scalar`, `is_rc_needed`, `needs_cow_check`, `is_reuse_candidate`, `is_rc_skip_eligible`, `is_local`, `from_arc_class` (currently `lattice/mod.rs:191-199, 379-451`)
  - `AimsState::CHAIN_HEIGHT` constant + `iteration_limit` (currently `lattice/mod.rs:453-479`)
  - All necessary imports (`use super::dimensions::*; use super::canonicalize::*; use super::borrow_source::BorrowSource; use crate::ir::ArcVarId; use crate::ArcClass;`)
  - Module doc explaining: "AimsState — the product lattice element. See `lattice/mod.rs` for the overview, `dimensions.rs` for the per-dimension enums, `canonicalize.rs` for the feasibility-enforcement rules, `borrow_source.rs` for the borrow side table."

- [ ] Create `compiler/ori_arc/src/aims/lattice/canonicalize.rs` containing:
  - `CanonicalizeFeedback` struct + impl (currently `lattice/mod.rs:35-66`)
  - `AimsState::canonicalize` method (currently `lattice/mod.rs:218-239`)
  - `AimsState::canonicalize_with_feedback` method (currently `lattice/mod.rs:241-268`)
  - `AimsState::canonicalize_single_pass` private method (currently `lattice/mod.rs:270-377`) — **including all 5 rules and their cross-dimension chain logic**. NOTE: this is the file Section 02 modifies to add `ArgEscaping` handling.
  - Module doc explaining the lattice rules and their dependencies on each other (Rule 8 must run before Rules 4/6 — preserve this comment from the current location)

- [ ] Create `compiler/ori_arc/src/aims/lattice/borrow_source.rs` containing:
  - `BorrowSource` enum (currently `lattice/mod.rs:482-502`)
  - `BorrowSource::exact`, `exact_field`, `source_var`, `join` methods (currently `lattice/mod.rs:504-552`)
  - Module doc explaining the side-table relationship to AimsState

- [ ] Create `compiler/ori_arc/src/aims/lattice/size_class.rs` containing:
  - `SizeClass` newtype + impl (currently `lattice/mod.rs:68-97`)
  - Brief module doc

- [ ] Rewrite `compiler/ori_arc/src/aims/lattice/mod.rs` to be a dispatch hub only:
  ```rust
  //! Unified ownership lattice for ARC analysis.
  //!
  //! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
  //! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
  //! more dimensions simultaneously.
  //!
  //! Module structure:
  //! - [`dimensions`]    — per-dimension enums (AccessClass, Consumption, ...)
  //! - [`state`]         — `AimsState` product type, constants, join, predicates
  //! - [`canonicalize`]  — feasibility invariant enforcement (Rules 4/6/8 etc.)
  //! - [`borrow_source`] — `BorrowSource` side table for borrow provenance
  //! - [`size_class`]    — `SizeClass` newtype for reuse compatibility
  //!
  //! References: Perceus (PLDI 2021), GHC demand analysis (POPL 2014),
  //! Lean 4 borrow inference (IFL 2019), Linearity ≠ Uniqueness (ESOP 2022),
  //! `OxCaml` (ICFP 2024).

  pub mod borrow_source;
  pub mod canonicalize;
  pub mod dimensions;
  pub mod size_class;
  pub mod state;

  #[cfg(test)]
  #[expect(
      clippy::unwrap_used,
      reason = "tests use unwrap for clearer failure messages"
  )]
  mod tests;

  pub use borrow_source::BorrowSource;
  pub use canonicalize::CanonicalizeFeedback;
  pub use dimensions::*;
  pub use size_class::SizeClass;
  pub use state::AimsState;
  ```
  Verify the resulting file is ≤80 lines.

- [ ] Update imports across the entire codebase that path-import individual symbols from `lattice::mod`. Most imports use the glob `use crate::aims::lattice::*` or named `use crate::aims::lattice::{AimsState, ...}` and will continue to work because of the `pub use` re-exports. Verify by running `cargo check -p ori_arc` after the split.

- [ ] Run `cargo test -p ori_arc` and verify ALL tests pass. The test file `lattice/tests.rs` is unchanged but the symbols it imports may have moved — the re-exports in `mod.rs` should make this transparent.

- [ ] Run `wc -l compiler/ori_arc/src/aims/lattice/*.rs` and verify:
  - `mod.rs` ≤80 lines
  - `state.rs` ≤300 lines
  - `canonicalize.rs` ≤300 lines
  - `borrow_source.rs` ≤100 lines
  - `size_class.rs` ≤100 lines
  - `dimensions.rs` unchanged at 281 lines
  - `tests.rs` unchanged at 2365 lines (exempt from limit)

- [ ] Run `./clippy-all.sh` and verify zero new warnings introduced by the split

---

## 00.3 Define seams for transfer/mod.rs split

**File(s):** `compiler/ori_arc/src/aims/transfer/mod.rs` (currently 524 lines), `compiler/ori_arc/src/aims/transfer/` (target directory for new sibling files)

**Context:** Same approach as 00.1 but for `transfer/mod.rs`. The current file has these natural sections, identified by `grep -n '^pub fn|^fn |^pub struct|^pub enum'`:

| Lines | Content | Natural sibling |
|---|---|---|
| 1-32 | Module doc + imports | (stays in `mod.rs`) |
| 33-69 | `DefTransfer` struct + helper impls | extract to `forward.rs` (the type lives with the functions that produce it) |
| 71-104 | `pub fn transfer_def` (the dispatch entry point) | extract to `forward.rs` |
| 106-232 | Per-instruction transfer helpers: `transfer_let`, `transfer_construct`, `transfer_project`, `transfer_apply_conservative`, `transfer_partial_apply`, `transfer_select`, `transfer_collection_reuse`, `transfer_reuse` | extract to `forward.rs` |
| 234-250 | `pub fn transfer_terminator_def` | extract to `forward.rs` |
| 252-340 | `pub fn backward_demands` | extract to `backward.rs` |
| 342-384 | `pub fn backward_terminator_demands` | extract to `backward.rs` |
| 386-432 | RC decision helpers: `is_rc_dec_unnecessary`, `is_rc_inc_elidable`, `cow_mode_from_uniqueness`, `CowModeFromAims` enum | extract to `rc_decisions.rs` |
| 433-526 | State helpers: `can_mutate_in_place`, `capture_state_update`, `consumed_state`, `shape_from_ctor` | extract to `state_helpers.rs` |

Target sibling structure after the split:

```
compiler/ori_arc/src/aims/transfer/
├── mod.rs            # ~50 lines: module doc + mod declarations + pub use re-exports
├── forward.rs        # NEW ~200 lines: DefTransfer + transfer_def + per-instruction helpers + transfer_terminator_def
├── backward.rs       # NEW ~140 lines: backward_demands + backward_terminator_demands
├── rc_decisions.rs   # NEW ~50 lines: is_rc_*, cow_mode_*, CowModeFromAims
├── state_helpers.rs  # NEW ~95 lines: can_mutate_in_place, capture_state_update, consumed_state, shape_from_ctor
└── tests.rs          # 1117 lines (existing, exempt from limit)
```

- [ ] Verify the seam list above matches the current `transfer/mod.rs` by re-reading the file. Confirm function locations have not shifted.
- [ ] Confirm the proposed seams group functions by **purpose** (forward transfer / backward demand / RC decisions / state construction), not by line count
- [ ] Document any deviation from the proposed seams

---

## 00.4 Execute transfer/mod.rs split

**File(s):** `compiler/ori_arc/src/aims/transfer/mod.rs`, plus four NEW files: `transfer/forward.rs`, `transfer/backward.rs`, `transfer/rc_decisions.rs`, `transfer/state_helpers.rs`

**Context:** Mechanical execution of the seams from 00.3. Same care as 00.2 about visibility, doc comments, derives, and import updates.

- [ ] Create `compiler/ori_arc/src/aims/transfer/forward.rs` containing:
  - `DefTransfer` struct + helper impls (currently `transfer/mod.rs:33-69`)
  - `pub fn transfer_def` (currently `transfer/mod.rs:71-104`)
  - All per-instruction helpers: `transfer_let`, `transfer_construct`, `transfer_project`, `transfer_apply_conservative`, `transfer_partial_apply`, `transfer_select`, `transfer_collection_reuse`, `transfer_reuse` (currently `transfer/mod.rs:106-232`)
  - `pub fn transfer_terminator_def` (currently `transfer/mod.rs:234-250`)
  - All necessary imports

- [ ] Create `compiler/ori_arc/src/aims/transfer/backward.rs` containing:
  - `pub fn backward_demands` (currently `transfer/mod.rs:252-340`)
  - `pub fn backward_terminator_demands` (currently `transfer/mod.rs:342-384`)
  - All necessary imports

- [ ] Create `compiler/ori_arc/src/aims/transfer/rc_decisions.rs` containing:
  - `pub fn is_rc_dec_unnecessary` (currently `transfer/mod.rs:386-393`)
  - `pub fn is_rc_inc_elidable` (currently `transfer/mod.rs:394-402`)
  - `pub fn cow_mode_from_uniqueness` (currently `transfer/mod.rs:403-416`)
  - `pub enum CowModeFromAims` (currently `transfer/mod.rs:417-432`)
  - All necessary imports

- [ ] Create `compiler/ori_arc/src/aims/transfer/state_helpers.rs` containing:
  - `pub fn can_mutate_in_place` (currently `transfer/mod.rs:433-454`)
  - `pub fn capture_state_update` (currently `transfer/mod.rs:455-497`)
  - `pub fn consumed_state` (currently `transfer/mod.rs:498-514`)
  - `fn shape_from_ctor` (private helper, currently `transfer/mod.rs:515-526`)
  - All necessary imports

- [ ] Rewrite `compiler/ori_arc/src/aims/transfer/mod.rs` to be a dispatch hub only:
  ```rust
  //! Transfer functions for the AIMS lattice.
  //!
  //! Each ARC IR instruction has a transfer function that defines how it
  //! transforms the [`AimsState`] of variables it touches:
  //!
  //! - **Forward (definition)**: what state does the destination variable get?
  //! - **Backward (demand)**: what cardinality demand does each use add?
  //!
  //! The dataflow analysis engine applies these functions in its worklist
  //! iteration. This module defines only the mathematical rules.
  //!
  //! Module structure:
  //! - [`forward`]       — `DefTransfer`, `transfer_def`, per-instruction helpers
  //! - [`backward`]      — `backward_demands`, `backward_terminator_demands`
  //! - [`rc_decisions`]  — RC elision predicates and COW mode mapping
  //! - [`state_helpers`] — state construction utilities
  //!
  //! References:
  //! - Perceus dup/drop placement (Reinking et al., PLDI 2021)
  //! - GHC demand analysis `seq_add`/`alt_join` (Sergey et al., POPL 2014)
  //! - Lean 4 `updateLiveVars` / `addInc` / `addDec` (Ullrich & de Moura, IFL 2019)

  pub mod backward;
  pub mod forward;
  pub mod rc_decisions;
  pub mod state_helpers;

  #[cfg(test)]
  #[expect(
      clippy::expect_used,
      reason = "tests use expect for clearer failure messages"
  )]
  mod tests;

  pub use backward::{backward_demands, backward_terminator_demands};
  pub use forward::{transfer_def, transfer_terminator_def, DefTransfer};
  pub use rc_decisions::{cow_mode_from_uniqueness, is_rc_dec_unnecessary, is_rc_inc_elidable, CowModeFromAims};
  pub use state_helpers::{can_mutate_in_place, capture_state_update, consumed_state};
  ```
  Verify the resulting file is ≤80 lines.

- [ ] Run `cargo test -p ori_arc` and verify ALL tests pass.

- [ ] Run `wc -l compiler/ori_arc/src/aims/transfer/*.rs` and verify each file is under its target.

- [ ] Run `./clippy-all.sh` and verify zero new warnings.

- [ ] **TPR checkpoint** — `/tpr-review` covering 00.1–00.4 (file split work). Codex sanity-checks that the chosen seams reflect actual concerns rather than mechanical chunking, and that no symbols, doc comments, or derive annotations were lost in the move.
  <!-- Per .claude/skills/create-plan/plan-schema.md TPR cadence: 4 implementation
       subsections is at the lower edge of "3+", but the file splits are the
       riskiest semantic-preserving operation in the section. Catching seam mistakes
       here is much cheaper than catching them after Section 02 has piled on. -->

---

## 00.5 Rewrite stale Section 09.x annotations in lattice files

**File(s):** `compiler/ori_arc/src/aims/lattice/canonicalize.rs` (created in 00.2), `compiler/ori_arc/src/aims/lattice/state.rs` (created in 00.2), and any other lattice sibling files containing the strings `Section 09.2`, `Section 09.3`, or `Section 09.5`

**Context:** Phase 2 research found 17 references to `Section 09.x` in the original `lattice/mod.rs` (line counts as of plan creation: 8, 9, 10, 35, 37, 40, 120, 180, 224, 280, 283, 284, 286, 324, 336, 350, 357, 370, 420). After the split in 00.2, these references now live in `state.rs`, `canonicalize.rs`, and the new `mod.rs` (in module doc comments).

The annotations were verified stale: the phrase "Effect Activation" — which the annotations cite as the originating plan section — appears in 5 source files in `compiler/ori_arc/src/aims/` but in **zero** plan files anywhere in `plans/` or `plans/completed/`. The originating plan was either renamed during a restructuring, deleted without cleanup, or never tracked formally.

The cleanup must **rewrite, not delete**. The annotations document load-bearing design rationale (e.g., "Rule 4 requires precise locality (Section 09.2)" — the "precise locality" claim is essential for the rule's soundness). Naive deletion would leave dangling parentheticals like "requires precise locality ()". The rewrite preserves the rationale while dropping the broken navigation pointer.

**Rewrite recipe:**

| Pattern | Replacement |
|---|---|
| `(Section 09.2)` | (delete the parenthetical entirely) |
| `(Section 09.3)` | (delete the parenthetical entirely) |
| `(Section 09.2/09.3)` | (delete the parenthetical entirely) |
| `(Section 09.5 Convergence Feedback)` | (delete the parenthetical, but keep the words "convergence feedback" if they appear in surrounding prose) |
| `since Section 09.2` | `since the lattice activation work` |
| `since Section 09.2 Effect Activation` | `since effect tracking was activated` |
| `since Section 09.2 Shape Activation` | `since shape tracking was activated` |
| `Section 09.2: precise locality computation` | `precise locality computation` |
| `Rule 4 (Section 09.2/09.3): BlockLocal + Owned + ≤Once → Unique` | `Rule 4: BlockLocal + Owned + ≤Once → Unique` |
| `Soundness: this rule requires precise locality (Section 09.2)` | `Soundness: this rule requires precise locality` |

- [ ] Re-verify the count and locations of stale annotations after 00.2's split. The 17 occurrences from the original `lattice/mod.rs` should now be distributed across the new sibling files. Use `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/lattice/` to enumerate.

- [ ] Apply the rewrite recipe to every occurrence in `lattice/canonicalize.rs`, `lattice/state.rs`, and the new `lattice/mod.rs`. Each rewrite must:
  1. Preserve the load-bearing rationale (the rule's soundness reason)
  2. Drop only the navigation pointer to the missing plan section
  3. Leave no dangling parenthetical, broken sentence, or orphan word

- [ ] After the rewrite, run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/lattice/` and verify it returns zero results

- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and verify no new doc warnings (broken `[link]` references, etc.) introduced by the rewrites

- [ ] Run `cargo test -p ori_arc` and verify all tests still pass (the rewrites are comments, so behavior is unchanged — but verify regardless)

---

## 00.6 Rewrite stale annotations in 4 other AIMS files

**File(s):** Per Pass 1 Agent 2's grep finding, 5 files in `compiler/ori_arc/src/aims/` contain "Effect Activation" annotations. After 00.5 cleans up the lattice files (which is one of those 5), 4 files remain:
- `compiler/ori_arc/src/aims/contract/mod.rs`
- `compiler/ori_arc/src/aims/interprocedural/tests.rs`
- `compiler/ori_arc/src/aims/intraprocedural/fip_balance.rs`
- `compiler/ori_arc/src/aims/intraprocedural/state_map.rs`

**Context:** Same rewrite discipline as 00.5. The annotations in these files cite the same missing plan section ("Effect Activation"), so they are stale by the same reasoning. Each file's annotations may differ in count and surrounding context — the rewrite must be done file-by-file, not via blind sed.

- [ ] Run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` and enumerate every remaining occurrence after 00.5
- [ ] For each occurrence, apply the rewrite recipe from 00.5
- [ ] Pay attention to occurrences in **test files** (`interprocedural/tests.rs`) — test files often have annotations like `// Tests for Section 09.2 Effect Activation behavior`. These should be rewritten to describe the *behavior being tested*, not the originating plan section. Example: `// Tests for effect activation behavior` or `// Tests for canonicalize Rule 4 firing`.
- [ ] After the rewrite, run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` and verify zero results across all of `aims/`
- [ ] Run `cargo test -p ori_arc` and verify all tests pass

---

## 00.7 Safety net — /add-bug for any residual stale annotations

**File(s):** None (this subsection is a verification scan that may produce a `/add-bug` invocation)

**Context:** Sections 00.5 and 00.6 cleaned up the known stale annotations in `compiler/ori_arc/src/aims/`. Codex Step 6B Finding 2 added: *"if any stale annotation remains outside the final touched set after the split, file `/add-bug` for that remainder immediately rather than silently leaving residue."* The compiler is large, and stale annotations may exist outside `aims/` that this plan cannot reasonably absorb (e.g., in `ori_llvm`, `ori_eval`, etc.).

The right move is to **scan once** for residuals across the broader compiler tree, and **either fix them in this plan or file a bug** so they don't get lost.

- [ ] Run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` (no plan filter) to scan the entire compiler for stale plan annotations
- [ ] If the scan returns any residuals OUTSIDE `compiler/ori_arc/src/aims/`:
  - **If the residual is in a file this plan does not otherwise touch and is small (1-2 references)**: file `/add-bug` with: subsystem = the affected crate, severity = `low`, source = `continue-roadmap` (this plan), title = "Stale Section 09.x plan annotation in <file>" with the exact line number and context
  - **If the residual is large (10+ references in a single file)**: file `/add-bug` with severity = `medium` and a note that the cleanup may warrant its own micro-plan
- [ ] If the scan returns no residuals outside `aims/`, document this in the section's completion notes ("Section 00.7 scan: zero residuals outside `aims/`, no `/add-bug` filed")
- [ ] Verify `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale references in the files this plan touched

---

## 00.R Third Party Review Findings

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

## 00.N Completion Checklist

- [ ] `wc -l compiler/ori_arc/src/aims/lattice/mod.rs` returns ≤80 lines
- [ ] `wc -l compiler/ori_arc/src/aims/lattice/state.rs` returns ≤300 lines
- [ ] `wc -l compiler/ori_arc/src/aims/lattice/canonicalize.rs` returns ≤300 lines
- [ ] `wc -l compiler/ori_arc/src/aims/lattice/borrow_source.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/lattice/size_class.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/transfer/mod.rs` returns ≤80 lines
- [ ] `wc -l compiler/ori_arc/src/aims/transfer/forward.rs` returns ≤250 lines
- [ ] `wc -l compiler/ori_arc/src/aims/transfer/backward.rs` returns ≤200 lines
- [ ] `wc -l compiler/ori_arc/src/aims/transfer/rc_decisions.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/transfer/state_helpers.rs` returns ≤150 lines
- [ ] `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns zero results
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale Section 09 references in this plan's touched files
- [ ] `cargo test -p ori_arc` green
- [ ] `timeout 150 ./test-all.sh` green — zero behavioral regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] No new doc warnings introduced (`cargo doc -p ori_arc 2>&1 | grep warning` returns no new entries)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 annotations — all temporary scaffolding (TPR, CROSS, BUG, §, Phase, section- refs) removed from `.rs` files
- [ ] All intermediate TPR checkpoint findings resolved (the 00.4 TPR checkpoint above)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 00
  - [ ] `00-overview.md` mission success criteria checkboxes updated (check off the file-size and stale-annotation criteria now satisfied)
  - [ ] `index.md` Section 00 status updated
  - [ ] Cross-links to other plans updated if this section resolved external blockers (none expected for this section)
  - [ ] Section 01's `depends_on` verified — Section 01 has `depends_on: ["00"]` already
- [ ] `/tpr-review` passed (final, full-section) — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review found no critical or major findings (or all findings triaged and fixed). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey: which `diagnostics/` scripts were run during the file split work? Where did `cargo check`/`cargo test` output reveal that an import had moved unexpectedly? Was there a missing helper for "compare two `git diff`s of a split file to verify byte-equivalent move"? Where did `wc -l` output need post-processing to find files over a threshold? Implement every accepted improvement NOW (zero deferral) and commit each as a SEPARATE `/commit-push` from the section's implementation work, e.g. `tools(diagnostics): add file-size-by-threshold helper to scripts/check-file-sizes.sh — surfaced by section-00 retrospective`. Verify each improvement actually solves the original friction. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. If genuinely no gaps, document briefly: "Retrospective: no tooling gaps — relied on existing wc -l, cargo check, plan-annotations.sh".

**Exit Criteria:** `wc -l compiler/ori_arc/src/aims/lattice/*.rs compiler/ori_arc/src/aims/transfer/*.rs` shows every non-test file ≤300 lines. `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns zero results. `cargo test -p ori_arc` passes the same test count as before this section started (no tests added or removed — this is purely structural cleanup). `timeout 150 ./test-all.sh` and `timeout 150 ./clippy-all.sh` are both green. The lattice and transfer modules now have a clean architectural seam structure that Section 02 can write into without immediately violating the 500-line limit.
