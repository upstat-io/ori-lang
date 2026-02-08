---
section: "08"
title: RC Elimination via Dataflow
status: not-started
goal: Eliminate redundant retain/release pairs using bidirectional dataflow analysis, reducing ARC overhead
sections:
  - id: "08.1"
    title: Lattice-Based RC State Machine
    status: not-started
  - id: "08.2"
    title: Bidirectional Dataflow Analysis
    status: not-started
  - id: "08.3"
    title: Matching & Elimination
    status: not-started
---

# Section 08: RC Elimination via Dataflow

**Status:** Not Started
**Goal:** After RC insertion (Section 07), optimize away redundant retain/release pairs. A retain immediately followed by a release on the same value can be eliminated. This pass reduces ARC overhead significantly.

**Pipeline position:** This pass runs AFTER constructor reuse expansion (Section 09). Input is ARC IR with RcInc/RcDec from both RC insertion (07) and reuse expansion (09). Execution order: 07 (RC insertion) → 09 (reuse expansion) → 08 (this pass). Section numbers indicate topic grouping, not execution order.

**Reference compilers:**
- **Swift** `lib/SILOptimizer/ARC/` -- 21 files implementing bidirectional dataflow with lattice states, ARCMatchingSet for pair elimination
- **Lean 4** implied by borrow analysis results -- unnecessary ops never inserted

**Key insight from Swift:** This optimization happens at the *ARC-annotated IR* level (between Section 07 and codegen), where we have full ownership semantics. NOT at the LLVM IR level where ownership information is lost.

**Heap layout context:** RC operations target the Roc-style refcount-at-negative-offset layout (see Section 01.6). The header is 8 bytes: `{ strong_count: i64 }`. The strong_count is at `ptr - 8` from the data pointer. Elimination of paired inc/dec operations saves not just the refcount arithmetic but also the pointer adjustment and memory access.

---

## 08.1 Lattice-Based RC State Machine

```rust
/// Bottom-up refcount state lattice (from Swift).
///
/// Tracks what we know about a value's reference count as we
/// scan backward through the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RcState {
    /// No information yet about this value.
    None,
    /// We've seen a Dec (release) for this value.
    /// Looking backward for a matching Inc (retain) to eliminate both.
    Decremented,
    /// The value is used between the Dec and a potential Inc.
    /// Can't eliminate because the value is needed alive.
    MightBeUsed,
    /// Multiple Dec paths exist (e.g., in different branches).
    /// Too complex to eliminate safely.
    MightBeDecremented,
}

/// Top-down refcount state (forward analysis).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopDownRcState {
    /// No information.
    None,
    /// We've seen an Inc (retain). Looking for a matching Dec.
    Incremented,
    /// Seen Inc, then a potential Dec through an alias.
    /// Transition: Incremented → MightBeDecremented when a Dec is seen
    /// on a value that might alias the tracked value.
    /// Cannot safely eliminate the original Inc because the alias
    /// Dec might be the one that actually frees the object.
    MightBeDecremented,
    /// Value might have other uses; can't guarantee the Inc is redundant.
    MightBeUsed,
}
```

- [ ] Define `RcState` lattice with meet/join operations
- [ ] Define `TopDownRcState` lattice (including `MightBeDecremented` for aliased Dec)
- [ ] Implement state transitions for each RC operation
- [ ] Implement `Incremented → MightBeDecremented` transition on aliased Dec
- [ ] Implement state transitions for value uses (kills elimination opportunity)

## 08.2 Bidirectional Dataflow Analysis

