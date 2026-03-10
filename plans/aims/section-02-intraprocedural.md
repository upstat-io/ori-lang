---
section: "02"
title: "Intraprocedural Analysis"
status: in-progress
reviewed: true  # 2026-03-10
goal: "Single backward dataflow pass computing AimsState for every variable at every program point"
inspired_by:
  - "GHC demand analysis backward pass (compiler/GHC/Core/Opt/DmdAnal.hs)"
  - "Lean 4 RC insertion (src/Lean/Compiler/IR/RC.lean)"
  - "ori_arc liveness (compiler/ori_arc/src/liveness/mod.rs)"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "State Map Data Structure"
    status: complete
  - id: "02.2"
    title: "Backward Dataflow Framework"
    status: complete
  - id: "02.2a"
    title: "Cardinality Semiring Operators"
    status: complete
  - id: "02.2b"
    title: "Terminator Edge States for Invoke"
    status: complete
  - id: "02.3"
    title: "Block-Level Analysis"
    status: complete
  - id: "02.4"
    title: "Control Flow Merge Points"
    status: complete
  - id: "02.5"
    title: "Pattern Match Handling"
    status: complete
  - id: "02.6"
    title: "Invoke Definition Handling"
    status: complete
  - id: "02.7"
    title: "Completion Checklist"
    status: in-progress
---

# Section 02: Intraprocedural Analysis

**Status:** Not Started
**Goal:** Implement a single backward dataflow pass over an `ArcFunction` that
computes an `AimsState` for every variable at every program point, consuming
interprocedural signatures from Section 03. The analysis must converge in bounded
iterations and produce correct states for all test cases.

**Context:** The current `ori_arc` runs derived ownership, then liveness analysis,
then COW annotation, then RC insertion, then reset/reuse (with a second liveness
pass and dominator tree build), then RC identity propagation, then RC elimination
— each as a separate pass over the function body. AIMS replaces all of these with one backward pass that computes
the unified state. The backward direction is natural because we need to know how
a value WILL be used (future demand) to decide what RC operations to emit NOW.

**Reference implementations:**
- **GHC** `compiler/GHC/Core/Opt/DmdAnal.hs`: Backward demand analysis computing
  usage annotations — the same direction and structure we need
- **Lean 4** `src/Lean/Compiler/IR/RC.lean`: Backward liveness + RC insertion —
  traverses bottom-up, maintaining live variable set
- **ori_arc** `liveness/mod.rs`: Current backward liveness analysis — the starting
  point for understanding the existing traversal

**Depends on:** Section 01 (lattice definition).

---

