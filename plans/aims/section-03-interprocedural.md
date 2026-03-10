---
section: "03"
title: "Interprocedural Analysis"
status: not-started
reviewed: true  # 2026-03-10
goal: "SCC-based fixed-point computing unified MemoryContract (ownership+uniqueness+demand+locality+effects+FIP) for all functions"
inspired_by:
  - "Lean 4 borrow inference (src/Lean/Compiler/IR/Borrow.lean)"
  - "ori_arc borrow (compiler/ori_arc/src/borrow/mod.rs)"
  - "ori_arc uniqueness inter (compiler/ori_arc/src/uniqueness/inter/mod.rs)"
  - "FP2 FIP certification (Lorenzen et al., ICFP 2023)"
  - "Oxidizing OCaml locality modes (Lorenzen et al., ICFP 2024)"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "MemoryContract — Unified Function Contract"
    status: not-started
  - id: "03.2"
    title: "SCC-Based Fixed-Point"
    status: not-started
  - id: "03.3"
    title: "Contract Inference Rules"
    status: not-started
  - id: "03.4"
    title: "Builtin Function Contracts"
    status: not-started
  - id: "03.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Interprocedural Analysis

**Status:** Not Started

**Goal:** Compute a `MemoryContract` for every function in the program via SCC-based
fixed-point iteration. The contract encodes per-parameter access class, consumption,
uniqueness, demand, locality, and escape behavior, plus return value properties, effect
summaries, and FIP certification status. This replaces both `infer_borrows_scc()` and
`analyze_program()` from the current system.

**Context:** The current `ori_arc` runs two separate interprocedural analyses:
borrow inference (`infer_borrows_scc` in `borrow/mod.rs`, SCC fixed-point producing
`AnnotatedSig`) and uniqueness analysis (`analyze_program` in `uniqueness/inter/mod.rs`,
SCC fixed-point producing `UniquenessSummary`). These are independent passes that
don't share information. AIMS fuses them into one SCC fixed-point that computes
both ownership and uniqueness simultaneously — and adds cardinality, locality, effects,
and FIP certification.

**Reference implementations:**
- **Lean 4** `src/Lean/Compiler/IR/Borrow.lean`: `collect_O` function that identifies
  variables needing ownership; monotonic `Borrowed → Owned` promotion
- **ori_arc** `borrow/mod.rs`: Current SCC-based borrow inference (already well-designed)
- **ori_arc** `uniqueness/inter/mod.rs`: Current interprocedural uniqueness analysis
- **FP²** (Lorenzen et al., ICFP 2023): FIP certification criterion — functions that
  run with no allocation, no deallocation, constant stack, given unique arguments

**Depends on:** Section 01 (lattice definition).

---

## 03.1 MemoryContract — Unified Function Contract

**File(s):** `compiler/ori_arc/src/aims/contract.rs` (NEW)

The function contract that AIMS computes and consumes. Replaces `AnnotatedSig`
and `UniquenessSummary`. The name `MemoryContract` reflects that this is a richer
object than a signature — it encodes what the function requires, what it guarantees,
and what it certifies.

