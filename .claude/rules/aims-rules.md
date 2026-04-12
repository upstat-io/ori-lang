---
paths:
  - "**arc**"
  - "**aims**"
---

# AIMS Formal Ruleset

This document defines the **laws** of AIMS — the ARC Intelligent Memory System. The implementation is judged against this document, not the other way around. If the code violates a rule stated here, the code has a bug. If a pending plan describes a capability, that capability is a rule here — the implementation is incomplete, not the spec.

## Mission — Non-Negotiable

AIMS SHALL produce memory management **superior to hand-coded C**, handled entirely by the compiler, where the Ori programmer **never thinks about memory**. This is not aspirational — it is the design target that justifies every rule below.

1. **Better than C.** A competent C programmer stack-allocates locals, pools temporaries, and manually tracks ownership. AIMS SHALL do all of this automatically via whole-program analysis — AND exceed it through interprocedural uniqueness proofs, cross-function reuse, and COW optimization that no human could maintain across a large codebase.

2. **Zero programmer burden.** Unlike Rust (borrow checker, lifetime annotations), unlike C (manual malloc/free), unlike C++ (RAII with use-after-move footguns), unlike Swift (manual weak/unowned annotations) — the Ori programmer writes normal code. AIMS infers everything. No ownership annotations. No lifetime markers. No borrow syntax. The programmer's mental model is "values exist; the compiler handles the rest."

3. **Pushing boundaries.** AIMS is a unified formal framework — not a bag of peephole passes. The product lattice, interprocedural contracts, FBIP certification, and layered verification form one coherent system. No existing compiler has this combination at this level of integration.

4. **Must actually work.** Every rule below is verifiable. Every optimization is provably correct or has a runtime safety net. An unverifiable rule is not a rule — it is a wish. Formal soundness is non-negotiable.

**The endgame**: emitted code where RC operations are rare enough to audit one-by-one, and the AIMS pipeline can justify each surviving operation by pointing at the specific proof step that failed.

---

## Notation

- `∧` = conjunction (and), `∨` = disjunction (or), `¬` = negation
- `:=` = assigned to, `⊔` = join (least upper bound), `≤` = lattice partial order
- **SHALL** = mandatory requirement (violation = implementation bug)
- **Dimensions** are written `DimensionName` with values `Value`. Example: `Uniqueness = Unique`
- **Rules** are numbered `CATEGORY-N`. Categories: `L` (lattice), `CN` (canonicalization), `TF` (transfer function), `DP` (decision predicate), `IC` (interprocedural contract), `IA` (intraprocedural analysis), `PL` (pipeline), `RL` (realization), `VF` (verification)

---

## §1 The Product Lattice

The AIMS lattice is a **product of finite dimensions**. Each dimension is a small bounded lattice. The product lattice join is componentwise join followed by canonicalization. The number of dimensions is NOT fixed — it evolves as analysis needs require. Adding a dimension requires proving finiteness and updating canonicalization.

### §1.1 Access — Ownership vs Borrowing

| Value | Meaning | RC Obligation |
|-------|---------|---------------|
| `Borrowed` | Temporary view of another value | None — caller manages |
| `Owned` | Value owns its allocation | Full RC responsibility |

Order: `Borrowed < Owned`. Join: `max`. Height: 1.
Lineage: Lean 4 borrow inference, OxCaml modality.

### §1.2 Consumption — Substructural Mode

| Value | Meaning | RC Implication |
|-------|---------|----------------|
| `Dead` | Not live at this point | No RC operations |
| `Linear` | Consumed exactly once (moved) | No inc needed; dec at death |
| `Affine` | May be dropped without use | Dec may be needed, no inc |
| `Unrestricted` | Freely copied and dropped | Full RC (inc on copy, dec on drop) |

Order: `Dead < Linear < Affine < Unrestricted`. Join: `max`. Height: 3.
Lineage: Chirimar et al. (substructural RC), QTT semiring.

### §1.3 Cardinality — Forward Usage Count

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Absent` | Never used after this point | Skip all RC |
| `Once` | Used exactly once | Move semantics |
| `Many` | Used multiple times or in loop | Full RC |

Order: `Absent < Once < Many`. Join: `max`. Height: 2.

**Sequential composition** (`seq_add`): `Absent + x = x`, `Once + Once = Many`, `Many + _ = Many`. This is QTT's `+` (resource accumulation along one execution path). Distributes over `alt_join`.

**Alternative composition** (`alt_join`): lattice join (`max`). Used at control-flow merge points where only one path executes.

Lineage: GHC demand analysis (POPL 2014), QTT (Atkey, LICS 2018).

### §1.4 Uniqueness — Runtime Reference Count Knowledge

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Unique` | Provably RC == 1 | COW fast path, reset/reuse |
| `MaybeShared` | Unknown RC | Runtime check needed |
| `Shared` | Provably RC > 1 | Always copy on write |

Order: `Unique < MaybeShared < Shared`. Join: `max`. Height: 2.
Lineage: Marshall et al. (ESOP 2022) — uniqueness is PAST guarantee ("not duplicated"), distinct from linearity which is FUTURE ("consumed once").