## 02.1 State Map Data Structure

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` (NEW)

The state map stores the computed `AimsState` for every variable at block
boundaries (entry and exit). Per-instruction states are NOT stored — they are
re-derived during emission by replaying transfer functions within each block
(see the `block_exit_states` / `block_entry_states` documentation below).

The state map is the **analysis fact source** for all downstream consumers:
RC emission, reuse emission, COW annotation, drop hints, and FIP certification.
COW annotations and drop hints are **derived packaging artifacts** computed by
combining analysis facts (keyed by `ArcVarId`) with final IR positions (from
a post-merge walk). The analysis proves the facts; the packaging step maps
them to the layout the LLVM emitter expects.

- [x] Define `AimsStateMap`:
  `compiler/ori_arc/src/aims/intraprocedural/state_map.rs` — struct with
  `block_exit_states`, `block_entry_states`, `invoke_edge_states`,
  `borrow_sources`, `events`, `scalars`, `changed` fields.
  Also defines `InvokeEdgeState` and `AimsEvent` types.

- [x] Implement `AimsStateMap::var_state_at_block_exit(block, var) -> AimsState`
- [x] Implement `AimsStateMap::var_state_at_block_entry(block, var) -> AimsState`
- [x] Implement `AimsStateMap::borrow_source(var) -> Option<&BorrowSource>` — returns
  the borrow provenance for a variable, or None if the variable is Owned or not tracked
- [x] Implement `AimsStateMap::set_borrow_source(var, source)` — update borrow provenance
  during transfer function application (called by Project, pattern binding transfers)
- [x] Implement `AimsStateMap::clear_borrow_source(var)` — remove provenance when
  a variable transitions to Owned (called at join when Borrowed meets Owned)
- [x] Implement `AimsStateMap::join_borrow_sources(var, other_sources)` — merge
  provenance at control flow join: same source → keep Exact; different → Unknown
- [x] Implement `AimsStateMap::invoke_edge_state(block) -> Option<&InvokeEdgeState>` —
  returns the per-edge demand state for a block ending in Invoke, or None
- [x] Implement `AimsStateMap::set_invoke_edge_state(block, state)` — store per-edge
  state during analysis when processing an Invoke terminator
- [x] Implement `AimsStateMap::is_converged() -> bool` — no block state changed in
  last iteration
- [x] Implement `AimsStateMap::events_in_block(block) -> &[AimsEvent]` — returns
  the event slice for a specific block from the per-block event map (empty slice
  if no events recorded for that block)
- [x] Implement `AimsStateMap::record_event(event)` — append a sparse event to the
  block's event list (keyed by the event's block field in the per-block map)
- [x] Consider memory layout: sparse `FxHashMap` chosen over dense `Vec<Vec<AimsState>>`
  — backward demand analysis produces mostly BOTTOM states; sparse storage avoids
  allocating entries for dead variables. Documented in struct doc comment.

---

## 02.2 Backward Dataflow Framework

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs` (NEW)

> **Warning: File size.** This section covers state map, backward dataflow, block-level analysis,
> merge points, pattern match handling, invoke semantics, and event tracking. Estimated ~1,200 lines
> far exceeds the 500-line limit. **Must split into submodules from the start:**
> - `aims/intraprocedural/mod.rs` — `analyze_function()` entry point, worklist loop (~200 lines)
> - `aims/intraprocedural/state_map.rs` — `AimsStateMap` data structure + events (~200 lines, from 02.1)
> - `aims/intraprocedural/block.rs` — per-block backward analysis (~300 lines, from 02.3)
> - `aims/intraprocedural/merge.rs` — control flow join handling (~200 lines, from 02.4)
> - `aims/intraprocedural/pattern.rs` — pattern match scrutinee/binding analysis (~150 lines, from 02.5)
> - `aims/intraprocedural/events.rs` — sparse event recording (context holes, reuse candidates, FIP gates) (~100 lines)

The core analysis loop: iterate backward over blocks in reverse postorder until
fixed-point convergence.

- [x] Implement `analyze_function(func, classifier, sigs, context_regions) -> AimsStateMap`
  in `compiler/ori_arc/src/aims/intraprocedural/mod.rs`. Marks scalar variables,
  computes postorder, iterates worklist until convergence or safety net triggers.
  Also created `contract.rs` with minimal `MemoryContract`, `ParamContract`,
  `ReturnInfo`, and `ContextRegion` types for the API signature.