- [ ] Define `MemoryContract`:
  ```rust
  /// Unified function contract for AIMS analysis.
  ///
  /// Encodes per-parameter requirements, return value guarantees,
  /// effect summaries, context behavior, and FIP certification.
  /// Computed by interprocedural fixed-point, consumed by
  /// intraprocedural analysis at call sites.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct MemoryContract {
      /// Per-parameter: what does the callee require?
      pub params: Vec<ParamContract>,
      /// Return value guarantees.
      pub return_info: ReturnContract,
      /// Effect summary: what memory effects may the function produce?
      pub effects: EffectSummary,
      /// Constructor-context behavior (Stage 3 TRMC).
      pub context_behavior: ContextBehavior,
      /// FIP certification status (Stage 2).
      pub fip: FipContract,
  }

  /// Migration alias — code that previously referenced AimsSig continues to work.
  pub type AimsSig = MemoryContract;

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct ParamContract {
      /// Access requirement: does the callee need ownership, or is borrowing enough?
      pub access: AccessClass,
      /// Consumption requirement: how is the parameter consumed?
      pub consumption: Consumption,
      /// Demand: how many times does the callee use this parameter?
      pub cardinality: Cardinality,
      /// May this parameter's value escape the callee (stored, returned, shared)?
      pub may_escape: bool,
      /// May this parameter's value be shared (refcount > 1) by the callee?
      pub may_share: bool,
      /// Locality lower bound: the callee guarantees this parameter stays at
      /// least this local (v1: always `Unknown`).
      pub locality_bound: Locality,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct ReturnContract {
      /// Return value uniqueness (is the returned value always fresh/unique?)
      pub uniqueness: Uniqueness,
      /// Whether the function preserves freshness: if all RC'd inputs are
      /// Unique, the output is guaranteed Unique.
      pub preserves_freshness: bool,
      /// Locality of the returned value (v1: `HeapEscaping` for most).
      pub locality: Locality,
      /// Shape class of the return value.
      pub shape: ShapeClass,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
  pub struct EffectSummary {
      /// May the function allocate on any code path?
      pub may_allocate: bool,
      /// Allocations are only on slow paths guarded by uniqueness checks
      /// (i.e., if all RC'd inputs are Unique, no allocation occurs).
      /// When `may_allocate == true && alloc_only_on_slow_path == true`,
      /// the function is FIP-eligible with Conditional preconditions.
      pub alloc_only_on_slow_path: bool,
      /// May the function create shared references?
      pub may_share: bool,
      /// May the function throw exceptions/panics?
      pub may_throw: bool,
  }

  /// Constructor-context behavior for TRMC (Stage 3).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
  pub struct ContextBehavior {
      /// Does this function preserve a constructor context passed to it?
      pub preserves_context: bool,
      /// Does this function consume a context hole?
      pub consumes_hole: bool,
  }

  /// FIP certification status (Stage 2).
  ///
  /// Based on FP² (Lorenzen et al., ICFP 2023): a function is FIP when
  /// it can run with no allocation, no deallocation, and constant stack
  /// space, provided arguments are unique.
  ///
  /// **Dependency note**: `BitSet` is from the `bit-set` crate (add to
  /// `ori_arc/Cargo.toml`). Alternative: use a `SmallVec<u64>` or
  /// `Vec<bool>` indexed by param position if a full BitSet crate
  /// dependency is undesirable.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum FipContract {
      /// Function cannot be certified FIP.
      Never,
      /// Function is FIP when the specified parameters are unique.
      /// The BitSet indexes into MemoryContract.params.
      Conditional { requires_unique_params: BitSet },
      /// Function is unconditionally FIP (all code paths allocation-free).
      Certified,
  }
  ```

- [ ] Define `MemoryContract::all_borrowed(n, fip_initial: FipContract)` — initial
  most-optimistic contract (all params borrowed, unique return, no effects).
  The caller controls FIP initialization via the `fip_initial` parameter:
  - Stage 1: pass `FipContract::Never` (FIP inference disabled, no iteration needed)
  - Stage 2: pass `FipContract::Certified` (most optimistic, refined downward
    during fixed-point iteration)
- [ ] Define `MemoryContract::join(&self, other: &Self) -> Self` — componentwise join
  for convergence detection
- [ ] **Variadic and default parameter handling**:
  By the time code reaches ARC IR, variadic parameters have been lowered to a
  single list parameter and default parameters have been resolved to explicit
  arguments. `MemoryContract.params` maps 1:1 to `ArcFunction.params`. No
  special cases are needed. Verify this assumption holds by checking that
  `func.params.len() == contract.params.len()` at every consumption site.
