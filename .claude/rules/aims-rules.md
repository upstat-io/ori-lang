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
- **Rules** are numbered `CATEGORY-N` or `CATEGORY-Na` (suffixed sub-rules). Categories: `L` (lattice), `CN` (canonicalization), `TF` (transfer function), `DP` (decision predicate), `IC` (interprocedural contract), `IA` (intraprocedural analysis), `PL` (pipeline), `RL` (realization), `VF` (verification)

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
| `BlockLocal` | Does not escape defining basic block | Stack candidate (uniqueness checked independently via DP-6/RL-14) |
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

**Current consumer status and EffectSummary relationship**: EffectClass is tracked per-variable in `AimsState` but is NOT currently consumed by any decision predicate (DP-1 through DP-9). The per-variable tracking exists so that future passes can make per-variable effect-aware decisions (e.g., hoisting RC operations past effect-free instructions, or narrowing the effect scope of a code region for alias analysis).

**CRITICAL: EffectSummary (IC-5) is computed from INSTRUCTIONS, not from per-variable EffectClass.** The interprocedural `EffectSummary` is computed by scanning the function's instruction stream and callee contracts — NOT by OR-ing per-variable EffectClass values. Specifically: `Construct` contributes `may_allocate=true`; `Apply/Invoke` with a callee contract contributes the callee's `EffectSummary`; `Apply/Invoke` without a contract contributes ALL. This means the per-variable EffectClass = ALL on call results does NOT poison the caller's EffectSummary — the EffectSummary reads the callee's CONTRACT, not the per-variable state. A function calling a pure callee (whose `EffectSummary` has all flags false) correctly inherits no effects from that call, even though the per-variable EffectClass on the call result is ALL.

`refine()` (TF-6) does NOT narrow EffectClass from call results because the per-variable EffectClass is not consumed by EffectSummary computation. Narrowing it would be precision work for future per-variable passes, not a correctness requirement for the current system.

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

### §1.9 Side-Table Domains

The product lattice is the primary analysis domain. Two auxiliary side tables carry per-variable provenance facts that the lattice dimensions alone cannot express. These are NOT lattice dimensions — they do not participate in join, do not have a height, and do not affect convergence bounds. They are **provenance annotations** consumed by specific decision predicates and realization rules.

#### Borrow Sources (`borrow_sources`)

A sparse map `ArcVarId → BorrowSource` tracking the origin of borrowed projections. Populated by TF-4 (Project): when `dst = src.field`, the map records `dst → BorrowSource { source: src, field: field }`.

**Join**: borrow sources do NOT join. Each projected variable has exactly one borrow source (set at definition by TF-4). If a variable is defined by a non-Project instruction, it has no borrow source entry.

**Consumers**:
- **DP-5** (`can_mutate_in_place`): queries BOTH `borrow_sources` (direct borrows) AND `project_alias_sources` (transitive aliases) to check for active overlapping borrows or live aliases. A `Unique` value with an active borrow from the same field is NOT safe for in-place mutation — the borrow would alias the mutated memory. Disjoint field borrows are safe (RL-10). Live transitive aliases block conservatively (no field-level granularity).
- **TF-14** (backward propagation): when a projected variable's locality widens, the widening propagates to its borrow source via this table.

**Invariants**:
- Every `Project` instruction creates exactly one borrow source entry.
- No other instruction creates borrow source entries in `borrow_sources`.
- The table is read-only after intraprocedural analysis completes.

#### Project Alias Sources (`project_alias_sources`)

A function-wide map `ArcVarId → SmallVec<ArcVarId>` tracking transitive aliases of projected values. This is the companion to `borrow_sources` that ensures demand propagation is sound across aliases, merges, and block parameters.

**Motivation**: `borrow_sources` maps only the direct `Project` destination to its source (e.g., `%3 = Project %2.0` → `%3 → %2`). But projected values flow through Let aliases (`%4 = Let Var(%3)`) and Jump arguments to block parameters (`Jump block1, args=[%3]` → param `%5`). Without tracking these aliases, a parent aggregate can be freed while a borrowed child flowing through an alias is still live.

**Construction**: precomputed once per function (the alias structure is static). Rules are applied in dependency order (1 → 2 → 3 → 4 → 5 → 6):
1. Direct Project destinations → Project ROOT source variables (tracing through Let Var aliases to find the actual aggregate). If `%2a = Let Var(%2)` and `%3 = Project %2a.field`, then borrow_sources records `%3 → {source: %2, field}` (the root aggregate, not the alias) and project_alias_sources records `%3 → [%2]`. This ensures DP-5 can detect mutations of the root aggregate regardless of intermediate aliases.
2. Let aliases: if `%4 = Let Var(%3)`, then `%4` maps to `[%3] ∪ sources(%3)`. When `%3` has no existing sources (is a root aggregate), `%4 → [%3]`. This ensures DP-5 can detect mutations of root aggregates that have been aliased via Let.
3. Jump-arg → block-param: if `Jump block1, args=[%3]` and block1 has param `%5`, then `%5` maps to `[%3] ∪ sources(%3)`. Same as Let — includes the immediate source even when %3 is a root.
4. CFG merge: if block param `%5` receives projected values from multiple predecessor Jump arguments tracing to different source aggregates, `%5` maps to the union of all sources.
5. **Select aliases**: if `%3 = Select(cond, %A, %B)` and %A or %B have existing sources in the map, then `%3` maps to the union: `%3 → [%A, %B] ∪ sources(%A) ∪ sources(%B)`. If neither has sources, `%3 → [%A, %B]`. This ensures DP-5 can detect borrow chains through conditional selections.
6. **Transitive closure for nested Projects**: if `%4 = Project %3.field` and `%3` maps to sources `S`, then `%4` maps to `[%3] ∪ S` (the immediate source PLUS all of the immediate source's own sources). This ensures that DP-5 can detect live borrows at ANY depth of the nesting chain. Without transitive closure, a nested projection `%4 = Project %3.1` where `%3 = Project %2.0` would only map `%4 → [%3]`, missing the root aggregate `%2` — DP-5 would then fail to block in-place mutation of `%2` while `%4` holds a live reference through `%3`.

**Example (simple chain)**:
```
%3 = Project %2.0
%4 = Let Var(%3)
Jump block1, args=[%4]
// block1 params: [%5]
```
Maps: `%3 → [%2]`, `%4 → [%2]`, `%5 → [%2]`

**Example (nested Projects)**:
```
%3 = Project %2.0        // direct borrow from %2
%4 = Project %3.1        // nested borrow — borrows field 1 of a projection of %2
```
Maps: `%3 → [%2]`, `%4 → [%3, %2]` (transitive: %3 maps to [%2], so %4 gets %3 ∪ [%2])

**Consumer**: `propagate_project_source_demand()` — invoked on EVERY iteration of the backward worklist. On each iteration, for each BORROW alias variable (entries originating from Rules 1 or 6 — direct Project destinations and their transitive chain) that is live, propagates `Locality = max(source_current, alias_locality)`, `Cardinality = seq_add(source_current, alias_cardinality)`, and if alias's locality >= HeapEscaping: `Access = Owned`. NON-BORROW aliases (Rules 2, 3, 5) do NOT trigger demand propagation — they are for DP-5 only.

**Demand construction**: the propagated demand uses `Access = Owned` (the source aggregate must support field cleanup RcDec, which requires Owned). Locality uses `max(source_current, alias_locality)`. Cardinality uses `seq_add`. Consumption is NOT propagated.

**Merge-point filtering**: at CFG merges, the union of project sources from multiple predecessors is an over-approximation (the analysis keeps all possible parent aggregates alive). The EMISSION side (RL-4, RL-5) must filter: variables that exist only on one predecessor path must NOT receive merge-block-level decrements. Instead, they receive decrements on their specific predecessor edge. This filtering is an emission concern, not an analysis concern — the analysis correctly over-approximates to keep parents alive on all paths.

**Select**: `Select` instructions participate in project alias tracking as CONDITIONAL aliases: if `%3 = Select(cond, %A, %B)`, then `%3 → [%A, %B]` (the Select result may be either operand at runtime). This is needed for DP-5 soundness: if `%4 = Project %3.0` and %A is later mutated, DP-5 must detect the borrow chain %4 → %3 → %A. Without Select in project_alias_sources, the chain breaks at %3. Select's backward DEMAND is handled separately by IA-5 step (1) (conditional alias transfer); the project_alias_sources entry handles the DP-5 SAFETY CHECK path.

**Propagation scope — why TF-14 and project_alias_sources propagate different things**: TF-14 (direct borrow sources, intra-block) propagates locality AND cardinality. `project_alias_sources` (cross-block) propagates with `Access = Owned`, locality, and cardinality (via seq_add). Both always use `Access = Owned` (not Borrowed) because source aggregates need Owned for field cleanup. The difference: TF-14 fires during the per-instruction backward walk; project_alias_sources fires at block boundaries in the worklist loop.

This mechanism makes TF-14 (backward demand propagation) sound across control flow. TF-14 fires for direct `borrow_sources` entries during the per-instruction backward walk; `project_alias_sources` extends that to transitive aliases within the worklist loop.

#### Return Provenance

The `ReturnContract` (IC-4) is extracted within the interprocedural pass (Step 1). Step 1 internally runs a PRELIMINARY intraprocedural backward analysis for each function or SCC member — this is a lighter version of Step 4 that operates on the pre-TRMC, pre-`compute_var_reprs` IR. Its purpose is solely to extract interprocedural contracts (ParamContract, ReturnContract, EffectSummary). Step 4 later runs the FULL intraprocedural analysis on the post-TRMC IR with contracts already available for call-site refinement. However, the `ReturnContract` extraction itself uses **structural definition tracing**, not the `AimsStateMap`:

- For each `Return { value }` terminator, trace `value` back through the definition chain (`Let`, `Apply`, `Construct`, etc.). The extractor classifies each return path along ALL four `ReturnContract` dimensions: `uniqueness`, `preserves_freshness`, `locality`, and `shape`.

**Per-instruction classification** (exhaustive over ArcInstr variants that can reach a Return):

| Definition type | Uniqueness | preserves_freshness | Locality | Shape |
|---|---|---|---|---|
| `Construct` (fresh allocation) | `Unique` | `true` | `BlockLocal` | `shape_from_ctor` |
| `Reuse` (reused allocation) | `Unique` | `true` | `BlockLocal` | `shape_from_ctor` |
| `CollectionReuse` | `Unique` | `true` | `BlockLocal` | `CollectionBuffer` |
| `PartialApply` (closure) | `Unique` | `true` | `BlockLocal` | `NonReusable` |
| `Apply`/`Invoke` with `Unique` return contract | inherited `Unique` | callee's `preserves_freshness` | callee's `locality` | callee's `shape` |
| `Apply`/`Invoke` with non-`Unique` contract | callee's uniqueness | callee's `preserves_freshness` | callee's `locality` | callee's `shape` |
| `Apply`/`Invoke` without contract | `MaybeShared` | `false` | `Unknown` | `NonReusable` |
| `ApplyIndirect`/`InvokeIndirect` | `MaybeShared` | `false` | `Unknown` | `NonReusable` |
| `Project` (field borrow) | `MaybeShared` | `false` | `HeapEscaping` | `NonReusable` |
| `IsShared` / `Reset` | SCALAR — excluded from contract (no RC) | — | — | — |
| `Let { Literal }` / `Let { PrimOp }` | SCALAR — excluded from contract (no RC) | — | — | — |
| `Let { Var(v) }` | follow `v`'s definition (recursive trace) | follow `v` | follow `v` | follow `v` |
| `Select { true_val, false_val }` | `join(true_path, false_path)` (scalar operands excluded per TF-8; mixed scalar/non-scalar → inherit non-scalar with MaybeShared) | `AND(true_path, false_path)` (scalar arm contributes `false` — scalars have no freshness) | `join(true_path, false_path)` | `join(true_path, false_path)` |
| Parameter (direct or via chain of `Let Var` aliases) | `MaybeShared` | `false` | `HeapEscaping` | `NonReusable` |
| Unresolvable (longer chains, block-params, etc.) | `MaybeShared` | `false` | `Unknown` | `NonReusable` |

**Tracing rules:**
- `Let { Var(v) }` is transparent: recurse into `v`'s definition. A chain of Let aliases follows until a non-Let instruction is reached.
- Block-params/phi-like aliases are NOT traced beyond the first Let — they fall into the conservative case (`MaybeShared`, `preserves_freshness = false`). This bounds the extraction to local definition chains, preventing SCC dependency on predecessor-block analysis state.
- Parameters traced directly or through a chain of `Let Var` aliases get `preserves_freshness = false` — a returned parameter is NOT fresh; it may alias the caller's argument. Even if the caller passes a fresh value, the returned pointer aliases the caller's copy, so `noalias` is unsound. Only `Construct`, `Reuse`, `CollectionReuse`, `PartialApply`, and callees with fresh return contracts produce `preserves_freshness = true`.
- `Select` is handled by tracing BOTH branch operands and joining the results per dimension: `uniqueness(join)`, `preserves_freshness(AND)`, `locality(join)`, `shape(join)`.
- Scalar definitions (`Literal`, `PrimOp`) do not contribute to the contract — they have no RC.

- Results from all return paths are joined (IC-4: `uniqueness(join), preserves_freshness(AND), locality(join), shape(join)`).

**Note on ReturnContract locality**: the contract's `locality` represents the value IN THE CALLER's scope, not the callee's internal IA-6 widening. Fresh allocations start as `BlockLocal` (from the caller's perspective, the received value is a new local). IA-6 widens returned values to `HeapEscaping` WITHIN the callee body, but this is the callee's own analysis — the `ReturnContract` exports the value's ORIGIN locality (BlockLocal for fresh, HeapEscaping for returned parameters). TF-6 then sets the CALLER's variable to the contract's locality, and the caller's own IA-4/IA-6 rules widen as needed. CN-6 does NOT fire on the contract's Unique because the caller's variable starts at BlockLocal (not HeapEscaping).

