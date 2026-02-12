---
plan: "dpr_arc-optimization_02112026"
title: "Design Pattern Review: ARC Optimization"
status: in-progress
---

# Design Pattern Review: ARC Optimization

## Ori Today

Ori's ARC optimization lives in `ori_arc`, a standalone crate (~3,500 lines) that operates on a basic-block SSA-like IR (`ArcFunction`/`ArcBlock`/`ArcInstr`/`ArcTerminator`). The pipeline runs six passes in a fixed order: **lower** (from canonical IR) -> **borrow inference** -> **liveness** -> **RC insertion** (07) -> **reset/reuse detection + expansion** (09) -> **RC elimination** (08). This ordering is well-documented and tested with integration tests in `lib.rs` that verify the correct order produces fewer RC ops than the wrong order.

The type classification system (`ArcClassifier`) provides a clean three-way split: `Scalar` (no RC needed), `DefiniteRef` (always needs RC), and `PossibleRef` (unresolved generics, conservative). Classification is cached via `RefCell<FxHashMap>` with cycle detection for recursive types. The classifier has a fast path for pre-interned primitive indices (0-11), avoiding hash lookups for common types. This design is directly inspired by Lean 4's `isScalar`/`isPossibleRef`/`isDefiniteRef` methods on `IRType`.

**What works well:**

- The IR design is solid. `ArcInstr` covers the right instruction vocabulary (Let, Apply, Project, Construct, RcInc, RcDec, IsShared, Set, Reset, Reuse), and `ArcTerminator` handles all control flow patterns including `Invoke` (which models potentially-throwing calls with normal/unwind successors). Variable types are tracked in a flat `Vec<Idx>` indexed by `ArcVarId`, giving O(1) type lookups.

- RC insertion (`rc_insert.rs`) implements a correct backward liveness-driven algorithm. It handles edge cases well: dead definitions get immediate `RcDec`, multi-use variables get `RcInc` before each additional use, borrowed-derived variables crossing into owned positions get explicit increments, and cross-block live variable gaps are handled via trampoline blocks that insert the necessary `RcDec` operations.

- Constructor reuse expansion (`expand_reuse.rs`) is the most sophisticated pass. It generates conditional fast/slow paths with two sub-optimizations: *projection-increment erasure* (skip `RcInc` for projected fields on the fast path since the memory is being reused in-place, restore them on the slow path) and *self-set elimination* (skip `Set` instructions when writing a field back to the same position it was projected from).

**Gaps and pain points:**

1. **Borrow inference is parameter-only.** `infer_borrows()` tracks ownership at the function parameter level but not per-variable within function bodies. This means a variable used once in a borrowed position and once in an owned position forces the parameter to `Owned`, inserting unnecessary `RcInc` at the call site. Lean 4 and Swift both track per-variable ownership.

2. **Reset/reuse detection is intra-block only.** `detect_reset_reuse()` scans each block independently with a simple forward pattern match (find `RcDec`, look for matching `Construct` later in the same block). Cross-block patterns (decrement in a predecessor, construct in a successor) are missed entirely. Lean 4's approach works on the full CFG.

3. **RC elimination is mostly intra-block.** The `eliminate_rc_ops()` pass uses bidirectional lattice states (`TopDownState`/`BottomUpState`) within blocks but only handles cross-block elimination for single-predecessor blocks. Multi-predecessor joins, loop-carried redundancies, and diamond patterns are all missed. Swift's ARC optimizer uses full dataflow analysis across the entire CFG.

4. **No explicit RC treatment for closure captures.** When a closure captures a variable, the current system treats `PartialApply` captured args as unconditionally owned. There is no analysis to determine whether a captured variable is only read (and could remain borrowed) or whether it escapes the closure's lifetime.

5. **Liveness analysis lacks RC-specific refinements.** The liveness pass (`compute_liveness()`) is a standard backward dataflow analysis. It does not distinguish between "live for reading" vs "live for ownership transfer," which means variables kept alive only for a final `RcDec` inflate live ranges and block reset/reuse opportunities.

## Prior Art

### Swift -- Dual-Pass Dataflow with Ownership Lattice

