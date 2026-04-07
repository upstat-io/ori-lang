---
section: "00"
title: "Hygiene Foundation"
status: not-started
reviewed: true
goal: "Restructure five BLOAT files (lattice/mod.rs 552, transfer/mod.rs 524, interprocedural/extract.rs 517, intraprocedural/state_map.rs 646, interprocedural/mod.rs 536) to live below the 450-line proactive-split threshold and eliminate 99 stale Section 09.x plan annotations across the AIMS subtree, with zero semantic change to any analysis behavior"
success_criteria:
  - "compiler/ori_arc/src/aims/lattice/mod.rs is ≤80 lines (becomes a dispatch hub: module doc + mod declarations + re-exports only)"
  - "Every sibling file under compiler/ori_arc/src/aims/lattice/ is ≤300 lines"
  - "compiler/ori_arc/src/aims/transfer/mod.rs is ≤80 lines (becomes a dispatch hub)"
  - "Every sibling file under compiler/ori_arc/src/aims/transfer/ is ≤250 lines"
  - "compiler/ori_arc/src/aims/interprocedural/extract.rs is ≤250 lines (becomes a dispatch hub for the contract-extraction algorithm; the heavy helpers move to sibling files)"
  - "compiler/ori_arc/src/aims/interprocedural/extract/consumed_params.rs is ≤200 lines (NEW: detect_consumed_params + alias propagation)"
  - "compiler/ori_arc/src/aims/interprocedural/extract/return_info.rs is ≤250 lines (NEW: extract_return_info + var_uniqueness + def maps)"
  - "compiler/ori_arc/src/aims/intraprocedural/state_map.rs is ≤450 lines (currently 646; Section 00.6 rewrites 14 stale annotations in this file, triggering the touching-bloat rule — Section 00.4c pre-splits it into impl-block siblings)"
  - "compiler/ori_arc/src/aims/interprocedural/mod.rs is ≤450 lines (currently 536; Section 00.6 rewrites 2 stale annotations in this file — Section 00.4c pre-splits the SCC fixpoint loop into a sibling file)"
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
  - id: "00.4a"
    title: "Define seams for interprocedural/extract.rs split"
    status: not-started
  - id: "00.4b"
    title: "Execute interprocedural/extract.rs split"
    status: not-started
  - id: "00.4c"
    title: "Pre-split BLOAT files state_map.rs (646 lines) and interprocedural/mod.rs (536 lines)"
    status: not-started
  - id: "00.5"
    title: "Rewrite stale Section 09.x annotations in lattice files"
    status: not-started
  - id: "00.6"
    title: "Rewrite stale annotations in the remaining 21 AIMS files (99 annotations total across 22 files; 00.5 covers ~19 lattice ones, 00.6 covers ~80 across the other 21)"
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
# TPR Checkpoint after 00.4b (all 3 file splits done, before annotation work begins)
---

# Section 00: Hygiene Foundation

**Status:** Not Started
**Goal:** Restructure **five BLOAT files** (`lattice/mod.rs` 552, `transfer/mod.rs` 524, `interprocedural/extract.rs` 517, `intraprocedural/state_map.rs` 646, `interprocedural/mod.rs` 536) to live below the 450-line proactive-split threshold, AND eliminate **99 stale `Section 09.x` plan annotations across 22 files** in `compiler/ori_arc/src/aims/`, with **zero semantic change** to any analysis behavior. Every test passing before this section must pass after, byte-identically. The section creates the substrate that Sections 02 and 05 will write into.

**Success Criteria:**

- [ ] `compiler/ori_arc/src/aims/lattice/mod.rs` is ≤80 lines (becomes a dispatch hub: module doc + `mod` declarations + `pub use` re-exports only) — verified by `wc -l`
- [ ] Every sibling file under `compiler/ori_arc/src/aims/lattice/` is ≤300 lines — verified by `wc -l compiler/ori_arc/src/aims/lattice/*.rs | awk '{if ($1 > 300 && $2 != "tests.rs") print $0}'` returning empty
- [ ] `compiler/ori_arc/src/aims/transfer/mod.rs` is ≤80 lines (becomes a dispatch hub) — verified by `wc -l`
- [ ] Every sibling file under `compiler/ori_arc/src/aims/transfer/` is ≤250 lines — verified the same way
- [ ] Every file under `compiler/ori_arc/src/aims/interprocedural/extract/` is ≤250 lines (post-split: `mod.rs`, `consumed_params.rs`, `return_info.rs`) — verified by `wc -l compiler/ori_arc/src/aims/interprocedural/extract/*.rs | awk '{if ($1 > 250 && $2 != "tests.rs") print $0}'` returning empty
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale `Section 09` references in `compiler/ori_arc/src/aims/` files
- [ ] `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns only spec references (`Spec: Clause N.M`) if any, never plan-section navigation pointers

- [ ] **Public API surface preserved by all FIVE splits** (lattice, transfer, interprocedural/extract, intraprocedural/state_map, interprocedural/mod). PRIMARY verification (always available): `rg '^pub (use|mod|fn|struct|enum|trait|const)' compiler/ori_arc/src/aims/lattice/ compiler/ori_arc/src/aims/transfer/ compiler/ori_arc/src/aims/interprocedural/ compiler/ori_arc/src/aims/intraprocedural/state_map/` shows ZERO new public paths after the splits. Submodules introduced by 00.2 (`state`, `canonicalize`, `borrow_source`, `size_class`), 00.4 (`forward`, `backward`, `rc_decisions`, `state_helpers`), 00.4b (`consumed_params`, `return_info`), and 00.4c (`events`, `cross_dim`, `borrow_provenance`, `invoke_shape`, `effects_fip`, `scc_loop`) are PRIVATE (`mod`, not `pub mod`) — only `pub use` (or `pub(super) use` / `pub(crate) use` for the extract submodules) re-exports reach external callers. OPTIONAL upgrade (only if installed in the future): `cargo public-api -p ori_arc` provides a stronger contract by showing the full Rust public API. **Note:** the grep fallback is a weaker textual proxy — it catches the common `pub mod foo;` accident but does not enforce a stable public API contract. Once `cargo public-api` is added to the repo's dev tooling, this verification should be upgraded to use it as primary and the grep as a sanity check.
- [ ] `./test-all.sh` green — zero behavioral regressions (no semantic changes were made)
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_arc` green
- [ ] Connects upward to mission criteria: "lattice/mod.rs ≤450 lines", "transfer/mod.rs ≤450 lines", "interprocedural/extract.rs ≤450 lines", "intraprocedural/state_map.rs ≤450 lines (Agent 3)", "interprocedural/mod.rs ≤450 lines (Agent 3)", "no stale Section 09.x annotations in AIMS files"