CN-6 operates on per-variable `AimsState` during the intraprocedural analysis that runs WITHIN Step 1. It may demote `Unique → MaybeShared` for returned values within the function body's per-variable state. But this does not affect the `ReturnContract` because the extractor traces definitions structurally — it reads the IR instruction types (Construct, Apply, etc.), NOT the per-variable lattice state. Fresh allocations returned from a function retain `Unique` in the `ReturnContract` (enabling RL-29 `noalias`) regardless of what CN-6 does to the per-variable state.

The key correctness property: `ReturnContract` extraction traces definitions STRUCTURALLY (IR instruction types and callee contracts), then VALIDATES the result against the variable's PRE-WIDENING escape state. The structural trace provides the optimistic contract; the validation downgrades it when the variable escaped. Specifically: the validation checks whether the returned variable (before IA-6's `HeapEscaping` widening and CN-6's Unique→MaybeShared demotion) has any USE that causes escape (e.g., `Set` into a global, passed to a callee as Owned with HeapEscaping locality). If such an escaping use exists BEFORE the return, the structural trace's `Unique` is downgraded to `MaybeShared` and `preserves_freshness` to `false`. The validation does NOT check the post-IA-6/CN-6 converged state — that would always downgrade because IA-6 unconditionally widens returned values to HeapEscaping, triggering CN-6. Instead, it checks the variable's escape footprint from its definition to its return, EXCLUDING the return itself.

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
*Exception — return values*: CN-6 fires unconditionally within the intraprocedural analysis (values widened by IA-6 do get demoted to MaybeShared in the function body). However, the `ReturnContract` extraction preserves the pre-widening uniqueness — see §1.9 Return Provenance for the precise mechanism. This ensures RL-29 (noalias on fresh allocation returns) is achievable even though CN-6 correctly fires within the function.

**CN-7** — ~~REMOVED.~~ The former rule about Shared+CollectionBuffer forcing COW mode was a decision predicate result, not a lattice state mutation. The behavior is fully covered by DP-9 (`Shared ⟹ StaticShared`). Canonicalization rules SHALL only mutate lattice dimensions — assigning `cow_mode` is a decision, not a state mutation.

**CN-8** — Borrowed locality ceiling: `Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal`.
*Rationale*: A borrowed reference cannot escape its defining function — it is a temporary view. Placed before CN-6 so locality is precise when that rule fires.

**Rule ordering**: CN-8 fires before CN-6 (prevents Borrowed+HeapEscaping/Unknown from reaching CN-6). All active rules (CN-1, CN-2, CN-3, CN-5, CN-6, CN-8) are monotone (move dimensions toward top or enforce same-level consistency). Current rules reach fixed point in one pass; multi-round loop is defensive infrastructure.

---

## §3 Transfer Functions