Swift's ARC optimizer (`lib/SILOptimizer/ARC/`) uses two complementary dataflow passes operating on SIL (Swift Intermediate Language). The **top-down pass** tracks `RcInc` operations forward through the CFG, building a state lattice per variable: `None -> Known -> MergeUp -> TopDown`. The **bottom-up pass** tracks `RcDec` operations backward with states: `None -> Known -> MergeUp -> BottomUp`. When a top-down `RcInc` and bottom-up `RcDec` meet on the same variable with no intervening aliasing or escaping uses, both are eliminated.

The key design choice is **ownership qualification** on every value reference. Swift's `ValueOwnershipKind` enum (`Unowned`, `Owned`, `Guaranteed`, `None`) is attached to every SSA value, not just function parameters. This allows the optimizer to reason precisely about which operations transfer ownership vs which merely observe a value. The `OwnershipChecker` validates these annotations, ensuring the optimizer's assumptions are sound.

Swift also has a **move-only value** analysis that tracks whether a value has been consumed, allowing the optimizer to prove that certain `RcDec` operations are the final use and can be replaced with a destructive move. This interacts with ARC optimization: if a value is provably consumed exactly once, the retain/release pair around it can be eliminated entirely.

The tradeoff: Swift's approach is the most thorough but also the most complex. The dual-pass dataflow requires careful handling of aliasing (Swift values can alias through class references), which Ori avoids since it has value semantics with ARC-managed heap storage.

### Lean 4 -- Explicit RC Graph with Fixpoint Borrow Inference

Lean 4's RC optimization (`src/Lean/Compiler/IR/RC.lean`, `Borrow.lean`, `ExpandResetReuse.lean`) operates on LCNF (Lean Compiler Normal Form), a basic-block IR similar to Ori's `ArcFunction`. The pipeline mirrors Ori's: `inferBorrow` -> `explicitRC` -> `expandResetReuse`.

The critical design choice is **per-variable borrow inference** combined with `DerivedValInfo`. Rather than tracking ownership only at function parameters, Lean tracks whether each local variable binding is derived from a borrowed parameter vs an owned one. A variable projected from a borrowed parameter inherits borrowed status; a variable returned from a function call gets owned status. This propagation is captured in `DerivedValInfo`, a per-variable annotation that flows through the entire function body.

Lean's `ExpandResetReuse` works on the full CFG, not just intra-block. It identifies `dec x; ... ctor(fields)` patterns across basic block boundaries by following the control flow graph. The expansion generates an `isShared` check and conditional branch, just like Ori's, but the pattern detection is more powerful because it operates inter-procedurally via the borrow annotations.

Lean also has a **speculative reset** optimization: when a variable is decremented and a constructor of the same type appears in *any* successor block (not just the immediately following code), the runtime check is inserted speculatively. If the fast path succeeds, allocation is avoided; if not, the slow path allocates fresh memory with no correctness impact.

The tradeoff: Lean's approach is clean and well-suited to functional languages. The per-variable borrow tracking adds modest complexity but significantly improves RC elimination rates. However, Lean does not handle mutable closures or aliasing since it is purely functional.

### Koka -- FBIP Language-Level Contracts

Koka takes a fundamentally different approach with FBIP (Functional But In-Place). Rather than optimizing RC operations after the fact, Koka's type system expresses reuse intent at the *language level*. Functions are checked for "frame-bounded" allocation: a function is FBIP if every allocation is balanced by a deallocation of the same-sized object within the function body.

The compiler (`src/Core/Borrowed.hs`, `src/Core/CheckFBIP.hs`) performs borrow analysis similar to Lean's, tracking which parameters are borrowed vs owned. But the key innovation is `CheckFBIP`: a verification pass that ensures user-written code (especially data structure traversals) meets the FBIP contract. If a function pattern-matches a constructor and rebuilds a constructor of the same type, Koka guarantees in-place reuse at the type system level rather than relying on runtime `isShared` checks.