- [x] Implement worklist with change tracking (avoid re-analyzing blocks whose
  predecessors haven't changed). Uses `AimsStateMap::reset_changed()` /
  `is_converged()` and `update_block_entry()` / `update_block_exit()` which
  return `bool` indicating whether state changed.
- [x] Convergence bound: chain height (15) × number of variables × number of blocks.
  Computed via `AimsState::iteration_limit()`.
- [x] **Non-convergence safety net**: if iteration exceeds the bound, widen remaining
  variables to TOP and log a `tracing::warn!`. Implemented in `widen_to_top()`.

---

## 02.2a Cardinality Semiring Operators

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/block.rs`

Cardinality analysis on ARC IR CFGs requires two distinct operators, not just
`max` (solutions.md Decision 2):

- [x] Implement `Cardinality::seq_add` — implemented in Section 01 (`lattice/mod.rs`).
  `Absent + x = x`, `Once + Once = Many`, `Many + _ = Many`.

- [x] Implement `Cardinality::alt_join` — implemented in Section 01 (`lattice/mod.rs`).
  `max(Once, Once) = Once` (not Many).

- [x] Rule: within a block, use `seq_add` for instruction-level demand composition.
  At successor joins (`compute_block_exit_state`), use `alt_join` (= join/max).
  Implemented in `block.rs`: `add_backward_demand` uses `seq_add`,
  `compute_block_exit_state` uses `join` (= `alt_join`).

- [x] Test the critical distinction:
  `branch_value_used_in_both_arms_is_once` verifies `alt_join(Once, Once) = Once`.
  `sequential_uses_in_same_block_are_many` verifies `seq_add` within blocks.

---

## 02.2b Terminator Edge States for Invoke

**File(s):** `compiler/ori_arc/src/aims/intraprocedural/merge.rs`

- [x] Define `TerminatorEdgeState` for Invoke handling:
  Defined as `InvokeEdgeState` in `state_map.rs` (Section 02.1) with `normal`
  and `unwind` fields, each `FxHashMap<ArcVarId, AimsState>`.

- [x] Rules:
  - Normal successor sees `dst` as defined (from callee contract or conservative)
  - Unwind successor does NOT see `dst`
  - Successor combination uses `alt_join` (normal and unwind are alternative paths)
  - Unwind edge state stored via `AimsStateMap::set_invoke_edge_state()` and
    queryable via `invoke_edge_state()` for Section 04 emission.
  Tested in `invoke_edge_state_basic_operations`.

---

## 02.3 Block-Level Analysis

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs`

Process a single block backward: start from the block's exit state (from successors),
walk instructions in reverse, applying transfer functions.

- [x] Implement `compute_block_entry_state(func, block_id, state_map, sigs)`:
  The analysis is BACKWARD: we compute the ENTRY state of a block from the
  EXIT states of the SAME block, which in turn come from the ENTRY states of
  SUCCESSOR blocks. "Successor" means CFG successor (control flow target),
  which is the DEMAND source in backward analysis.

  - Start with the entry states of all CFG successor blocks (these represent
    the demand that successor blocks place on variables flowing out of the
    current block). Retrieve successors via
    `graph::successor_block_ids(&block.terminator)`, pub(crate) free fn.
  - **Use `alt_join` (max) for successor combination** — at a branch/switch,
    only ONE successor executes per dynamic run, so successor demands are
    alternative (solutions.md Decision 2). At a Jump (single successor),
    alt_join is trivially the successor's state.
  - This produces the block's EXIT state (demand at the end of the block).
  - Apply transfer function for the terminator (backward: terminator may
    add demand on its operands, e.g., Branch adds Once to cond).
  - Walk instructions in reverse order, applying transfer functions.
    Each instruction adds demand to its operands via `seq_add` (the operand
    is used once by this instruction AND by the remaining demand from later
    instructions — sequential composition).
  - **Use `seq_add` for within-block demand accumulation**
  - Return the computed entry state for this block

- [x] Handle each `ArcInstr` variant (field names match `ir/instr.rs`):
  - `Let { dst, ty, value }` — for `ArcValue::Var(v)`: dst inherits v's state (including
    access class); for `ArcValue::Literal(_)`: dst gets `SCALAR`; for `ArcValue::PrimOp`:
    dst based on result type determined by `classifier.arc_class(ty)`:
    `Scalar` → `SCALAR`, `DefiniteRef`/`PossibleRef` → `FRESH`.
    PrimOp operand variables get demand bump (cardinality `seq_add(Once)`).
    Note: PrimOps in ARC IR are arithmetic, comparison, and bitwise operations
    that always produce scalars. A PrimOp producing a ref type would be a
    compiler bug (PrimOps don't allocate), but the analysis handles it
    conservatively via `arc_class`.
  - `Apply { dst, ty, func, args, arg_ownership }` — dst gets `(Owned, *, *, callee_return_uniqueness)`
    from callee's `MemoryContract.return_info`; args' demands set per callee's
    `ParamContract` (access, consumption, cardinality)
  - `ApplyIndirect { dst, ty, closure, args }` — dst gets `TOP` (conservative — unknown callee);
    closure gets demand bump; all args get `(Owned, Unrestricted, Many)`
  - `PartialApply { dst, ty, func, args }` — dst is `FRESH` `(Owned, Linear, Once, Unique)`;
    captured variables get `(Owned, Unrestricted, Many)` (stored in closure, may be
    invoked multiple times). Captured vars' `locality` promoted to `HeapEscaping`
    (closure may outlive the defining function and escape to callers).
  - `Project { dst, ty, value, field }` — dst is `(Borrowed, Linear, Once, value.uniqueness)`
    with `BorrowSource::Exact(value)`; value stays unchanged
  - `Construct { dst, ty, ctor, args }` — dst is `FRESH` with shape from ctor kind;
    in backward analysis, adds demand on each arg via `seq_add(Once)` (each arg is
    consumed once by the constructor)
  - `RcInc/RcDec` — these should NOT appear in the input (AIMS generates them);
    if encountered (during migration/dual-pipeline), treat as no-op for analysis
  - `Reset/Reuse/IsShared/Set/SetTag` — should NOT appear (AIMS generates them);
    if encountered, panic with clear message (invariant violation)
  - `Select { dst, ty, cond, true_val, false_val }` — dst gets componentwise join of true_val
    and false_val states (including access, consumption, uniqueness); cond gets `Once`
    cardinality bump
  - `CollectionReuse { old_var, dst, ty, ctor, args }` — dst is `FRESH` with shape
    `CollectionBuffer`; in backward analysis, adds demand on old_var (consumed) and
    each arg via `seq_add(Once)`

- [x] Handle `ArcTerminator` variants (field names match `ir/mod.rs`):
  - `Return { value }` — value gets at least `Once` cardinality; contributes to function summary
  - `Jump { target, args }` — args flow into target block's params
  - `Branch { cond, then_block, else_block }` — cond gets `Once`; split state
  - `Switch { scrutinee, cases: Vec<(u64, ArcBlockId)>, default }` — scrutinee gets
    demand via `seq_add(Once)` (read for tag test). Note: ARC IR `Switch` does NOT
    carry pattern bindings or destructuring info — it only routes control based on
    a discriminant value. Pattern bindings were lowered into `Project` instructions
    in each case's target block body by the decision tree compiler. The analysis
    handles these bindings through the normal `Project` transfer function when
    processing each target block.
  - `Invoke { dst, ty, func, args, arg_ownership, normal, unwind }` — like Apply but with
    two successor blocks. **Critical**: `dst` is defined only in the `normal` successor,
    NOT in the `unwind` successor. The unwind path must track live variables across the
    invoke for cleanup RC dec emission. Use `graph::collect_invoke_defs()` to build the
    invoke-def map.
  - `Resume` / `Unreachable` — terminal, no successor state

---

## 02.4 Control Flow Merge Points

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs`

In **backward** analysis, the merge direction is reversed from forward analysis:
demand flows from successors to predecessors. A block's **exit state** (demand at the
end of the block) is computed by joining the **entry states of all CFG successor blocks**
(the demand that each successor places on incoming values). This is the standard
backward-dataflow merge.

- [x] Implement join at block exit (demand from successors):
  - For each variable demanded by ANY successor: join states componentwise via `alt_join`
  - Variables demanded by one successor but absent in another: treat the absent
    successor as contributing `BOTTOM` (Dead, Unique, Absent) for that variable.
    The join then produces the demanding successor's state (since `join(x, BOTTOM) == x`).
  - Variables absent in ALL successors: omit from the exit state (implicitly BOTTOM = no demand)
  - This is where the lattice join operation is critical for soundness

- [x] Handle loop back-edges:
  - Back-edges create cycles in the dataflow graph
  - Initialize loop header states to BOTTOM
  - Fixed-point iteration handles convergence
  - Loop-carried variables may promote from `Once` to `Many` (used in each iteration)

- [x] Widening (if needed for performance):
  - The chain height is 15 (sum of per-dimension chain heights), so widening is
    unlikely to be needed
  - If iteration count exceeds `15 × num_variables × num_blocks` (the theoretical
    convergence bound), widen remaining variables to TOP and log
    `tracing::warn!` — see Section 01.7 non-convergence safety net

---

## 02.5 Pattern Match Handling

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs`

Pattern matching (Switch terminator) requires careful handling: the scrutinee is
NOT consumed by the Switch itself, and each case branch introduces new bindings
that borrow from the scrutinee.

- [x] Analyze scrutinee at Switch terminator:
  - The Switch terminator only reads the scrutinee's discriminant tag (a scalar
    operation — no ownership transfer). This adds `Once` demand via `seq_add`.
  - **Critical: the scrutinee is NOT dead after the Switch.** Successor blocks
    create `Project` instructions that borrow from it. The scrutinee's liveness
    is extended by these borrows — it must remain alive as long as any
    `BorrowSource::Exact(scrutinee)` variable is live. In backward analysis,
    this happens naturally: the `Project` transfer functions in successor blocks
    add demand on the scrutinee, which propagates backward through the Switch
    to keep the scrutinee alive in the predecessor.
  - The scrutinee transitions to Dead only when ALL derived borrows are dead.
    Reuse of the scrutinee's allocation is possible only AFTER this point.
    This is detected naturally: when the scrutinee finally transitions to Dead
    with `Unique` uniqueness and a reusable `ShapeClass`, standard reuse
    detection (Section 05) applies. No special handling at the Switch is needed.
  - **Implication for reuse/RC:** No RcDec or Reset should be emitted for the
    scrutinee at the Switch site. RC cleanup happens when the last derived
    borrow dies, which is typically at the end of the match arms.

