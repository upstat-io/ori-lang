---
section: "08"
title: RC Elimination via Dataflow
status: complete
goal: Eliminate redundant retain/release pairs using bidirectional dataflow analysis, reducing ARC overhead
sections:
  - id: "08.1"
    title: Lattice-Based RC State Machine
    status: complete
    note: rc_elim.rs — TopDownState (Incremented/MightBeUsed), BottomUpState (Decremented/MightBeUsed)
  - id: "08.2"
    title: Bidirectional Dataflow Analysis
    status: complete
    note: rc_elim.rs — top_down_block_pass (forward scan), bottom_up_block_pass (backward scan), intra-block V1
  - id: "08.3"
    title: Matching & Elimination
    status: complete
    note: rc_elim.rs — apply_eliminations with cascading iteration, 27 tests
---

# Section 08: RC Elimination via Dataflow

**Status:** Complete — 08.1 lattice states implemented (simplified from plan — no `MightBeDecremented` needed for intra-block V1). 08.2 bidirectional intra-block analysis implemented (top-down forward, bottom-up backward). 08.3 matching and elimination with cascading iteration. 27 tests covering all instruction types.
**Goal:** After RC insertion (Section 07), optimize away redundant retain/release pairs. A retain immediately followed by a release on the same value can be eliminated. This pass reduces ARC overhead significantly.

**Pipeline position:** This pass runs AFTER constructor reuse expansion (Section 09). Input is ARC IR with RcInc/RcDec from both RC insertion (07) and reuse expansion (09). Execution order: 07 (RC insertion) → 09 (reuse expansion) → 08 (this pass). Section numbers indicate topic grouping, not execution order.

**Reference compilers:**
- **Swift** `lib/SILOptimizer/ARC/` -- 21 files implementing bidirectional dataflow with lattice states, ARCMatchingSet for pair elimination
- **Lean 4** implied by borrow analysis results -- unnecessary ops never inserted

**Key insight from Swift:** This optimization happens at the *ARC-annotated IR* level (between Section 07 and codegen), where we have full ownership semantics. NOT at the LLVM IR level where ownership information is lost.

**Heap layout context:** RC operations target the Roc-style refcount-at-negative-offset layout (see Section 01.6). The header is 8 bytes: `{ strong_count: i64 }`. The strong_count is at `ptr - 8` from the data pointer. Elimination of paired inc/dec operations saves not just the refcount arithmetic but also the pointer adjustment and memory access.

---

## 08.1 Lattice-Based RC State Machine

**Simplified from plan:** The plan specified four states (`None`, `Decremented`, `MightBeUsed`, `MightBeDecremented`) for bottom-up and (`None`, `Incremented`, `MightBeDecremented`, `MightBeUsed`) for top-down. The implementation uses two states per direction:

```rust
/// Top-down: tracking a forward scan for Inc→Dec pairs.
enum TopDownState {
    Incremented { inc_pos: usize },  // Seen Inc, looking for Dec
    MightBeUsed,                      // Variable used — can't eliminate
}

/// Bottom-up: tracking a backward scan for Dec→Inc pairs.
enum BottomUpState {
    Decremented { dec_pos: usize },  // Seen Dec, looking backward for Inc
    MightBeUsed,                      // Variable used — can't eliminate
}
```

**Why simpler than planned:** The `MightBeDecremented` state (for aliased Dec in different branches) is needed for cross-block analysis where branch merges create ambiguity. For V1 intra-block analysis, each block is analyzed independently — there are no branch merges to consider. The `None` state is implicit (variable not in the HashMap). This simplification is correct and sufficient.

**State transitions:**