Koka's `Borrowed.hs` module tracks borrowing with a lattice: `Own -> Borrow -> Mixed`. The `Mixed` state (used both borrowed and owned) triggers a warning, nudging the programmer to restructure code for better reuse. This is a form of **cooperative optimization**: the compiler and programmer work together, rather than the compiler heroically optimizing arbitrary code.

The tradeoff: FBIP is the most elegant solution for functional data structure transformations, but it requires programmer awareness and cannot optimize arbitrary imperative-style code. For Ori, which supports both functional and imperative patterns, pure FBIP is too restrictive -- but the *verification pass* concept is valuable.

## Proposed Best-of-Breed Design

### Core Idea

The proposed design extends Ori's existing pipeline with three targeted improvements drawn from all three reference compilers, plus one Ori-unique opportunity:

1. **Per-variable borrow tracking** (from Lean 4): Replace parameter-only borrow inference with `DerivedOwnership` annotations on every local variable. This is the single highest-impact change -- it unlocks better RC elimination, better reset/reuse detection, and better closure analysis.

2. **Full-CFG RC elimination** (from Swift): Extend the current intra-block bidirectional lattice to a proper dataflow analysis that handles multi-predecessor joins and loop-carried redundancies. This is the second-highest-impact change.

3. **Cross-block reset/reuse detection** (from Lean 4): Extend pattern matching to find `RcDec` -> `Construct` pairs across block boundaries using liveness and dominance information.

4. **FBIP verification diagnostics** (inspired by Koka, unique to Ori): Since Ori has mandatory testing and capability-based effects, add an *optional* FBIP analysis pass that *reports* (not errors on) missed reuse opportunities. This gives programmers actionable feedback without restricting the language.

### Key Design Choices