Transfer functions define how each ARC IR instruction updates the lattice state. There are two directions: **forward** (defining a variable's initial state) and **backward** (computing demand from operand uses).

### Forward (Definition)

Every `ArcInstr` and `ArcTerminator` variant that defines a `dst` variable has an explicit forward transfer rule below. Instructions that produce no definition (side-effect-only) are listed as `N/A`. This list SHALL be exhaustive — adding a new instruction variant without a corresponding TF rule is a spec gap.

**TF-1** — Scalar literal (`Let { value = Literal }`): `dst.state := SCALAR`. Int, float, bool, char, byte, duration, size — no RC.

**TF-2** — Variable binding (`Let { value = Var(v) }`): `dst.state := state(v)`. Inherits source state.

**TF-2a** — PrimOp (`Let { value = PrimOp { .. } }`): `dst.state := SCALAR`. Primitive operations (arithmetic, comparison, bitwise) produce scalars.

**TF-3** — Construct allocation (`Construct`): `dst := FRESH(shape_from_ctor(ctor))`. Fresh means `(Owned, Linear, Once, Unique, BlockLocal, shape, {may_alloc=true})`. All constructors produce fresh heap memory with RC = 1. Shape mapping: `Struct → ReusableCtor(Struct)`, `EnumVariant → ReusableCtor(EnumVariant)`, `ListLiteral|SetLiteral|MapLiteral → CollectionBuffer`, `Tuple|Closure → NonReusable`.

**TF-4** — Field projection (`Project`): `dst := (Borrowed, Linear, Once, source.uniqueness, source.locality, NonReusable, NONE)`. Projection borrows a view of the source. Borrow source tracked in the `borrow_sources` side table (§1.9).

**TF-5** — Direct function call without contract (`Apply`, no `MemoryContract`): `dst := CONSERVATIVE`, defined as `(Owned, Unrestricted, Many, MaybeShared, Unknown, NonReusable, ALL)`. Note: `MaybeShared` (NOT `Shared`) is deliberate — an unknown callee's return value has unknown uniqueness, so it gets a runtime IsShared check (dynamic COW). `Shared` would pessimize by unconditionally copying. `MaybeShared` is below the lattice TOP for Uniqueness (`Shared`); `CONSERVATIVE` is therefore NOT the lattice-theoretic TOP element — it is the operationally correct default for unknown calls.

**TF-5a** — Indirect function call (`ApplyIndirect`): `dst := CONSERVATIVE` (same as TF-5). Closures have no interprocedural contract — always conservative.

**TF-6** — Direct function call with contract (`Apply`, with `MemoryContract`): `dst := refine(CONSERVATIVE, callee.return_contract)`. The `refine` function narrows the CONSERVATIVE default using the callee's `ReturnContract`:

```
refine(base, contract) → AimsState:
  result := base                           // start from CONSERVATIVE
  result.uniqueness := contract.uniqueness // Unique, MaybeShared, or Shared
  result.locality   := contract.locality   // narrows Unknown → actual scope
  result.shape      := contract.shape      // narrows NonReusable → actual shape
  // preserves_freshness is NOT an AimsState dimension — it is a contract
  // flag consumed by downstream contract extraction (§1.9 Return Provenance).
  // When a caller returns a value from a callee that preserves_freshness,
  // the caller's OWN return contract inherits that freshness transitively.
  // It does NOT modify the call-site result's AimsState directly.
  return result
```

Dimensions NOT narrowed by `refine`: Access (stays `Owned` — call results are always owned by the caller), Consumption (stays `Unrestricted` — the caller may use the result arbitrarily), Cardinality (stays `Many` — refined by actual usage during backward demand), Effect (stays `ALL` — the EffectSummary governs this, not the ReturnContract).

**TF-6a** — Invoke with contract (`Invoke`, with `MemoryContract`): Same as TF-6 (`refine(CONSERVATIVE, callee.return_contract)`). The unwinding edge does not affect the definition on the normal path.

**TF-6b** — Invoke without contract (`Invoke`, no `MemoryContract`): Same as TF-5 (`CONSERVATIVE`).

**TF-6c** — Indirect invoke (`InvokeIndirect`): Same as TF-5a (`CONSERVATIVE`). No contract available.

**TF-7** — Closure capture (`PartialApply`): `dst := FRESH(NonReusable)`. Closures are not reusable. Captured variables updated via `capture_state_update` (TF-13).

**TF-8** — Conditional selection (`Select`): if both operands are SCALAR (per L-9), `dst := SCALAR`. If one operand is SCALAR (including immortal constants per IA-8) and the other is not, the SCALAR operand is excluded from the join and `dst` inherits the non-SCALAR operand's state, BUT with `uniqueness := MaybeShared` (the result may be an immortal constant at runtime, so in-place mutation is unsafe — DP-4 will emit a dynamic COW check). Otherwise, `dst := state(true_val) ⊔ state(false_val)` (merge of both branches).

**TF-9** — Reuse (`Reuse { token }`): `dst := FRESH(shape_from_ctor(ctor))`. Reused memory gets fresh state.

**TF-9a** — Collection reuse (`CollectionReuse`): `dst := FRESH(CollectionBuffer)`. The old collection's allocation is recycled by the runtime; the result is a fresh collection with RC = 1.

**TF-10** — IsShared (`IsShared`): `dst := SCALAR`. Produces a boolean — not a refcounted value. *Rationale*: `IsShared` queries the refcount header and returns a plain `bool`. The result is a scalar control-flow input, not a managed allocation.

**TF-10a** — Reset (`Reset`): `dst := SCALAR` (the reuse token). *Rationale*: The reuse token is an opaque handle to the dead value's memory — it is NOT a refcounted allocation itself. It is consumed exactly once by the corresponding `Reuse` instruction. For **same-block reuse** (RL-11): the pairing is validated structurally at emission time — every Reset has exactly one Reuse in the same block, no intervening throwing instructions. For **cross-block reuse** (RL-12): the token crosses blocks but its availability is guaranteed by dominance/post-dominance analysis with a no-throw constraint; the safety model differs from same-block pairing. In both cases, the backward demand system does not track token liveness (tokens are SCALAR per L-9); the safety guarantee comes from the emission pass (same-block) or dominance analysis (cross-block).

**TF-N/A** — Side-effect-only instructions produce no definition:
- `RcInc`: increments refcount of existing variable. No `dst`.
- `RcDec`: decrements refcount of existing variable. No `dst`.
- `Set`: in-place field mutation on existing variable. No `dst`.
- `SetTag`: in-place tag mutation on existing variable. No `dst`.

### Backward (Demand)

Each operand of each instruction generates a `(variable, cardinality)` demand. This list SHALL be exhaustive — every `ArcInstr` and `ArcTerminator` variant appears below.

**TF-11** — Standard `(operand, cardinality=Once, consumption=Linear)` demand per argument. The `consumption=Linear` is load-bearing: without it, CN-1 would erase the demand. **Accumulation**: when a variable receives demands from MULTIPLE instructions (e.g., used as argument twice, or used directly AND through an alias), demands are ADDITIVELY combined for cardinality: `Once + Once = Many` (sequential resource counting, not lattice max). Consumption `seq_add` matrix: `Dead + X = X`, `X + Dead = X`, `Linear + Linear = Unrestricted`, `Linear + Affine = Unrestricted`, `Affine + Affine = Unrestricted`, `X + Unrestricted = Unrestricted` (any two non-Dead uses → Unrestricted). This is `seq_add`, not lattice join — critical for correct use-counting. IA-5 step (1) alias transfers also use `seq_add` for cardinality: if %4 = Let Var(%3) and %4 has Once cardinality, %3's existing demand is ADDED to (not joined with) the transferred Once. Applies to:

| Instruction | Demands |
|-------------|---------|
| `Let { value = Var(v) }` | NONE — transparent alias. IA-5 step (1) transfers the destination's full accumulated demand to `v`. An additional `(v, Once)` from step (2) would double-count the alias creation, inflating cardinality (a pure rename could turn Once into Many). Analogous to TF-12's suppression for PartialApply. |
| `Let { value = Literal }` | none |
| `Let { value = PrimOp { args } }` | `(arg, Once)` per arg |
| `Construct { args }` | `(arg, Once)` per arg |
| `Project { value }` | `(value, Once)` |
| `Apply { args }` | `(arg, cardinality=Once, consumption=Linear)` per arg; refined by callee contract (IC-3): Borrowed `Absent` → zero demand; Owned `Absent` → `(arg, Once, Linear)` (ownership transfer to callee for RL-5 dec — NOT zero demand); Owned non-Absent: `arg.locality := max(arg.locality, max(param.locality, ArgEscaping))`, `arg.access := Owned`; Borrowed with `may_share = true`: `arg.uniqueness := MaybeShared` |
| `ApplyIndirect { closure, args }` | `(closure, Once)`, `(arg, Once)` per arg |
| `Set { base, value }` | `(base, Once)`, `(value, Once)` |
| `SetTag { base }` | `(base, Once)` |
| `IsShared { var }` | `(var, Once)` |
| `Reset { var }` | `(var, Once)` |
| `Reuse { token, args }` | `(arg, Once)` per arg. Note: NO demand on `token` — the token is SCALAR (TF-10a) and L-9 excludes scalars from the demand system. Token availability is structurally guaranteed by the `Reset`/`Reuse` pairing (see TF-10a rationale). |
| `CollectionReuse { old_var, args }` | `(old_var, Once)`, `(arg, Once)` per arg |
| `Select { cond, true_val, false_val }` | `(cond, Once)` only. `true_val` and `false_val` receive NO TF-11 demand — IA-5 step (1) transfers the destination's full accumulated demand to both branch operands (conditional alias). Adding `(Once)` would double-count, inflating cardinality (same rationale as `Let Var` suppression above). |
| `RcInc { var }` | none (RC operation, not a use) |
| `RcDec { var }` | none (RC operation, not a use) |

**TF-11a** — Terminator backward demands:

| Terminator | Demands |
|------------|---------|
| `Return { value }` | `(value, Once)` |
| `Jump { args }` | `(arg, Once)` per arg |
| `Branch { cond }` | `(cond, Once)` |
| `Switch { scrutinee }` | `(scrutinee, Once)` |
| `Invoke { args }` | `(arg, Once)` per arg; refined by callee contract (IC-3, same as Apply above) |
| `InvokeIndirect { closure, args }` | `(closure, Once)`, `(arg, Once)` per arg |
| `Resume` | none (terminal) |
| `Unreachable` | none (terminal) |

**TF-12** — PartialApply emits NO backward demand from the standard demand system. Captured argument demand is handled entirely by `capture_state_update` (TF-13) to avoid double-counting.

**TF-13** — `capture_state_update(current, closure_state)` (OxCaml LAM rule):
- **Access promotion**: if `closure_state.locality >= HeapEscaping` (closure escapes to heap): `access := Owned`. A captured variable that escapes through a closure MUST be owned — Borrowed variables are function-scoped views that become dangling when the function returns. Without this promotion, CN-8 would clamp a Borrowed capture's locality to FunctionLocal, while the closure escapes to heap, creating a dangling reference. When access is promoted to Owned, the closure's environment increments the captured variable's RC, extending its lifetime.
- If closure cardinality ≤ Once: `consumption := seq_add(current.consumption, Affine)`, `cardinality := seq_add(current.cardinality, Once)` (additive — closure capture counts as an additional use), `locality := max(current.locality, closure_state.locality)`. Promotes Linear → Affine (closure capture is a non-destructive use, not a linear move); preserves uniqueness. Locality incorporates both current state AND closure's escape scope — if the closure escapes to heap, captured variables must also be at least HeapEscaping. No artificial `FunctionLocal` floor: a block-local closure capturing a block-local variable preserves BlockLocal locality (both are scoped to the same block).
- If closure cardinality > Once: `consumption := Unrestricted`, `cardinality := Many`, `locality := max(current.locality, closure_state.locality)`. Multiple invocations = multiple consumptions.

**TF-13 SHALL be monotone**: if `a ≤ b` then `capture_state_update(a, c) ≤ capture_state_update(b, c)`.

**TF-14** — Project backward demand propagation: `src.locality := max(src.locality, dst.locality)`, `src.cardinality := seq_add(src.cardinality, dst.cardinality)`, and if `dst.locality >= HeapEscaping`: `src.access := Owned`. This matches IA-5 step (1) Project semantics — conditional Owned promotion only for escaping borrows. Without this propagation, `src` can be freed or stack-allocated while a reference to its projected field escapes or outlives it.

---

## §4 Decision Predicates

Decision predicates map lattice states to RC/COW/reuse decisions. Most are pure functions of `AimsState`. Exception: DP-5 (`can_mutate_in_place`) additionally consults the `borrow_sources` side table, the `project_alias_sources` side table, and the live-variable set at the mutation point — see §1.9 for the side-table specifications.

**DP-1** — `is_rc_needed(s) ⟺ s.access = Owned ∧ s.consumption ≠ Dead ∧ ¬s.is_scalar()`.
Only owned, live, non-scalar variables carry RC obligations.

**DP-2** — `is_rc_dec_unnecessary(s) ⟺ s.cardinality = Absent ∨ s.consumption = Dead`.
No ADDITIONAL decrement beyond what the terminal emission rules (RL-2 last-use/scope-exit, RL-4 edge cleanup, RL-5 dead-at-entry) already handle. This predicate gates supplementary RC operations.

**DP-3** — `is_rc_inc_elidable(s) ⟺ s.cardinality = Once ∧ s.consumption = Linear`.
Moved once — no duplication, no increment needed.

**DP-4** — `needs_cow_check(s) ⟺ s.uniqueness = MaybeShared`.
Only MaybeShared needs a runtime uniqueness check. Unique takes fast path statically; Shared takes slow path statically.

**DP-5** — `can_mutate_in_place(s, var, field, point) ⟺ s.access = Owned ∧ s.uniqueness = Unique ∧ no_active_overlapping_borrows(var, field, point)`.
Unique ownership permits direct mutation without COW, BUT only when no active borrow from the same source overlaps the mutated field. Borrows (via `Project`) do not increment RC — a `Unique` value with an active borrow is still RC = 1 but mutating it would corrupt the borrowed reference. The `field` parameter identifies WHICH field is being mutated — required for the disjointness check (RL-10).

`no_active_overlapping_borrows(var, field, point)` is defined as: for all variables that borrow from `var` — checking BOTH direct borrows in `borrow_sources` AND transitive aliases in `project_alias_sources`:
1. **Direct borrows**: for all `b` in `borrow_sources` where `borrow_sources[b].source = var`: `b` is NOT live at `point`, OR `borrow_sources[b].field` is disjoint from `field`.
2. **Transitive aliases**: for all `a` in `project_alias_sources` where `var ∈ project_alias_sources[a]` AND `a` is NOT already a direct borrow in `borrow_sources` (i.e., `a ∉ borrow_sources` or `borrow_sources[a].source ≠ var`): `a` is NOT live at `point`. Direct borrows (variables in `borrow_sources` with `source = var`) are handled EXCLUSIVELY by Step 1 with field-level granularity — they MUST NOT also trigger Step 2's conservative check, because Step 2 blocks unconditionally regardless of field, which would make Step 1's disjoint-field optimization (RL-10) dead code. Aliases that are NOT direct borrows do not carry field-level granularity, so ANY live non-borrow alias of a projection from `var` blocks in-place mutation conservatively. This is sound — field tracking through alias chains is a precision extension, not a soundness requirement.

"Active" means live at the mutation point — a borrow (or alias) that has gone dead is not a conflict. The liveness intersection is required; checking `borrow_sources`/`project_alias_sources` without liveness would permanently disable COW fast paths for any object that was ever borrowed, even after all borrows and aliases die.

Note: DP-5 is NOT a pure function of `AimsState` alone — it additionally consults `borrow_sources`, `project_alias_sources` (§1.9), and the live-variable set at the program point. See Appendix C for the truth table (which includes the side-table input columns).

**DP-6** — `is_reuse_candidate(s) ⟺ s.access = Owned ∧ s.uniqueness ≠ Shared ∧ s.shape ≠ NonReusable`.
Reuse requires owned, non-shared, reusable shape.

**DP-7** — `is_rc_skip_eligible(s) ⟺ s.locality ≤ FunctionLocal ∧ s.access = Owned ∧ s.consumption = Linear ∧ s.cardinality = Once ∧ s.uniqueness = Unique ∧ ¬s.is_scalar()`. **Scope**: evaluated on the CALLEE's parameter state (not the caller's argument). The callee's parameter may be `FunctionLocal` even though the caller's argument was promoted to `ArgEscaping` by TF-11 — the parameter contract (IC-2/IC-3) reflects how the callee USES the parameter. If the callee uses a parameter locally (FunctionLocal + Linear + Once + Unique), the caller's ABI inc and callee's dec cancel. The ArgEscaping floor in TF-11 applies to the CALLER's argument state, not the callee's parameter contract.
Scope: **parameter inc/dec pair elision only.** For Owned parameters where the caller increments and the callee decrements, if the parameter is local+linear+unique, the inc/dec pair cancels — skip both. This does NOT apply to the final dec that triggers the free for fresh allocations; that dec is always needed for heap-allocated values (only stack-promoted values via RL-14 skip it). The `Uniqueness = Unique` requirement is load-bearing: a Shared value's +1 inc from the caller is never balanced → leak.