- [x] Analyze case bindings:
  Pattern bindings in ARC IR are `Project` instructions in each case's target block
  (emitted by the decision tree compiler). They are NOT special — the normal
  `Project` transfer function handles them:
  - `Project(dst, scrutinee, field)` → `dst` gets `(Borrowed, *, *, scrutinee.uniqueness)`
    with `BorrowSource::Exact(scrutinee)` — this is a borrowed view, regardless of
    whether the scrutinee is Unique, MaybeShared, or Shared.
  - No RC inc is needed on pattern bindings because they are borrows. The scrutinee's
    RC obligations are handled by the scrutinee's own lifecycle (dec when dead).
  - This is consistent with the plan's access/consumption split: borrowing does not
    add RC obligations.

- [x] Cross-branch variable states:
  - Variables live across multiple branches get the join of all branch states
  - Variables dead in some branches get `consumption = Affine` (may need RC dec in dead
    branches)

---

## 02.6 Invoke Definition Handling

**File(s):** `compiler/ori_arc/src/aims/intraprocedural.rs`

`Invoke` instructions define their `dst` variable only in the `normal` successor
block, not the `unwind` block. This requires special handling in the backward
analysis.

- [x] Use `graph::collect_invoke_defs(func)` (pub(crate) in `graph/mod.rs`) to build
  a map of `ArcBlockId → Vec<ArcVarId>` for invoke-defined variables