### §1.5 Locality — Escape Classification

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `BlockLocal` | Does not escape defining basic block | Stack candidate, unique by construction |
| `FunctionLocal` | Does not escape defining function | Stack candidate |
| `ArgEscaping` | Escapes via argument but not to heap | Callee-scoped, no heap persistence |
| `HeapEscaping` | May escape to the heap | Requires heap allocation |
| `Unknown` | Conservative default | No optimization |

Order: `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown`. Join: `max`. Height: 4.

Both `seq_add` and `alt_join` coincide with `join` for Locality — escape scope widens monotonically.

Lineage: OxCaml locality modes (ICFP 2024), Go `leakCallee`.

### §1.6 Shape — Structural Classification for Reuse

| Value | Meaning |
|-------|---------|
| `NonReusable` | Not a candidate for allocation reuse (top) |
| `ReusableCtor(Struct)` | Struct constructor — reuse-eligible |
| `ReusableCtor(EnumVariant)` | Enum variant constructor — reuse-eligible |
| `CollectionBuffer` | Collection buffer (list, map, set) |
| `ContextHole` | TRMC constructor-context hole |

**Flat lattice**: equal values stay; unequal → `NonReusable`. Height: 1.

### §1.7 Effect — Memory Effect Classification

Three independent boolean flags. Join is componentwise OR. Height: 3.

| Flag | Meaning | Blocks |
|------|---------|--------|
| `may_alloc` | May allocate heap memory | FIP certification |
| `may_share` | May create shared references | Uniqueness preservation |
| `may_throw` | May throw/panic | Cleanup path correctness |

### §1.8 Lattice Properties

**L-1** — Join SHALL be commutative: `a ⊔ b = b ⊔ a`.

**L-2** — Join SHALL be associative: `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`.

**L-3** — Join SHALL be idempotent: `a ⊔ a = a`.

**L-4** — The partial order `a ≤ b ⟺ a ⊔ b = b` SHALL be reflexive, antisymmetric, and transitive.

**L-5** — Every dimension SHALL have finite height. The product lattice height is the sum of dimension heights.

**L-6** — Transfer functions SHALL be monotone: if `a ≤ b` then `f(a) ≤ f(b)`.

**L-7** — Canonicalization SHALL be idempotent: `canonicalize(canonicalize(s)) = canonicalize(s)`.

**L-8** — Canonicalization SHALL preserve join results: `canonicalize(a ⊔ b) = a ⊔ b` (join output is already canonical).

**L-9** — `SCALAR` is a sentinel, NOT a lattice element. Joining with `SCALAR` is undefined. Analysis SHALL exclude scalars.

**L-10** — Adding a new dimension requires: (a) proving finite height, (b) defining join, (c) updating canonicalization, (d) proving the new product lattice satisfies L-1 through L-8.

---

## §2 Canonicalization Rules

Canonicalization enforces cross-dimensional feasibility invariants. It runs after every join and every transfer function. Rules are applied in a bounded loop (max 3 rounds) until fixed point.

**CN-1** — Dead ↔ Absent bidirectional: `Consumption = Dead ⟹ Cardinality := Absent` and `Cardinality = Absent ⟹ Consumption := Dead`.
*Rationale*: Dead means zero future uses; absent means never used. These are the same fact from different perspectives.

**CN-2** — Linear + Absent infeasible: `Consumption = Linear ∧ Cardinality = Absent ⟹ Consumption := Dead`.
*Rationale*: Linear requires at least one use; absent has none. Defensive guard.

**CN-3** — Shared blocks reuse: `Uniqueness = Shared ∧ Shape ∈ {ReusableCtor(_)} ⟹ Shape := NonReusable`.
*Rationale*: Shared values have RC > 1; resetting would corrupt other references.

**CN-4** — ~~REMOVED (BUG-04-057).~~ The former rule promoted `MaybeShared → Unique` for `BlockLocal + Owned + ≤Once` states. This was anti-monotone — it injected optimistic information at join points, breaking associativity (L-2) and transitivity (L-4). Uniqueness is established by transfer functions (TF-3: FRESH allocations start Unique) and preserved or lost through joins — never re-derived in canonicalization.

**CN-5** — Unique + Dead preserves reusable shape. No rule collapses shape for Unique + Dead states.
*Rationale*: A unique dead value's memory IS reusable — the allocation is available for reset.

**CN-6** — Wide-locality uniqueness ceiling: `Locality ≥ HeapEscaping ∧ Uniqueness = Unique ⟹ Uniqueness := MaybeShared`.
*Rationale*: A value stored in a heap structure may have aliases via heap paths. Uses `≥` because `Unknown` subsumes `HeapEscaping`.
*Exception — return values*: CN-6 SHALL NOT apply to values whose HeapEscaping locality comes solely from being returned (IA-6). Return transfers ownership, creating a unique reference at the caller. The `ReturnContract` extraction SHALL use the pre-return-widening uniqueness. Fresh allocations returned from a function remain `Unique` in the `ReturnContract` even though their intraprocedural locality widens to HeapEscaping. This ensures RL-29 (noalias on fresh allocation returns) is achievable.