**Context:** Phase 2 research, the pre-Agent-3 third-party review, and Agent 3's codebase scan surfaced **multiple** foundational obstacles for the rest of this plan. The plan touches **five** BLOAT files (all over the 500-line limit) that must be pre-split before any other section runs:

| File | Lines | Over limit by | Why this plan touches it | Pre-split subsection |
|---|---|---|---|---|
| `compiler/ori_arc/src/aims/lattice/mod.rs` | 552 | 52 | Section 02 adds the `ArgEscaping` variant + canonicalize verification + lattice tests | 00.1, 00.2 |
| `compiler/ori_arc/src/aims/transfer/mod.rs` | 524 | 24 | Section 02 reads transfer functions; Section 00.6 rewrites 3 stale annotations | 00.3, 00.4 |
| `compiler/ori_arc/src/aims/interprocedural/extract.rs` | 517 | 17 | Section 02.6 modifies the `ParamContract { ..., may_escape: false, ... }` literal at lines 79-92 | 00.4a, 00.4b (Agent 3 addition) |
| `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` | 646 | 146 | Section 02.5 reads predicate sites at lines 429-440; Section 00.6 rewrites 14 stale annotations | 00.4c (Agent 3 addition) |
| `compiler/ori_arc/src/aims/interprocedural/mod.rs` | 536 | 36 | Section 00.6 rewrites 2 stale annotations in this file | 00.4c (Agent 3 addition) |

Per `impl-hygiene.md` §File Organization: *"Proactive split: split at ~450 lines if you know more code is coming. Don't wait until over the limit. Touching a file over 500 lines without splitting is a finding."* All five files are pre-split in subsections 00.1–00.4c BEFORE Sections 00.5/00.6 (annotation rewrites) or Section 02 (semantic implementation) write to them.