- [x] When computing entry state for a block that is a normal successor of an
  Invoke: include the `dst` variable in the block's defined set
- [x] When computing entry state for a block that is an unwind successor of an
  Invoke: do NOT include the `dst` variable — it is not defined here
- [x] Unwind blocks need cleanup analysis: variables live across the invoke that
  are NOT the `dst` need `RcDec` on the unwind path

---

## 02.7 Completion Checklist

- [x] `AimsStateMap` correctly stores per-block entry/exit states
- [x] Backward dataflow converges for all test functions (no infinite loops)
- [x] Non-convergence safety net triggers `tracing::warn!` if iteration exceeds bound
- [x] All `ArcInstr` variants handled by transfer functions (all 15 variants:
  Let, Apply, ApplyIndirect, PartialApply, Project, Construct, RcInc, RcDec,
  IsShared, Set, SetTag, Reset, Reuse, CollectionReuse, Select)
- [x] All `ArcTerminator` variants handled (all 7 variants:
  Return, Jump, Branch, Switch, Invoke, Resume, Unreachable)
- [x] `ArcValue` sub-variants handled (`Var`, `Literal`, `PrimOp`)
- [x] Transfer functions set `access` (Borrowed/Owned) correctly for all instructions:
  `Project` → `Borrowed`, `Construct`/`PartialApply`/`Apply` → `Owned`,
  `CollectionReuse` dst → `Owned` (solutions.md Decision 1)