**CN-7** — ~~REMOVED.~~ The former rule about Shared+CollectionBuffer forcing COW mode was a decision predicate result, not a lattice state mutation. The behavior is fully covered by DP-9 (`Shared ⟹ StaticShared`). Canonicalization rules SHALL only mutate lattice dimensions — assigning `cow_mode` is a decision, not a state mutation.

**CN-8** — Borrowed locality ceiling: `Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal`.
*Rationale*: A borrowed reference cannot escape its defining function — it is a temporary view. Placed before CN-4 and CN-6 so locality is precise when those rules fire.

**Rule ordering**: CN-8 fires before CN-6 (prevents Borrowed+HeapEscaping/Unknown from reaching CN-6). CN-4 is removed (BUG-04-057). All active rules are monotone (move dimensions toward top or enforce same-level consistency). Current rules reach fixed point in one pass; multi-round loop is defensive infrastructure.

---

## §3 Transfer Functions

Transfer functions define how each ARC IR instruction updates the lattice state. There are two directions: **forward** (defining a variable's initial state) and **backward** (computing demand from operand uses).

### Forward (Definition)

**TF-1** — Scalar literal: `dst.state := SCALAR`. Int, float, bool, char, byte, duration, size — no RC.

**TF-2** — Variable binding (`Let { value = Var(v) }`): `dst.state := state(v)`. Inherits source state.

**TF-3** — Construct allocation: `dst := FRESH(shape_from_ctor(ctor))`. Fresh means `(Owned, Linear, Once, Unique, BlockLocal, shape, {may_alloc=true})`. All constructors produce fresh heap memory with RC = 1.

**TF-4** — Field projection: `dst := (Borrowed, Linear, Once, source.uniqueness, source.locality, NonReusable, NONE)`. Projection borrows a view of the source. Borrow source is tracked in a sparse side table.

**TF-5** — Function call (no contract): `dst := TOP`. Conservative — return value may escape, be shared, have any effect.

**TF-6** — Function call (with contract): `dst := refine(TOP, callee.return_contract)`. Contract narrows uniqueness, freshness, locality.

**TF-7** — Closure capture (`PartialApply`): `dst := FRESH(NonReusable)`. Captured variables updated via `capture_state_update`.

**TF-8** — Conditional selection (`Select`): `dst := state(true_val) ⊔ state(false_val)`. Merge of both branches.

**TF-9** — Reuse (`Reuse { token }`): `dst := FRESH(shape_from_ctor(ctor))`. Reused memory gets fresh state.

**TF-10** — IsShared/Reset: `dst := SCALAR`. Produces boolean or consumed value.

### Backward (Demand)

Each operand of each instruction generates a `(variable, cardinality)` demand:

**TF-11** — Most instructions emit `(operand, Once)` per argument: Construct, Project, Set (base + value), Return, Jump args, Branch cond, Switch scrutinee. **Apply/Invoke**: emit `(arg, Once)` conservatively; callee contract refinement (IC-3, via interprocedural contracts) narrows demands using `ParamContract.cardinality` — an `Absent` parameter produces zero demand at the caller. **Select**: emit `(cond, Once)` and `(true_val, Once)` and `(false_val, Once)` as per-variable demands. If `true_val = false_val` (same variable), only one demand is emitted (alternative, not additive).

**TF-12** — PartialApply emits NO backward demand from the standard demand system. Captured argument demand is handled entirely by `capture_state_update` to avoid double-counting.

**TF-13** — `capture_state_update(current, closure_state)` (OxCaml LAM rule):
- If closure cardinality ≤ Once: `consumption := max(current.consumption, Affine)`, `cardinality := max(current.cardinality, Once)`, `locality := max(current.locality, closure_state.locality)`. Preserves linearity/uniqueness. Locality incorporates both current state AND closure's escape scope — if the closure escapes to heap, captured variables must also be at least HeapEscaping. No artificial `FunctionLocal` floor: a block-local closure capturing a block-local variable preserves BlockLocal locality (both are scoped to the same block).
- If closure cardinality > Once: `consumption := Unrestricted`, `cardinality := Many`, `locality := max(current.locality, closure_state.locality)`. Multiple invocations = multiple consumptions.

**TF-13 SHALL be monotone**: if `a ≤ b` then `capture_state_update(a, c) ≤ capture_state_update(b, c)`.

**TF-14** — Project backward demand propagation: when a projected variable `dst` accumulates demand (locality widening, extended liveness), that demand SHALL be propagated to the borrow source `src` via the `borrow_sources` side table: `src.locality := max(src.locality, dst.locality)`. Without this propagation, `src` can be freed or stack-allocated while a reference to its projected field escapes or outlives it. This is the Ori equivalent of Lean 4's borrow liveness extension.

---

## §4 Decision Predicates

Decision predicates map lattice states to RC/COW/reuse decisions. Each is a pure function of `AimsState`.

**DP-1** — `is_rc_needed(s) ⟺ s.access = Owned ∧ s.consumption ≠ Dead ∧ ¬s.is_scalar()`.
Only owned, live, non-scalar variables carry RC obligations.

**DP-2** — `is_rc_dec_unnecessary(s) ⟺ s.cardinality = Absent ∨ s.consumption = Dead`.
No ADDITIONAL decrement beyond what the emission rules (RL-4 edge cleanup, RL-5 dead-at-entry) already handle. This predicate gates supplementary RC operations — the LAST dec that triggers the free is handled by RL-4/RL-5, not by DP-2.

**DP-3** — `is_rc_inc_elidable(s) ⟺ s.cardinality = Once ∧ s.consumption = Linear`.
Moved once — no duplication, no increment needed.

**DP-4** — `needs_cow_check(s) ⟺ s.uniqueness = MaybeShared`.
Only MaybeShared needs a runtime uniqueness check. Unique takes fast path statically; Shared takes slow path statically.

**DP-5** — `can_mutate_in_place(s) ⟺ s.access = Owned ∧ s.uniqueness = Unique ∧ no_active_overlapping_borrows(s)`.
Unique ownership permits direct mutation without COW, BUT only when no active borrow from the same source overlaps the mutated field. Borrows (via `Project`) do not increment RC — a `Unique` value with an active borrow is still RC = 1 but mutating it would corrupt the borrowed reference. The `borrow_sources` side table tracks active borrows; disjoint field borrows (RL-10) are safe.

**DP-6** — `is_reuse_candidate(s) ⟺ s.access = Owned ∧ s.uniqueness ≠ Shared ∧ s.shape ≠ NonReusable`.
Reuse requires owned, non-shared, reusable shape.

**DP-7** — `is_rc_skip_eligible(s) ⟺ s.is_local() ∧ s.access = Owned ∧ s.consumption = Linear ∧ s.uniqueness = Unique ∧ ¬s.is_scalar()`.
Scope: **parameter inc/dec pair elision only.** For Owned parameters where the caller increments and the callee decrements, if the parameter is local+linear+unique, the inc/dec pair cancels — skip both. This does NOT apply to the final dec that triggers the free for fresh allocations; that dec is always needed for heap-allocated values (only stack-promoted values via RL-14 skip it). The `Uniqueness = Unique` requirement is load-bearing: a Shared value's +1 inc from the caller is never balanced → leak.

**DP-8** — `is_local(s) ⟺ s.locality ∈ {BlockLocal, FunctionLocal}`.

**DP-9** — `cow_mode(s)`:
- `Unique ⟹ StaticUnique` (in-place, no check)
- `MaybeShared ⟹ Dynamic` (runtime IsShared check)
- `Shared ⟹ StaticShared` (unconditional copy)

**DP-10** — ~~REMOVED (unsound).~~ The former rule claimed `Owned ∧ Linear ∧ Once ⟹ RC == 1`. This is false: backward analysis proves "no future duplication" (consumption/cardinality are FUTURE guarantees) but NOT "no existing aliases" (uniqueness is a PAST guarantee). A shared allocation passed as Owned+Linear+Once still has RC > 1 from aliases created before this program point. Uniqueness is ONLY established by (a) the Uniqueness dimension directly, or (b) fresh allocation (TF-3: FRESH starts Unique). Cross-dimensional "proofs" that derive past from future are unsound.

---

## §5 Interprocedural Contracts

**IC-1** — The call graph SHALL be decomposed into SCCs. SCCs SHALL be processed in topological order (callees before callers).

**IC-2** — Each parameter initializes to most optimistic: `(Borrowed, Dead, Absent, BlockLocal, Unique)`. Fixed-point iteration promotes toward conservative. Escape is derived from Locality (`locality > FunctionLocal ⟹ escapes`), not stored as a separate fact — the Locality dimension is the SSOT for escape classification.

**IC-3** — Parameter contract join is componentwise max: `access(max), consumption(max), cardinality(max), locality(max), uniqueness(max), may_share(OR)`. If join changes any dimension, iterate again. Note: `may_share` IS a per-parameter property (whether the callee may increment the parameter's RC) and remains on `ParamContract` — it is orthogonal to Locality. Only `may_escape` was removed (derived from `locality > FunctionLocal`).

**IC-4** — Return contract: `uniqueness(join), preserves_freshness(AND), locality(join), shape(join)`. Freshness requires ALL return paths to preserve it.

**IC-5** — Effect summary: `may_allocate(OR), may_deallocate(OR), may_share(OR), may_throw(OR), has_unbounded_stack(OR), alloc_only_on_slow_path(AND)`.

**IC-6** — FIP contract: `Never` absorbs all; `Conditional` absorbs `Bounded/Certified`; `Bounded(n) ⊔ Bounded(m) = Bounded(max(n,m))`. FipContract::Certified ⟺ zero unmatched allocations/deallocations in realized IR.

**IC-7** — Convergence: finite domain guarantees termination. The iteration limit SHALL be derived from the domain heights: parameter contract 5 dimensions (access=1 + consumption=3 + cardinality=2 + locality=4 + uniqueness=2 = 12 per param), return contract 4 dimensions (uniqueness=2 + freshness=1 + locality=4 + shape=1 = 8 total), EffectSummary 6 boolean fields. Formula: `param_count × 12 + 8 + 6`. If exceeded, widen all contracts to most conservative and emit a diagnostic. This document (`aims-rules.md`) is authoritative for the convergence bound; `arc.md` references it.

**IC-8** — ~~REMOVED (unsound — same root cause as DP-10).~~ The former rule derived parameter uniqueness from caller consumption patterns (`Owned ∧ Linear ∧ Once`). This is unsound: a caller with a `MaybeShared` argument that it uses linearly still holds a reference whose RC may be > 1 from upstream aliases. Parameter uniqueness is established by the SCC fixpoint (IC-2/IC-3): if ALL callers pass arguments whose `Uniqueness` dimension is `Unique`, the fixpoint converges to `ParamContract.uniqueness = Unique` naturally. No post-fixpoint tightening is needed or sound.

---

## §6 Intraprocedural Analysis

**IA-1** — Analysis direction is BACKWARD. Future-use demand determines RC operations. Compute block exit states (demand from successors), then entry states (supply from predecessors).

**IA-2** — Blocks SHALL be processed in reverse postorder (successors before predecessors in the backward direction).

**IA-3** — Block exit state = `⊔(successor.entry_states)` using `alt_join` (only one successor executes per dynamic run).

**IA-4** — Cross-block locality widening: variables flowing across block boundaries SHALL have `locality := max(locality, FunctionLocal)`.

**IA-5** — Block entry state is computed by walking instructions in reverse from exit state: (1) apply forward transfer for definitions, (2) apply backward demands for operands, (3) remove defined variables.

**IA-6** — Return values SHALL be widened: `access := Owned`, `locality := max(locality, HeapEscaping)`. Returned values escape the function.

**IA-7** — Convergence by monotone iteration: iterate worklist until no block state changes or iteration limit exceeded. Limit: `CHAIN_HEIGHT × |variables| × |blocks|`. If exceeded, widen all to TOP — this is a safety net, not expected behavior.

**IA-8** — Immortal variables (heap-allocated constants with MAX_REFCOUNT) SHALL be excluded from analysis entirely — treated as scalars.

**IA-9** — N-ary join at CFG merge points SHALL be permutation-invariant: fold order over successor states SHALL NOT affect the result. (Follows from L-1 + L-2.)

---

## §7 Pipeline Ordering

The AIMS pipeline has a fixed step ordering. Each ordering constraint is load-bearing.

```
Step 1:  analyze_program()         — Interprocedural: MemoryContract per function (SCC fixpoint)
Step 2:  apply_ownership()         — Populate ArcParam.ownership from contracts
Step 3:  compute_var_reprs()       — ValueRepr per variable (Scalar/DefiniteRef/PossibleRef)
Step 3a: normalize_function()      — TRMC context region detection
Step 4:  analyze_function()        — Backward dataflow → converged AimsStateMap
Step 5:  realize_rc_reuse()        — Phase 1: RC + reuse + arg_ownership (pre-merge)
Step 5a: verify_fip_contract()     — FIP enforcement
Step 6:  verify()                  — ARC IR structural sanity
Step 7:  run_aims_verify()         — AIMS contract vs IR consistency
Step 8:  detect/rewrite tail calls — CFG optimization
Step 8a: unwind_cleanup()          — Invoke-unwind RC cleanup
Step 9:  merge_blocks()            — CFG cleanup
Step 10: realize_annotations()     — Phase 2: COW + drop hints (post-merge)
Step 11: verify()                  — Final sanity
Step 12: FBIP enforcement          — Read-only diagnostic
```

**PL-1** — Steps 1-2 (interprocedural) SHALL run once across all functions before any per-function step. Contracts are prerequisites for call-site refinement.

**PL-2** — Step 4 (analysis) SHALL precede Step 5 (realization). State map drives emission decisions.

**PL-3** — Step 5 (realization phase 1) SHALL precede Step 9 (merge). Position-keyed state maps are invalidated by merge.

**PL-4** — Step 10 (realization phase 2) SHALL follow Step 9 (merge). Phase 2 uses ArcVarId-keyed lookups that survive merge.

**PL-4a** — Step 8a (unwind_cleanup) SHALL precede Step 9 (merge_blocks). Unwind cleanup alters CFG connectivity (invoke/resume edges); merging before cleanup produces invalid CFG state.

**PL-5** — No pass SHALL rely on stale summaries. If a step modifies IR or updates an effect summary, all downstream consumers SHALL see updated values.

**PL-6** — Adding a new pass requires updating the pipeline ordering and proving it does not violate any existing ordering constraint.

### TRMC (Tail-Recursion Modulo Cons)

TRMC is a pipeline-integrated rewrite that enables tail-call optimization for functions that construct on the return path. It is an active subsystem with formal rules, not future work.

**PL-7** — TRMC normalization (Step 3a) SHALL detect tail-recursive functions that return constructor applications. The `ContextBehavior` contract on `MemoryContract` tracks four fields (per Leijen & Lorenzen, JFP 2025): `preserves_context` (bool — function preserves the constructor context hole through recursive calls), `consumes_hole` (bool — function fills the context hole with a value), `requires_unique_context` (bool — context hole must be uniquely owned for soundness), `may_resume_nonlinearly` (bool — function may resume at the context hole more than once, e.g. via exceptions).

**PL-8** — TRMC candidate predicate: a function is a TRMC candidate when it is self-recursive, the recursive call appears in tail position of a constructor argument, and the constructor is in the return path. The context region is the span from the recursive call to the constructor that wraps its result.

**PL-9** — TRMC rewrite: the candidate function is normalized to accept a `ContextHole` parameter (Shape = ContextHole). The recursive call fills the hole with the partially-constructed value instead of allocating a new frame. The rewrite SHALL be idempotent — a second pass produces identical IR.

**PL-10** — TRMC soundness verification: after Step 3a rewrite AND after Step 4 intraprocedural analysis (which operates on the rewritten IR), `verify_trmc_soundness()` SHALL confirm that the rewritten function preserves observable behavior. This verification runs between Step 4 and Step 5 (before realization). If verification fails, the rewrite SHALL be rolled back to the pre-TRMC IR AND Step 4 SHALL be re-run on the restored CFG (per PL-5: no stale summaries).

**PL-11** — `ContextBehavior` join: `preserves_context(AND)`, `consumes_hole(AND)`, `requires_unique_context(OR)`, `may_resume_nonlinearly(OR)`. Conservative: preservation and consumption require ALL paths to agree; unique-context and non-linear-resumption are any-path (soundness obligations widen).

---

## §8 Realization & Post-Lattice Optimization

### RC Emission

**RL-1** — RC increment SHALL be emitted when a value is duplicated (passed to Owned parameter while still live).

**RL-2** — RC decrement SHALL be emitted at the last use of an owned value or at scope exit.

**RL-3** — RC operations SHALL be ELIDED when the lattice proves they are unnecessary (DP-2, DP-3, DP-7).

**RL-4** — Edge-specific decrements: a variable alive at block exit but dead at successor entry SHALL receive a decrement on that specific CFG edge.

**RL-5** — Dead-at-entry cleanup: a block parameter with `Cardinality = Absent` at entry SHALL receive an immediate decrement.

### COW (Copy-on-Write)

**RL-6** — Static unique mutation (`Uniqueness = Unique`): emit in-place mutation, no IsShared check.

**RL-7** — Dynamic COW (`Uniqueness = MaybeShared`): emit IsShared check, branch to in-place (unique) or copy (shared) paths.

**RL-8** — Static shared mutation (`Uniqueness = Shared`): emit unconditional copy before mutation.

**RL-9** — COW compound contraction: diamond CFG patterns `IsShared → Branch → (clone+Set | Set) → Merge` SHALL be contracted into a single compound instruction.

**RL-10** — Disjoint field borrows SHALL NOT trigger COW: if receiver borrows field F and all other borrows are from different fields, the borrow is safe without COW check.

### Allocation Reuse

**RL-11** — Same-block reuse: a dying value's allocation SHALL be reused for a fresh allocation of the same type in the same block, if: (a) the death precedes the allocation, (b) no intervening use exists, AND (c) the dying value has `Uniqueness = Unique`. Reusing a non-unique allocation corrupts other aliases. For `MaybeShared` values, dynamic reuse (IsShared check + conditional) is available via DP-6 but requires a runtime guard.

**RL-12** — Cross-block reuse: a dying value's allocation SHALL be reused across blocks when the death block dominates the allocation block AND the allocation block post-dominates the death block (ensuring the reuse is unconditional on all paths between death and allocation), and the dying value is statically unique.

**RL-13** — ~~REMOVED (unsound — same root cause as DP-10).~~ The former rule claimed `Construct + Once = RC == 1 at death`. This is false: the one use may be "store into a data structure," which creates an alias via RcInc. At death, the original variable's allocation is still alive via the alias. Reuse would overwrite live memory. Reuse eligibility is determined SOLELY by the Uniqueness dimension (DP-6 + RL-11/RL-12).

### Stack Promotion

**RL-14** — Non-escaping allocations (`Locality ≤ FunctionLocal`) with fixed size SHALL be stack-allocated via `alloca`. No RC header. No RC operations. Stack deallocation at scope exit.

**RL-15** — Non-escaping dynamic-size allocations SHALL use a function-local bump allocator. Entire region freed at function return.

**RL-15a** — ArgEscaping allocations (`Locality = ArgEscaping`) SHALL be stack-allocated in the CALLER. The caller's stack frame strictly outlives the callee's execution, so the allocation survives the callee's use. Uniqueness is preserved (CN-6 does not fire for ArgEscaping — only HeapEscaping and above). No RC header is needed. This is the key optimization that bridges "not local" and "not heap" — a value that escapes into a callee but not to the heap gets caller-stack lifetime without heap overhead.

**RL-16** — Escaping allocations (`Locality ≥ HeapEscaping`) SHALL be heap-allocated with full RC header.

### RC Header Compression

**RL-17** — Sharing bound analysis SHALL determine maximum simultaneous reference count. `NoEscape → Unique (no header)`, straight-line N incs → `Bounded(N+1)`, loops/recursion/global → `Unbounded`.

**RL-18** — RC header width SHALL be narrowed: `Unique → none`, `Bounded(≤127) → i8`, `Bounded(≤32767) → i16`, `Bounded(≤2^31-1) → i32`, `Unbounded → i64`.

### Unified Representation Constraint

**RL-18a** — All escape-driven decisions (stack allocation via RL-14/15/16, header width via RL-17/18, atomic vs non-atomic via RL-19/20/21) SHALL consume the single unified `Locality` dimension. Parallel escape enums (`EscapeState`, `ThreadLocality`, `HeapEscapeStatus`, or any equivalent shadow representation) are FORBIDDEN. The `Locality` dimension is the SSOT for all escape classification. This constraint ensures that extending escape analysis extends ONE dimension, not N parallel data structures that drift independently.

### Non-Atomic RC

**RL-19** — Thread-local values (no cross-thread escape) SHALL use non-atomic RC operations (plain load/store instead of atomic CAS). Thread-locality is derived from `Locality` + call-graph analysis: if no escape path crosses a thread boundary (spawn, channel send, FFI), the value is thread-local. The `Locality` dimension provides escape scope; thread-sharing analysis is a program-wide property layered on top (similar to how RL-21 detects the whole-program no-spawn case). Future Locality extension with explicit `ThreadShared` level is planned but not required — thread analysis can derive from `HeapEscaping` + call-graph thread-boundary detection.

**RL-20** — Thread-shared values SHALL use atomic RC operations.

**RL-21** — If a program has no spawn/channel operations, ALL values are thread-local and ALL RC is non-atomic.

### KnownSafe Pair Elimination

**RL-22** — When the physical refcount is provably positive (outer RcInc in scope, no intervening decrement), inner `RcInc`/`RcDec` pairs on the same variable SHALL be eliminated.

**RL-23** — KnownSafe flag propagation at join points: `true` only if ALL predecessors agree.

### PRE-Style Global RC Motion

**RL-24** — Bidirectional dataflow (bottom-up release analysis + top-down retain analysis) SHALL identify matching `(Inc, Dec)` pairs across basic blocks.

**RL-25** — A pair is eliminable when: KnownSafe = true OR both forward/backward paths are safe, AND no CFG hazard (path count alignment).

**RL-26** — RC motion SHALL NOT move operations across calls that may observe RC (effect-aware barrier check).

### Selective Barriers

**RL-27** — At call sites, RC operations SHALL only be flushed for variables whose callee parameters are `Owned` + non-`Dead`. Borrowed parameters and pure functions require no barrier.

**RL-28** — Unknown callees (FFI, indirect, no contract) require conservative flush of all pending RC operations.

### AIMS → LLVM Fact Export

**RL-29** — Fresh allocation returns (`ReturnContract.uniqueness = Unique`) SHALL be marked with LLVM `noalias`.

**RL-30** — Effect-based call annotations: pure calls (`may_allocate = false ∧ may_deallocate = false ∧ may_throw = false` and no writes to parameters) SHALL receive LLVM `memory(none)`. Readonly calls (reads but no writes, no allocation, no deallocation) SHALL receive LLVM `memory(read)`. Allocating calls (`may_allocate = true`) that also access arguments SHALL receive `memory(argmem: readwrite, inaccessiblemem: readwrite)`. Pure allocators (no arg access) SHALL receive `memory(inaccessiblemem: readwrite)`. `memory(argmem: readwrite)` ALONE is WRONG for allocators (misses global heap state); omitting `argmem` is WRONG for functions that allocate AND read/write arguments (optimizer assumes args untouched).

**RL-31** — Disjoint borrowed parameters SHALL receive `!alias.scope` + `!noalias` metadata pairs.

### Borrow Inference

**RL-32** — All non-scalar parameters initialize to `Borrowed`. Fixed-point iteration promotes to `Owned` based on demand.

**RL-33** — Projection propagation: if a projected field becomes `Owned`, the source variable SHALL be promoted to `Owned`.

**RL-34** — Tail call preservation: never insert `RcDec` after a tail call. Transfer ownership instead — BUT only when the callee parameter is `Owned`. If the callee parameter is `Borrowed`, ownership transfer violates the ABI (callee expects a borrow, caller's dec never fires → leak). A call to a `Borrowed` parameter cannot be optimized as a tail call unless the caller can arrange for the dec to happen before the call.

---

## §9 Verification Layers

The verification stack is **layered**. Each layer catches a different class of inconsistency.

**VF-1** — Layer 1 (Structural): ARC IR well-formedness. Checks: use-before-def, dangling block refs, RC on scalar, dec on borrowed, arg ownership length mismatch. Runs at two checkpoints: after AIMS emission and after full pipeline.

**VF-2** — Layer 2 (AIMS Contract): filters structural verifier output to AIMS-specific inconsistencies. Currently checks: parameters declared `Absent` must have no live uses.

**VF-3** — Layer 3 (Oracle): re-derives `MemoryContract` from realized IR and compares against inferred contract along access, consumption, and effects dimensions. Unsafe mismatches (analysis more optimistic than realization) are errors.

**VF-4** — Layer 4 (FIP Certification): proves `FipContract::Certified` functions have zero unmatched allocations/deallocations. Failures are wrapped as `FipStructural` in the shared verification stream.

**VF-5** — Every active subsystem SHALL be end-to-end verified: implementation + invariant enforcement + tests. Missing any of the three = incomplete.

**VF-6** — Contracts and realization SHALL agree. If `MemoryContract` says `FipContract::Certified`, realized IR SHALL have zero unmatched alloc/dealloc.

**VF-7** — Active rewrites SHALL be sound: identical observable behavior. Structural tests alone are insufficient — behavioral verification is required.

**VF-8** — The verification stack applies to ALL rules in this document — including rules from §8 that are not yet implemented. An unimplemented rule without a corresponding verification layer planned is a spec gap.

---

## §10 Prior Art Cross-Reference

AIMS draws from multiple traditions. No single prior system has this combination.

### Lean 4 — Counting Immutable Beans (Ullrich & de Moura, IFL 2019)
- **Adopted**: Binary borrow inference (Owned/Borrowed) with monotone fixpoint. RC insertion driven by liveness analysis. Reset/reuse via IsShared runtime check (ExpandResetReuse.lean).
- **Extended by AIMS**: Lean uses binary liveness (live/dead); AIMS uses a multi-dimensional product lattice with substructural consumption, cardinality, locality, shape, and effects — enabling richer optimization decisions (RC-skip for linear parameters, escape-driven stack promotion, effect-based LLVM annotations). Lean's reuse is same-type only; AIMS adds dynamic reuse with runtime uniqueness guards.

### Koka Perceus (Reinking, Lorenzen, Leijen & de Moura, PLDI 2021)
- **Adopted**: Perceus garbage-free RC with reuse. Functional-but-in-place (FBIP) certification. Allocation credit tracking. Tail-recursion modulo cons (TRMC).
- **Extended by AIMS**: Koka certifies at the function level (FIP/FBIP/neither); AIMS certifies at the instruction level via the product lattice and contracts. Koka's allocation tracking is a tree; AIMS uses EffectSummary fields on interprocedural contracts.

### Swift ARC Optimizer
- **Adopted**: KnownSafe flag for nested pair elimination (RL-22/23). Bidirectional dataflow for RC motion (RL-24/25). Effect-aware barriers (RL-27/28).
- **Extended by AIMS**: Swift uses a 4-state per-direction lattice (None→Decremented→MightBeUsed→MightBeDecremented) plus an independent KnownSafe boolean flag. AIMS's product lattice is broader — Consumption subsumes the 4-state lattice, while AIMS's KnownSafe rules (RL-22/23) correspond to Swift's independent KnownSafe flag. Swift's barrier analysis is per-pointer; AIMS uses interprocedural contracts for whole-function reasoning.

### GHC Demand Analysis (POPL 2014)
- **Adopted**: Cardinality dimension (0-1-ω semiring from QTT). `seq_add` (sequential) vs `alt_join` (alternative) composition. Distributivity of `seq_add` over `alt_join`.
- **Extended by AIMS**: GHC's demand analysis drives thunk evaluation strategy in a lazy language; AIMS drives RC elimination in a strict language. GHC uses `multCard` for nested evaluation contexts (lazy closures); AIMS omits it — strict evaluation means every body executes exactly once per call.

### OxCaml Locality Modes (ICFP 2024)
- **Adopted**: Locality dimension with BlockLocal/FunctionLocal/HeapEscaping ordering. Borrowed → locality ceiling FunctionLocal (CN-8). Once-closures preserve uniqueness (TF-13 LAM rule).
- **Extended by AIMS**: OxCaml requires programmer annotation (`local_` keyword); AIMS infers locality automatically. AIMS adds `ArgEscaping` between FunctionLocal and HeapEscaping for callee-scoped non-heap escape.

### Clang/LLVM ObjC ARC
- **Adopted**: Compile-time statistics for optimization measurement. PRE-style global RC code motion. COW compound contraction.
- **Extended by AIMS**: Clang operates on a per-pointer state machine with no interprocedural reasoning; AIMS uses whole-program SCC-based contracts. Clang's KnownSafe is a local per-pointer flag; AIMS's KnownSafe (RL-22/23) is a post-emission physical-refcount analysis — distinct from the AIMS lattice, operating on realized RC ops rather than abstract state.

### What Makes AIMS Unique

No prior system combines all of: (1) a multi-dimensional product lattice, (2) interprocedural contracts via SCC fixpoint, (3) FBIP certification, (4) escape-driven stack promotion, (5) RC header compression, (6) thread-locality analysis for non-atomic RC, (7) AIMS→LLVM fact export, (8) zero programmer annotation — all in a single unified framework where every decision is derived from one lattice and verified by a layered stack. Each individual technique exists in prior art; the integration into a single formally-grounded pipeline does not.