Second, `compiler/ori_arc/src/aims/` contains **99 stale `Section 09.x` plan-navigation annotations across 22 files** (counted via `Grep "Section 09\." compiler/ori_arc/src/aims/` during this plan's accuracy review). The distribution is concentrated:

| File | Count |
|---|---|
| `lattice/mod.rs` | 19 |
| `intraprocedural/block.rs` | 17 |
| `intraprocedural/state_map.rs` | 14 |
| `intraprocedural/tests.rs` | 14 |
| `intraprocedural/mod.rs` | 4 |
| `contract/mod.rs`, `interprocedural/{tests,extract}.rs`, `lattice/tests.rs`, `transfer/mod.rs` | 3 each |
| `interprocedural/mod.rs`, `intraprocedural/{effects,fip_balance,post_convergence}.rs`, `transfer/tests.rs` | 2 each |
| `contract/context.rs`, `realize/{decide,tests}.rs`, `emit_reuse/{detect,mod}.rs`, `normalize/mod.rs`, `intraprocedural/state_map/tests.rs` | 1 each |

The phrase "Effect Activation" (which the annotations cite as the originating plan section) appears in many source files in `compiler/ori_arc/src/aims/` but in **zero** plan files anywhere in `plans/` or `plans/completed/`. The originating plan was either renamed during a restructuring, deleted without cleanup, or never tracked formally. Per `impl-hygiene.md` §Comments: *"Plan annotations are temporary scaffolding... They MUST be removed when the plan completes... Stale annotations from completed plans are hygiene violations (DRIFT category)."*

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

- [ ] Rewrite `compiler/ori_arc/src/aims/lattice/mod.rs` to be a dispatch hub only.

  ```rust
  //! Unified ownership lattice for ARC analysis.
  //!
  //! [`AimsState`] is a product of seven dimensions, each a small finite lattice.
  //! Join is componentwise. Transfer functions (in `transfer.rs`) update one or
  //! more dimensions simultaneously.
  //!
  //! Module structure (internal — consumers import via the re-exports below):
  //! - `dimensions`    — per-dimension enums (AccessClass, Consumption, ...)
  //! - `state`         — `AimsState` product type, constants, join, predicates
  //! - `canonicalize`  — feasibility invariant enforcement (Rules 4/6/8 etc.)
  //! - `borrow_source` — `BorrowSource` side table for borrow provenance
  //! - `size_class`    — `SizeClass` newtype for reuse compatibility
  //!
  //! References: Perceus (PLDI 2021), GHC demand analysis (POPL 2014),
  //! Lean 4 borrow inference (IFL 2019), Linearity ≠ Uniqueness (ESOP 2022),
  //! `OxCaml` (ICFP 2024).

  // PRIVATE submodules — internal organization, NOT public API.
  // Consumers MUST use the re-exports below; never import via the submodule path.
  mod borrow_source;
  mod canonicalize;
  pub mod dimensions; // currently `pub mod dimensions` — preserved unchanged
  mod size_class;
  mod state;

  #[cfg(test)]
  #[expect(
      clippy::unwrap_used,
      reason = "tests use unwrap for clearer failure messages"
  )]
  mod tests;

  // Re-exports — these define the public API surface of `ori_arc::aims::lattice`.
  // External callers see `ori_arc::aims::lattice::AimsState`, NOT
  // `ori_arc::aims::lattice::state::AimsState`. Adding a second path would be
  // a silent public-API widening (impl-hygiene.md violation).
  pub use borrow_source::BorrowSource;
  pub use canonicalize::CanonicalizeFeedback;
  pub use dimensions::*;
  pub use size_class::SizeClass;
  pub use state::AimsState;
  ```
  Verify the resulting file is ≤80 lines.

  **Note on `dimensions`**: the existing pre-split `lattice/mod.rs` already declares
  `pub mod dimensions;` (line 22 of the current file). Preserve that visibility for
  backward compatibility — `dimensions::*` is already part of the public API surface
  via the existing glob re-export. The OTHER new submodules (`state`, `canonicalize`,
  `borrow_source`, `size_class`) are NEW and MUST be private — only their symbols
  go through `pub use`.

- [ ] **Public API surface verification — load-bearing check.** Before and after the
  split, capture the lattice public API surface and diff it. The split must be a pure
  internal reorganization with byte-identical external paths.

  ```bash
  # PRIMARY: hand-rolled grep (always available, runs in any environment)
  rg -n '^pub (use|mod|fn|struct|enum|trait|const)' \
     compiler/ori_arc/src/aims/lattice/ > /tmp/api-before.txt
  # ... do the split ...
  rg -n '^pub (use|mod|fn|struct|enum|trait|const)' \
     compiler/ori_arc/src/aims/lattice/ > /tmp/api-after.txt
  diff /tmp/api-before.txt /tmp/api-after.txt
  # Expected: only line-number/file-path changes due to the move.
  # No NEW pub paths (e.g., `state::AimsState` appearing as a second path).

  # OPTIONAL upgrade (only if cargo public-api is installed):
  # cargo public-api --version  # if this works, run the strong verification:
  cargo public-api -p ori_arc > /tmp/api-before.txt 2>/dev/null || true
  # ... do the split ...
  cargo public-api -p ori_arc > /tmp/api-after.txt 2>/dev/null || true
  diff /tmp/api-before.txt /tmp/api-after.txt
  # Expected diff: ZERO lines. Any addition or removal is a regression.
  ```

  **Note on tool strength:** the grep fallback is a weaker textual proxy — it
  catches the common `pub mod foo;` accident but does not enforce a stable public
  API contract (e.g., it cannot distinguish between two `pub use` paths reaching
  the same symbol vs adding a second name). Once `cargo public-api` is added to
  the repo's dev tooling, upgrade this verification to use it as primary.

  **Acceptance criterion**: every external symbol path that worked before the split
  works after the split, AND no new external paths exist. If `ori_arc::aims::lattice::state::AimsState`
  is reachable from outside the `lattice` module after the split, the `mod state;`
  visibility was wrongly set to `pub mod state;` — fix it.

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

- [ ] Rewrite `compiler/ori_arc/src/aims/transfer/mod.rs` to be a dispatch hub only.

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
  //! Module structure (internal — consumers import via the re-exports below):
  //! - `forward`       — `DefTransfer`, `transfer_def`, per-instruction helpers
  //! - `backward`      — `backward_demands`, `backward_terminator_demands`
  //! - `rc_decisions`  — RC elision predicates and COW mode mapping
  //! - `state_helpers` — state construction utilities
  //!
  //! References:
  //! - Perceus dup/drop placement (Reinking et al., PLDI 2021)
  //! - GHC demand analysis `seq_add`/`alt_join` (Sergey et al., POPL 2014)
  //! - Lean 4 `updateLiveVars` / `addInc` / `addDec` (Ullrich & de Moura, IFL 2019)

  // PRIVATE submodules — internal organization, NOT public API.
  mod backward;
  mod forward;
  mod rc_decisions;
  mod state_helpers;

  #[cfg(test)]
  #[expect(
      clippy::expect_used,
      reason = "tests use expect for clearer failure messages"
  )]
  mod tests;

  // Re-exports define the public API surface. External callers see
  // `ori_arc::aims::transfer::transfer_def`, NOT
  // `ori_arc::aims::transfer::forward::transfer_def`. Adding a second path
  // would be a silent public API widening.
  pub use backward::{backward_demands, backward_terminator_demands};
  pub use forward::{transfer_def, transfer_terminator_def, DefTransfer};
  pub use rc_decisions::{cow_mode_from_uniqueness, is_rc_dec_unnecessary, is_rc_inc_elidable, CowModeFromAims};
  pub use state_helpers::{can_mutate_in_place, capture_state_update, consumed_state};
  ```
  Verify the resulting file is ≤80 lines.

- [ ] **Public API surface verification (transfer)** — same recipe as 00.2's verification step.
  Capture the transfer public surface with the PRIMARY grep approach
  (`rg '^pub (use|mod|fn|struct|enum|trait|const)' compiler/ori_arc/src/aims/transfer/`)
  before and after the split. The diff must show ZERO new public paths. If
  `ori_arc::aims::transfer::forward::transfer_def` is reachable, `mod forward;` was
  wrongly written as `pub mod forward;` — fix it before proceeding. The OPTIONAL
  `cargo public-api -p ori_arc` upgrade is only available if the tool is installed
  (it is not installed in this repo as of plan creation; see 00.2 for details).

- [ ] Run `cargo test -p ori_arc` and verify ALL tests pass.

- [ ] Run `wc -l compiler/ori_arc/src/aims/transfer/*.rs` and verify each file is under its target.

- [ ] Run `./clippy-all.sh` and verify zero new warnings.

---

## 00.4a Define seams for interprocedural/extract.rs split

**File(s):** `compiler/ori_arc/src/aims/interprocedural/extract.rs` (currently 517 lines — over the 500-line limit), `compiler/ori_arc/src/aims/interprocedural/extract/` (target directory for new sibling files; the split converts the leaf file into a directory)

**Context:** Independent third-party review (codex pre-Agent-3) flagged that `extract.rs` is 517 lines and Section 02.6 modifies the `ParamContract` literal at lines 79-92 (one of the 7 struct literal sites Pass 1 enumerated). Per `compiler.md` §File Size: *"500 line limit (excl. tests). Stop and split before exceeding."* Per `impl-hygiene.md` §File Organization: *"Touching a file over 500 lines without splitting is a finding."* Section 00 must absorb this split into its hygiene scope so Section 02 can write into post-split files instead of compounding the BLOAT.

The current `extract.rs` has three natural responsibilities, identified by re-reading the file:

| Lines | Content | Natural sibling |
|---|---|---|
| 1-19 | Module doc + imports | (stays in `mod.rs`) |
| 21-177 | `extract_contract` (the orchestrator entry point — builds `param_vars`, calls helpers, assembles `MemoryContract`, computes `fip` decision) | (stays in `mod.rs` — this is the dispatch hub) |
| 179-224 | `compute_context_behavior` (TRMC context computation — Section 13.1) | (stays in `mod.rs` — small, tightly coupled to `extract_contract`'s `MemoryContract` assembly; ~46 lines) |
| 226-344 | `detect_consumed_params` (alias propagation across `Let{Var}` and `Jump`/`Branch` block-param passing, then Apply/Invoke/Return scanning to find which parameters reach an `Owned` callee position) | extract to `consumed_params.rs` (~119 lines, self-contained algorithm) |
| 346-517 | `extract_return_info` + `var_uniqueness` + `callee_return_uniqueness` + `build_definition_map` + `build_invoke_def_map` (return-uniqueness analysis: walks Return terminators, traces each returned variable through Let/Apply/Construct/Reuse/etc. to determine `Uniqueness`) | extract to `return_info.rs` (~172 lines, self-contained algorithm) |

**Target sibling structure after the split:**

```
compiler/ori_arc/src/aims/interprocedural/extract/
├── mod.rs              # ~225 lines: module doc + imports + extract_contract + compute_context_behavior
├── consumed_params.rs  # NEW ~125 lines: detect_consumed_params + alias propagation
└── return_info.rs      # NEW ~180 lines: extract_return_info + var_uniqueness + callee_return_uniqueness + def map builders
```

**Why these seams (not other plausible cuts):**

- `extract_contract` and `compute_context_behavior` are the *only* functions in this file that build a `MemoryContract` (the entire file's purpose). Keeping them together preserves the dispatch-hub pattern from `compiler.md` §Module Roles ("`mod.rs` dispatches: routes to submodules").
- `detect_consumed_params` and `extract_return_info` are pure helpers — `extract_contract` calls each exactly once and forwards no shared state. They have zero coupling to each other (they don't share helpers, don't transitively call each other, don't share imports beyond the IR types). Splitting them into separate files is safe.
- The `build_definition_map` and `build_invoke_def_map` helpers are private to `extract_return_info` and live in the same file as their only caller — they move with `return_info.rs`.
- Splitting `extract_contract` itself would violate the "single responsibility per file" rule (the orchestration logic IS the responsibility of `extract.rs`, even after the split).

- [ ] Re-read `compiler/ori_arc/src/aims/interprocedural/extract.rs` end-to-end and confirm the seam list above matches the current file structure (line numbers may have drifted; re-derive from the current source)
- [ ] Confirm `extract_contract` and `compute_context_behavior` together are ≤250 lines after the split (the section-00 target). If they exceed 250 lines, the seam choice is wrong and another helper must be extracted.
- [ ] Confirm `detect_consumed_params` is self-contained: it imports nothing from `extract_return_info` or its helpers. Verify by `grep -n 'def_map\|invoke_defs\|var_uniqueness\|callee_return_uniqueness' compiler/ori_arc/src/aims/interprocedural/extract.rs | head -50` and inspecting `detect_consumed_params`'s body — it should reference none of those names.
- [ ] Confirm `extract_return_info` and its helpers (`var_uniqueness`, `callee_return_uniqueness`, `build_definition_map`, `build_invoke_def_map`) are self-contained: they import nothing from `detect_consumed_params`. Verify the same way in reverse.
- [ ] Document any deviation from the proposed seams in the section's `Context` paragraph for traceability

---

## 00.4b Execute interprocedural/extract.rs split

**File(s):** Convert `compiler/ori_arc/src/aims/interprocedural/extract.rs` (leaf file) into `compiler/ori_arc/src/aims/interprocedural/extract/` (directory) with `mod.rs`, `consumed_params.rs`, and `return_info.rs`. This is a leaf-to-directory promotion — the parent `interprocedural/mod.rs` already declares `pub(crate) mod extract;` and that declaration is unchanged (Rust resolves `mod extract;` to either `extract.rs` or `extract/mod.rs`).

**Context:** Mechanical execution of the seams from 00.4a. Same care as 00.2 and 00.4 about visibility, doc comments, derives, and import updates. The split is **leaf-to-directory** (different from 00.2/00.4 which were monolith-to-siblings within an existing directory) — verify Rust resolves the new layout the same way as the old layout.

- [ ] Create the directory `compiler/ori_arc/src/aims/interprocedural/extract/`
- [ ] Move `extract.rs` content into `compiler/ori_arc/src/aims/interprocedural/extract/mod.rs`, retaining ONLY:
  - Module doc (`//!` block at lines 1-5)
  - Imports — but split into three groups: (a) external (`ori_ir::Name`, `rustc_hash::*`), (b) crate (`crate::ir::*`, `crate::tail_call::*`, `crate::ArcClassification`), (c) relative (`super::super::contract::*`, `super::super::intraprocedural::*`, `super::super::lattice::*`)
  - `pub(crate) fn extract_contract` (currently lines 21-177)
  - `fn compute_context_behavior` (currently lines 179-224, including its doc comment)
  - Two `mod` declarations + their `pub(super) use` re-exports (see below)
- [ ] Add to `extract/mod.rs` after the imports:
  ```rust
  // PRIVATE submodules — internal organization, NOT public API.
  // The only external entry point is `extract_contract` above; the helpers
  // are private to the extract module.
  mod consumed_params;
  mod return_info;

  use consumed_params::detect_consumed_params;
  use return_info::extract_return_info;
  ```
- [ ] Create `compiler/ori_arc/src/aims/interprocedural/extract/consumed_params.rs` containing:
  - Module doc: `//! Parameter consumption detection: which parameters flow (possibly through Let aliases or block-parameter passing) to a callee that consumes them at an Owned position. Ref: Lean 4 src/Lean/Compiler/IR/Borrow.lean — returned params are always Owned.`
  - Necessary imports (`use ori_ir::Name; use rustc_hash::{FxHashMap, FxHashSet}; use crate::ir::{ArcInstr, ArcFunction, ArcTerminator, ArcVarId}; use super::super::super::contract::MemoryContract; use super::super::super::lattice::AccessClass;`)
  - `pub(super) fn detect_consumed_params` (currently `extract.rs:226-344`, including the full doc comment and the Lean 4 reference)
  - **Visibility**: `pub(super)` — only the parent `extract` module needs to call it
- [ ] Create `compiler/ori_arc/src/aims/interprocedural/extract/return_info.rs` containing:
  - Module doc: `//! Return uniqueness analysis: walks Return terminators, traces each returned variable to its definition through Let/Apply/Construct/Reuse/etc., and joins the results across all return paths.`
  - Necessary imports (`use ori_ir::Name; use rustc_hash::{FxHashMap, FxHashSet}; use crate::ir::{ArcInstr, ArcFunction, ArcTerminator, ArcVarId}; use crate::ArcClassification; use super::super::super::contract::{MemoryContract, ReturnContract}; use super::super::super::lattice::Uniqueness;`)
  - `pub(super) fn extract_return_info` (currently `extract.rs:353-402`, including the full doc comment)
  - `fn var_uniqueness` (private helper, currently `extract.rs:407-468`)
  - `fn callee_return_uniqueness` (private helper, currently `extract.rs:471-483`)
  - `fn build_definition_map` (private helper, currently `extract.rs:490-500`)
  - `fn build_invoke_def_map` (private helper, currently `extract.rs:506-517`)
- [ ] Delete the original `compiler/ori_arc/src/aims/interprocedural/extract.rs` file (its contents are now distributed across `extract/mod.rs`, `extract/consumed_params.rs`, and `extract/return_info.rs`)
- [ ] Run `cargo check -p ori_arc` and verify zero errors. If any error mentions an import path, the relative `super::super::super::` paths in the new sibling files are wrong — double-check (the new files are at depth `aims/interprocedural/extract/foo.rs`, so `super::super::super::contract` resolves to `aims::contract`).
- [ ] **Public API surface verification (extract)** — same recipe as 00.2's verification step, but scoped to `interprocedural/extract/`. Capture the public surface before/after:
  ```bash
  # Primary: hand-rolled grep (cargo public-api may not be installed)
  rg -n '^pub (use|mod|fn|struct|enum|trait|const|\(crate\) (use|mod|fn))' \
     compiler/ori_arc/src/aims/interprocedural/extract.rs > /tmp/extract-api-before.txt
  # ... do the split ...
  rg -n '^pub (use|mod|fn|struct|enum|trait|const|\(crate\) (use|mod|fn))' \
     compiler/ori_arc/src/aims/interprocedural/extract/ > /tmp/extract-api-after.txt
  diff /tmp/extract-api-before.txt /tmp/extract-api-after.txt
  ```
  Acceptance: only `extract_contract` should appear as `pub(crate)` in the after-output (matching the before-output). Both `consumed_params.rs` and `return_info.rs` should expose only `pub(super)` symbols, which `cargo public-api` and the grep above will NOT capture as public API. If `pub fn detect_consumed_params` or `pub fn extract_return_info` (without `(super)`) appears in the after-output, the visibility was wrongly widened — fix it before proceeding.
- [ ] Run `cargo test -p ori_arc` and verify ALL tests pass. There are no tests in the original `extract.rs` (verify with `grep -c '#\[test\]' compiler/ori_arc/src/aims/interprocedural/extract.rs` returning 0 before the split), so the only risk is import drift in `interprocedural/tests.rs` or callers in other AIMS files.
- [ ] Run `wc -l compiler/ori_arc/src/aims/interprocedural/extract/*.rs` and verify:
  - `mod.rs` ≤250 lines
  - `consumed_params.rs` ≤200 lines
  - `return_info.rs` ≤250 lines
  - **Each file < 450 lines (proactive split threshold)** — every leaf is comfortably under the limit and will remain so even if Section 13.1 (TRMC context behavior) adds another helper to `mod.rs` later
- [ ] Run `./clippy-all.sh` and verify zero new warnings. Watch specifically for `clippy::module_inception` (which fires when a module and its parent share a name — should NOT fire here because `extract/mod.rs` is the extract module itself, not nested).
- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and verify no broken doc links introduced by the move

---

## 00.4c Pre-split BLOAT files state_map.rs and interprocedural/mod.rs

**File(s):**
- `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` (currently **646 lines** — 146 over the 500-line limit)
- `compiler/ori_arc/src/aims/interprocedural/mod.rs` (currently **536 lines** — 36 over the limit)

**Context:** Agent 3's codebase scan identified two BLOAT files that this plan touches in Section 00.6 (stale annotation rewrites): `state_map.rs` has **14** stale `Section 09.x` annotations to rewrite, and `interprocedural/mod.rs` has **2** stale annotations to rewrite. Per `compiler.md` §File Size and `impl-hygiene.md` §File Organization: *"Touching a file over 500 lines without splitting is a finding."* Both files must be pre-split BEFORE Section 00.6 begins rewriting their annotations, otherwise the annotation work compounds the BLOAT violation.

**`state_map.rs` (646 lines)** — pure data-structure file: a single `AimsStateMap` struct with ~40 method implementations grouped by responsibility. The natural split is into multiple `impl AimsStateMap { ... }` blocks across sibling files (a common Rust pattern; the same struct can have impl blocks in different modules):

| Lines (current) | Content | Natural sibling |
|---|---|---|
| 1-130 | Module doc + imports + `InvokeEdgeState`, `AimsEvent` types | extract to `state_map/events.rs` |
| 132-214 | `AimsStateMap` struct definition + fields | (stays in `state_map/mod.rs`) |
| 215-280 | Constructors + scalar/immortal/excluded predicates (`new`, `set_permanent_scalar`, `is_scalar`, `set_immortals`, `is_immortal`, `is_excluded`) | (stays in `state_map/mod.rs` — core construction) |
| 280-400 | Block boundary state queries (`var_state_at_block_*`, `block_*_states`, `update_block_*`, `is_converged`, `reset_changed`) | (stays in `state_map/mod.rs` — core query API) |
| 400-450 | Cross-dimension counters (`cross_dimension_detected`, `count_cross_dim_states`) — **contains the predicate sites Section 02.5 audits at lines 429-440** | extract to `state_map/cross_dim.rs` |
| 450-510 | Borrow provenance (`borrow_source`, `set_borrow_source`, `borrows_from_source`, `join_borrow_sources`) | extract to `state_map/borrow_provenance.rs` |
| 510-550 | Invoke edge state + var shapes | extract to `state_map/invoke_shape.rs` |
| 550-646 | Events, effects accumulator, FIP balance helpers | extract to `state_map/effects_fip.rs` |

**Target structure:**
```
compiler/ori_arc/src/aims/intraprocedural/state_map/
├── mod.rs                # ~280 lines: struct + fields + constructors + core block-state API
├── events.rs             # NEW ~130 lines: InvokeEdgeState + AimsEvent
├── cross_dim.rs          # NEW ~60 lines: cross-dimension detection + count_cross_dim_states (the file Section 02.5 reads)
├── borrow_provenance.rs  # NEW ~60 lines: BorrowSource side-table impl
├── invoke_shape.rs       # NEW ~50 lines: invoke edge state + per-var shape map
├── effects_fip.rs        # NEW ~100 lines: effects accumulator + FIP balance helpers
└── tests.rs              # 535 lines (already exists at state_map/tests.rs; exempt from limit)
```

**Note**: `state_map.rs` (the file) becomes `state_map/mod.rs` (the directory). The existing `state_map/tests.rs` (535 lines, declared via `#[cfg(test)] mod tests;` in the current `state_map.rs`) stays untouched and is now found through the new `state_map/mod.rs`'s `mod tests;` declaration. The leaf-to-directory promotion is identical to 00.4b's `extract.rs → extract/`.

**`interprocedural/mod.rs` (536 lines)** — has these natural sections (verified by re-reading):

| Lines | Content | Natural sibling |
|---|---|---|
| 1-49 | Module doc + imports + `pub(crate) use extract::extract_contract` | (stays in `mod.rs`) |
| 51-135 | `pub fn analyze_program` (the orchestration entry — builds call graph, computes SCCs, processes them in topo order, reports FIP coverage) | (stays in `mod.rs` — the public entry point) |
| 137-148 | `analyze_scc_single` (helper) | (stays in `mod.rs` — small) |
| 150-end | `analyze_scc_fixpoint` (the SCC fixpoint loop), `tighten_uniqueness_from_callers` (post-fixpoint pass), and any other helpers | extract to `interprocedural/scc_loop.rs` |

**Target structure:**
```
compiler/ori_arc/src/aims/interprocedural/
├── mod.rs              # ~150 lines: module doc + analyze_program + analyze_scc_single + re-exports
├── scc_loop.rs         # NEW ~250 lines: analyze_scc_fixpoint + tighten_uniqueness_from_callers + helpers
├── extract/            # (created by 00.4b)
│   ├── mod.rs
│   ├── consumed_params.rs
│   └── return_info.rs
└── tests.rs            # 1243 lines (existing, exempt)
```

- [ ] **state_map.rs split — preflight:**
  - [ ] Re-read `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` end-to-end and confirm the line ranges above match the current file structure (line numbers may have drifted)
  - [ ] Confirm no method spans the proposed seam boundaries (a method that crosses a boundary must move with its predecessor or be re-cut)
  - [ ] Verify `state_map/tests.rs` already exists (it does — 535 lines, already declared via `#[cfg(test)] mod tests;` in the current `state_map.rs:10-15`)

- [ ] **state_map.rs split — execute (leaf-to-directory promotion):**
  - [ ] Create the directory `compiler/ori_arc/src/aims/intraprocedural/state_map/` (it already exists for `tests.rs` — verify with `ls`)
  - [ ] Create `state_map/events.rs` containing `InvokeEdgeState`, `AimsEvent`, `AimsEvent::block` impl, and the necessary imports (`use crate::ir::ArcBlockId; use rustc_hash::FxHashMap; use crate::ir::ArcVarId; use super::super::super::lattice::AimsState;`)
  - [ ] Create `state_map/cross_dim.rs` containing `impl AimsStateMap { ... }` with `cross_dimension_detected`, `set_cross_dimension_detected`, `count_cross_dim_states`. This file contains the **two predicate sites** Section 02.5 audits (lines 428-440 in the current state_map.rs at the time of plan creation).
  - [ ] Create `state_map/borrow_provenance.rs` containing `impl AimsStateMap { ... }` with `borrow_source`, `set_borrow_source`, `clear_borrow_source`, `borrows_from_source`, `join_borrow_sources`
  - [ ] Create `state_map/invoke_shape.rs` containing `impl AimsStateMap { ... }` with `invoke_edge_state`, `set_invoke_edge_state`, `var_shape`, `set_var_shape`
  - [ ] Create `state_map/effects_fip.rs` containing `impl AimsStateMap { ... }` with `events_in_block`, `record_event`, `effect_summary`, `accumulate_effect`, `set_fip_balance`, `fip_construct_count`, `fip_token_balanced`, `fip_net_allocation`, `num_blocks`, `num_vars`
  - [ ] Move the rest of `state_map.rs` into `state_map/mod.rs`: module doc, imports (now also `mod events; mod cross_dim; mod borrow_provenance; mod invoke_shape; mod effects_fip; pub use events::{InvokeEdgeState, AimsEvent};`), `AimsStateMap` struct definition, constructors, scalar/immortal/excluded predicates, block-boundary query API (`var_state_at_block_*`, `block_*_states`, `update_block_*`, `is_converged`, `reset_changed`), and `#[cfg(test)] mod tests;` declaration
  - [ ] Delete the original `state_map.rs` file (its contents are now in `state_map/mod.rs` and the 5 sibling impl files)
  - [ ] Run `cargo check -p ori_arc` and verify zero errors. The most likely error is path drift in the imports — `super::super::lattice::AimsState` may need to become `super::super::super::lattice::AimsState` from inside an `impl AimsStateMap` sibling file (one level deeper).
  - [ ] Run `cargo test -p ori_arc` and verify ALL tests pass — `state_map/tests.rs` (the existing 535-line file) is unchanged but its imports may need updating if it path-imports specific items via `use super::*;` (it does — the existing `mod tests;` declaration sits inside the current `state_map.rs`'s impl scope, so `super::*` resolves to the impl's items)
  - [ ] Run `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/*.rs` and verify each file is ≤300 lines (target: `mod.rs` ≤300, all siblings ≤150 except `events.rs` which can go to ~150)

- [ ] **interprocedural/mod.rs split — preflight + execute:**
  - [ ] Re-read `compiler/ori_arc/src/aims/interprocedural/mod.rs` end-to-end and confirm the seam at line ~150 (between `analyze_scc_single` and `analyze_scc_fixpoint`) is the correct cut. The seam separates "orchestration entry + non-recursive case" (stays in `mod.rs`) from "fixpoint iteration + post-fixpoint demand propagation" (moves to `scc_loop.rs`).
  - [ ] Create `compiler/ori_arc/src/aims/interprocedural/scc_loop.rs` containing `pub(super) fn analyze_scc_fixpoint`, `pub(super) fn tighten_uniqueness_from_callers`, and any private helpers they call. Necessary imports: same as the parent `mod.rs` (re-derive from the moved code).
  - [ ] Update `interprocedural/mod.rs` to declare `mod scc_loop;` (private — internal organization, NOT public API) and `use scc_loop::{analyze_scc_fixpoint, tighten_uniqueness_from_callers};` so the existing call sites at `analyze_program` resolve.
  - [ ] Run `cargo check -p ori_arc` and verify zero errors
  - [ ] Run `cargo test -p ori_arc` and verify ALL tests pass — `interprocedural/tests.rs` (1243 lines) exercises the SCC pipeline and would catch any regression
  - [ ] Run `wc -l compiler/ori_arc/src/aims/interprocedural/mod.rs compiler/ori_arc/src/aims/interprocedural/scc_loop.rs` and verify both ≤300 lines

- [ ] **Public API surface verification (state_map + interprocedural):** Same primary grep approach as 00.2/00.4/00.4b. Capture the public surface of `compiler/ori_arc/src/aims/intraprocedural/state_map/` and `compiler/ori_arc/src/aims/interprocedural/` before/after the splits and diff. Acceptance: the only NEW public paths should be the new sibling `mod` declarations themselves (which must be `mod`, not `pub mod`); all existing `pub fn` symbols must be reachable from the same `ori_arc::aims::intraprocedural::state_map::*` and `ori_arc::aims::interprocedural::*` paths as before.

- [ ] Run `./clippy-all.sh` and verify zero new warnings introduced by either split

- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and verify no broken doc links

- [ ] **TPR checkpoint** — `/tpr-review` covering 00.1–00.4c (all FIVE file splits done: lattice, transfer, extract, state_map, interprocedural). Codex sanity-checks that the chosen seams reflect actual concerns rather than mechanical chunking, that no symbols, doc comments, or derive annotations were lost in any of the moves, and specifically verifies the two leaf-to-directory promotions (extract → extract/, state_map → state_map/) and the multiple-impl-block pattern in state_map's siblings.
  <!-- Per .claude/skills/create-plan/plan-schema.md TPR cadence: 7 implementation
       subsections (00.1-00.4c) is well above the "3+" threshold, and the file splits
       are the riskiest semantic-preserving operation in the section. Catching seam
       mistakes here is much cheaper than catching them after Section 02 or Section 00.6
       have piled on. This TPR checkpoint covers ALL splits — including the 00.4c
       additions found by Agent 3's codebase scan. -->

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

## 00.6 Rewrite stale annotations in the remaining ~27 post-split AIMS files

**File(s):** After 00.5 cleans up the lattice files, the remaining files containing `Section 09.x` annotations are listed below. **Paths reflect the post-Section-00.4/00.4b/00.4c layout** — the original `state_map.rs`, `interprocedural/extract.rs`, `interprocedural/mod.rs`, and `transfer/mod.rs` all changed structure, so the 14/3/2/3 occurrences must be re-enumerated against the post-split files.

- `compiler/ori_arc/src/aims/contract/mod.rs` (3)
- `compiler/ori_arc/src/aims/contract/context.rs` (1)
- `compiler/ori_arc/src/aims/interprocedural/mod.rs` (2 — but re-enumerate after 00.4c split; some occurrences may have moved into `interprocedural/scc_loop.rs`)
- `compiler/ori_arc/src/aims/interprocedural/scc_loop.rs` (NEW post-00.4c — re-enumerate)
- `compiler/ori_arc/src/aims/interprocedural/tests.rs` (3)
- `compiler/ori_arc/src/aims/interprocedural/extract/mod.rs` (post-00.4b — was `extract.rs` with 3 occurrences; some may have moved into `extract/consumed_params.rs` or `extract/return_info.rs`)
- `compiler/ori_arc/src/aims/interprocedural/extract/consumed_params.rs` (NEW post-00.4b — re-enumerate)
- `compiler/ori_arc/src/aims/interprocedural/extract/return_info.rs` (NEW post-00.4b — re-enumerate)
- `compiler/ori_arc/src/aims/intraprocedural/mod.rs` (4)
- `compiler/ori_arc/src/aims/intraprocedural/block.rs` (17)
- `compiler/ori_arc/src/aims/intraprocedural/effects.rs` (2)
- `compiler/ori_arc/src/aims/intraprocedural/fip_balance.rs` (2)
- `compiler/ori_arc/src/aims/intraprocedural/post_convergence.rs` (2)
- `compiler/ori_arc/src/aims/intraprocedural/state_map/mod.rs` (post-00.4c — was `state_map.rs` with 14 occurrences; the count is preserved, but the occurrences are now distributed across `mod.rs` + the 5 new sibling files `events.rs`, `cross_dim.rs`, `borrow_provenance.rs`, `invoke_shape.rs`, `effects_fip.rs` per where the original code lived)
- `compiler/ori_arc/src/aims/intraprocedural/state_map/cross_dim.rs` (NEW post-00.4c — likely contains the `count_cross_dim_states` doc comment annotations Section 02.5 references)
- `compiler/ori_arc/src/aims/intraprocedural/state_map/{events,borrow_provenance,invoke_shape,effects_fip}.rs` (NEW post-00.4c — re-enumerate)
- `compiler/ori_arc/src/aims/intraprocedural/state_map/tests.rs` (1)
- `compiler/ori_arc/src/aims/intraprocedural/tests.rs` (14)
- `compiler/ori_arc/src/aims/realize/decide.rs` (1)
- `compiler/ori_arc/src/aims/realize/tests.rs` (1)
- `compiler/ori_arc/src/aims/emit_reuse/mod.rs` (1)
- `compiler/ori_arc/src/aims/emit_reuse/detect.rs` (1)
- `compiler/ori_arc/src/aims/normalize/mod.rs` (1)
- `compiler/ori_arc/src/aims/transfer/{forward,backward,rc_decisions,state_helpers}.rs` (post-00.4 — was `transfer/mod.rs` with 3 occurrences; re-enumerate per post-split file)
- `compiler/ori_arc/src/aims/transfer/tests.rs` (1)

**Total**: ~80 occurrences across **27 files** (post-split count — was 21 files before 00.4b/00.4c added 6 new sibling files). The total number of occurrences is unchanged because rewriting is comment-only; only the file distribution changed.

**Context:** Same rewrite discipline as 00.5. The annotations in these files cite the same missing plan section ("Section 09.x"), so they are stale by the same reasoning. Each file's annotations may differ in count and surrounding context — the rewrite must be done file-by-file, not via blind sed. The largest files (`block.rs` with 17, `intraprocedural/tests.rs` with 14, plus the 14 originally in `state_map.rs` now distributed across the 6 post-split files) deserve extra care because most of their annotations are inside multi-line doc comments where naive deletion will leave dangling sentences.

**Critical pre-flight (post-00.4c):** Before starting 00.6, run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` and **re-enumerate** the actual file distribution. The pre-00.4c distribution above is a starting point; the post-split distribution may differ slightly because some annotations (especially in `state_map.rs`'s doc comments) move with their methods to different sibling files. Treat the grep output as the authoritative worklist.

- [ ] Run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` and **enumerate every remaining occurrence** after 00.5. The output is the worklist for this subsection.
- [ ] For each file, apply the rewrite recipe from 00.5. Work through the file systematically — finish one file before starting the next, so partial states don't accumulate. Read each annotation in context (5 lines before, 5 lines after) before deciding whether to delete the parenthetical, rewrite the surrounding sentence, or restructure a paragraph.
- [ ] **`block.rs` (17 occurrences)** is the largest non-lattice file. The annotations are concentrated in the cross-block widening producer (around line 92-100) and the return widening producer (around line 147-159). These are the same producer sites Section 02.5 references. Re-read the file end-to-end and decide per-occurrence whether the annotation is in: (a) a doc comment for a function (rewrite to drop the navigation), (b) an inline rationale (rewrite to keep the rationale), or (c) a TODO/FIXME (delete the navigation, keep the TODO substance if any).
- [ ] **`state_map/*.rs` (14 occurrences total, post-00.4c distribution)** are the second-largest cluster. Most are in the `count_cross_dim_states` doc comment which Section 00.4c moves to `state_map/cross_dim.rs`; this is the file Section 02.5's verification references. Preserve the rule references (`Rule 4`, `Rule 6`, `Rule 8`) but drop the `(Section 09.x)` parentheticals. Re-enumerate per sibling file using `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/intraprocedural/state_map/` because the original line numbers are no longer valid after the 00.4c split.
- [ ] **`intraprocedural/tests.rs` (14 occurrences)** — test files. Per 00.5's discipline, rewrite each `// Tests for Section 09.x ...` to describe the *behavior being tested*, not the originating plan. Example: `// Tests for effect activation behavior` or `// Tests for cross-block locality widening (Rule 8 boundary)`.
- [ ] Pay attention to occurrences in **test files** generally — they often have annotations like `// Tests for Section 09.2 Effect Activation behavior`. Rewrite each to describe the behavior rather than the plan reference.
- [ ] After processing each file, run `cargo test -p ori_arc` to catch any accidental code change (the rewrites should be comment-only — verify regardless).
- [ ] After all ~27 post-split files are processed (re-enumerated by the pre-flight grep), run `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` and verify **zero results across all of `aims/`**.
- [ ] Run `cargo test -p ori_arc` one final time and verify all tests pass.
- [ ] Run `cargo doc -p ori_arc 2>&1 | grep warning` and verify no broken `[link]` references introduced by the rewrites.

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
- [ ] `wc -l compiler/ori_arc/src/aims/interprocedural/extract/mod.rs` returns ≤250 lines
- [ ] `wc -l compiler/ori_arc/src/aims/interprocedural/extract/consumed_params.rs` returns ≤200 lines
- [ ] `wc -l compiler/ori_arc/src/aims/interprocedural/extract/return_info.rs` returns ≤250 lines
- [ ] The original `compiler/ori_arc/src/aims/interprocedural/extract.rs` file no longer exists (verified by `test ! -f compiler/ori_arc/src/aims/interprocedural/extract.rs`); the directory `compiler/ori_arc/src/aims/interprocedural/extract/` has replaced it
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/mod.rs` returns ≤300 lines
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/events.rs` returns ≤150 lines
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/cross_dim.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/borrow_provenance.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/invoke_shape.rs` returns ≤100 lines
- [ ] `wc -l compiler/ori_arc/src/aims/intraprocedural/state_map/effects_fip.rs` returns ≤150 lines
- [ ] The original `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` (leaf file) no longer exists (verified by `test ! -f compiler/ori_arc/src/aims/intraprocedural/state_map.rs`); the directory `compiler/ori_arc/src/aims/intraprocedural/state_map/` has replaced it
- [ ] `wc -l compiler/ori_arc/src/aims/interprocedural/mod.rs` returns ≤300 lines (was 536; now contains analyze_program + analyze_scc_single + re-exports)
- [ ] `wc -l compiler/ori_arc/src/aims/interprocedural/scc_loop.rs` returns ≤300 lines (NEW: analyze_scc_fixpoint + tighten_uniqueness_from_callers)
- [ ] **Public API surface preserved by all FIVE splits**: PRIMARY verification is the grep approach (`rg '^pub (use|mod|fn|struct|enum|trait|const)' compiler/ori_arc/src/aims/lattice/ compiler/ori_arc/src/aims/transfer/ compiler/ori_arc/src/aims/interprocedural/ compiler/ori_arc/src/aims/intraprocedural/state_map/`) before/after diff shows ZERO new public paths. The new submodules (`state`, `canonicalize`, `borrow_source`, `size_class`, `forward`, `backward`, `rc_decisions`, `state_helpers`, `consumed_params`, `return_info`, `events`, `cross_dim`, `borrow_provenance`, `invoke_shape`, `effects_fip`, `scc_loop`) are private; only `pub use` (or `pub(super) use` / `pub(crate) use` for the extract submodules) re-exports reach external callers. OPTIONAL: if `cargo public-api -p ori_arc` becomes available, run it as a stronger contract check.
- [ ] `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns zero results
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 stale Section 09 references in this plan's touched files
- [ ] `cargo test -p ori_arc` green
- [ ] `timeout 150 ./test-all.sh` green — zero behavioral regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] No new doc warnings introduced (`cargo doc -p ori_arc 2>&1 | grep warning` returns no new entries)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan locality-representation-unification` returns 0 annotations — all temporary scaffolding (TPR, CROSS, BUG, §, Phase, section- refs) removed from `.rs` files
- [ ] All intermediate TPR checkpoint findings resolved (the 00.4c TPR checkpoint above, which now covers all FIVE splits: lattice, transfer, interprocedural/extract, intraprocedural/state_map, interprocedural/mod)
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

**Exit Criteria:** `wc -l compiler/ori_arc/src/aims/lattice/*.rs compiler/ori_arc/src/aims/transfer/*.rs compiler/ori_arc/src/aims/interprocedural/extract/*.rs compiler/ori_arc/src/aims/intraprocedural/state_map/*.rs compiler/ori_arc/src/aims/interprocedural/*.rs` shows every non-test file ≤300 lines (lattice, transfer ≤250 typical, extract sub-files ≤250, state_map sub-files ≤300, interprocedural/mod and scc_loop ≤300). `grep -rn 'Section 09\.' compiler/ori_arc/src/aims/` returns zero results. `cargo test -p ori_arc` passes the same test count as before this section started (no tests added or removed — this is purely structural cleanup). `timeout 150 ./test-all.sh` and `timeout 150 ./clippy-all.sh` are both green. All five touched module trees (lattice, transfer, interprocedural/extract, intraprocedural/state_map, interprocedural/mod) now have a clean architectural seam structure that Section 02 can write into without immediately violating the 500-line limit.