- [x] Cardinality uses `seq_add` within blocks and `alt_join` at successor joins
  (solutions.md Decision 2)
- [x] `TerminatorEdgeState` for Invoke correctly separates normal/unwind edge states
- [x] Loop back-edges handled correctly (states converge through iteration;
  loop-carried `Once` promotes to `Many` via `seq_add`)
- [x] Pattern match scrutinee stays alive through Switch (tag read only); transitions
  to Dead only after all derived borrows die (Section 02.5)
- [x] `BorrowSource` side table updated for `Project` and pattern bindings (Decision 1/5)
- [x] `BorrowSource` joined correctly at merge points (same source → Exact, different → Unknown)
- [x] `BorrowSource` cleared when variable transitions to `AccessClass::Owned`
- [x] `BorrowSource` queryable by emission passes (Section 04 reads it for
  uniqueness-preserving borrow decisions)
- [x] Invoke dst-in-normal-only semantics correct
- [x] Scalar variables short-circuited (never analyzed)
- [x] State map queryable by emission passes
- [ ] Sparse event table records reusable allocation candidates <!-- deferred: Stage 2 — event table structure exists, population requires Section 05 reuse detection -->
- [ ] Sparse event table records local-allocation eligibility (v1: conservative) <!-- deferred: Stage 2 — requires escape analysis refinement -->
- [ ] Sparse event table records FIP gates (Stage 2: when FipContract is available) <!-- deferred: Stage 2 — requires Section 07 FIP contracts -->
- [ ] Constructor-context events recorded when normalize/ pass has run (Stage 3) <!-- deferred: Stage 3 — requires TRMC normalize pass -->
- [x] All 10 validation corpus tests pass with expected cardinality at key points

- [x] **Validation corpus** (10 hand-traced test cases with expected cardinality):
  1. Straight-line single-use value → `Once` at use, `Absent` after
  2. `if` with one use in each branch → `Once` per execution (not `Many`)
  3. `if` with use in one branch, none in the other → `Once` (alt_join)
  4. Simple loop with one use per iteration → `Many` (seq_add across iterations)
  5. Nested loop → `Many` (inner loop body promotes to Many)
  6. `Switch` with pattern-bound values → bindings inherit scrutinee uniqueness
  7. `Invoke` with live values across unwind → correct edge-specific states
  8. `Project` followed by source reuse → source stays `Unique`
  9. COW-heavy collection update → receiver `Once`, result `Unique`
  10. `PartialApply` capture → captured vars `Many` + `HeapEscaping`

**Exit Criteria:** `cargo t -p ori_arc -- aims::intraprocedural` passes. All 10
validation corpus test cases produce exactly the expected states at every key program
point. Analysis converges within the theoretical bound (15 × |variables| × |blocks|)
for all test functions. The `AimsStateMap`
is the analysis fact source for RC, reuse, COW, and FIP — no emission pass
maintains a separate analysis. COW annotations and drop hints are derived
packaging artifacts, not independent fact sources (see Section 04.3, 04.4).
