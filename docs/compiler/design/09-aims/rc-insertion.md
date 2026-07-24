---
title: "Unified Realization"
description: "Ori Compiler Design — AIMS Unified Ownership, Reuse, COW, and Drop Realization"
order: 905
section: "AIMS"
---

# Unified Realization

## The Emission Problem

Given a converged `AimsStateMap` that describes the complete memory fate of every variable at every program point, how do you translate those abstract lattice states into concrete, backend-neutral ARC IR facts and instructions? This is the realization problem — reading the analysis results and freezing all memory-management decisions before physical execution planning.

Traditional ARC optimizers sequence RC insertion, reuse detection/expansion, RC elimination, COW annotation, and drop hints, with each pass owning separate analysis and output. AIMS replaces them with a **single realization step** that reads the converged lattice state and emits all decisions in one coordinated walk.

The benefit is consistency — all decisions come from one data source, so logical ownership-event placement, reuse, COW, and drops cannot disagree. It also eliminates the need for a separate logical pair-elimination pass in the common case, because precise placement based on full lattice information avoids creating redundant event pairs. A counter-selecting physical plan may still optimize mechanism-local operations after layout selection.

These memory-lifetime decisions form part of AIMS's backend-neutral ownership, drop, and effect plan; the evaluator branches from canonical IR as the representation-abstract behavioral oracle. VM, LLVM, native, compiled-WASM, and JIT paths consume the same realized facts through physical projections without re-deriving ownership, drop, COW, or reuse policy.

## Two-Phase Realization

Realization proceeds in two phases, separated by `merge_blocks()`:

### Phase 1: Ownership Events + Reuse + Arg Ownership (Pre-Merge)

`realize_rc_reuse()` walks the IR forward, reading the converged `AimsStateMap` at each instruction:

1. **Arg ownership**: For each `Apply` and `Invoke`, determine per-argument ownership from the callee's `MemoryContract`. Write `arg_ownership` on the call instruction.

2. **Logical ownership-event decisions**: For each variable definition and use, consult the lattice state:
   - **Cardinality = Many**: Variable is used more than once — freeze the additional owner-credit events; the current carrier spells them `RcInc` with count = uses - 1
   - **Cardinality = Once**: Variable is used exactly once — no additional owner-credit event
   - **Cardinality = Absent**: Variable is never used after this point — freeze its release/cleanup event; the current carrier spells it `RcDec`
   - **Access = Borrowed**: Freeze no transferred owner credit or matching release; the source lifetime governs

3. **Reuse decisions**: For each logical death/construction proximity where `ShapeClass` matches and `Uniqueness` allows, emit `Reset`/`Reuse` pairs. The current carrier discovers the death through `RcDec`. See [Reuse Emission](reset-reuse.md) for details.

4. **Logical semantics binding**: Each `RcInc` and `RcDec` is bound to the
   exact logical value/drop semantics it must preserve. The shipped carrier
   stores a `RcStrategy` computed from `ValueRepr`; production stores stable
   `ValueSemanticsId`/`ExecutableDropPlanId` references and leaves physical
   strategy to the selected layout plan:

| Strategy | What It Does |
|----------|-------------|
| `HeapPointer` | Apply the RC operation to the value's sole RC-managed reference |
| `FatPointer` | Apply the RC operation to the reference component while preserving metadata |
| `Closure` | Apply the RC operation to the captured environment when one exists |
| `AggregateFields` | Traverse logical RC-bearing fields recursively |
| `InlineEnum` | Apply operation-specific enum policy; decrement processes the active variant's logical payload |
| `Iterator` | Treat iterator state as uniquely owned: increment is a no-op and decrement drops the state |
| `UserDrop` | Invoke the user-defined drop action once without RC arithmetic |

- This table documents the shipped migration mapping.
- The AIMS-owned portion is the logical event, reference-bearing field identity,
  multiplicity/order/edge, and drop obligation.
- `HeapPointer`, `FatPointer`, iterator helper selection, counter/header
  representation, pointer width, field offset, ABI, register, and slot layout
  are physical vocabulary.
- `VmLayoutPlan` and `CompiledLayoutPlan(TargetSpec)` choose and validate those
  encodings without changing the logical plan.

### Phase 2: COW + Drop Hints (Post-Merge)

After `merge_blocks()` cleans up the CFG, `realize_annotations()` walks the post-merge IR to emit annotations:

1. **COW annotations**: For each mutation site, determine the COW mode from the lattice:
   - **`StaticUnique`**: the facts prove exactly one logical owner — record that only the in-place path is required
   - **`StaticShared`**: the facts prove multiple logical owners — record that the copy path is required
   - **`Dynamic`**: logical owner multiplicity is unknown — record that a runtime sharing observation is required

   The selected physical plan may implement that observation with a counter,
   header, tag, side table, constant, or another proven mechanism. AIMS does
   not require a reference counter.

2. **Drop hints**: For each `RcDec`, check if the target is proven unique and freeze that fact for every physical consumer. The current LLVM projection maps it to `ori_buffer_drop_unique` instead of `ori_buffer_rc_dec`; the runtime names are not part of AIMS policy.

**Critical ordering**: Phase 2 uses `ArcVarId`-keyed lookups via `var_state_at_block_entry()`, not position-keyed state maps. Position-keyed maps (`block_entry_states`, `block_exit_states`) are invalidated by `merge_blocks()`. The `ArcVarId`-keyed lookups work because `merge_blocks()` preserves entry block IDs.

## Decision Functions

All realization decisions flow through a unified `decide()` function that reads the `AimsState` for a variable and returns the appropriate action. The decision logic for each output:

### Logical Ownership-Event Placement