**DP-8** — `is_local(s) ⟺ s.locality ∈ {BlockLocal, FunctionLocal}`.
`ArgEscaping` is explicitly NOT local — the value escapes the defining function's scope (into a callee). While `ArgEscaping` values don't reach the heap, they cross the function boundary, which means the caller's inc/dec pair cannot be unconditionally elided (the callee may store the reference in a callee-local structure that outlives the call in unwinding scenarios). DP-7 (RC skip) requires `is_local()` precisely because local values never cross function boundaries. `ArgEscaping` values get their optimization through stack promotion in the caller (RL-15a), not through RC elision.

**DP-9** — `cow_mode(s)`:
- `Unique ⟹ StaticUnique` (in-place, no check)
- `MaybeShared ⟹ Dynamic` (runtime IsShared check)
- `Shared ⟹ StaticShared` (unconditional copy)

**DP-10** — ~~REMOVED (unsound).~~ The former rule claimed `Owned ∧ Linear ∧ Once ⟹ RC == 1`. This is false: backward analysis proves "no future duplication" (consumption/cardinality are FUTURE guarantees) but NOT "no existing aliases" (uniqueness is a PAST guarantee). A shared allocation passed as Owned+Linear+Once still has RC > 1 from aliases created before this program point. Uniqueness is ONLY established by (a) the Uniqueness dimension directly, or (b) fresh allocation (TF-3: FRESH starts Unique). Cross-dimensional "proofs" that derive past from future are unsound.

---

## §5 Interprocedural Contracts

**IC-1** — The call graph SHALL be decomposed into SCCs. SCCs SHALL be processed in topological order (callees before callers).

**IC-2** — Each parameter initializes to most optimistic: `(Borrowed, Dead, Absent, BlockLocal, Unique, may_share=false)`. Fixed-point iteration promotes toward conservative. The `may_share = false` initial assumes the callee does not increment the parameter's RC; if ANY call site's analysis shows the callee may share, the fixpoint promotes to `may_share = true`. Escape is derived from Locality (`locality > FunctionLocal ⟹ escapes`), not stored as a separate fact — the Locality dimension is the SSOT for escape classification.