- [ ] Implement conversion: `MemoryContract → AnnotatedSig` (for compatibility during migration):
  - `ParamContract.access == Borrowed` → `Ownership::Borrowed`
  - `ParamContract.access == Owned` → `Ownership::Owned`
  - `ParamContract.consumption == Dead` → `Ownership::Borrowed` (dead params need no RC)
  - **Consumption → DerivedOwnership mapping** (for code that reads `DerivedOwnership`):
    - `(Owned, Linear)` → `DerivedOwnership::Owned` (consumed once, standard ownership)
    - `(Owned, Affine)` → `DerivedOwnership::Owned` (may drop, still owned)
    - `(Owned, Unrestricted)` → `DerivedOwnership::Owned` (full RC)
    - `(Borrowed, *)` → `DerivedOwnership::BorrowedFrom` (borrowed view)
    - `FRESH` state at return → `DerivedOwnership::Fresh`
  - **ReturnContract → AnnotatedSig.return_ownership mapping**:
    - `return_info.uniqueness == Unique` + `preserves_freshness` → unique return
    - `return_info.uniqueness == Shared` → shared return
    - `return_info.uniqueness == MaybeShared` → conservative (unknown)
- [ ] Implement conversion: `MemoryContract → UniquenessSummary` (for compatibility during migration):
  - `return_info.uniqueness` maps directly
  - `return_info.preserves_freshness` maps directly
  - `params` maps: each `ParamContract` → `Uniqueness::MaybeShared` (current system doesn't
    track per-param uniqueness from callers)

---

## 03.2 SCC-Based Fixed-Point

**File(s):** `compiler/ori_arc/src/aims/interprocedural.rs` (NEW)

> **Note: File size.** Estimated ~800 lines. Should split: `interprocedural.rs` for the
> SCC loop (~300 lines), `contract.rs` for `MemoryContract` type + conversions (~250 lines, from 03.1),
> `builtins.rs` for builtin signatures (~300 lines, from 03.4). Each under 500 lines.

The interprocedural analysis follows the same SCC structure as the current borrow
inference but computes unified signatures.

Note: `uniqueness::inter::analyze_program` already exists. The AIMS version lives in
`aims::interprocedural` to avoid naming collision.

- [ ] Implement `analyze_program(functions, classifier, builtins, interner) -> FxHashMap<Name, MemoryContract>`
  (where `classifier: &dyn ArcClassification`, `interner` needed for builtin method name lookup):
  ```rust
  pub fn analyze_program(
      functions: &[ArcFunction],
      classifier: &dyn ArcClassification,
      builtins: &BuiltinOwnershipSets,
      interner: &ori_ir::StringInterner,
  ) -> FxHashMap<Name, MemoryContract> {
      let call_graph = CallGraph::build(functions);
      let sccs = compute_sccs(&call_graph);
      let topo = topological_order(&sccs);

      let mut sigs: FxHashMap<Name, MemoryContract> = FxHashMap::default();

      // Initialize all signatures to most-optimistic (all borrowed, unique return).
      // Stage 1: FipContract::Never (FIP inference disabled).
      // Stage 2: FipContract::Certified (most optimistic, refined during iteration).
      let fip_initial = FipContract::Never; // Stage 1
      for func in functions {
          let n_params = func.params.len();
          sigs.insert(func.name, MemoryContract::all_borrowed(n_params, fip_initial.clone()));
      }

      // Process SCCs in topological order (callees before callers)
      // Note: is_recursive() requires &CallGraph. compute_sccs() returns
      // forward topo order, so topological_order() wraps iter().collect().
      for scc in &topo {
          if !scc.is_recursive(&call_graph) {
              // Non-recursive: single pass
              analyze_scc_single(&scc, functions, classifier, &mut sigs, builtins);
          } else {
              // Recursive/mutual: fixed-point iteration
              analyze_scc_fixpoint(&scc, functions, classifier, &mut sigs, builtins);
          }
      }

      sigs
  }
  ```