```
if state.access == Borrowed:
    no transferred credit or matching release (source lifetime governs)
elif var is scalar or immortal:
    no dynamic ownership event
elif state.cardinality == Absent:
    freeze release/cleanup (current carrier: RcDec)
elif state.cardinality == Many:
    freeze additional owner credits (current carrier: RcInc, count = uses - 1)
else:
    no extra event (single use, naturally consumed)
```

### COW Mode

```
if state.uniqueness == Unique:
    StaticUnique (in-place path only)
elif state.uniqueness == Shared:
    StaticShared (copy path only)
else:
    Dynamic (runtime uniqueness decision required)
```

### Drop Hint

```
if state.uniqueness == Unique:
    mark unique-drop fact
else:
    leave the standard release/cleanup fact
```

## Borrowed Parameter Handling

Borrowed parameters receive special treatment:

- **Borrowed params** receive no transferred owner credit and create no matching release. Returning a borrowed parameter creates a caller-owned result credit; the current carrier spells that event `RcInc`.

- **Borrowed-derived variables** (projections or aliases of borrowed params) carry no independent owner credit until an **owned position** persists another owner. Realization freezes an additional credit there; the current carrier spells it `RcInc`:
  - `Construct` arguments
  - `PartialApply` captures (unless the capture is at a borrowed callee position and the closure does not escape)
  - `Apply`/`ApplyIndirect` arguments at owned positions
  - `Return` values

### Closure Capture Optimization

When `PartialApply` captures a borrowed-derived variable, the normal rule creates an additional owner credit because the closure persists the value. The current carrier spells that credit `RcInc`. The event can be omitted when both conditions hold:

1. The callee expects the corresponding parameter as `Borrowed`
2. The closure does not escape the current block (`dst` is not in `live_out`)

## Edge Cleanup

After per-block realization, variables that are live at a predecessor's exit but not live at a successor's entry create "edge gaps" — they need a release/cleanup event at the transition. The current carrier spells that event `RcDec`. Three cases arise:

**Single-predecessor blocks**: release/cleanup events are prepended to the successor block's body.

**Multi-predecessor with identical gaps**: All predecessors strand the same variables. The release/cleanup events are prepended to the successor block.

**Multi-predecessor with differing gaps**: Different predecessors strand different variables. A **trampoline block** (critical edge splitting) is created for each edge that needs cleanup.

## Why AIMS Needs No Separate Logical Pair-Elimination Pass

Traditional pipelines need a separate RC elimination pass because their sequential architecture creates redundant pairs:
- Perceus insertion creates pairs that borrow inference later makes unnecessary
- Reuse expansion creates slow-path pairs that interact with surrounding operations
- Identity normalization creates root-level pairs from projected ones

AIMS avoids most of these redundancies because:
- **Contracts are known before realization**: Borrow decisions are already incorporated into the lattice state
- **Reuse decisions are coordinated**: The same state map drives both ownership-event placement and reuse, so realization does not create unnecessary pairs around reuse sites
- **No identity normalization needed**: The lattice tracks variables through projections, so logical ownership decisions already bind the correct roots

The current compiled projection produces a minimal set of RC operations that
requires no further cleanup. Architecturally, AIMS freezes the minimal logical
ownership-event plan; RC instructions are one physical realization of it.

## Prior Art

**[Koka](https://github.com/koka-lang/koka)** and the [Perceus paper](https://www.microsoft.com/en-us/research/publication/perceus-garbage-free-reference-counting-with-reuse/) (Reinking et al., 2021) formalize liveness-based RC insertion and prove that it produces the minimum number of RC operations for a given ownership assignment. AIMS's realization can produce equivalent or better logical event plans because it has richer information (7 dimensions vs binary liveness). Physical storage and instruction selection remain target-plan decisions.

**[Lean 4](https://github.com/leanprover/lean4)** (`src/Lean/Compiler/IR/RC.lean`) implements liveness-based RC insertion independently. AIMS differs by combining ownership-event placement, reuse/COW admissibility, and drop obligations into one realization step rather than running them as separate semantic passes. A physical planner may then choose RC for a particular representation.

**[Swift](https://github.com/swiftlang/swift)** (`lib/SILOptimizer/ARC/`) takes the opposite approach: Swift inserts RC eagerly during SILGen and then optimizes them away using bidirectional dataflow analysis ("insert liberally, eliminate aggressively"). AIMS is "analyze comprehensively, freeze logical events precisely" — no separate logical elimination pass is needed; a selected physical plan still owns mechanism-local optimization.

## Design Tradeoffs

**Unified realization vs separate passes.** AIMS freezes ownership events, reuse/COW admissibility, and drop obligations from one data source in one walk. The benefit is consistency and a minimal logical plan. The cost is a more complex realization step and the requirement that the `AimsStateMap` is complete and correct before any fact is frozen. A bug in the lattice analysis affects all projections simultaneously. The shipped carrier still materializes RC-shaped records here; that is migration debt, not authority for a physical mechanism.

**Two-phase split at merge_blocks.** Phase 1 (RC + reuse) runs before block merging, Phase 2 (COW + drops) runs after. This is necessary because `merge_blocks()` invalidates position-keyed state maps, but COW and drop decisions need the simplified post-merge CFG. The split adds complexity but is structurally required.

**Logical identity binding.**

- The shipped `RcInc`/`RcDec` embeds `RcStrategy`, which prevents current
  consumers from repeating ownership analysis but leaks physical-shape
  vocabulary into the shared carrier.
- Production binds each event to stable `ValueSemanticsId` and
  `ExecutableDropPlanId` facts instead.
- Computing policy from the variable's type inside a VM, LLVM, native,
  compiled-WASM, or JIT consumer remains forbidden; physical strategy is
  selected once by that consumer's validated layout plan.