**IC-3** — Parameter contract join is componentwise max: `access(max), consumption(max), cardinality(max), locality(max), uniqueness(max), may_share(OR)`. If join changes any dimension, iterate again. Note: `may_share` IS a per-parameter property (whether the callee may increment the parameter's RC) and remains on `ParamContract` — it is orthogonal to Locality. Only `may_escape` was removed (derived from `locality > FunctionLocal`).

**IC-4** — Return contract: `uniqueness(join), preserves_freshness(AND), locality(join), shape(join)`. Freshness requires ALL return paths to preserve it.

**IC-5** — Effect summary (interprocedural join): `may_allocate(OR), may_deallocate(OR), may_share(OR), may_throw(OR), has_unbounded_stack(OR)`. Note: `alloc_only_on_slow_path` is intraprocedural-only (post-realization, NOT part of the SCC fixpoint — see derivation below) and does NOT participate in the interprocedural join.

**Derivation**: EffectSummary is computed by scanning the function's instruction stream (NOT from per-variable EffectClass — see §1.7). Key instruction contributions:
- `Construct`, `Reuse`, `CollectionReuse`, `PartialApply`: `may_allocate = true`
- Deallocation: `may_deallocate = true` when the function's callee contracts include any callee with `may_deallocate = true`, OR the function takes Owned parameters (potential dec at exit), OR the analysis shows any owned variable transitioning to dead. Computed from analysis state and callee contracts, not from literal `RcDec` instructions.
- Sharing: `may_share = true` when the function may increment any parameter's RC (the parameter is used at cardinality > Once within the callee, or is stored into a data structure), OR any callee has `may_share = true`. Sharing instructions include: `Construct`/`Reuse`/`CollectionReuse` that store an existing variable (not a fresh allocation) as a field, and any instruction that duplicates a reference.
- `Apply`/`Invoke` with callee contract: inherit callee's `EffectSummary` via OR
- `Apply`/`Invoke` without contract, `ApplyIndirect`/`InvokeIndirect`: ALL effects conservative = `may_allocate=true, may_deallocate=true, may_share=true, may_throw=true, has_unbounded_stack=true, alloc_only_on_slow_path=false`
- `Invoke`/`InvokeIndirect` (unwinding edge): `may_throw = true`
- `Resume`: `may_throw = true`
- Functions with unbounded recursion: `has_unbounded_stack = true`
- `alloc_only_on_slow_path`: AND — computed POST-REALIZATION (after Step 10), not during Step 1. This field is derived from the realized ARC IR where COW branches (`IsShared` + `Branch`) exist. It is `true` only when every allocation in the realized IR is dominated by a COW slow path. At Step 1 time, this field is initialized to `false` (PESSIMISTIC). Because PL-1 runs Step 1 globally before any per-function realization, callers always read `false` during the SCC fixpoint. This field is therefore **intraprocedural-only** — it is refined per-function after that function's realization (Step 10) and consumed only by that function's own LLVM export (RL-30), NOT inherited by callers. Per-instruction classification (post-realization): unconditional `Construct`/`Reuse`/`CollectionReuse` → `false`; allocation inside a COW slow-path arm → `true`. Callee values are NOT inherited (intraprocedural-only).

**IC-6** — FIP contract: `Never` absorbs all; `Conditional` absorbs `Bounded/Certified`; `Bounded(n) ⊔ Bounded(m) = Bounded(max(n,m))`. FipContract::Certified ⟺ zero unmatched allocations/deallocations in realized IR. **Timing**: FipContract is computed POST-REALIZATION (Step 5a `verify_fip_contract()`), NOT during the Step 1 SCC fixpoint. Step 1 initializes FipContract to a conservative default (`Never`) and the real value is computed after realization when the alloc/dealloc IR is available. IC-7 includes FipContract in the convergence bound for theoretical completeness, but in practice the FipContract field does not iterate during Step 1.

**IC-7** — Convergence: finite domain guarantees termination. The iteration limit SHALL be derived from the domain heights: parameter contract 6 dimensions (access=1 + consumption=3 + cardinality=2 + locality=4 + uniqueness=2 + may_share=1 = 13 per param), return contract 4 dimensions (uniqueness=2 + freshness=1 + locality=4 + shape=1 = 8 total), EffectSummary 5 boolean fields (excluding `alloc_only_on_slow_path` which is post-realization, not part of the Step-1 fixpoint), ContextBehavior 4 boolean fields (PL-7/PL-11), FipContract (IC-6): `Certified < Bounded(1) < ... < Bounded(N) < Conditional < Never` — the order is ascending toward conservative (`Never` = most conservative/pessimistic = TOP, absorbs all in join; `Certified` = most precise = BOTTOM). Height: `alloc_count(function) + 2` (Certified to Never through Bounded levels). `Bounded(n) ⊔ Bounded(m) = Bounded(max(n,m))` is monotonically non-decreasing and bounded by alloc count. Formula: `param_count × 13 + 8 + 5 + 4 + alloc_count + 2`. In practice, the FIP contract converges quickly (1-2 iterations); the formula is a theoretical upper bound. If exceeded, widen all contracts to most conservative and emit a diagnostic. This document (`aims-rules.md`) is authoritative for the convergence bound; `arc.md` references it.

**IC-8a** — Address-taken functions and closures: functions whose call sites cannot be fully enumerated (indirect calls, closures stored in data structures) SHALL have their parameters initialized to CONSERVATIVE: `(Owned, Unrestricted, Many, Unknown, MaybeShared, may_share=true)`. This prevents the SCC fixpoint from retaining optimistic assumptions for functions that may be called from unknown sites. Closures with known call sites (e.g., immediately-invoked or single-use) are NOT subject to this rule — they participate in the normal SCC fixpoint.

**IC-8** — ~~REMOVED (unsound — same root cause as DP-10).~~ The former rule derived parameter uniqueness from caller consumption patterns (`Owned ∧ Linear ∧ Once`). This is unsound: a caller with a `MaybeShared` argument that it uses linearly still holds a reference whose RC may be > 1 from upstream aliases. Parameter uniqueness is established by the SCC fixpoint (IC-2/IC-3): if ALL callers pass arguments whose `Uniqueness` dimension is `Unique`, the fixpoint converges to `ParamContract.uniqueness = Unique` naturally. No post-fixpoint tightening is needed or sound.

---

## §6 Intraprocedural Analysis

**IA-1** — Analysis direction is BACKWARD. Future-use demand determines RC operations. Compute block exit states (demand from successors), then entry states (supply from predecessors).

**IA-2** — Blocks SHALL be processed in reverse postorder (successors before predecessors in the backward direction).

**IA-3** — Block exit state = `⊔(successor.entry_states)` using `alt_join` (only one successor executes per dynamic run).

**IA-4** — Cross-block locality widening: variables flowing across block boundaries SHALL have `locality := max(locality, FunctionLocal)`.

**IA-5** — Block entry state is computed by walking instructions in reverse from exit state. At each instruction: (1) apply backward interpretation of the forward transfer; (2) apply backward demands for operands per TF-11; (3) remove the defined variable from the state map.

**Step (1) detail — backward transfer depends on instruction type:**
- **`Let { Var(v) }` (transparent alias)**: the destination's accumulated demand transfers to the source. Cardinality and Consumption use `seq_add` (Once + Once = Many, Linear + Linear = Unrestricted — per TF-11's additive accumulation rule). Locality uses `max` (join). This ensures a variable used both directly AND through an alias is correctly counted as Many.
- **`Project` (borrow — NOT a transparent alias)**: `src.access := Owned` only if `dst.locality >= HeapEscaping` (borrow escapes to heap — source needs Owned for field cleanup). Otherwise `src.access` is unchanged (local borrows preserve the source's existing access, enabling borrow inference). `src.cardinality := seq_add(src.cardinality, dst.cardinality)`, `src.locality := max(src.locality, dst.locality)` (via TF-14). Consumption is NOT transferred. The standard TF-11 demand `(value, cardinality=Once, consumption=Affine)` additionally records the Project's direct operand use. `Affine` (not `Linear`) because Project is a non-destructive borrow.
- **`Select` (conditional alias)**: the destination's full accumulated demand transfers to BOTH `true_val` and `false_val` via `seq_add` (same as Let Var — additive accumulation for cardinality/consumption, max for locality) — because at runtime, one of them IS the destination. E.g., if `%5 = Select(cond, %3, %4)` and `%5` has `Many` cardinality, both `%3` and `%4` receive `Many`. This is the backward interpretation of TF-8: `dst := state(true_val) ⊔ state(false_val)` means both operands should absorb `dst`'s demand. `cond` receives only the TF-11 demand `(cond, Once)` (it's a boolean control input, not an alias).
- **`Construct`, `Reuse`, `CollectionReuse` (aggregate builders)**: the destination's LOCALITY transfers to all arguments: `arg.locality := max(arg.locality, dst.locality)`. Arguments SHALL ALWAYS be promoted to `access := Owned` (regardless of destination locality). Even for local structs, field cleanup at scope exit requires `RcDec` on fields, which requires the fields to be Owned. Without unconditional Owned promotion, field decs would underflow on Borrowed values. This prevents the "dangling stack-to-heap pointer" problem. Arguments also receive their standard TF-11 `(arg, Once)` demand from step (2). Cardinality is NOT transferred — the Construct uses each argument once regardless of how many times the result is used.
- **`Set { base, value }` (in-place field mutation)**: transfers `base.locality` to `value`: `value.locality := max(value.locality, base.locality)`. If `base.locality > FunctionLocal`, also `value.access := Owned` (prevents CN-8 clamping from destroying the escape signal). This prevents storing a FunctionLocal variable into a HeapEscaping aggregate and then incorrectly stack-allocating it.
- **Non-aliasing definitions** (`Apply`, `PartialApply`, etc.): step (1) has no backward transfer — the destination's demand does not flow to its arguments. Only step (2) applies. (`PartialApply` uses TF-13 for capture demand, which is separate from step (1).)

**Step (2)** adds the instruction's own operational demand per TF-11. Steps (1) and (2) are complementary: step (1) transfers accumulated state through aliasing, step (2) adds the instruction's direct operand demands.

**IA-6** — Return values SHALL be widened: `access := Owned`, `locality := max(locality, HeapEscaping)`. Returned values escape the function.

**IA-7** — Convergence by monotone iteration: iterate worklist until no block state changes or iteration limit exceeded. Limit: `CHAIN_HEIGHT × |variables| × |blocks|`, where `CHAIN_HEIGHT` is the maximum chain length of the product lattice = sum of per-dimension heights: Access(1) + Consumption(3) + Cardinality(2) + Uniqueness(2) + Locality(4) + Shape(1) + Effect(3) = **16**. If exceeded, widen all to the lattice-theoretic TOP: `(Owned, Unrestricted, Many, Shared, Unknown, NonReusable, ALL)` — note this uses `Shared` (true TOP for Uniqueness), not `MaybeShared` (which is CONSERVATIVE for unknown calls). This is a safety net, not expected behavior.

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
Step 4a: verify_trmc_soundness()   — TRMC structural verification (PL-10); rollback to pre-TRMC + re-run Steps 3-4 on failure (Step 3 recomputes ValueRepr for restored IR per PL-5)
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

**PL-9** — TRMC rewrite: the candidate function is internally normalized to accept a `ContextHole` parameter (Shape = ContextHole). This is an INTERNAL ABI change — the external calling convention is preserved via a wrapper thunk that allocates the initial ContextHole and delegates to the rewritten function. The recursive call fills the hole with the partially-constructed value instead of allocating a new frame. The rewrite SHALL be idempotent — a second pass produces identical IR. PL-10's "arity preserved" refers to the EXTERNAL arity (visible to callers); the internal rewritten function has an extra parameter but is never called directly from outside.

**PL-10** — TRMC structural verification: after Step 3a rewrite AND after Step 4 intraprocedural analysis (which operates on the rewritten IR), `verify_trmc_soundness()` SHALL confirm: (a) the ContextHole parameter is threaded correctly through recursive calls, (b) no allocation-free path introduces a new allocation, (c) the rewritten CFG is well-formed (consistent block parameters, valid RC state), (d) function arity and calling convention are preserved, and (e) constructor arguments are evaluated in the same order as the original (no side-effect reordering). This is a STRUCTURAL check, NOT a behavioral equivalence proof — full semantic equivalence checking is beyond the current verification scope. These five checks constitute VF-7 tier (a) for TRMC. VF-7 additionally requires tiers (b) and (c) — behavioral tests and a proof sketch — for TRMC to be active. This verification runs between Step 4 and Step 5 (before realization). If verification fails, the rewrite SHALL be rolled back to the pre-TRMC IR AND Steps 3-4 SHALL be re-run on the restored CFG (Step 3 recomputes ValueRepr, Step 4 re-analyzes — per PL-5: no stale summaries). The rollback-on-failure mechanism provides a safety net: unsound rewrites are caught and reversed, preserving the pre-TRMC behavior.

**PL-11** — `ContextBehavior` initialization and join. Initial values (most optimistic): `preserves_context = true`, `consumes_hole = true`, `requires_unique_context = false`, `may_resume_nonlinearly = false`. These are the weakest constraints — the SCC fixpoint promotes toward conservative. Derivation: `preserves_context` = true when all recursive calls pass the context hole unchanged; `consumes_hole` = true when all return paths fill the hole; `requires_unique_context` = true when any path requires the hole to be uniquely owned for soundness; `may_resume_nonlinearly` = true when any path may resume at the hole more than once. Join: `preserves_context(AND)`, `consumes_hole(AND)`, `requires_unique_context(OR)`, `may_resume_nonlinearly(OR)`. Conservative: preservation and consumption require ALL paths to agree; unique-context and non-linear-resumption are any-path (soundness obligations widen).

---

## §8 Realization & Post-Lattice Optimization

**Relationship to §7 Pipeline**: §7 defines the AIMS analysis and realization pipeline (Steps 1-12). §8 defines the realization RULES that Steps 5, 5a, 8, 8a, 10 apply, PLUS post-pipeline optimization passes (RL-22 through RL-26: KnownSafe pair elimination, PRE-style RC motion) that operate on the realized ARC IR AFTER the §7 pipeline completes. Post-pipeline passes are logically separate from the main pipeline — they take the fully realized, verified IR as input and optimize RC operations without re-running the analysis. PL-6 ("adding a new pass requires updating the pipeline ordering") governs passes WITHIN the pipeline; post-pipeline optimizations operate on the pipeline's OUTPUT and do not need slots in §7. **Post-pipeline verification**: after ALL post-pipeline optimization passes complete, the verification stack SHALL re-run: VF-1 (structural well-formedness), VF-2 (AIMS contract consistency — contracts still match realized IR), and VF-3 (oracle consistency). All three layers are required per VF-8. **VF-7 status of post-pipeline passes**: RL-22 through RL-26 are active rewrites under VF-7 and require all three tiers: (a) structural re-verify (VF-1) PLUS contract-consistency re-verify (VF-2/VF-3 — verifying that the rewrites did not invalidate inferred contracts or LLVM-exported facts), (b) behavioral tests (RC-motion regression tests), (c) documented proof sketch (RC operations only, no observable behavior change beyond memory timing). The three-tier requirement is the same for all active rewrites. If a post-pipeline pass is later integrated into the pipeline, it would then need a §7 slot per PL-6.

### RC Emission

**RL-1** — RC increment SHALL be emitted when a value is duplicated (passed to Owned parameter while still live).

**RL-2** — RC decrement SHALL be emitted at the last use of an owned value or at scope exit. This applies only to values that remain HEAP-ALLOCATED or that are stack-allocated WITH RC headers (RL-15a Owned/MaybeShared cases). Headerless stack-promoted values (RL-14 with Unique, RL-15a Borrowed+Unique) have no RC header and no RC operations — their cleanup is automatic via stack frame deallocation. Stack-allocated values WITH headers (RL-15a Owned case, using immortal RC) do receive RC operations from the callee but the immortal RC prevents actual deallocation. For heap-allocated values, RL-2 includes UNUSED owned non-scalar values (consumption = Dead from definition): if a variable is defined by a heap-allocating instruction (TF-3, TF-5, TF-5a, TF-6, TF-6a, TF-6b, TF-6c, TF-7, TF-9, TF-9a) but has no uses (consumption = Dead, cardinality = Absent), an immediate `RcDec` SHALL be emitted at the definition point. Without this, unused call results and discarded allocations leak. DP-1 (`is_rc_needed`) returns false for Dead variables, but that gates supplementary RC tracking — the definitional cleanup dec is mandatory regardless of DP-1. DP-2 (`is_rc_dec_unnecessary`) does NOT suppress the definitional cleanup dec — DP-2 gates ADDITIONAL decs beyond the terminal cleanup handled by RL-2/RL-4/RL-5.

**RL-3** — RC operations SHALL be ELIDED when the lattice proves they are unnecessary (DP-2, DP-3, DP-7).

**RL-4** — Edge-specific decrements: an OWNED non-scalar variable alive at block exit but dead at successor entry SHALL receive a decrement on that specific CFG edge. **Jump argument exemption**: variables passed as `Jump` arguments transfer ownership to the successor block parameter — they are NOT "dead at successor entry" (the block param IS the successor's name for the same value). RL-2 and RL-4 do NOT emit decs for Jump arguments; the successor's block parameter inherits the RC obligation. Borrowed variables do NOT receive decrements.

**RL-5** — Dead-at-entry cleanup: an OWNED non-scalar block parameter with `Cardinality = Absent` at entry SHALL receive an immediate decrement. Borrowed parameters with Absent cardinality do not need decrements.

### COW (Copy-on-Write)

**RL-6** — Static unique mutation (`Uniqueness = Unique`): emit in-place mutation, no IsShared check.

**RL-7** — Dynamic COW (`Uniqueness = MaybeShared`): emit IsShared check, branch to in-place (unique) or copy (shared) paths.

**RL-8** — Static shared mutation (`Uniqueness = Shared`): emit unconditional copy before mutation.

**RL-9** — COW compound contraction: diamond CFG patterns `IsShared → Branch → (clone+Set | Set) → Merge` SHALL be contracted into a single compound instruction.

**RL-10** — Disjoint field mutation SHALL NOT trigger COW: if receiver is mutated at field F (via `Set`) and all active borrows from the same source are from different fields, the mutation is safe without COW check. DP-5 enforces this via the field-aware disjointness check in Step 1. **`SetTag` is excluded from disjoint-field optimization**: changing an enum's discriminant tag invalidates ALL payload fields (the new variant may have a different memory layout), so `SetTag` conflicts with ALL active borrows regardless of field. DP-5 SHALL treat `SetTag` as overlapping every field.

### Allocation Reuse

**RL-11** — Same-block reuse: a dying value's allocation SHALL be reused for a fresh allocation of the same type in the same block, if: (a) the `Reset` (death) precedes the `Reuse` (allocation), (b) no intervening instruction between the `Reset` and `Reuse` may throw (`may_throw`), may allocate, or may use the dying value or any alias of it (an intervening throwing instruction would leak the reuse token if the exception path doesn't consume it; an intervening allocation could invalidate the reuse opportunity; an intervening use of the dying value or its alias is impossible by definition since the value is dead, but aliases through `project_alias_sources` must be checked), AND (c) the dying value has `Uniqueness = Unique`. Reusing a non-unique allocation corrupts other aliases. **RL-11a** — Dynamic reuse for `MaybeShared` values: emit `IsShared` check; if unique at runtime → `Reset`/`Reuse` (fast path); if shared → fresh allocation (slow path). This requires DP-6 eligibility (`Owned ∧ ≠ Shared ∧ reusable shape`).

**RL-12** — Cross-block reuse: a dying value's allocation SHALL be reused across blocks when the death block dominates the allocation block AND the allocation block post-dominates the death block (EXCLUDING unwind edges), AND both blocks share the same innermost loop header (or neither is in a loop) — prevents multiplicity mismatch between sequential loops at the same nesting depth, the dying value is statically unique, AND no block on the path between Reset and Reuse contains a potentially-throwing instruction (`Invoke`, `InvokeIndirect`, or any instruction with `may_throw`). The no-throw constraint prevents token leaks on unwind paths: if an exception unwinds between Reset and Reuse, the token (SCALAR, not tracked by RC) would be permanently leaked because the Reuse never executes. Cross-block reuse uses the same `Reset`/`Reuse` pair as RL-11 with the dominance precondition guaranteeing token availability on normal paths.

**RL-13** — ~~REMOVED (unsound — same root cause as DP-10).~~ The former rule claimed `Construct + Once = RC == 1 at death`. This is false: the one use may be "store into a data structure," which creates an alias via RcInc. At death, the original variable's allocation is still alive via the alias. Reuse would overwrite live memory. Reuse eligibility is determined SOLELY by the Uniqueness dimension (DP-6 + RL-11/RL-12).

### Stack Promotion

**RL-14** — Non-escaping allocations (`Locality ≤ FunctionLocal ∧ Uniqueness = Unique`) with fixed size SHALL be stack-allocated via `alloca`. No RC header. No RC operations. Stack deallocation at scope exit. The `Uniqueness = Unique` requirement is load-bearing: a `MaybeShared` value that is stack-promoted without a header would crash on `IsShared` (DP-4). **Heap children**: if a headerless stack allocation contains reference-type fields pointing to heap-allocated values, the stack value has no Drop/RC mechanism to release those children. The emission phase SHALL emit explicit `RcDec` for each heap-allocated field at scope exit AND on CFG edges where the parent dies (matching RL-4's edge-specific cleanup). VF-1 exempts these field-drop Project+RcDec sequences from the DecOnBorrowed check.

**RL-14a** — Non-escaping fixed-size allocations with `Uniqueness ≠ Unique` (`Locality ≤ FunctionLocal`) SHALL be stack-allocated via `alloca` WITH RC header initialized to `MAX_REFCOUNT` (immortal — prevents free on stack pointer). **Heap children**: field-level `RcDec` at scope exit per RL-14.

**RL-15** — Non-escaping dynamic-size allocations (`Locality ≤ FunctionLocal`) SHALL use a function-local bump allocator. Bump-allocated objects retain RC headers if `Uniqueness ≠ Unique`. **Heap children**: same as RL-14. Entire bump region freed at function return.

**RL-15a** — ArgEscaping allocations (`Locality = ArgEscaping`) SHALL be stack-allocated in the CALLER. If the variable is passed to MULTIPLE callees, the allocation strategy uses the MOST CONSERVATIVE requirement among all uses. Strategy per callee parameter contract:
- **Callee parameter = `Borrowed` AND `Uniqueness = Unique` AND `may_share = false`**: headerless stack allocation (no RC operations emitted by the callee). **Heap children**: field-level `RcDec` at caller scope exit per RL-14 (VF-1 exemption applies). If `may_share = true`, the callee may `RcInc` (writing the header), so immortal RC header is required instead.
- **Callee parameter = `Borrowed` AND `Uniqueness ≠ Unique`** (MaybeShared or Shared): caller-stack allocation WITH RC header. The callee may emit `IsShared` checks (DP-4), which read the header. Headerless allocation would crash.
- **Callee parameter = `Owned`**: caller-stack allocation WITH RC header, initialized to `MAX_REFCOUNT` (immortal). The callee may emit `RcDec` or `IsShared`, so the header is required. The immortal RC prevents the callee's `RcDec` from triggering free() on a stack pointer — dec on an immortal value is a no-op. The caller-stack lifetime still outlives the callee's execution.
- **Closure capture (PartialApply)**: when a variable is captured by an ArgEscaping closure, TF-13 does NOT promote access to Owned (only >= HeapEscaping triggers Owned promotion). CN-8 clamps Borrowed+ArgEscaping to Borrowed+FunctionLocal, so the captured variable's effective locality is FunctionLocal. This means the variable is handled by RL-14 (non-escaping, FunctionLocal stack allocation) rather than RL-15a. No separate ArgEscaping closure-capture rule is needed — the CN-8 clamping routes these values to the correct RL-14 path.
- **Note on CN-8**: `Borrowed` values have their locality clamped to `≤ FunctionLocal` by CN-8, so a `Borrowed` value never reaches `ArgEscaping` in the lattice. `ArgEscaping` only arises for `Owned` values that escape into a callee but not to the heap. The `Borrowed` callee parameter case above refers to the CALLEE'S contract for the PARAMETER (how the callee treats the received value), not the value's own Access dimension at the caller. The caller's value is `Owned`+`ArgEscaping`; the callee receives it as `Borrowed` (per the callee's parameter contract from IC-3).

This is the key optimization that bridges "not local" and "not heap" — a value that escapes into a callee but not to the heap gets caller-stack lifetime.

**RL-16** — Escaping allocations (`Locality ≥ HeapEscaping`) SHALL be heap-allocated with full RC header.

### RC Header Compression

**RL-17** — Sharing bound analysis SHALL determine maximum simultaneous reference count. `is_local(s) ∧ Unique → no header`, straight-line N incs → `Bounded(N+1)`, loops/recursion/global → `Unbounded`. (Note: `is_local(s)` is defined by DP-8: `locality ∈ {BlockLocal, FunctionLocal}`. The former `NoEscape` term is replaced by this Locality-based condition per RL-18a.)

**RL-18** — RC header width SHALL be narrowed based on RL-17's sharing-bound result, ONLY for values that do NOT escape the compilation unit (non-escaping types where layout is never observed by unspecialized callees or `dyn Trait` dispatch): `Unique (and is_local per RL-17) → none`, `Bounded(≤127) → i8`, `Bounded(≤32767) → i16`, `Bounded(≤2^31-1) → i32`, `Unbounded → i64`. Escaping types (passed through `dyn Trait`, FFI, or compilation unit boundaries) SHALL always use full-width (i64) headers to maintain ABI consistency ("same type = same layout").

### Unified Representation Constraint

**RL-18a** — All escape-driven decisions (stack allocation via RL-14/15/16, header width via RL-17/18, atomic vs non-atomic via RL-19/20/21) SHALL consume the single unified `Locality` dimension. Parallel escape enums (`EscapeState`, `ThreadLocality`, `HeapEscapeStatus`, or any equivalent shadow representation) are FORBIDDEN. The `Locality` dimension is the SSOT for all escape classification. This constraint ensures that extending escape analysis extends ONE dimension, not N parallel data structures that drift independently.

### Non-Atomic RC

**RL-19** — Thread-local values (no cross-thread escape) SHALL use non-atomic RC operations (plain load/store instead of atomic CAS). Thread-locality is derived from `Locality` + call-graph analysis: if no escape path crosses a thread boundary (spawn, channel send, FFI), the value is thread-local. The `Locality` dimension provides escape scope; thread-sharing analysis is a program-wide property layered on top (similar to how RL-21 detects the whole-program no-spawn case). Future Locality extension with explicit `ThreadShared` level is planned but not required — thread analysis can derive from `HeapEscaping` + call-graph thread-boundary detection.

**RL-20** — Thread-shared values SHALL use atomic RC operations.

**RL-21** — If a program has no spawn/channel operations AND no FFI calls that export Ori-managed pointers to foreign code, ALL values are thread-local and ALL RC is non-atomic. FFI is included because foreign code may hand the pointer to another thread (per RL-19's thread-boundary escape classification).

### KnownSafe Pair Elimination

**RL-22** — When the physical refcount is provably positive (outer RcInc in scope, no intervening decrement), inner `RcInc`/`RcDec` pairs on the same variable SHALL be eliminated. **KnownSafe definition**: a variable is `KnownSafe = true` at a program point when there exists a dominating `RcInc` for that variable with no intervening `RcDec` between the inc and the current point. This means the physical RC is at least 2 (the original + the dominating inc), so an inner dec cannot trigger a free. Initial value: `false` at function entry (no dominating inc). Transfer: `RcInc(v)` → `KnownSafe(v) := true`; `RcDec(v)` → `KnownSafe(v) := false` AND `KnownSafe(a) := false` for ALL aliases of `v`; `Set`/`SetTag` → `KnownSafe(v) := false` for all `v` (field overwrite/tag change implicitly drops the old value, which may recursively decrement shared references); function calls with `may_deallocate` → `KnownSafe(v) := false` for all `v`. Alias tracking uses SSA def-use chains (Let Var, Jump arg-to-param, Select).

**RL-23** — KnownSafe flag propagation at join points: `true` only if ALL predecessors agree.

### PRE-Style Global RC Motion

**RL-24** — Bidirectional dataflow (bottom-up release analysis + top-down retain analysis) SHALL identify matching `(Inc, Dec)` pairs across basic blocks.

**RL-25** — A pair is eliminable when: KnownSafe = true OR both forward/backward paths are safe, AND no CFG hazard (path count alignment).

**RL-26** — RC motion SHALL NOT move operations across calls that may observe RC. "May observe RC" uses the same conditions as RL-27: the callee has any `Owned` + non-`Dead` parameter, OR any `Borrowed` parameter with `may_share = true`. Pure calls (all Borrowed + no may_share) are transparent to RC motion.

### Selective Barriers

**RL-27** — At call sites, RC operations SHALL be flushed for variables whose callee parameters are `Owned` + non-`Dead`, OR `Borrowed` with `may_share = true` (the callee may write RC headers). Borrowed parameters with `may_share = false` and pure functions (all params Borrowed + no may_share) require no barrier.

**RL-28** — Unknown callees (FFI, indirect, no contract) require conservative flush of all pending RC operations.

### AIMS → LLVM Fact Export

**RL-29** — Fresh allocation returns (`ReturnContract.preserves_freshness = true AND ReturnContract.uniqueness = Unique`) SHALL be marked with LLVM `noalias`. The gate is `preserves_freshness`, not `uniqueness` alone — a return that is merely `Unique` (e.g., a uniquely-held parameter passed through) is not necessarily fresh (may alias the caller's copy). `preserves_freshness` is the proof that the returned pointer was produced by a fresh allocation or by a callee whose return contract also preserves freshness.

**RL-30** — Effect-based call annotations. Parameter access classification is derived from IC-3 `ParamContract.access` AND `may_share`: `Borrowed` with `may_share = false` = reads only, `Borrowed` with `may_share = true` = reads AND writes (RC header updates are writes to inaccessible memory), `Owned` = reads and writes. Parameters with `ParamContract.cardinality = Absent` (dead per IC-2/IC-3) have no access. "No writes to parameters" = all non-dead params (cardinality ≠ Absent) are `Borrowed` AND `may_share = false`. "No arg access" = all params have `cardinality = Absent`. Pure calls with NO arg access AND no sharing (`may_allocate = false ∧ may_deallocate = false ∧ may_throw = false ∧ may_share = false` and all params have `cardinality = Absent`) SHALL receive LLVM `memory(none)`. If `may_share = true` AND params are Borrowed (reads args + writes RC headers) → `memory(argmem: read, inaccessiblemem: readwrite)`. If `may_share = true` AND no params → `memory(inaccessiblemem: readwrite)`. Pure calls that READ but don't write parameters (`may_allocate = false ∧ may_deallocate = false ∧ may_throw = false` and all non-dead params are `Borrowed` with `may_share = false`) SHALL receive LLVM `memory(argmem: read)`. Allocating calls (`may_allocate = true`) that also access arguments SHALL receive `memory(argmem: readwrite, inaccessiblemem: readwrite)`. Pure allocators (no arg access) SHALL receive `memory(inaccessiblemem: readwrite)`. Note: `alloc_only_on_slow_path` (IC-5) is an intraprocedural annotation for potential future use in split-path LLVM attribute emission (where the fast path would receive tighter memory attributes than the slow path); it does NOT modify the function-level RL-30 attributes above, which are whole-function summaries. Deallocating non-allocating calls (`may_deallocate = true, may_allocate = false`): `memory(inaccessiblemem: readwrite)` (+ `argmem: readwrite` if Owned params or `may_share`). Non-allocating argument writers (Owned params, `may_share = false`): `memory(argmem: readwrite)`. **Fallback**: any effect combination not explicitly listed above SHALL receive the conservative `memory(readwrite)`. `memory(argmem: readwrite)` ALONE is WRONG for allocators; omitting `argmem` is WRONG for functions that access arguments.

**RL-31** — Disjoint borrowed parameters SHALL receive `!alias.scope` + `!noalias` metadata pairs. "Disjoint" means no two `Borrowed` parameters can alias the same memory at runtime. The proof requires a **cross-function provenance summary** not carried by IC-2/IC-3 alone: each call site must prove that the actual arguments passed to distinct `Borrowed` parameters trace to different source aggregates or disjoint fields. This is a separate analysis pass that computes per-call-site disjointness from the callers' `borrow_sources` and `project_alias_sources` tables. Normative procedure: at each call site, for each pair of `Borrowed` parameters `(p_i, p_j)`, check whether ALL actual arguments passed to `p_i` and `p_j` have provably different root sets. For arguments that ARE in `project_alias_sources`, the root set is extracted per the root-extraction procedure. For arguments NOT in `project_alias_sources`: if the argument traces to a FRESH allocation (`Construct`/`Reuse`/`CollectionReuse`) via definition tracing, it is its own disjoint root. Otherwise, the argument conservatively FAILS disjointness (parameters, heap loads, and other non-fresh values may alias external memory). This restriction to fresh allocations prevents unsound `noalias` on SSA-distinct variables that may alias the same memory. "Different source aggregates" = the arguments trace to non-overlapping ROOT variable sets. Roots are extracted by filtering `project_alias_sources[arg]` to variables that have no UPSTREAM source in the map — i.e., they are not a key in `project_alias_sources` themselves, OR their source list is empty (they are the original non-projected aggregates at the end of the alias chain). For `%4 → [%3, %2]` where `%3 → [%2]`, the root is `{%2}` (the only variable with no upstream). When the root sets of two arguments are disjoint, the parameters are disjoint. **Verification**: RL-31 disjointness proofs SHALL be checked by VF-2 (contract consistency) — incorrect alias metadata is a contract violation detectable by comparing emitted `!noalias` metadata against the disjointness analysis result. "Disjoint fields of the same source" = the originating `Project` instructions have different field indices. For transitive aliases (Let Var, block-param, Select), trace back to the originating Project to read the field; aliases without a resolvable Project origin conservatively fail the disjointness check. If ANY call site fails the check, the parameter pair does NOT receive alias metadata. The disjointness proof is conservative: if any call site cannot prove disjointness, the parameter does not receive alias metadata. Note: `borrow_sources` is function-local (§1.9), so the cross-function disjointness proof operates on CALLERS' local provenance tables at call sites, not on a global provenance carrier.

### Borrow Inference

**RL-32** — All non-scalar parameters initialize to `Borrowed`. Fixed-point iteration promotes to `Owned` based on demand.

**RL-33** — Projection propagation: if a projected field becomes `Owned`, the source variable SHALL be promoted to `Owned`.

**RL-34** — Tail call preservation: never insert `RcDec` after a tail call. Transfer ownership instead — BUT only when the callee parameter is `Owned`. If the callee parameter is `Borrowed`, ownership transfer violates the ABI (callee expects a borrow, caller's dec never fires → leak). A call to a `Borrowed` parameter cannot be optimized as a tail call unless the caller can arrange for the dec to happen before the call.

---

## §9 Verification Layers

The verification stack is **layered**. Each layer catches a different class of inconsistency.

**VF-1** — Layer 1 (Structural): ARC IR well-formedness. Checks: use-before-def, dangling block refs, RC on scalar, dec on borrowed (EXCEPT field-drop projections emitted by RL-14/RL-15 scope cleanup — these Project+RcDec sequences are marked as field drops and exempt from the DecOnBorrowed check), arg ownership length mismatch. Runs at three checkpoints: (1) after AIMS emission (Step 6), (2) after full pipeline (Step 11), (3) after post-pipeline optimization passes (§8 RL-22 through RL-26).

**VF-2** — Layer 2 (AIMS Contract): filters structural verifier output to AIMS-specific inconsistencies. Checks: (a) parameters declared `Absent` must have no live uses; (b) RL-31 alias metadata (`!noalias`/`!alias.scope`) must be backed by a valid disjointness proof from the RL-31 normative procedure — metadata without proof is a contract violation.

**VF-3** — Layer 3 (Oracle): re-derives `MemoryContract` from realized IR and compares against inferred contract along access, consumption, and effects dimensions. Unsafe mismatches (analysis more optimistic than realization) are errors.

**VF-4** — Layer 4 (FIP Certification): proves `FipContract::Certified` functions have zero unmatched allocations/deallocations. Failures are wrapped as `FipStructural` in the shared verification stream.

**VF-5** — Every active subsystem SHALL be end-to-end verified: implementation + invariant enforcement + tests. Missing any of the three = incomplete.

**VF-6** — Contracts and realization SHALL agree. If `MemoryContract` says `FipContract::Certified`, realized IR SHALL have zero unmatched alloc/dealloc.

**VF-7** — Active rewrites SHALL be sound: identical observable behavior. Each active rewrite requires ALL THREE of: (a) **compile-time structural verification** — the pass validates well-formedness and rolls back on failure; (b) **test-time behavioral verification** — dedicated tests exercise pre/post-rewrite behavior for representative programs; (c) **documented proof sketch** — the spec identifies the semantic preconditions under which structural validity implies behavioral equivalence. For TRMC: (a) = PL-10's five structural checks, (b) = TRMC spec tests that verify input/output equivalence with and without the rewrite, (c) = the constrained-rewrite argument (context-hole threading only, arity preserved, evaluation order unchanged). For post-pipeline RL-22/26 passes: (a) = VF-1+VF-2+VF-3 re-verify (structural + contract consistency + oracle, per §8), (b) = RC-motion regression tests, (c) = the constrained-rewrite argument (RC operations only, no observable behavior change beyond memory timing). Rewrites lacking any of the three tiers are not active.

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

---

## Appendix A: Forward Transfer Matrix

Exhaustive mapping of every `ArcInstr` and `ArcTerminator` variant to its output `AimsState` per dimension. **These are PRE-CANONICALIZATION values** — the transfer function output before §2 canonicalization rules fire. For example, TF-4 `Project` shows `src.loc` as the raw locality, but CN-8 will clamp it to `FunctionLocal` if the raw value exceeds `FunctionLocal` (because the result is `Borrowed`). Instructions that produce no definition are marked `—`. The `Rule` column references the governing TF rule from §3.

| Instruction | Rule | Access | Consumption | Cardinality | Uniqueness | Locality | Shape | Effect |
|---|---|---|---|---|---|---|---|---|
| `Let { Literal }` | TF-1 | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR |
| `Let { Var(v) }` | TF-2 | `state(v)` | `state(v)` | `state(v)` | `state(v)` | `state(v)` | `state(v)` | `state(v)` |
| `Let { PrimOp }` | TF-2a | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR |
| `Construct` | TF-3 | Owned | Linear | Once | Unique | BlockLocal | `shape_from_ctor` | `{may_alloc=T}` |
| `Project` | TF-4 | Borrowed | Linear | Once | `src.uniq` | `src.loc` | NonReusable | NONE |
| `Apply` (no contract) | TF-5 | Owned | Unrestricted | Many | MaybeShared | Unknown | NonReusable | ALL |
| `Apply` (contract) | TF-6 | Owned | Unrestricted | Many | `contract.uniq` | `contract.loc` | `contract.shape` | ALL |
| `ApplyIndirect` | TF-5a | Owned | Unrestricted | Many | MaybeShared | Unknown | NonReusable | ALL |
| `PartialApply` | TF-7 | Owned | Linear | Once | Unique | BlockLocal | NonReusable | `{may_alloc=T}` |
| `Select` | TF-8 | `⊔`* | `⊔`* | `⊔`* | `⊔`* | `⊔`* | `⊔`* | `⊔`* |
| `IsShared` | TF-10 | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR |
| `Reset` | TF-10a | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR | SCALAR |
| `Reuse` | TF-9 | Owned | Linear | Once | Unique | BlockLocal | `shape_from_ctor` | `{may_alloc=T}` |
| `CollectionReuse` | TF-9a | Owned | Linear | Once | Unique | BlockLocal | CollectionBuffer | `{may_alloc=T}` |
| `RcInc` | TF-N/A | — | — | — | — | — | — | — |
| `RcDec` | TF-N/A | — | — | — | — | — | — | — |
| `Set` | TF-N/A | — | — | — | — | — | — | — |
| `SetTag` | TF-N/A | — | — | — | — | — | — | — |
| `Invoke` (contract) | TF-6a | Owned | Unrestricted | Many | `contract.uniq` | `contract.loc` | `contract.shape` | ALL |
| `Invoke` (no contract) | TF-6b | Owned | Unrestricted | Many | MaybeShared | Unknown | NonReusable | ALL |
| `InvokeIndirect` | TF-6c | Owned | Unrestricted | Many | MaybeShared | Unknown | NonReusable | ALL |

Legend: `⊔` = componentwise join of branch operands. `⊔*` = join with TF-8 scalar exclusion: if both operands are SCALAR → SCALAR; if one is SCALAR → inherits non-SCALAR operand BUT with `uniqueness := MaybeShared` (prevents in-place mutation of immortal constants); otherwise normal join (L-9). `CONSERVATIVE` = `(Owned, Unrestricted, Many, MaybeShared, Unknown, NonReusable, ALL)` — the operationally correct default for unknown calls (NOT the lattice-theoretic TOP; `MaybeShared` is deliberately below `Shared` to enable dynamic COW checks). `contract.uniq`/`contract.loc`/`contract.shape` = narrowed from CONSERVATIVE by `callee.return_contract` via the `refine()` function (see TF-6); Access, Consumption, Cardinality, and Effect stay at CONSERVATIVE values. `NONE` = `{may_alloc=F, may_share=F, may_throw=F}`. `ALL` = `{may_alloc=T, may_share=T, may_throw=T}`. `shape_from_ctor`: Struct→ReusableCtor(Struct), EnumVariant→ReusableCtor(EnumVariant), ListLiteral/SetLiteral/MapLiteral→CollectionBuffer, Tuple/Closure→NonReusable.

Terminators `Return`, `Jump`, `Branch`, `Switch`, `Resume`, `Unreachable` produce no definitions (no `dst`). `Invoke`/`InvokeIndirect` are listed above (they define `dst` on the normal path).

---

## Appendix B: Infeasible State Table

The following dimension combinations are eliminated by canonicalization. They SHALL NOT appear in any converged `AimsStateMap`. An implementation that produces any of these states has a canonicalization bug.

| Infeasible Combination | Eliminating Rule | Result |
|---|---|---|
| `Consumption = Dead ∧ Cardinality ≠ Absent` | CN-1 | `Cardinality := Absent` |
| `Cardinality = Absent ∧ Consumption ≠ Dead` | CN-1 | `Consumption := Dead` |
| `Consumption = Linear ∧ Cardinality = Absent` | CN-2 (+ CN-1) | `Consumption := Dead, Cardinality := Absent` |
| `Uniqueness = Shared ∧ Shape ∈ ReusableCtor(*)` | CN-3 | `Shape := NonReusable` |
| `Access = Borrowed ∧ Locality > FunctionLocal` | CN-8 | `Locality := FunctionLocal` |
| `Locality ≥ HeapEscaping ∧ Uniqueness = Unique` | CN-6 | `Uniqueness := MaybeShared` |

**Derived infeasible combinations** (follow from composing two or more active rules):

| Derived Combination | Why Infeasible |
|---|---|
| `Access = Borrowed ∧ Locality ≥ HeapEscaping ∧ Uniqueness = Unique` | CN-8 fires first (Locality → FunctionLocal), so the state never reaches CN-6's trigger condition. The three-way combination is doubly eliminated. |
| `Access = Borrowed ∧ Locality = Unknown` | CN-8 clamps all Borrowed+wide-locality to FunctionLocal — `Unknown` is unreachable for Borrowed values. |
| `Access = Borrowed ∧ Locality = HeapEscaping` | Same as above — CN-8 forces FunctionLocal. |
| `Access = Borrowed ∧ Locality = ArgEscaping` | Same as above — CN-8 fires for any Locality > FunctionLocal. |
| `Consumption = Dead ∧ Cardinality ∈ {Once, Many}` | CN-1 forces Cardinality := Absent when Consumption = Dead. |
| `Consumption ∈ {Linear, Affine, Unrestricted} ∧ Cardinality = Absent` | CN-1 forces Consumption := Dead when Cardinality = Absent. |

**Reachable boundary states** (NOT infeasible — documented to prevent false positives):

| Boundary State | Why Reachable |
|---|---|
| `Unique ∧ Dead ∧ ReusableCtor(*)` | CN-5: unique dead values preserve reuse shape (allocation available for reset) |
| `MaybeShared ∧ BlockLocal` | Fresh values retain BlockLocal if single-block (IA-4); MaybeShared from join with a MaybeShared path (e.g., at a CFG merge where one predecessor has Unique and another has MaybeShared) |
| `Owned ∧ Absent ∧ Dead` | Dead parameters passed by callers — receives dec at entry (RL-5) |

---

## Appendix C: Decision Predicate Truth Tables

Each predicate is a function of the lattice state. Most are pure functions of `AimsState`; DP-5 additionally consults `borrow_sources`, `project_alias_sources`, and the live-variable set at the mutation point (see §1.9 and §4 DP-5). The tables below show the exact conditions for each decision. All predicates operate on canonical states only (post-canonicalization).

### DP-1: `is_rc_needed(s)`

| Access | Consumption | is_scalar | Result |
|---|---|---|---|
| Borrowed | any | any | **false** |
| Owned | Dead | any | **false** |
| Owned | ≠ Dead | true (SCALAR) | **false** |
| Owned | ≠ Dead | false | **true** |

### DP-2: `is_rc_dec_unnecessary(s)`

| Cardinality | Consumption | Result |
|---|---|---|
| Absent | (must be Dead via CN-1) | **true** |
| any | Dead | **true** |
| Once | ≠ Dead | **false** |
| Many | ≠ Dead | **false** |

### DP-3: `is_rc_inc_elidable(s)`

| Cardinality | Consumption | Result |
|---|---|---|
| Once | Linear | **true** |
| Once | ≠ Linear | **false** |
| ≠ Once | any | **false** |

### DP-4: `needs_cow_check(s)`

| Uniqueness | Result |
|---|---|
| Unique | **false** (static unique path) |
| MaybeShared | **true** (runtime check needed) |
| Shared | **false** (static shared path) |

### DP-5: `can_mutate_in_place(s, var, field, point)`

| Access | Uniqueness | Active direct borrow overlaps `field`? | Active alias of borrow from `var`? | Result |
|---|---|---|---|---|
| ≠ Owned | any | any | any | **false** |
| Owned | ≠ Unique | any | any | **false** |
| Owned | Unique | yes (same or overlapping field, live at `point`) | any | **false** |
| Owned | Unique | no | yes (any alias live at `point`) | **false** |
| Owned | Unique | no | no | **true** |

"Active direct borrow" checked via `borrow_sources` side table: entries where `borrow_sources[b].source = var` and `b` is live at `point` and `borrow_sources[b].field` overlaps `field`. "Active alias" checked via `project_alias_sources`: entries where `var ∈ project_alias_sources[a]` and `a` is live at `point` AND `a` is NOT a direct borrow of `var` in `borrow_sources` (direct borrows are handled field-aware by Step 1 — see DP-5 body). Non-borrow aliases block conservatively (no field-level granularity). Both checks are intersected with the live-variable set at the mutation point (§1.9). DP-5 is the sole predicate that is NOT a pure function of `AimsState` — it additionally requires `borrow_sources`, `project_alias_sources`, and liveness.

### DP-6: `is_reuse_candidate(s)`

| Access | Uniqueness | Shape | Result |
|---|---|---|---|
| ≠ Owned | any | any | **false** |
| Owned | Shared | any | **false** |
| Owned | ≠ Shared | NonReusable | **false** |
| Owned | Unique or MaybeShared | ReusableCtor(*) or CollectionBuffer or ContextHole | **true** |

### DP-7: `is_rc_skip_eligible(s)`

| is_local | Access | Consumption | Cardinality | Uniqueness | is_scalar | Result |
|---|---|---|---|---|---|---|
| false | any | any | any | any | any | **false** |
| true | ≠ Owned | any | any | any | any | **false** |
| true | Owned | ≠ Linear | any | any | any | **false** |
| true | Owned | Linear | ≠ Once | any | any | **false** |
| true | Owned | Linear | Once | ≠ Unique | any | **false** |
| true | Owned | Linear | Once | Unique | true | **false** |
| true | Owned | Linear | Once | Unique | false | **true** |

### DP-8: `is_local(s)`

| Locality | Result |
|---|---|
| BlockLocal | **true** |
| FunctionLocal | **true** |
| ArgEscaping | **false** — escapes into callee; see DP-8 rationale in §4 |
| HeapEscaping | **false** |
| Unknown | **false** |

### DP-9: `cow_mode(s)`

| Uniqueness | Result |
|---|---|
| Unique | `StaticUnique` — in-place, no check |
| MaybeShared | `Dynamic` — runtime IsShared check |
| Shared | `StaticShared` — unconditional copy |