- [ ] Implement `analyze_scc_single` — single-pass analysis for non-recursive functions
- [ ] **External/FFI function handling**: Functions without an `ArcFunction` body
  (extern "c", extern "js", and any function not lowered to ARC IR) cannot be
  analyzed. They must receive hardcoded contracts:
  - FFI functions: `MemoryContract::all_owned(n)` (conservative — all params
    owned, return MaybeShared, effects Unknown, FIP Never). This matches the
    current pipeline's behavior of treating extern calls as fully consuming.
  - Built-in functions: use `aims/builtins.rs` contracts (Section 03.4)
  - Lambda/closure invocations (ApplyIndirect): no contract lookup — handled
    by the intraprocedural transfer function with conservative assumptions
  - Functions not in the `functions` slice (external crate calls, if separate
    compilation is later added): conservative `all_owned` contract
  The SCC loop must check `functions.iter().find(|f| f.name == name)` and
  fall back to builtin or conservative contracts for unanalyzable functions.
- [ ] Implement `analyze_scc_fixpoint` — iterate until signatures stabilize:
  - Monotonicity for parameter access: can only increase from `Borrowed` to `Owned`
  - Monotonicity for parameter consumption: can only increase
    (`Dead → Linear → Affine → Unrestricted`)
  - Monotonicity for return uniqueness: can only weaken
    (`Unique → MaybeShared → Shared`)
  - Monotonicity for FipContract: can only weaken
    (`Certified → Conditional → Never`)
  - Convergence: at most N iterations where N = sum of (params × dimensions)
    across SCC functions. In practice, much faster.

---

## 03.3 Contract Inference Rules

**File(s):** `compiler/ori_arc/src/aims/interprocedural.rs`

Rules for computing the full `MemoryContract`, adapted from Lean 4's `collect_O`
with AIMS extensions for uniqueness, cardinality, locality, effects, and FIP.

- [ ] A parameter must be `access == Owned` if:
  - It is returned by the function (ownership transfers to caller)
  - It is stored in a constructed value (ownership moves into data structure)
  - It is passed to a callee at an owned position
  - It is used in a Reset instruction (reset requires ownership)
  - It is captured in a partial application (closure takes ownership)
  - It is applied as a function via indirect call (unknown callee)

- [ ] A parameter must be `consumption == Linear` if owned and:
  - It is consumed exactly once on all code paths

- [ ] A parameter must be `consumption == Unrestricted` if:
  - It is used in a partial application (captured in closure, may be invoked multiple times)
  - It is applied as a function (unknown callee)
  - It is passed to an unknown/indirect call

- [ ] A parameter's cardinality is `Many` if:
  - It appears in a loop body (the back-edge causes `seq_add(Once, Once) = Many`)
  - It is used by multiple instructions within a single basic block
    (`seq_add` accumulates to `Many`), OR it is used across multiple basic
    blocks where the uses are not mutually exclusive (i.e., the blocks are
    not alternative successors of a branch/switch — they are sequentially
    reachable via the CFG). Note: uses in mutually exclusive branches
    combine via `alt_join` (= max), NOT `seq_add`, so one use in `then`
    and one use in `else` remains `Once`.

- [ ] Return value is `Unique` if:
  - All return paths produce freshly constructed values
  - All return paths produce results of COW operations (both paths → unique)
  - All return paths return values from callees with `Unique` return summaries

- [ ] `preserves_freshness` inference:
  `preserves_freshness` is `true` when: if ALL RC-tracked (non-scalar) input
  parameters are `Unique` at the call site, then the return value is guaranteed
  `Unique`. This enables callers to propagate uniqueness through the call.
  Inference rules:
  - `true` if the function always constructs a fresh value (all return paths
    are Construct or COW results) — regardless of parameter uniqueness
  - `true` if the function returns a parameter directly and that parameter is
    the sole source of the return value (identity/passthrough)
  - `true` if the function returns a value from a callee with
    `preserves_freshness == true` and all of that callee's inputs satisfy
    the freshness condition recursively
  - `false` if any return path returns a value stored in a shared data
    structure or captured in a closure
  - `false` if any return path returns a value from a callee without
    `preserves_freshness`
  This is the AIMS equivalent of the current `UniquenessSummary.preserves_freshness`.