| Current State | Event | New State |
|---------------|-------|-----------|
| (not tracked) | `RcInc(x)` | `Incremented(pos)` (top-down) |
| `Incremented` | `RcDec(x)` | → **MATCH** (eliminate pair) |
| `Incremented` | non-RC use of `x` | `MightBeUsed` |
| `Incremented` | `RcInc(x)` again | `Incremented(new_pos)` (restart) |
| `MightBeUsed` | `RcDec(x)` | removed (can't match) |
| (not tracked) | `RcDec(x)` | `Decremented(pos)` (bottom-up) |
| `Decremented` | `RcInc(x)` | → **MATCH** (eliminate pair) |
| `Decremented` | non-RC use of `x` | `MightBeUsed` |
| `Decremented` | `RcDec(x)` again | `Decremented(new_pos)` (restart) |
| `MightBeUsed` | `RcInc(x)` | removed (can't match) |

**Key safety property:** Only `RcInc; ...; RcDec` pairs (in program order) are eliminated. `RcDec; ...; RcInc` is NOT safe because the `RcDec` might free the object, making the subsequent `RcInc` a use-after-free.

- [x] Define `TopDownState` lattice (Incremented / MightBeUsed)
- [x] Define `BottomUpState` lattice (Decremented / MightBeUsed)
- [x] Implement state transitions for RcInc/RcDec (match endpoints)
- [x] Implement state transitions for non-RC uses (invalidation)
- [x] Handle batched `RcInc { count > 1 }` conservatively (treated as use)

## 08.2 Bidirectional Dataflow Analysis

**Two-pass approach per block (intra-block V1):**

1. **Top-down pass (forward):** Scan instructions 0..N. When we see `RcInc(x, 1)`, start tracking. When we see `RcDec(x)` and state is `Incremented`, match! Any non-RC instruction using `x` transitions to `MightBeUsed`.

2. **Bottom-up pass (backward):** Scan instructions N..0. When we see `RcDec(x)`, start tracking. When we see `RcInc(x, 1)` and state is `Decremented`, match! Same invalidation rule.

```rust
/// Scan a block forward looking for Inc→Dec pairs.
fn top_down_block_pass(block_idx: usize, body: &[ArcInstr], candidates: &mut Vec<EliminationCandidate>) {
    let mut state: FxHashMap<ArcVarId, TopDownState> = FxHashMap::default();
    for (j, instr) in body.iter().enumerate() {
        match instr {
            ArcInstr::RcInc { var, count: 1 } => { state.insert(*var, TopDownState::Incremented { inc_pos: j }); }
            ArcInstr::RcDec { var } => {
                if let Some(TopDownState::Incremented { inc_pos }) = state.get(var) {
                    candidates.push(EliminationCandidate { var: *var, block: block_idx, inc_pos: *inc_pos, dec_pos: j });
                }
                state.remove(var);
            }
            other => { for used in other.used_vars() { invalidate_td(&mut state, used); } }
        }
    }
}
```

**V1: intra-block only.** Cross-block elimination following Swift's `GlobalARCSequenceDataflow` is future work. For V1, each block is analyzed independently. This is sufficient to eliminate the most common redundant pairs (adjacent Inc+Dec, Inc+Dec with unrelated instructions between them).

- [x] Implement `top_down_block_pass` scanning forward
- [x] Implement `bottom_up_block_pass` scanning backward
- [x] Both passes find the same pairs for intra-block (safety net)
- [x] Deduplication of candidates from both passes
- [x] Conservative: batched Inc (count > 1) treated as use, not matched

## 08.3 Matching & Elimination

```rust
/// Public API: eliminate redundant RC pairs with cascading.
pub fn eliminate_rc_ops(func: &mut ArcFunction) -> usize {
    let mut total = 0;
    loop {
        let eliminated = eliminate_once(func);
        if eliminated == 0 { break; }
        total += eliminated;
    }
    total
}
```

**Cascading elimination:** After removing pairs, new adjacent pairs may be exposed. Example: `Inc(x); Inc(x); Dec(x); Dec(x)` — first pass removes inner pair `(1,2)`, second pass removes outer pair `(0,1)` (renumbered). The loop iterates until no more pairs are found.

**Elimination mechanics:** Candidates are grouped by block. For each block, the body and spans are rebuilt excluding the removed positions. Spans are maintained in parallel to preserve debug info (Section 13).

- [x] Implement `apply_eliminations` (rebuild body/spans excluding removed positions)
- [x] Implement cascading iteration (loop until fixed point)
- [x] Track elimination count for diagnostics (tracing::debug)
- [x] Span vectors correctly maintained after elimination

---

**Exit Criteria:** ✅ Redundant retain/release pairs are identified and eliminated. The elimination is provably safe (no use-after-free, no leaks). Simple patterns like `Inc(x); Dec(x)` with no intervening use are eliminated. 27 tests verify correctness across all ARC IR instruction types.

**Future work:**
- Cross-block elimination (Swift's `GlobalARCSequenceDataflow` pattern)
- Loop-aware analysis (currently conservative: intra-block only, loops not special-cased)
- Batched Inc matching (reduce `RcInc { count: N }` with N paired Decs)