1. **Per-variable `DerivedOwnership` tracking** (Lean 4's `DerivedValInfo`). Every `ArcVarId` gets an ownership annotation: `Owned`, `BorrowedFrom(ArcVarId)`, or `Fresh`. Projections from borrowed variables inherit borrowed status. Function call results are `Owned`. Constructed values are `Fresh` (just allocated, refcount = 1). This replaces the current boolean `is_borrow_derived` check in `rc_insert.rs` with a richer lattice that flows through the entire function body. Ori's value semantics (no aliasing) make this simpler than Swift's version, since we never need to worry about two variables pointing to the same object through different paths.

2. **Dataflow-based RC elimination** (Swift's dual-pass, adapted). Extend `eliminate_rc_ops()` with proper forward/backward dataflow over the full CFG. The forward pass propagates `RcInc` availability; the backward pass propagates `RcDec` availability. At merge points, use meet (intersection) for soundness. At loop headers, start with bottom (no availability) and iterate to fixpoint. The current intra-block lattice (`TopDownState`/`BottomUpState`) is preserved as the inner loop of each dataflow iteration, avoiding a full rewrite. Ori's lack of aliasing means we do not need Swift's `MayAlias` analysis, which is the primary source of complexity in Swift's version.

3. **Dominance-guided reset/reuse** (Lean 4's approach, adapted). Replace the current forward-scan pattern matcher with a dominance-tree walk. A `RcDec(x)` in block B can be paired with a `Construct` in block C if: (a) B dominates C, (b) `x` is not live at C's entry, (c) the types match, and (d) there is no other `RcDec(x)` or use of `x` on any path from B to C. Dominance is already implicitly available from the CFG structure; adding an explicit dominator tree is a ~50-line addition to `graph.rs`.

4. **Liveness refinement: ownership-sensitive liveness** (novel, addresses identified gap). Split live sets into `live_for_use` (variable will be read) and `live_for_drop` (variable is only alive to be decremented). Variables in `live_for_drop` do not block reset/reuse of their memory, since the drop *is* the reuse trigger. This requires a small extension to `BlockLiveness` but significantly improves reset/reuse detection rates.

5. **Closure capture analysis** (addresses identified gap). When processing `PartialApply`, check whether each captured variable is used in a borrowed or owned position within the closure body (requires looking up the closure's `ArcFunction` borrow annotations). Captured variables used only in borrowed positions remain borrowed at the capture site, avoiding unnecessary `RcInc`/`RcDec` pairs.

6. **FBIP diagnostics** (Koka-inspired, Ori-unique). Add an optional analysis pass (gated behind `ori check --strict` or a per-function `@fbip` annotation) that reports when a function *almost* achieves in-place reuse but misses due to a specific pattern. Example diagnostic: "Function `map` deconstructs `list[T]` and constructs `list[U]` -- add `@fbip` to verify in-place reuse when `T == U`." This is not an optimization pass; it is a diagnostic that helps programmers write reuse-friendly code. It fits Ori's philosophy of mandatory verification and explicit effects.

### What Makes Ori's Approach Unique

Ori's ARC system operates in a sweet spot that none of the reference compilers occupy:

- **Unlike Swift**, Ori has value semantics with no aliasing. This eliminates the need for alias analysis in RC elimination, making the full-CFG dataflow pass significantly simpler. Swift's `MayAlias` queries are the primary source of false negatives in its ARC optimizer; Ori does not have this problem.

- **Unlike Lean 4**, Ori supports mutable local variables and imperative control flow. This means Ori's reset/reuse detection must handle `Set` instructions (mutating fields in place) and loop-carried variables, which Lean does not encounter. However, Ori's `Set` instruction is already in the IR and the expansion pass already handles it.

- **Unlike Koka**, Ori does not require FBIP compliance. But Ori's mandatory testing and `@test` annotations create a natural home for FBIP *verification*: when a function is annotated `@fbip`, the test runner can verify that the function achieves in-place reuse for all tested inputs. This is cooperative optimization (like Koka) without the language-level restriction.

- **Ori's capability-based effects** create a unique opportunity for RC optimization. A function with `uses Pure` (no side effects) can be analyzed more aggressively: its arguments cannot escape through I/O, its return value is the only observable output, and intermediate allocations are guaranteed to be temporary. This purity information can be fed into borrow inference to default more parameters to borrowed.

### Concrete Types & Interfaces

```rust
/// Per-variable ownership annotation (replaces parameter-only Ownership).
/// Tracks where each variable's ownership comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedOwnership {
    /// This variable owns its value (function call result, literal, etc.).
    Owned,
    /// This variable borrows from another variable (projection, alias).
    BorrowedFrom(ArcVarId),
    /// This variable was just constructed (refcount = 1, no other refs).
    Fresh,
}

/// Extended liveness information separating use-liveness from drop-liveness.
#[derive(Clone, Debug)]
pub struct RefinedLiveness {
    /// Variables that will be read (used in an instruction operand).
    pub live_for_use: FxHashSet<ArcVarId>,
    /// Variables that are only alive to be decremented (no reads remain).
    pub live_for_drop: FxHashSet<ArcVarId>,
}

/// Dominator tree for cross-block reset/reuse detection.
pub struct DominatorTree {
    /// idom[block_id] = immediate dominator block_id (None for entry).
    idom: Vec<Option<ArcBlockId>>,
}

impl DominatorTree {
    /// Build dominator tree from CFG using Cooper-Harvey-Kennedy algorithm.
    pub fn build(func: &ArcFunction) -> Self { /* ... */ }

    /// Returns true if `a` dominates `b`.
    pub fn dominates(&self, a: ArcBlockId, b: ArcBlockId) -> bool { /* ... */ }

    /// Iterate blocks in dominator tree preorder.
    pub fn preorder(&self) -> impl Iterator<Item = ArcBlockId> { /* ... */ }
}

/// Forward dataflow state for RC elimination (extends current TopDownState).
#[derive(Clone, Debug)]
struct ForwardRcState {
    /// Variables with available (un-cancelled) RcInc, keyed by var.
    available_incs: FxHashMap<ArcVarId, IncInfo>,
}

/// Backward dataflow state for RC elimination (extends current BottomUpState).
#[derive(Clone, Debug)]
struct BackwardRcState {
    /// Variables with available (un-cancelled) RcDec, keyed by var.
    available_decs: FxHashMap<ArcVarId, DecInfo>,
}

/// FBIP analysis result for a single function.
#[derive(Clone, Debug)]
pub struct FbipReport {
    /// Reuse opportunities that were successfully detected.
    pub achieved: Vec<ReuseOpportunity>,
    /// Reuse opportunities that were missed, with reasons.
    pub missed: Vec<MissedReuse>,
    /// Whether the function is fully FBIP-compliant.
    pub is_fbip: bool,
}

/// A missed reuse opportunity with diagnostic information.
#[derive(Clone, Debug)]
pub struct MissedReuse {
    pub dec_span: Option<Span>,
    pub construct_span: Option<Span>,
    pub reason: MissedReuseReason,
}

#[derive(Clone, Debug)]
pub enum MissedReuseReason {
    /// Types don't match (different sizes).
    TypeMismatch { dec_type: Idx, construct_type: Idx },
    /// Variable used between decrement and construct.
    IntermediateUse { use_span: Option<Span> },
    /// Cross-block pattern not on dominator path.
    NoDominance,
    /// Variable is shared (multi-reference), runtime check needed.
    PossiblyShared,
}

// Updated borrow inference signature:
/// Infer per-variable derived ownership for a function.
pub fn infer_derived_ownership(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
) -> Vec<DerivedOwnership>; // indexed by ArcVarId

// Updated RC elimination signature:
/// Eliminate redundant RC operations using full-CFG dataflow.
pub fn eliminate_rc_ops_dataflow(
    func: &mut ArcFunction,
    ownership: &[DerivedOwnership],
);

// Updated reset/reuse detection signature:
/// Detect reset/reuse opportunities across the full CFG.
pub fn detect_reset_reuse_cfg(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    dom_tree: &DominatorTree,
    liveness: &[RefinedLiveness], // indexed by block
);
```

**Updated pipeline:**

```rust
fn run_arc_pipeline(func: &mut ArcFunction, classifier: &dyn ArcClassification) {
    // Phase 1: Analysis
    let sigs = collect_function_sigs(func);
    let ownership = infer_derived_ownership(func, &sigs, classifier);
    let dom_tree = DominatorTree::build(func);
    let liveness = compute_refined_liveness(func, classifier);

    // Phase 2: RC insertion (uses ownership for smarter decisions)
    insert_rc_ops(func, classifier, &liveness, &ownership);

    // Phase 3: Reset/reuse (uses dominator tree for cross-block)
    detect_reset_reuse_cfg(func, classifier, &dom_tree, &liveness);
    expand_reset_reuse(func, classifier);

    // Phase 4: RC elimination (full-CFG dataflow)
    eliminate_rc_ops_dataflow(func, &ownership);
}
```

## Implementation Roadmap

### Phase 1: Foundation (COMPLETED 2026-02-12)

- [x] Add `DerivedOwnership` enum to `ownership.rs`, alongside existing `Ownership` — 3 variants: `Owned`, `BorrowedFrom(ArcVarId)`, `Fresh`
- [x] Implement `infer_derived_ownership()` in `borrow.rs` -- forward SSA pass over basic blocks with transitive borrowing resolution; handles projections, let aliases, construct, apply, block params
- [x] Add `DominatorTree` to `graph.rs` using Cooper-Harvey-Kennedy algorithm — `build()`, `dominates()`, `idom_for()` methods
- [x] Add `RefinedLiveness` to `liveness.rs` -- `compute_refined_liveness()` splits into `live_for_use` and `live_for_drop` sets
- [x] Add unit tests for each new analysis: 25+ derived ownership tests (simple/branching/loop/mutual recursion/tail calls/projections); dominator tree correctness; refined liveness distinguishing use vs drop

### Phase 2: Core optimizations

- [ ] Extend `detect_reset_reuse()` to use dominator tree and refined liveness for cross-block pattern detection -- keep intra-block fast path, add inter-block slow path (dominator tree exists but not yet integrated; detection is still intra-block only)
- [ ] Extend `eliminate_rc_ops()` with forward/backward dataflow over full CFG -- preserve existing intra-block lattice as inner loop, add iterative fixpoint outer loop with meet at join points (intra-block lattice exists; no fixpoint loop yet)
- [ ] Update `insert_rc_ops()` to use `DerivedOwnership` instead of `compute_borrows()` -- variables with `BorrowedFrom` status skip `RcInc` when passed to borrowed parameters (infrastructure ready, not fully wired)
- [ ] Add closure capture analysis in `PartialApply` handling -- look up callee borrow annotations to determine which captures can remain borrowed
- [ ] Integration tests verifying cross-block reset/reuse fires on linked-list `map` pattern
- [ ] Integration tests verifying full-CFG elimination removes retain/release pairs across diamond patterns

### Phase 3: Polish and diagnostics

- [x] Implement `FbipReport` analysis pass (read-only, no IR mutation) -- `FbipReport`, `ReuseOpportunity`, `MissedReuse`, `MissedReuseReason` types + `analyze_fbip()` entry point in `fbip.rs`
- [ ] Wire FBIP diagnostics into `ori check --strict` output via `ori_diagnostic`
- [ ] Add `@fbip` function annotation support in parser and evaluator (optional, for explicit verification)
- [ ] Benchmark ARC pipeline on standard test suite -- measure RC op count reduction from Phase 2 changes
- [ ] Document the extended pipeline in `docs/compiler/design/10-llvm-backend/` ARC section
- [ ] Investigate purity-aware borrow inference: functions with `uses Pure` default captured variables to borrowed

## References

### Ori Source Files
- `compiler/ori_arc/src/lib.rs` -- crate entry, pipeline tests, exports
- `compiler/ori_arc/src/ir.rs` -- ARC IR types (ArcFunction, ArcBlock, ArcInstr, ArcTerminator)
- `compiler/ori_arc/src/classify.rs` -- ArcClassifier with 3-way classification and caching
- `compiler/ori_arc/src/borrow.rs` -- iterative fixed-point borrow inference (parameter-level) + `infer_derived_ownership()` per-variable forward SSA pass (Phase 1)
- `compiler/ori_arc/src/liveness.rs` -- backward dataflow liveness analysis + `RefinedLiveness` with use/drop split (Phase 1)
- `compiler/ori_arc/src/rc_insert.rs` -- Perceus-style RC insertion with edge cleanup
- `compiler/ori_arc/src/reset_reuse.rs` -- intra-block reset/reuse detection (cross-block pending Phase 2)
- `compiler/ori_arc/src/rc_elim.rs` -- bidirectional intra-block + limited cross-block elimination (full-CFG pending Phase 2)
- `compiler/ori_arc/src/expand_reuse.rs` -- constructor reuse expansion with sub-optimizations
- `compiler/ori_arc/src/ownership.rs` -- Ownership/AnnotatedParam/AnnotatedSig + `DerivedOwnership` enum (Phase 1)
- `compiler/ori_arc/src/graph.rs` -- shared CFG utilities (predecessors, invoke defs) + `DominatorTree` with CHK algorithm (Phase 1)
- `compiler/ori_arc/src/fbip.rs` -- `FbipReport`, `ReuseOpportunity`, `MissedReuse` types + `analyze_fbip()` (Phase 3)
- `compiler/ori_arc/src/test_helpers.rs` -- shared test utilities for ARC unit tests
- `compiler/ori_llvm/src/codegen/arc_emitter.rs` -- LLVM codegen for ARC operations

### Reference Compiler Sources
- Swift: `swift/lib/SILOptimizer/ARC/` -- dual-pass dataflow ARC optimization
- Swift: `swift/include/swift/AST/Ownership.h` -- ValueOwnershipKind enum
- Swift: `swift/lib/SIL/` -- SIL IR with ownership annotations
- Lean 4: `lean4/src/Lean/Compiler/IR/RC.lean` -- explicit RC insertion
- Lean 4: `lean4/src/Lean/Compiler/IR/Borrow.lean` -- per-variable borrow inference with DerivedValInfo
- Lean 4: `lean4/src/Lean/Compiler/IR/ExpandResetReuse.lean` -- CFG-level reset/reuse expansion
- Koka: `koka/src/Core/Borrowed.hs` -- borrow analysis with Own/Borrow/Mixed lattice
- Koka: `koka/src/Core/CheckFBIP.hs` -- FBIP verification pass