- [ ] Tail call preservation adjustment (codegen-soundness constraint):
  **Note:** This is NOT a semantic ownership requirement — it is a codegen-soundness
  constraint that is pragmatically applied during contract inference. The concern is
  purely caller-local: can this function's tail calls be compiled as jumps? The
  adjustment is applied to the function's OWN contract (not its callees' contracts)
  to ensure the resulting code is implementable with TCO.
  - **Rule**: After inference determines that a parameter COULD be `Borrowed`, check
    whether the function's body contains a syntactic tail call that passes that
    parameter to an `access == Owned` position. If so, promote the parameter to
    `Owned` in this function's contract. Reason: if the parameter were `Borrowed`,
    the function would need to RC-dec it after the tail call returns, which prevents
    tail-call elimination.
  - **Timing**: This adjustment runs as a post-inference fixup step within
    `analyze_scc_single`/`analyze_scc_fixpoint`, AFTER the core ownership inference
    rules have converged but BEFORE the contract is finalized. It is monotonic
    (Borrowed → Owned) and cannot cause non-convergence.
  - "Syntactic tail position" means: an `Apply` whose `dst` is immediately used by
    a `Return` terminator in the same block. This is a structural property of the
    ARC IR that can be checked without running the tail-call detection pass (step 10
    in the pipeline). The tail-call detection pass rewrites self-recursive tail calls
    into loops — that is an optimization, not an analysis. The check here only needs
    to identify "would this call be in tail position?" which is a simpler syntactic
    check on the IR.
  - This is a codegen-soundness fixup, not a core inference rule
    (solutions.md Decision 1).

- [ ] A parameter's `may_escape` is `true` if:
  - It is returned by the function
  - It is stored in a constructed value that escapes
  - It is passed to a callee at an escaping position

- [ ] A parameter's `may_share` is `true` if:
  - It is captured in a closure (partial application)
  - It is stored in a data structure alongside other references to the same value
  - It is passed to a callee at a sharing position

- [ ] `EffectSummary` inference:
  Note: `EffectSummary` is a per-function summary computed during interprocedural
  analysis from the function body's instructions. It is distinct from `EffectClass`
  (Section 01), which is a per-variable, per-program-point lattice dimension in
  the intraprocedural `AimsState`. The function-level `EffectSummary` aggregates
  instruction-level effects across the entire function body, not per-variable
  `EffectClass` states.
  - `may_allocate` = true if any Construct, PartialApply, or allocating builtin
    appears in the function body (not behind a uniqueness-guarded fast path)
  - `may_share` = true if any code path stores a reference in a shared data
    structure, captures it in a closure, or passes it to a callee whose
    `ParamContract.may_share == true`
  - `may_throw` = true if any Invoke (panicking call) or explicit panic appears

- [ ] `FipContract` inference:
  - **Stage 1 (v1):** `FipContract` is set to `Never` for all functions and is
    NOT iterated during the fixed point. FIP inference is disabled entirely.
    The `all_borrowed` initializer receives `FipContract::Never`, and the
    fixed-point loop does not update the `fip` field.
  - **Stage 2:** FIP inference is a new inference rule added in Stage 2, not a
    change to existing Stage 1 code. When enabled:
    - Initialize `FipContract` to `Certified` (most optimistic) via
      `all_borrowed(n, FipContract::Certified)`
    - Demote to `Conditional { requires_unique_params }` if the function is
      allocation-free only when specific parameters are unique (reuse fast paths)
    - Demote to `Never` if any code path unconditionally allocates
    - For recursive SCCs: `FipContract` may only weaken through iterations
      (Certified -> Conditional -> Never)

---

## 03.4 Builtin Function Contracts

**File(s):** `compiler/ori_arc/src/aims/builtins.rs` (NEW)

Hardcoded contracts for built-in functions and operators that aren't analyzed.