**Two-pass approach (from Swift's ARCSequenceDataflowEvaluator):**

1. **Bottom-up pass:** For each Dec, scan backward looking for a matching Inc
2. **Top-down pass:** For each Inc, scan forward looking for a matching Dec

```rust
/// Per-variable RC state for all tracked values in a block.
///
/// Keyed by ArcVarId (ARC IR variable), not Name (AST identifier).
/// RC ops are ArcInstr::RcInc/RcDec instructions within ArcBlocks.
pub struct RcStateMap {
    /// Variable → bottom-up RC state
    bottom_up: FxHashMap<ArcVarId, RcState>,
    /// Variable → top-down RC state
    top_down: FxHashMap<ArcVarId, TopDownRcState>,
}

/// Position of an instruction within an ARC function.
/// (block index, instruction index within block body)
pub type InstrPos = (ArcBlockId, usize);

/// Run bidirectional dataflow analysis on an ARC IR function.
///
/// Operates on the ARC IR after RC insertion (Section 07).
/// The function's blocks already contain RcInc/RcDec instructions.
pub fn analyze_rc_dataflow(
    func: &ArcFunction,
) -> Vec<EliminationCandidate> {
    let mut candidates = Vec::new();

    // Bottom-up: find Dec→Inc pairs (scan backward within blocks)
    let bottom_up_results = bottom_up_pass(func);
    for (dec_pos, inc_pos) in bottom_up_results.matched_pairs() {
        candidates.push(EliminationCandidate { inc: inc_pos, dec: dec_pos });
    }

    // Top-down: find Inc→Dec pairs (scan forward within blocks)
    let top_down_results = top_down_pass(func);
    for (inc_pos, dec_pos) in top_down_results.matched_pairs() {
        candidates.push(EliminationCandidate { inc: inc_pos, dec: dec_pos });
    }

    // Deduplicate and validate
    candidates.sort();
    candidates.dedup();
    candidates
}

struct EliminationCandidate {
    /// Position of the RcInc instruction to eliminate.
    inc: InstrPos,
    /// Position of the RcDec instruction to eliminate.
    dec: InstrPos,
}
```

**V1 implementation is intra-block only.** The bottom-up and top-down passes scan within individual basic blocks. Cross-block elimination following Swift's `GlobalARCSequenceDataflow` approach (which propagates RC state across block boundaries using a global dataflow framework) is future work. The intra-block analysis is sufficient to eliminate the most common redundant pairs (e.g., `inc x; use x; dec x` within a single block).

**Conservative at loop boundaries** (from Swift):
- At loop entry/exit, reset state to `None` (conservative)
- Loop-specific analysis can be added later (future work)

- [ ] Implement `bottom_up_pass` scanning backward
- [ ] Implement `top_down_pass` scanning forward
- [ ] Handle branching: merge states from both branches
- [ ] Handle loops: reset state at loop boundaries (conservative)
- [ ] Collect elimination candidates

## 08.3 Matching & Elimination

```rust
/// Validate and apply RC eliminations on an ARC IR function.
///
/// Safety checks before eliminating an Inc/Dec pair:
/// 1. Same variable (same ArcVarId)
/// 2. No intervening use that depends on the value being alive
/// 3. No intervening Dec on the same variable (double-free risk)
/// 4. If in different blocks, both paths must have the pair
pub fn eliminate_rc_pairs(
    func: &mut ArcFunction,
    candidates: &[EliminationCandidate],
) -> usize {
    let mut eliminated = 0;
    for candidate in candidates {
        if is_safe_to_eliminate(candidate, func) {
            // Remove the RcInc instruction at candidate.inc position
            remove_instr(func, candidate.inc);
            // Remove the RcDec instruction at candidate.dec position
            remove_instr(func, candidate.dec);
            eliminated += 1;
        }
    }
    eliminated
}
```

- [ ] Implement safety validation for elimination candidates
- [ ] Implement elimination (remove matched Inc/Dec pairs)
- [ ] Track elimination statistics for diagnostics
- [ ] Iterate: elimination may enable further elimination (cascading)

---

**Exit Criteria:** Redundant retain/release pairs are identified and eliminated. The elimination is provably safe (no use-after-free, no leaks). Simple patterns like `inc x; use x; dec x` around a borrowed parameter call are eliminated.