- [ ] Port `BuiltinOwnershipSets` to `MemoryContract` format:
  
  > **Warning: Complexity.** The current `BuiltinOwnershipSets` (267 lines in
  > `borrow/builtins/mod.rs`) encodes nuanced type-qualified ownership rules
  > (e.g., `add` is borrowing for `str` but consuming for `List`). The AIMS
  > `builtins.rs` must replicate ALL of this complexity. Budget ~300 lines.
  > Review `borrow/builtins/mod.rs` line-by-line before implementing.
  - Borrowing builtins (read-only access): `Borrowed` mode, `Once` cardinality
  - Consuming builtins (take ownership): `Linear` mode, `Once` cardinality
  - COW builtins (may mutate): `Linear` mode, `Unique` return
  - **Consuming-receiver-only** builtins: receiver `Linear`, other args `Borrowed`
    (current: `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` for map/set methods)
  - **Consuming-second-arg** builtins: both receiver and arg[1] `Linear`
    (current: `CONSUMING_SECOND_ARG_METHOD_NAMES` for `add`/`concat`)

- [ ] Hardcode signatures for collection operations:
  - `list.push(value:)` — receiver: `Linear`, value: `Linear`, returns: `Unique`
  - `list.pop()` — receiver: `Linear`, returns: `Unique`
  - `list.add(other:)` — receiver: `Linear`, other: `Linear`, returns: `Unique`
  - `list.concat(other:)` — receiver: `Linear`, other: `Linear`, returns: `Unique`
  - `str.concat(other:)` — receiver: `Linear`, other: `Borrowed`, returns: `Unique`
  - `map.insert(key:, value:)` — receiver: `Linear`, key: `Owned`, value: `Owned`, returns: `Unique`
  - `set.insert(value:)` — receiver: `Linear`, value: `Linear`, returns: `Unique`

- [ ] Hardcode signatures for iterator operations:
  - `iter.next()` — receiver: `Linear`, returns: `Unique` (produces new iterator state)

- [ ] Port COW summaries from `uniqueness::inter::build_cow_summaries`:
  - All COW builtin methods return `Unique` (both fast and slow paths produce RC == 1)
  - **Sharing-return methods** (`slice`, `substring`) return `MaybeShared` — they
    share backing storage with the receiver (current: `sharing_builtin_names()` in
    `borrow/builtins/mod.rs`). These must NOT be marked `Unique`.
  - These need `interner` to map method name strings to `Name` values

- [ ] Hardcode signatures for **protocol builtins** (e.g., `__index`, `__eq`):
  - Protocol builtins have per-arg ownership from `ProtocolBuiltin::arg_ownership()`
    (current: `protocol` field in `BuiltinOwnershipSets`, populated from
    `borrow/builtins/mod.rs`). Each protocol has an explicit ownership pattern
    (e.g., `__index` borrows receiver, borrows key).
  - Port these per-arg patterns to `MemoryContract` format.

---

## 03.5 Completion Checklist

- [ ] `MemoryContract` struct defined with params, return_info, effects,
  context_behavior, and fip fields
- [ ] `ParamContract` includes may_escape, may_share, locality_bound
- [ ] `FipContract` enum defined (Never, Conditional, Certified)
- [ ] SCC-based fixed-point converges for all test programs
- [ ] Non-recursive SCCs analyzed in single pass
- [ ] Recursive SCCs use monotonic fixed-point iteration
- [ ] All promotion rules implemented (ownership, uniqueness, cardinality)
- [ ] Locality and effect inference rules implemented (v1: conservative defaults OK)
- [ ] FipContract inference implemented (v1: all `Never` is acceptable; Stage 2 enables)
- [ ] Tail call preservation rule working
- [ ] Builtin contracts hardcoded for all built-in functions
  (must cover all 5 sets from `BuiltinOwnershipSets` + COW summaries)
- [ ] `MemoryContract → AnnotatedSig` conversion working for migration
- [ ] `MemoryContract → UniquenessSummary` conversion working for migration
- [ ] `preserves_freshness` correctly computed for each function

**Exit Criteria:** `cargo t -p ori_arc -- aims::interprocedural` passes. Computed
contracts match or improve upon current `infer_borrows_scc` output for all test
functions. Fixed-point converges within the theoretical bound for all test
programs.
