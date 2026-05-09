# Proposal: AIMS Burden Tracking

**Status:** Approved
**Author:** Eric (with Claude assistance)
**Created:** 2026-05-08
**Approved:** 2026-05-08
**Affects:** Compiler (`ori_arc`, `ori_llvm`, `ori_registry`), spec (Annex E §AIMS), in-tree rules (`canon.md §7.1`, `missions.md §AIMS`, `aims-rules.md`, `arc.md`, `registry.md`)
**Depends On:** *(none)*
**Related:** `clang-arc-lessons` (plan; downstream optimization layer)

**Spec governance note:** Annex E §AIMS substantive rewrites in §"Spec & Grammar Impact" are **conditionally authoritative** — they are committed as the target spec content but take effect only when the §Prototype Gate (Phase A) passes. If the gate fails, the proposal returns to draft and the rewrites are withdrawn. If the gate passes, `/sync-aims-spec` post-§10 implementation propagates them to Annex E unconditionally. This conditional-approval shape follows the `aims-spec-promotion-proposal.md` precedent.

---

## Redraft History

This is a clean redraft following four rounds of `/tpr-review` against an iterative seed draft. Rounds 2, 3, and 4 produced unanimous **Modify-substantially** verdicts; the architectural direction is settled. This redraft consolidates all R1–R4 findings, removes iterative-edit contradictions, and frames the design honestly: Perceus algorithmic core + Ori's data-driven registry pattern. Approval (2026-05-08) accepts the proposal as a unified document; the §Alternative 3 decomposition path was offered but the unified shape was selected.

---

## Summary

Replace the AIMS realization-time predicate stack (PIN-1..6 + `class_payload_of` + `ssa_alias_classes`) with a two-layer architecture:

1. **Phase 5 ARC Lowering emits structurally-maximal burden operations** — `BurdenInc` immediately before every transfer point that consumes an owned value, `BurdenDec` immediately following every last-use along every reachable CFG path. No flow analysis, no fixpoint, no class-equivalence quotient. Burden ops are emitted from per-type `BurdenSpec` data registered in `BurdenRegistry`, sibling to existing `MethodRegistry` / `PatternRegistry` / `DerivedTrait` registries.
2. **Phase 6 AIMS Lattice optimizes** — the existing seven-dimension product lattice + interprocedural `MemoryContract` SCC fixpoint runs unchanged over the burden-emitted IR. Its role is purely to ELIMINATE redundant burden ops via DP-2 / DP-3 / existing pair-elimination. It never constructs ownership decisions.

Phase 7 Realization becomes mechanical: `BurdenInc` → `RcInc`, `BurdenDec` → `RcDec` (or compound COW / reuse where the lattice annotated one). The architectural payoff is that ownership decisions move from accreted query-time predicates to per-type registered data, with a single optimization pass over a balanced baseline.

**Honest framing**: this is Perceus (Reinking et al., PLDI 2021) integrated into Ori's data-driven registry pattern. Roc adopted Perceus directly (`roc#825`, `roc#5258`); this proposal adopts Perceus with the BurdenRegistry data layer as Ori's idiomatic adaptation. The contribution is the registry-as-data pattern, not divergence from Perceus.

---

## Motivation

### The Problem in Practice

AIMS today derives ownership at realization time from a stack of overlapping predicates over an SSA-alias-class graph populated in Phase 6:

- `compute_ssa_alias_classes` populates `class_payload_of` edges asserting transitive containment between value classes
- `class_alive_after`, `pin4_class_emits_dec_set`, `pin6_any_ancestor_will_cover`, `is_rc_managed` are five overlapping predicates that together decide where each `RcDec` lives
- A pre-Step-4.5 syntactic-liveness fallback in `class_alive_after` masked a population-time defect in `class_payload_of` for two years before its removal exposed the underlying bug class

Each predicate was added to plug a hole the prior layer couldn't model.

### Empirical Evidence

- **AIMS/ARC active bug cluster** in `bug-tracker/plans/`: BUG-04-039, 074, 086, 090, 093, 094, 095, 096, 097, 098, 099, 111, 118. Strict classification (predicate-stack-caused only): BUG-04-104 (closed), 106, 107, 111, 118 — five bugs directly attributable to the SSA-alias-class + PIN-1..6 introduction. Broader classification (ARC-adjacent): adds 074, 086, 090, 093, 094, 095, 096, 097, 098, 099. The strict-five count is sufficient empirical evidence for the architectural claim; the broader count is informational. The proposal's argument does not depend on counting borderline bugs.
- **BUG-04-104's fix-plan went through 23 rounds of TPR review** — Round 3 added PIN-4, Round 4 added PIN-5, Rounds 18–19 introduced PIN-6 + revised it, Round 22 capped at "Set conservative heuristic"
- **Fix-cascade chain**: BUG-04-077 disabled collection-element narrowing for soundness (re-enablement still tracked as BUG-04-039); BUG-04-104 introduced SSA alias classes + PIN-1..PIN-5; BUG-04-106 and 107 surfaced because the new model didn't cover closures; BUG-04-111 over-suppressed via PIN-4; BUG-04-118 regressed 16 of 25 `match_alias::*` tests when Step 4.5 removed the masking fallback
- **Each fix lands a new noun**: `EmissionSite` enum, `Wrapped` variant for transitive-drop, "post-convergence path-sensitive `class_payload_of`," "same-class dec dedup + closure bridge"

### Root-Cause Statement (BUG-04-118 §01, verbatim)

> When the AIMS lattice records `class_payload_of` edges (class A is contained in class B's transitive-drop variant payload), the population-time logic does NOT check whether class A has any apply-aliased member whose lifetime extends past class B's destructuring. Recording the edge in that case causes PIN-6 to enter its defensive grandparent fallthrough — when no covering grandparent exists, PIN-6 returns false and class A's canonical dec fires at A's later last-use point. But class B's transitive-drop walk in the earlier block ALREADY decremented A's slot — so A's later canonical dec is the OVERSHOOT that double-frees A's allocation.

### Where the Mission Stays Intact

`missions.md §AIMS`:

> Every RC operation that survives to the emitted IR points at a specific proof failure: the lattice could not prove the operation was redundant.

This framing is sound. The argument here is that the realization mechanism is the bug, not the mission. The proposal preserves the mission and the lattice's optimization role; it relocates ownership decisions to type-registered data.

---

## Design — Phase 5 / 6 / 7 Architecture

### Phase 5 (ARC Lowering) — Trivial Burden Emission

`ori_arc::lower::burden_lower` reads each owned non-scalar SSA value's `BurdenSpec` from the registry and emits burden ops based purely on SSA def-use structure. No flow analysis, no fixpoint, no lattice consultation.

For each owned `ArcVarId v`:
- `BurdenInc(v)` immediately before EVERY transfer point that consumes `v` (Apply with Owned param, Construct with Owned arg, Set with Owned value, PartialApply capture, Return with Owned value)
- `BurdenDec(v)` immediately following EVERY last-use of `v` along EVERY reachable CFG path
- Burden walks at function-exit `Return` and unwind-edge `Resume` terminators

The Phase 5 emission is structurally maximal: it overcounts deliberately. Each owned value gets a `BurdenInc` at every transfer + `BurdenDec` at every last-use. This is correct (every owned reference is balanced per path) but suboptimal (redundant inc/dec pairs that the lattice will eliminate).

### Phase 6 (Lattice Analysis) — Optimization Over Burden Baseline

The seven-dimension product lattice (`Access × Consumption × Cardinality × Uniqueness × Locality × Shape × Effect` per `aims-rules.md §1`) is preserved. **The lattice ALGORITHM is unchanged; its CONSUMPTION MODE shifts** from emission-decision (where to emit RC ops) to elimination-decision (which burden ops to remove). The dataflow engine, the seven dimensions, the canonicalization rules, and the interprocedural contract machinery are identical.

What requires minor adaptation per `aims-rules.md §3 Forward Transfer Matrix` exhaustiveness requirement (per opencode R10 F1):
- `BurdenInc` and `BurdenDec` join the existing TF-N/A category alongside `RcInc` and `RcDec` (no `dst`, no forward state to compute, no backward demand to propagate)
- DP-2 (`is_rc_dec_unnecessary`) and DP-3 (`is_rc_inc_elidable`) apply per-instruction at burden-op sites for elimination decisions, rather than per-variable at emission-candidate sites for emission decisions

The lattice's role narrows from constructor-of-correctness to optimizer:

- **Cardinality + Consumption** dimensions drive DP-2 (`is_rc_dec_unnecessary`) and DP-3 (`is_rc_inc_elidable`) elimination of redundant burden ops
- **Uniqueness** drives COW (`StaticUnique` in-place vs `Dynamic` runtime check vs `StaticShared` unconditional copy), unchanged
- **Shape + Uniqueness** drive FBIP / Reset / Reuse (DP-6, RL-11, RL-11a), unchanged
- **Locality** drives stack promotion (RL-14) and RC-header compression (RL-17, RL-18), unchanged
- **Effect summary** flags continue to inform interprocedural contracts and LLVM fact export (RL-29, RL-30, RL-31), unchanged
- **TRMC** structural rewrite (PL-7..PL-11) operates over the burden-emitted IR, unchanged
- **Immortal pre-pass** continues to feed AimsStateMap as a typed pre-pass input, unchanged
- **Interprocedural `MemoryContract`** SCC fixpoint computes `ParamContract` / `ReturnContract` / `EffectSummary` exactly as today

**Critical invariant (with honest qualification)**: the lattice only ever ELIMINATES burden-emitted ops; it never CONSTRUCTS them. A bug in the lattice that misses an elimination produces extra RC traffic. **However, an OVER-elimination bug — where DP-2 removes a `BurdenDec` that should fire — produces a leak; an over-elimination of an inc-dec pair where the elimination is wrong because alias-tracking input is wrong CAN produce double-frees.** The burden model dissolves the predicate-stack-EMISSION-side failure mode but inherits the lattice's elimination-side dependency on accurate alias-tracking infrastructure (`project_alias_sources`, `borrow_sources`). See §Honest Acknowledgment of Limits.

### Phase 7 (Realization) — Mechanical Lowering

`realize_rc_reuse` becomes a mechanical lowering pass:
- For each surviving `BurdenInc` after Phase 6 optimization, emit `RcInc`
- For each surviving `BurdenDec` after Phase 6 optimization, emit `RcDec` (or compound COW / reuse instruction where the lattice annotated one)
- COW realization, reuse expansion, drop-hint emission, and arg-ownership emission stay where they are today (`realize_annotations` Phase 2 post-merge)

The phase has no predicate stack. There is no `class_alive_after`, no `pin4_class_emits_dec_set`, no `pin6_any_ancestor_will_cover`, no `class_payload_of` query, no canonical-rep walk, no syntactic-liveness fallback. The decision *where to emit each RC op* was made at Phase 5 from `BurdenSpec` and refined at Phase 6; Phase 7 emits what the prior phases authored.

---

## Design — BurdenRegistry Data Layer

### BurdenSpec — Pure-Const Data Model

`BurdenSpec` is registered at type-definition time, sibling to `MethodRegistry` / `PatternRegistry` / `DerivedTrait`. Conforms to `registry.md §Invariants`: no heap types (no `String` / `Vec` / `Box` / `Arc` / `HashMap`), zero dependencies, pure const data.

Representation uses `&'static [...]` slices identical in shape to existing registry tables:

```
struct BurdenSpec {
    self_heap_alloc: bool,                       // type sits behind heap allocation needing RC header
    owned_fields: &'static [OwnedField],         // owned references paid in reverse decl order
    borrowed_fields: &'static [BorrowedField],   // no decs; lifetime tied via type system
    variant_burdens: &'static [VariantBurden],   // empty for non-sum types
    element_burden: Option<TypeId>,              // collections: per-element recursion
    compiled_drop: Option<FnSym>,                // Some(fn) for recursive types
    user_drop: Option<FnSym>,                    // Some(fn) when type implements Drop trait
}

struct OwnedField {
    field_path: &'static [u32],   // const path through nested fields
    field_type: TypeId,            // recurse for transitive burden
}

struct VariantBurden {
    variant_id: VariantId,
    transfers_on_match: &'static [TransferRule],
    retained_owned: &'static [OwnedField],
}
```

Registration is split by type origin (per codex R8 Finding 3 — `ori_registry` is pure-const per `registry.md §Invariants` so cannot store user-defined-type metadata, which lives in `ori_types::TypeRegistry` and uses heap-backed structures):

- **Builtin types** (primitives, stdlib types, prelude traits): `ori_registry::burden::BURDEN_TABLE` is a `&'static [(TypeId, BurdenSpec)]` lookup table generated at compile time. Pure-const, registry.md-compliant. Lookup: `BurdenRegistry::lookup_builtin(type_id) -> Option<&'static BurdenSpec>`.
- **User-defined types** (structs, sum types, type aliases declared in user code): registered in `ori_types::TypeRegistry` via a new `BurdenSpec` field on each `TypeDef`. Heap-backed (consistent with TypeRegistry's existing shape), keyed by `Idx` (the ARC IR's identity for user-defined types). Lookup: `TypeRegistry::burden(idx) -> Option<&BurdenSpec>` (lifetime tied to the registry).
- **Phase 5 lookup contract**: `ori_arc::lower::burden_lower` queries `BurdenSpec` via a unified `lookup_burden(ty: ResolvedType) -> Option<&BurdenSpec>` helper that dispatches to `ori_registry::burden::lookup_builtin` for builtin TypeIds and `ori_types::TypeRegistry::burden` for user-defined Idxs. The helper preserves the registry-purity boundary (builtin lookups are pure-const; user-defined lookups carry the TypeRegistry lifetime).

This split is consistent with how Ori already partitions per-type metadata: builtin methods live in `ori_registry::methods::METHOD_TABLE` (pure-const); user-defined methods live in `ori_types::TypeRegistry`. BurdenSpec joins this same partition.

### Existential Types (`impl Trait`) — Best-Effort Devirtualization, Indirection Fallback

`impl Trait` returns hide the concrete type from the caller. The caller cannot statically query `BurdenRegistry::lookup(concrete_type_id)`. Resolution strategy (per gemini R5 Finding 4 — best-effort devirtualization to avoid hidden vtable dispatch as default):

1. **Internal-only `impl Trait` returns** (callee monomorphized within same crate, no public API surface): Ori's existing monomorphization specializes the concrete type at the call site; the burden walk uses the concrete `BurdenSpec` directly. No indirection.

2. **Cross-crate `impl Trait` returns** (callee in upstream crate, caller in downstream): the callee crate generates a per-instantiation **burden-table thunk** `_ori_burden_drop_<callee>_<instantiation_id>` that performs the burden walk for that concrete instantiation. The thunk address is stored in the return value's wide pointer (alongside the data pointer, similar to how trait objects carry vtables today). Caller-side burden walk dispatches via the thunk pointer.

3. **Devirtualization opportunities** (compiler optimization, §05 deliverable): when LLVM can prove the concrete `impl Trait` instantiation at a call site (single instantiation visible to the optimizer), LLVM's existing devirtualization replaces the thunk call with a direct call to the concrete burden-walk inlined. This is a standard LLVM optimization applied to the new burden-thunk dispatch sites.

4. **Cost analysis**: indirection is one extra pointer load + one indirect call per `impl Trait` drop site. For idiomatic Ori code where most `impl Trait` returns are internal (mission per `missions.md §Ori`), devirtualization eliminates the cost. For genuine cross-crate `impl Trait`, the cost is bounded (one indirection per drop site) and matches the existing trait-object vtable cost.

The §05 implementation prioritizes Internal-only devirtualization; cross-crate indirection is the fallback that preserves correctness when devirtualization fails.

### Generic Burden Composition — Pre-Monomorphization

Generic types' BurdenSpecs are **monomorphized at type-instantiation time**, not resolved via runtime TypeId chasing. Rationale: Phase 5 emission needs concrete `BurdenSpec` to walk; deferring monomorphization to codegen would require Phase 5 to emit indirect dispatch on each burden walk, defeating the trivial-emission design.

Concretely: when `Result<{str:int}, str>` is first instantiated in the program, the compiler generates a concrete `BurdenSpec` entry composed from `{str:int}::Burden` + `str::Burden` and registers it under the monomorphized TypeId. This matches existing monomorphization for methods and derive-method dispatch.

Cost: registry size grows with monomorphization breadth. Mitigation: deduplication by structural-burden-signature (two monomorphizations producing identical `BurdenSpec` share one entry). For the standard library's generic types this is a modest, bounded growth.

### Closures — Capture Burden Composition

Closures are empirically the most-stressed value shape (BUG-04-090, 095, 098, 099, 104, 106, 107). The burden model handles them via explicit env-field composition.

A closure value of type `Closure<R>` has:

```
BurdenSpec for Closure<R>:
    self_heap_alloc: true                      // closure env is heap-allocated
    owned_fields: &[
        OwnedField { field_path: env.captured_0, field_type: <type of capture #0> },
        OwnedField { field_path: env.captured_1, field_type: <type of capture #1> },
        ...
    ]
    borrowed_fields: borrows captured by reference
    variant_burdens: &[]
    element_burden: None
```

Capture variants:
- **Capture-by-value of an Owned binding**: capture site is a transfer point. Source binding's burden ownership transfers to the closure's env field. Closure's env burden walk fires `BurdenDec` on the field at closure last-use.
- **Capture-by-reference**: env stores a borrow; closure carries the borrow as `borrowed_fields` entry. No drop on borrowed env field; lifetime tied via type system.
- **Captures-of-captures (nested closures)**: outer closure's env field IS itself a Closure type with its own BurdenSpec. Burden walk recurses.
- **Capture of a projection of a captured value**: captured projection has its own borrow source via existing `borrow_sources`. Burden walk treats it as `borrowed_fields` with lifetime tied to parent closure's lifetime.

`PartialApply` is one transfer point per captured argument. A binding consumed by `PartialApply` AND passed to an `Owned` callee in the same expression has transfer-count = 2 → one `BurdenInc` lands.

### Recursive and Self-Referential Types — Compiled Drop Glue

For non-recursive types, `BurdenSpec` walk is finite at compile time. For recursive types (`struct Node { next: Option<Node> }`, mutually-recursive `enum Tree`), naive walks would not terminate.

Resolution: when a `BurdenSpec` walk visits the same TypeId twice (cycle detected via `visited: HashSet<TypeId>` at registration time), the compiler emits a compiled drop function `_ori_burden_drop_<mangled_type>` once per type, and `BurdenSpec.compiled_drop: Some(fn)` points at it.

**Critical clarification (per codex R9 Finding 3): `compiled_drop` is the drop-glue body invoked from the zero-refcount branch of `ori_rc_dec`, NOT a direct call at every release site.** Phase 5 emission for recursive types emits `BurdenDec(v)` at ordinary release sites, identical to non-recursive types — the burden op is a reference release, not a drop invocation. At runtime, `ori_rc_dec` performs the atomic decrement and only invokes `compiled_drop` when the refcount reaches zero. This preserves shared-reference correctness: a recursive value with `rc > 1` released via `BurdenDec` decrements its count without invoking the drop body, exactly matching the non-recursive case. The recursive aspect is purely how the drop body is compiled (per-type function rather than inline walk), not how the dec sites work.

```
Phase 5 emission for recursive type (e.g., struct Node { next: Option<Node> }):
    Same as non-recursive: BurdenDec(v) at release sites; BurdenInc(v) before transfers.

Codegen for the type's drop glue (compile-time):
    compiled_drop_<Node>(v):
        // body walks the BurdenSpec structurally:
        //   BurdenDec(v.next)  ← recursively triggers compiled_drop_<Node> via ori_rc_dec if next's rc=0
        //   free(v)            ← release self heap allocation

Runtime invocation chain for a Node going out of scope at refcount=1:
    BurdenDec(v) → RcDec(v) at Phase 7 → ori_rc_dec(v) atomically decrements → rc=0 → invoke compiled_drop_<Node>(v) → recursively decs v.next → ... → frees self
```

This matches Lean 4's `IR/RC.lean` per-type drop call shape, Rust drop glue, and C++ destructor compilation. The `BurdenSpec` stays first-class data — the spec describes what gets walked; for cyclic types the walk is COMPILED into the type's drop glue function (called only when refcount=0), not inlined at every release site.

### `Drop` Trait Interaction — AUGMENT, with Partial-Move Restriction

**Architectural distinction (per codex R8 Finding 4): `BurdenDec` and drop glue are different things.**

- **`BurdenDec(v)`** is a **reference release**: it decrements `v`'s RC by one. It does NOT directly invoke user drop or walk owned fields. At the runtime level, `BurdenDec` lowers to `RcDec(v)` at Phase 7, and `ori_rc_dec` checks the refcount: if > 0 after decrement, return; if = 0, invoke the type's drop glue.
- **Drop glue** is a **per-type compiled function** (`_ori_burden_drop_<mangled_type>`) invoked by `ori_rc_dec` when refcount reaches zero. The drop glue is what runs the user's `@drop` method, walks the owned-field BurdenSpec (recursively releasing each field's reference), and finally frees the self allocation.

The user's `@drop` method runs FIRST inside the drop glue (matching Rust's Drop semantics), THEN the compiler walks owned fields:

```
ori_rc_dec(v):
    refcount = atomic_dec(v.header.refcount)
    if refcount > 0: return
    # refcount reached zero — invoke drop glue
    drop_glue_<type_of_v>(v)

drop_glue_<type_of_v>(v):
    if user_drop is Some(f):
        Apply { func: f, args: [v] }     # user's @drop runs FIRST; sees fields valid
    for each owned_field in BurdenSpec.owned_fields (reverse decl order):
        BurdenDec(v.field)               # release the field's reference
                                         # (recursively triggers field's drop glue if refcount=0)
    if self_heap_alloc:
        free(v)                          # release the self allocation
```

Phase 5 emission produces `BurdenDec` operations at last-use sites; the drop glue is generated once per type at codegen time and lives in compiled code (matching the existing `ori_llvm/codegen/arc_emitter/drop_gen.rs` shape). Phase 5 burden ops do NOT directly run user drop or walk fields — they emit references-releases that `ori_rc_dec` routes to drop glue when needed.

**Partial Move Restriction**: Ori shall FORBID partial moves of fields on types that implement a custom `Drop` trait. Tracking conditionally-moved fields would require dynamic drop flags (significant runtime layout/ABI cost not in scope) or per-CFG-path drop tracking that conflicts with the trivial-Phase-5 design.

Compiler enforcement: at type-check time, if a type T implements `Drop` AND a field-projection of T is consumed in a way that would require dropping T with that field absent, emit error `EDROP_PARTIAL_MOVE`. Allowed: full moves, references / borrows, match-destructuring (consumes whole value with binding all fields).

This restriction extends to **closure captures**: capturing a partially-moved field of a `Drop` type at `PartialApply` is treated as a partial move and rejected.

Scope: applies ONLY to types implementing `Drop`. Types without `Drop` allow partial moves; the burden walk handles partial cleanup via the **per-CFG-path moved-out tracking** specified below.

### Non-Drop Partial-Move Obligation Model

For types NOT implementing `Drop` (the common case), partial field moves are permitted. Phase 5 emission tracks moved-out fields statically — no dynamic drop flags required.

Mechanism (per codex R11):

1. At canonicalization, for each owned aggregate value `v` of type `T` (where `T` does NOT implement `Drop`), compute a per-CFG-path `moved_out_fields: BitSet<FieldId>` tracking which fields of `v`'s `BurdenSpec.owned_fields` have been consumed-by-transfer along that path.
2. The BitSet is updated at each transfer point that consumes a field-projection: `let f = v.field` (where `f` is consumed) sets the corresponding bit in `moved_out_fields[v]`.
3. At `v`'s last-use site (where the burden walk fires), the compiler emits `BurdenDec` operations for ONLY the fields NOT in `moved_out_fields[v]` — i.e., fields whose ownership was retained.
4. At CFG joins, the burden walk uses the **per-predecessor** `moved_out_fields[v]` at that join (the burden walk emission at the join point depends on which predecessor's path the value arrived from).

For the static tracking to work, the moved-out state must be computable purely from source structure — same property that enables the trivial Phase 5 emission. Cases where this property holds:
- **Direct field projection**: `let f = v.field` where `f` is then transferred (or its lifespan ends). The projection site is syntactic; the compiler statically knows `v.field` is moved out.
- **Match destructuring**: `match v { Constructor { f, g, .. } -> ... }` binds field-projections to `f` and `g`; `..` excludes other fields. The destructuring shape is syntactic.

Cases where partial-move tracking would require dynamic flags (and are therefore restricted, matching the Drop trait restriction's rationale):
- Conditional partial moves: `if cond then let f = v.field else /* nothing */`. Detected at type-check time and rejected with `EBURDEN_CONDITIONAL_PARTIAL_MOVE`. Caller must move v fully or restructure to avoid conditional partial-move.

This restriction parallels the Drop trait restriction: both forbid conditional partial moves to avoid dynamic drop flags. Drop types forbid ALL partial moves (because the user drop body might assume all fields valid); non-Drop types forbid only CONDITIONAL partial moves.

### Terminator Burden-Op Ordering

Burden ops (`BurdenInc` / `BurdenDec`) interact with control-flow terminators per the following ordering rules (per codex R11):

- **`Return v`**: the return is itself a consuming transfer point. NO `BurdenInc(v)` is emitted at the return (the value's ownership transfers to the caller's binding, not duplicated). Any `BurdenDec` for owned LOCALS (other than `v`) emits BEFORE the `Return` terminator — locals must be released before control leaves the function.
- **`Resume v`**: same as `Return` — `Resume` is a consuming transfer point on the unwind path. Locals other than `v` get `BurdenDec` before the `Resume`.
- **`Jump block_label(args=[v])`** to a block whose param is declared `Owned`: the jump-arg is a consuming transfer point. `BurdenInc(v)` emits BEFORE the `Jump` only if `v` is alive on the post-Jump path of the predecessor block (multi-transfer scenario); otherwise the existing reference transfers.
- **`Jump block_label(args=[v])`** to a block whose param is declared `Borrowed`: NO `BurdenInc`/`BurdenDec` (borrowed param doesn't transfer ownership; `v`'s burden continues in the predecessor scope).
- **Tail calls** (where the terminator IS the call): `BurdenInc(arg)` for each consumed Owned-param arg emits as part of the argument-passing sequence BEFORE the tail-call terminator. After the tail-call, the caller's frame is reused by the callee — no return-side burden ops fire.
- **`Branch cond`** and **`Switch scrutinee`**: terminators that consume only the cond/scrutinee value (which is typically scalar). NO burden-op interaction beyond emission for the scalar-condition value (which is itself trivial since scalars have empty BurdenSpec).
- **`Unreachable`**: terminator marking unreachable code. NO burden-op emission (control never reaches this point).

These ordering rules are mechanical: at each terminator, walk the block's local-variable scope; emit `BurdenDec` for locals NOT consumed-by-transfer at the terminator (per the moved-out tracking above); for consumed-by-transfer locals at the terminator, the transfer IS the dec (no separate `BurdenDec`). The Phase 5 emitter applies these rules uniformly across all CFG terminators.

### `Value` Trait Composition — Empty Burden

Types with the `Value` trait have inline storage, bitwise copy, no ARC. Empty `BurdenSpec`:

```
BurdenSpec {
    self_heap_alloc: false,
    owned_fields: &[],
    borrowed_fields: &[],
    variant_burdens: &[],
    element_burden: None,
    compiled_drop: None,
    user_drop: None,
}
```

Generic composition handles mixed-`Value` containers correctly. `Result<ValueType, HeapType>` at monomorphization:
- `Ok(vt: ValueType)` variant burden inherits `ValueType::Burden = empty` → no drop work
- `Err(ht: HeapType)` variant burden inherits `HeapType::Burden = full` → drops fire normally

Compiler validates `Value` trait conformance during type-check; conforming types automatically receive empty BurdenSpec via registration.

### Unwind Paths — Invoke / Resume Edges

ARC IR has explicit unwind edges via `Invoke` (call-with-unwind) and `Resume` (terminator). Burden emission on unwind paths:
- At each `Invoke`'s unwind successor block, the compiler emits burden walks for every owned SSA value live at the unwind site that has not been consumed-by-transfer along the normal path before the `Invoke`
- At `Resume` terminators, walk burden of every still-owned local + parameter not consumed pre-resume

This integrates with `aims-rules.md §7 Step 8a (unwind_cleanup)`: unwind_cleanup runs over the burden-emitted ARC IR rather than deriving cleanup decs from the predicate stack. Its existing role is preserved; the cleanup ops are now `BurdenDec` walks queried from the registry.

### `Set` / `SetTag` Implicit Field Drops

Current `aims-rules.md §RL-2` handles the old value's dec when `Set` replaces a field. In the burden model, `Set { base, field, value }` is two operations:
1. The `value` is a transfer point (Owned argument transferred to base's field slot)
2. The OLD value at `base.field` (if any) needs a burden walk before being overwritten

Phase 5 emits both: `BurdenInc(value)` before the `Set` (transfer); `BurdenDec(base.field.old_value)` immediately before the `Set` mutation (the old value's last-use is the overwrite). Phase 6's existing analysis recognizes that the old value's last-use IS the `Set` site.

`SetTag { base, tag }` invalidates the entire variant payload. Burden emission walks the OLD variant's burden before the tag change.

### BUG-04-077 Collection Narrowing — Acknowledged Gap

`BurdenSpec.element_burden: Option<TypeId>` recursively defines per-element burden for collections. This handles standard cases but does NOT directly model collection narrowing's soundness invariant from BUG-04-077. Re-enablement of collection narrowing is tracked separately as BUG-04-039 and is NOT promised by this proposal. The §07 regression sweep success criterion is "collection narrowing remains disabled, matching current state."

---

## Honest Acknowledgment of Limits

These are R1–R4 reviewer findings the proposal accepts as honest limits, not solved problems.

### Alias-Tracking Inheritance (opencode R4 Critical Finding 4)

The burden model dissolves the predicate-stack-EMISSION-side failure mode but **inherits** the lattice's elimination-side dependency on accurate alias-tracking. Phase 6's DP-2 / DP-3 elimination uses `project_alias_sources` and `borrow_sources` — the same alias-tracking infrastructure whose population-time defects caused BUG-04-118.

Failure mode: an over-elimination bug (DP-2 incorrectly removes a `BurdenDec` that should fire because alias-tracking misreports the value as covered by another's drop) is a leak. An over-elimination of an inc-dec pair where the elimination is wrong because alias-tracking is wrong CAN produce double-frees.

Mitigation:
- The §Prototype Gate explicitly verifies the lattice's alias-tracking handles the BUG-04-118 shape correctly (criterion: `inner` survives Result's drop without DP-2 over-elimination)
- `project_alias_sources` and `borrow_sources` population correctness is itself in scope for the burden model — the same Step 4.5b apparatus that established contract-SSOT in `compute_ssa_alias_classes` continues to apply
- Direct Perceus (without registry) is the documented fallback if the prototype gate reveals alias-tracking inheritance defects

### Bug Shape Relocates, Does Not Vanish

The earlier framing "BUG-04-118-class bugs become unrepresentable" was overstated (opencode R4 Major Finding 2). The honest claim:

- The **predicate-stack-emission failure mode** (where `class_payload_of` population determined dec emission and population-time defects produced double-frees) is dissolved. There is no `class_payload_of`-driven emission in the burden model; emission is type-data-driven and per-path balanced by construction.
- The **alias-tracking elimination failure mode** (where DP-2 / DP-3 query `project_alias_sources` / `borrow_sources` to decide which paired ops to eliminate) inherits the same alias-tracking correctness requirement. Bugs here would produce different shapes than BUG-04-118 — they manifest as missed eliminations (extra RC traffic) most of the time, but as memory bugs when alias-tracking input is wrong.

The burden model changes WHERE alias tracking matters (elimination, not emission). It does NOT eliminate the need for accurate alias tracking. The architectural payoff is reduced surface, not zero surface.

### RL-29 / RL-30 Mostly Duplicate Contracts

The earlier framing "BurdenSpec is a queryable second-consumer for RL-29/30/31 LLVM fact export" was overstated (gemini R3 Major, opencode R4 Major Finding 1). Honest accounting:

- **RL-29 (`noalias`)** consumes `ReturnContract.preserves_freshness` + `uniqueness`. BurdenSpec's `self_heap_alloc` partially overlaps with `preserves_freshness` derivation but adds little novel precision.
- **RL-30 (`memory(...)`)** consumes `ParamContract.access` + `may_share` + `EffectSummary`. BurdenSpec's `borrowed_fields` vs `owned_fields` partially overlaps with `ParamContract.access` but adds little novel precision.
- **RL-31 (alias-scope disjointness)** is the genuinely novel consumer. Type-level field-graph disjointness is a proof technique the contract layer cannot express; BurdenSpec's `field_type` chains can prove that two parameters' reachable owned-field graphs are disjoint at the type level.

The registry's value rests on:
1. RL-31 type-level disjointness (one concrete novel consumer)
2. Future cycle collector (when shipped, walks BurdenSpec for cycle identification)
3. Ori-pattern conformance (BurdenRegistry joins `MethodRegistry` / `PatternRegistry` / `DerivedTrait` as the per-type metadata family)

The registry framing is NOT validated by consumer-count breadth. It is validated by RL-31 + Ori-pattern fit + future-consumer roadmap. If the §Prototype Gate's RL-31 burden-aware path does not ship or does not demonstrate value, the fallback is direct Perceus without the registry layer.

### Sharing Detection Is Flow Analysis (Bounded)

Phase 5's trivial emission strategy avoids the per-binding flow analysis the seed draft proposed. But Phase 6's elimination work IS flow analysis — using the existing AIMS lattice fixpoint over the 7-dimension product. The "no flow analysis" claim is wrong; the honest claim is "the existing flow analysis runs over a structurally-balanced baseline rather than constructing the baseline itself."

This shifts complexity from new analysis (predicate stack accretion) to the existing lattice's optimization role. Whether this is a net simplification depends on the lattice's elimination correctness on the BUG-04-118 shape — verified by the Prototype Gate.

---

## Comparison with Prior Art

### Perceus (Koka, Roc)

This proposal IS Perceus, integrated into Ori's data-driven registry pattern:
- **Phase placement**: identical — emission at Phase 5 lowering over SSA
- **Last-use precision**: identical — drops at SSA def-use last-use
- **Sharing handling**: identical algorithmic shape (Phase 5 emits at every transfer; Phase 6 eliminates redundancies)
- **Lattice optimizes paired baseline**: Perceus does this too — its pair-elimination optimizer runs on the dup/drop-paired IR

What this proposal adds to Perceus: the per-type `BurdenSpec` data layer registered in `BurdenRegistry`. Perceus computes per-type drop information structurally at compile time as compiled IR (Lean 4 / Koka generate per-type drop code). This proposal makes that drop information first-class queryable data in a registry, enabling RL-31 type-level disjointness queries and future cycle-collector consumption.

Roc adopted Perceus directly (`roc#825` "Improve RC based on Perceus", `roc#5258` "Perceus style reference counting with frame limited reuse"). This proposal adopts Perceus too, with the registry layer as Ori's idiomatic adaptation.

### Counting Immutable Beans (Lean 4)

Lean 4 has a borrow-vs-owned parameter convention (`Borrow.lean`) and a per-function RC insertion pass (`RC.lean`). The borrow inference is data (parameter annotations); RC insertion still walks IR per-function.

This proposal adopts Lean 4's borrow-vs-owned parameter convention (already present in Ori as `ArcParam.ownership`). It does NOT replace per-function RC insertion — Phase 5 IS per-function emission, the same shape as `RC.lean`. The proposal extends Lean 4's pattern by registering per-type drop data alongside the per-function emission pass.

### Rust Drop Glue / C++ Destructors

Both compilers generate per-type drop functions structurally — at type definition time, the compiler emits a function that walks fields. This is structurally identical to BurdenSpec's compiled-drop fallback for recursive types. The difference: in Rust and C++ the drop is compiled code, not queryable data. This proposal elevates the drop information to data in a registry for non-recursive types; recursive types still use compiled drop functions.

### Swift Ownership Pipeline Re-Architecture

`swift#26539`, `swift#32407`, `swift#32885` — Swift restructured its ownership pipeline rather than continuing to extend the existing one. Cited as evidence that re-architecture (rather than continued predicate extension) is a precedent in production compilers. Swift's path is closer to Perceus than to burden-tracking; relevant as architectural-restructure precedent, not specific destination.

---

## Alternatives Considered

### Alternative 1: Direct Perceus Adoption (No Registry)

Phase 5 emits paired `dup`/`drop` ops in IR via per-function liveness; per-type drop info is compiler-generated functions (Lean 4 / Koka shape), NOT queryable registry data. Lattice optimizes the paired baseline. Proven by Roc.

Reasons to consider: proven, well-documented, lower implementation risk, simpler scope (no new registry).

This proposal commits to the registry-augmented path AND documents direct Perceus as the FALLBACK if the §Prototype Gate's RL-31 burden-aware design does not validate the registry layer's value. Direct Perceus is not rejected — it is the safety net.

### Alternative 2: Continue with Predicate Stack

Each new value-shape bug introduces a new predicate (PIN-7, PIN-8, …). Empirical evidence in §Motivation argues against this trajectory. R3-author feedback (BUG-04-118 §05 R3) confirms the marginal cost-per-shape is rising.

### Alternative 3: Two-Proposal Decomposition

Per opencode R4 Finding 8: split into (a) Phase 5/6 Perceus-style architecture and (b) BurdenRegistry data layer. (a) approve immediately; (b) provisional approval gated on Prototype Gate.

This proposal is structured to permit decomposition. If the reviewer set prefers staged approval, the §Design — Phase 5/6/7 Architecture section becomes Proposal A and the §Design — BurdenRegistry Data Layer section becomes Proposal B. Both proposals share the §Motivation, §Honest Acknowledgment of Limits, §Comparison with Prior Art, and §Roadmap sections. The decomposition is offered; this draft is structured as one proposal covering both.

---

## Purity Analysis

**Can be pure Ori?** NO. This is a compiler-internal architectural change. The AIMS pipeline is a compile-time analysis layer; Ori programs do not interact with it directly.

**Recommendation**: Proceed as compiler feature.

---

## Spec & Grammar Impact

**Grammar:** No impact.

**Annex E §AIMS substantive rewrites are CONDITIONALLY APPROVED, contingent on the §Prototype Gate.** Per the spec-promotion governance precedent (`aims-spec-promotion-proposal.md`, approved 2026-04-30), substantive spec changes are gated by `/create-draft-proposal` → `/review-draft-proposal`. This proposal IS that gate. The normative rewrites below become **conditionally authoritative on this proposal's approval**: they are committed as the target spec content, but they take effect only when the §Prototype Gate passes (the gate validates that the burden-tracking architecture actually works as designed). If the Prototype Gate fails, the proposal returns to draft and the spec rewrites are withdrawn. If the Prototype Gate passes, the rewrites become unconditionally authoritative and `/sync-aims-spec` post-implementation propagates them. This conditional-approval shape protects against approving substantive spec changes that subsequent prototyping reveals as unworkable.

### Approved Annex E §AIMS — Normative Rewrites (ISO/IEC voice)

§AIMS.6 (Pipeline Ordering) — REPLACE Phase 5 + Phase 7 prose with:

> **Phase 5 (ARC Lowering).** The compiler shall lower CanExpr to ArcFunction. During lowering, for each owned non-scalar SSA value `v`, the compiler shall query `BurdenRegistry::lookup(type)` for `v`'s `BurdenSpec` and shall emit:
> - `BurdenInc(v)` immediately before every transfer point that consumes `v`
> - `BurdenDec(v)` immediately following every last-use of `v` along every reachable CFG path
> - Burden walks at function-exit `Return` and unwind-edge `Resume` terminators per `BurdenSpec`
>
> Phase 5 emission shall not perform sharing analysis, transfer-point counting, or branch-asymmetric optimization. Phase 5 emission shall satisfy the per-path balance predicate of §AIMS.9.1.1.
>
> **Phase 6 (Lattice Analysis).** The existing AIMS pipeline shall analyze the burden-emitted ARC IR. The lattice's role is OPTIMIZATION: DP-2 (`is_rc_dec_unnecessary`), DP-3 (`is_rc_inc_elidable`), and existing pair-elimination passes shall remove redundant burden ops the lattice can prove redundant. The lattice shall NOT construct burden ops; it shall only eliminate them.
>
> **Phase 7 (ARC Realization).** The compiler shall mechanically lower remaining `BurdenInc` to `RcInc` and `BurdenDec` to `RcDec` (or to compound COW / reuse instructions per existing RL-6 through RL-13 where the lattice annotated one). Realization shall not derive ownership decisions; it shall lower the optimized burden baseline directly.

§AIMS.9 (Verification Layers) — VF-1 gains §AIMS.9.1.1:

> **§AIMS.9.1.1 Burden-Balance Check.** VF-1 shall verify that ARC IR exiting Phase 5 satisfies the **per-edge balance constraint** for each owned SSA value `v`. The check runs as forward graph-dataflow on the CFG, NOT as path enumeration (Ori has loops; path enumeration would not terminate).
>
> Formulation as linear edge-balance dataflow:
>
> 1. **Per-edge balance**: for each CFG edge `e` from block `B_pred` to block `B_succ`, the live reference count of `v` flowing across `e` (call it `rc_e(v)`) shall be a non-negative integer. The dataflow computes `rc_e(v)` for every edge such that:
>    - At `v`'s definition block, the outgoing-edge count is `1` (initial owned reference)
>    - At each `BurdenInc(v)` instruction, the post-instruction count is `count_before + 1`
>    - At each `BurdenDec(v)` instruction, the post-instruction count is `count_before − 1`
>    - At each consuming-transfer-point on `v`, the post-instruction count is `count_before − 1`
>    - At each block boundary, all outgoing-edge counts equal the block-exit count
>    - At CFG joins, all incoming-edge counts must agree (the lattice's existing meet-on-merge)
> 2. **Net-zero cycle obligation**: for every SCC (loop or strongly-connected sub-CFG), the sum of `BurdenInc(v) − BurdenDec(v) − consuming-transfers-on-v` over instructions in the SCC must equal zero. This ensures loops cannot accumulate reference debt.
> 3. **Terminator transfer**: terminators that consume `v` (e.g., `Return v`, `Jump block(args=[v])` to an Owned-param block, `Resume v`) decrement the count on the relevant outgoing edge. Non-consuming terminators leave the count unchanged.
> 4. **Per-path-terminal release**: along every reachable path from `v`'s definition to a path-terminal point (function-exit Return, unwind Resume, or last-use BurdenDec), the running count must reach zero exactly at the path-terminal.
>
> Equivalence to the simpler per-path equation (informational): for an acyclic CFG segment, the dataflow check is equivalent to `1 + count(BurdenInc on path) = count(BurdenDec on path) + count(consuming-transfers on path)` — but the dataflow formulation handles loops correctly via the SCC obligation.
>
> Worked examples (all balanced):
> - Plain owned value, no transfer, one `BurdenDec` at last-use: edge counts `1 → 0` across the dec. Balanced.
> - Owned value passed to one Owned-param Apply: edge counts `1 → 0` across the transfer. Balanced.
> - Loop where iteration creates and consumes a fresh `v`: SCC sum = `0 (inc inside iter) − 1 (transfer at iter end) + 1 (initial def at iter top, accounted via §AIMS.9.1.1 rule 1) = 0`. Balanced.
>
> A function failing the dataflow check produces `BurdenImbalance` error with the offending edge or SCC and the divergent counts. The check uses the existing AIMS dataflow infrastructure (the lattice's CFG-iteration engine; see `aims-rules.md §7 Pipeline Ordering`); no new algorithmic machinery is introduced.

§AIMS.4 (Transfer Functions), §AIMS.5 (Canonicalization Rules), §AIMS.8 (Realization Rules): unchanged in normative content. §AIMS.4 gains a clarifying §AIMS.4.0 header noting the transfer functions operate over burden-emitted baseline.

### Approved `canon.md §7.1` Invariant 5 Reframing

> Ownership is itself a typed pre-pass input (registered on the type as BurdenSpec, queried at Phase 5 lowering), not a property the lattice derives. New capabilities shall extend a lattice dimension, extend a contract field, OR extend BurdenSpec — never spawn a parallel emission path.

### Approved `missions.md §AIMS` Conflict-Resolution Rule Reframing

> When a new capability could be added as either a side pipeline or a lattice/contract/burden extension, the typed-data extension wins. The lattice operates as an optimizer over the burden-emitted baseline; ownership decisions live in BurdenSpec, not in flow analysis. New analyses shall NOT spawn parallel RC emission paths, shadow uniqueness trackers, or independent escape enums that bypass the lattice or the burden registry.

---

## Roadmap Impact

Implementation lands at `plans/aims-burden-tracking/` with `feature_plan: true` + `proposal: aims-burden-tracking-proposal.md`. Two-phase structure gated by Prototype Gate.

### Phase A0 — Design Validation Gate (§00 only)

**§00 is its OWN gating phase** (Phase A0; per opencode R10 F3 — eliminates the §00-placement ambiguity that arose when §00 was nested inside Phase A alongside §01-§04). Phase A0 evaluates the registry layer's value claim BEFORE any registry implementation work begins. If §00 passes, work proceeds to Phase A1. If §00 fails, the proposal falls back to direct Perceus (per `roc#825` / `roc#5258`) without the BurdenRegistry layer.

- **§00** — RL-31 burden-aware design walkthrough. Concrete code path showing how `BurdenSpec.field_type` chains derive type-level disjointness proofs; worked examples for `merge(a: {str:int}, b: [int])` and similar disjoint-Borrowed-param pairs; comparison with current `borrow_sources` / `project_alias_sources` per-call-site provenance approach demonstrating what burden adds. If the walkthrough cannot demonstrate a precision the contract layer cannot achieve, **Phase A0 FAILS** and direct Perceus becomes the path forward without entering Phase A1.

### Phase A1 — Prototype Implementation (sections §01–§04a, gated on Phase A0 pass)

- **§01** — BurdenRegistry data structures + registration API + integration with TypeRegistry
- **§02** — Burden composition via type parameters at monomorphization; existing `DropInfo`/`DropKind` lift into `BurdenSpec`
- **§03** — Phase 5 ARC lowering emits trivial burden ops; transfer points wired
- **§04** — Recursive-type compiled drop-glue fallback; closure capture composition; Drop trait AUGMENT + partial-move restriction; Value trait empty-burden composition
- **§04a** — Minimal lattice adaptation: register `BurdenInc`/`BurdenDec` in `aims-rules.md §3 Forward Transfer Matrix` as TF-N/A (no `dst`, no forward state, no backward demand — same as existing `RcInc`/`RcDec`); wire DP-2/DP-3 to apply at burden-op sites for elimination decisions. This adaptation is sufficient to run lattice elimination over the burden baseline in standalone verification mode for Prototype Gate criterion 6 (per opencode R10 F2 — resolves the gate circularity). The full Phase 6 lattice rewrite at §05 builds on this adaptation.

### Prototype Gate (BLOCKS §05+)

1. **BUG-04-118 emission-side dissolution**: regression sweep against the 16 originally-failing `match_alias::*` tests passes 16/16 with no double-frees produced by Phase 5 emission alone (lattice elimination NOT required for correctness)
2. **BUG-04-104/106/107/111 wins preserved**: regression sweep against `generics::*` and closure tests preserves the wins those bugs locked in
3. **Lattice alias-tracking correctness for the BUG-04-118 shape**: Phase 6 DP-2 / DP-3 over the burden baseline does NOT over-eliminate `inner`'s dec when `inner` survives Result's drop; verified via instrumented test exercising the exact shape
4. **No new failure mode**: full `./test-all.sh` corpus passes or fails identically to current baseline
5. **RL-31 burden-aware design walkthrough**: this is the §00 deliverable that lands FIRST in Phase A (per the §Phase A reordering). It must demonstrate via concrete code path + worked examples that `BurdenSpec.field_type` chains enable type-level disjointness proofs that `borrow_sources` / `project_alias_sources` per-call-site provenance cannot express. Failure of this criterion at §00 falls back to direct Perceus BEFORE §01-§04 registry implementation; failure of this criterion at §10 verification (post-§01-§04) returns the proposal to draft.
6. **Lattice clawback empirically adequate**: at least three tight-loop microbenchmarks (representative shapes: closures-inside-loops, sum-payload-extraction, conditional-transfer-in-branch) where Phase 5 burden + Phase 6 elimination produces RC operations within 5% of current AIMS baseline AS MEASURED, with concrete count comparisons documented per benchmark. If the gap exceeds 5% on any benchmark, the proposal documents the specific lattice extensions needed to close it BEFORE §05 advances. "Documents extensions needed" without measured numbers does NOT satisfy criterion 6.

If any criterion fails, the proposal returns to draft. If all pass, §05+ proceed.

### Phase B — Full Migration (sections §05–§10, gated on Phase A pass)

- **§05** — Phase 6 lattice rewrite: existing dimensions operate as optimizers over burden baseline; RL-31 burden-aware path implementation
- **§06** — Phase 7 mechanical lowering: `BurdenInc` → `RcInc`, `BurdenDec` → `RcDec`
- **§07** — Migration of remaining value-shape coverage (apply-aliases, closures, sum-payload, jumpargs, unwind paths) — the 16 BUG-04-118 originally-failing tests as regression pin corpus + additional shape coverage per Q6
- **§08** — Rule-file sync: `arc.md` + `aims-rules.md` + `canon.md §7.1` + `missions.md §AIMS` updates land with implementation; `/sync-aims-spec` regenerates Annex E §AIMS post-§10 verification
- **§09** — Simplification + scope reduction (NOT full removal): `ssa_alias_classes.rs` + `class_payload_of` are simplified — they continue to track apply-aliasing for the lattice's optimization passes (`project_alias_sources`, `borrow_sources`, Wrapped variant, BUG-04-111 §05 Step 4.5b apparatus). PIN-1..6 predicates and emission-site SSOT predicates retire; the alias-class machinery becomes lattice-input-only. `post_convergence.rs` is **partially retired, NOT removed as a unit** (per opencode R5 code-graph verification: 6/8 functions serve non-predicate purposes — same-slot dec dedup, alias-source widening, etc.). The path-sensitive `class_payload_of` patches that were introduced specifically to plug predicate-stack soundness gaps retire; the remaining functions migrate or simplify per per-function audit at §09 implementation. The §09 deliverable includes an explicit per-function disposition table showing which `post_convergence.rs` functions retire, migrate, or persist as lattice optimization input.
- **§10** — Verification: VF-1 burden-balance check, VF-7 rewrite-soundness tier, regression sweep, dual-execution parity confirmation

### Bug Plan Closure

Approval and implementation closes (subject to verification at §10):
- **Direct closure**: BUG-04-111, BUG-04-118 (predicate-stack failure modes dissolved at emission)
- **Likely closure**: BUG-04-074, 086, 090, 093, 094, 095, 096, 097, 098, 099 — verify per-bug at §10 (some may have failure modes that survive into the alias-tracking elimination layer)
- **Conditional**: BUG-04-039 (collection narrowing re-enablement) explicitly out of scope; remains open after this proposal

### `clang-arc-lessons` Handoff

Existing plan triggers Phase 0 reentry on approval. §02 (selective barriers), §03 (KnownSafe), §04 (COW contraction), §05 (PRE-style RC motion) translate cleanly onto the burden-emitted baseline as Phase 6 lattice optimizations.

### Dual-System Coexistence Handshake

During Phase A → Phase B migration, burden ops and predicate-stack-derived ops co-exist. The handshake is **class-closed** (per codex R9 Finding 2): coverage is keyed by the dec-obligation alias-class, NOT individual `ArcVarId`. A class is marked covered ONLY when EVERY member of the class AND EVERY transitive-payload obligation is burden-owned; mixed-coverage classes fall through to the predicate stack.

Mechanism:
- Phase 5 burden walk marks each owned `ArcVarId` as **burden-emitted** in `burden_emitted: BitSet<ArcVarId>` on `ArcFunction`.
- A separate **class-coverage check** runs after Phase 5: for each alias-class `C` from `ssa_alias_classes`, the class is `class_covered: BitSet<ClassId>` set IF AND ONLY IF (a) every `ArcVarId` member of `C` is in `burden_emitted` AND (b) every transitive payload obligation reachable via `class_payload_of` from `C` is also class-covered.
- The existing predicate-stack realization reads `class_covered`. For a class in `class_covered`, predicate-stack-derived dec emission is SKIPPED — the burden walk has authoritatively claimed responsibility for the entire class + transitive payload.
- Classes NOT yet fully covered (mixed coverage during per-section migration) fall through to the predicate stack as today. NO partial-class skipping; preserves the predicate-stack correctness for the uncovered subset.
- After §07 completes (all value shapes covered), `class_covered` is universal, and §09 simplification removes the predicate stack's correctness role.
- The handshake mechanism removes itself in §09 once all classes are covered.

This addresses the under-drop risk codex R9 Finding 2 identified: a mixed covered/uncovered class CANNOT cause the predicate stack to skip emission for the uncovered members, because the class as a whole isn't in `class_covered` until every member + transitive-payload is burden-owned.

---

## Migration / Breaking Changes

**Surface language:** None.
**Stdlib:** None.
**FFI envelope:** None — `unsafe` + `extern "c"` types have empty BurdenSpec (caller-managed; resource obligations remain caller's responsibility per existing FFI contract).

**Compiler-internal:**
- `ori_arc::aims::intraprocedural::ssa_alias_classes` — radically simplified per §09; not removed. Consumers in `cleanup_redundant.rs`, `dead_cleanup/mod.rs`, `walk_dec.rs`, `edge_cleanup.rs`, `walk.rs`, `state_map.rs` continue to consume `project_alias_sources` / `borrow_sources` for lattice optimization.
- `ori_arc::aims::realize::walk_dec.rs` — `class_alive_after`, `pin6_any_ancestor_will_cover` removed; mechanical RcInc/RcDec emission retained
- `ori_arc::aims::emit_rc::dead_cleanup::emission_site.rs` — `pin4_class_emits_dec_set`, `canonical_rep_for`, `var_emits_dec_in_block` removed
- `ori_arc::aims::intraprocedural::post_convergence.rs` — **partial retirement** (path corrected per codex R10 F4: actual location is `intraprocedural/`, not `realize/`). Only the path-sensitive `class_payload_of` patches retire (predicate-stack soundness patches no longer load-bearing). Per-function audit at §09: same-slot dec dedup, alias-source widening, and other non-predicate functions migrate or simplify based on per-function disposition table. NOT a full module deletion.
- `ori_registry::burden` — new module
- `ori_arc::lower::burden_lower` — new module for Phase 5 burden emission
- `ori_arc::ir::instr.rs` — `BurdenInc { var }` / `BurdenDec { var }` instructions
- Compiled drop-glue functions (`_ori_burden_drop_<mangled_type>`) for recursive types

**Non-AIMS consumers of `DropInfo` / `DropKind`** (LLVM codegen, evaluator) — `compute_drop_info` becomes a thin wrapper around `BurdenRegistry::lookup` returning a structurally-equivalent shape during transition. §02 audits and re-points consumers.

### Cycle Handling

**Out of scope.** Cycle collection is a separable concern addressed by a separate proposal.

---

## Open Questions

These are GENUINE uncertainties that the §Prototype Gate addresses or that defer to follow-on work.

### Q1: Lifespan determination — benchmark commitment (gated by Prototype Gate criterion 6)

Last-syntactic-use lifespan matches Perceus. Lattice clawback for missed-elision cases must produce identical RC ops to current AIMS on representative hot loops. §05 Phase 0 deliverable includes a perf microbenchmark commitment.

### Q2: Existential types (`impl Trait` returns)

When a function returns `impl Trait` and the concrete type is hidden from the caller, the caller cannot statically query `BurdenRegistry::lookup(concrete_type_id)`. Resolution: caller-side burden walk dispatches via vtable-like burden-spec-table indirection at the `impl Trait` boundary (one indirection per opaque-return call site). Trade-off: indirection cost vs. monomorphization breadth. Decision deferred to §02 implementation.

### Q3: Sharing detection complexity (RESOLVED by Phase 5 trivial / Phase 6 optimization split)

The seed draft's per-binding sharing-detection algorithm was retired. Phase 5 emits trivially; Phase 6's existing flow analysis handles elimination. The complexity stays in the existing lattice's optimization role rather than accreting in a new analysis layer.

### Q4: BurdenSpec second consumer (RESOLVED via RL-31 + Ori-pattern)

The Phase 0 deliverable for §05 includes RL-31 burden-aware design. The registry's value rests on RL-31 type-level disjointness + Ori-pattern conformance + future cycle collector. Direct Perceus (without registry) is the documented fallback.

### Q5: Migration testability

The 16 BUG-04-118 originally-failing tests are a starting regression corpus. §07 adds shape-explicit coverage for: closures-inside-loops with conditional capture, recursive types via compiled drop-glue, Drop-trait collision cases, Value/HeapType mixed sum-type variants, unwind-path drop emission.

### Q6: TRMC interaction

TRMC's structural rewrite (`aims-rules.md §7 PL-7..PL-11`) currently runs at Step 3a, before Phase 4 analysis. The burden walk runs at Phase 5 over the post-TRMC IR. Interaction: TRMC's `ContextHole` parameter must have a burden spec (likely empty, since the hole is an unfilled allocation). Resolution: §02 includes TRMC-aware burden composition; the existing TRMC `verify_trmc_soundness` (PL-10) gains a burden-balance check on rewritten IR.

### Q7: Sendable / channel transfer interaction

Ori's `Sendable` trait governs which types can cross thread boundaries via channels. Channel send is a transfer point (Owned argument transferred to consumer). Burden walks must correctly handle the cross-thread case: sender's burden ownership transfers to channel; channel's BurdenSpec carries the transferred value's burden until consumer receives. Resolution: §02 includes Sendable-aware burden transfer; `Producer<T>` / `Consumer<T>` BurdenSpecs are composed from T's burden + channel-state burden.

### Q8: Iterator handles and non-ARC resources

Iterator handles, file descriptors, and other non-ARC resources fit the burden model via `user_drop`: types implementing `Drop` carry the resource-cleanup logic in their user drop method. The AUGMENT semantics ensure user drop runs first (releasing the resource), then compiler burden walks owned fields.

### Q9: Drop trait field-drop ordering vs user drop body

User `@drop (self) -> void` runs FIRST; compiler burden walk runs SECOND. The compiler burden walk decrements `owned_fields` in REVERSE declaration order (matching the `drop-trait-proposal.md` approved field-ordering). User drop body sees all fields valid (still owned). After user drop returns, the compiler walks fields in reverse declaration order, recursing into each field's BurdenSpec.

### Q10: Value trait + Drop trait mutual exclusivity

Per `ori-syntax.md §Value`, Value types have inline storage, bitwise copy, no ARC. Per `drop-trait-proposal.md`, Drop types perform user-defined cleanup at refcount-zero. **These are mutually exclusive**: a Value type has no heap allocation backing, so refcount-zero cleanup never fires. The compiler enforces this at type-definition time: a type implementing both `Value` and `Drop` produces error `EVALUE_DROP_CONFLICT`. Standard library types are designed to be one or the other, never both.

### Q11: Sendable + atomic vs non-atomic burden ops

Sendable types cross thread boundaries via channels. Channel send is a transfer point (Owned argument transferred to consumer). Burden walks must use atomic RC operations for Sendable types crossing thread boundaries to satisfy memory-ordering correctness. Non-Sendable types (or Sendable types provably staying in one thread per `aims-rules.md §RL-19`) use non-atomic RC. The atomic/non-atomic decision is a Phase 7 realization choice (existing AIMS apparatus), not a Phase 5 emission choice — `BurdenInc`/`BurdenDec` are uniform; their realization to atomic vs non-atomic `RcInc`/`RcDec` happens at Phase 7 per existing RL-19/RL-20/RL-21.

### Q12: FFI empty BurdenSpec soundness

Foreign types (`CPtr`, `JsValue`, types from `extern "c" from "lib"` blocks) have empty BurdenSpec because the Ori compiler cannot describe the foreign type's internal structure. **This is sound only when the foreign type has caller-managed lifetime** — Ori does NOT decrement; the FFI consumer is responsible for resource cleanup per the existing FFI contract (`unsafe { ptr_read(...) }`, `uses Unsafe` capability). Foreign types passed to Ori with Ori-managed lifetime require an explicit `BurdenSpec` registered via `extern` block annotation (e.g., `#free(fn)` per `ori-syntax.md §Deep FFI`). The empty-BurdenSpec default is correct for the borrowed/caller-managed case; the explicit-burden case requires user annotation per existing FFI deep-annotations.

### Q13: Recursive type-graph cycles in burden composition (compile-time, distinct from runtime cycle collection)

Recursive types (`struct Node { next: Option<Node> }`) produce TypeId cycles in `BurdenSpec::field_type` chains. Runtime cycle collection (Bacon-Rajan, generational refs) is OUT of scope. Compile-time cycle handling IS in scope: per §Recursive and Self-Referential Types, BurdenSpec walks detect cycles via `visited: HashSet<TypeId>` at registration time and emit compiled drop-glue functions for cyclic types. This handles compile-time recursion in burden composition; runtime cycle leaks (where a Node's `next` field forms a runtime reference cycle) remain a separate concern handled by future cycle-collection proposal.

### Q14: Whether this should be approved at all

Reviewers are explicitly invited to recommend rejection if the architectural argument fails to hold up. **Do not approve out of momentum.** Direct Perceus adoption per Roc `roc#825` + `roc#5258` is the documented fallback; if the §Prototype Gate's lattice alias-tracking criterion (3) reveals that the burden model inherits BUG-04-118-class failure modes through the elimination layer, the proposal returns to draft and direct Perceus becomes the path forward.

---

## References

- `bug-tracker/plans/BUG-04-118/section-01-root-cause-analysis.md` — most recent fault in the predicate-stack chain
- `bug-tracker/plans/BUG-04-111/00-overview.md` — Step 4.5 syntactic-fallback removal
- `bug-tracker/plans/completed/BUG-04-104/00-overview.md` — SSA alias classes + PIN-1..PIN-5 introduction (23-round TPR record)
- `bug-tracker/plans/completed/BUG-04-077/` — collection-element narrowing disabled for soundness (BUG-04-039 tracks re-enablement; out of scope here)
- `plans/clang-arc-lessons/00-overview.md` — sibling AIMS-optimization plan; Phase 0 reentry on approval
- `compiler_repo/docs/ori_lang/v2026/spec/annex-e-system-considerations.md` — current Annex E §AIMS surface
- `compiler_repo/docs/ori_lang/proposals/approved/aims-spec-promotion-proposal.md` — predecessor proposal that promoted AIMS to spec
- `compiler_repo/docs/ori_lang/proposals/approved/drop-trait-proposal.md` — Drop trait surface (AUGMENT semantics here)
- `.claude/rules/aims-rules.md` — formal AIMS ruleset (§1 lattice, §3 transfer functions, §4 decision predicates, §5 contracts, §7 pipeline ordering, §8 realization rules, §9 verification layers)
- `.claude/rules/arc.md` — shipped surface overview
- `.claude/rules/registry.md` — pattern AIMS BurdenRegistry joins (§Invariants — pure-const, no heap types)
- `.claude/rules/canon.md §7.1` — Five Load-Bearing Invariants
- `.claude/rules/missions.md §AIMS` — AIMS conflict-resolution rule
- Reinking, Lorenzen, Leijen, de Moura. *Perceus: Garbage Free Reference Counting with Reuse* (PLDI 2021)
- Ullrich, de Moura. *Counting Immutable Beans: Reference Counting Optimized for Purely Functional Programming* (IFL 2019)
- `roc#825`, `roc#5258` — Roc's Perceus adoption
- `swift#26539`, `swift#32407`, `swift#32885` — Swift ownership pipeline re-architecture

---

## TPR History

This redraft consolidates findings from four `/tpr-review` rounds against an iterative seed draft (2026-05-08). Round summaries:
- **R1**: codex Modify-substantially, gemini Reject, opencode Modify-substantially. Major themes: sharing detection IS flow analysis; conditional transfers force dynamic drop flags; recursive types break compile-time walk; Drop trait collision; Value trait composition; spec governance.
- **R2**: All three Modify-substantially (gemini moved off Reject after Phase 5 emission moved to `ori_arc` SSA, branch-asymmetric handling specified, recursive-types compiled-drop-glue added, Drop trait AUGMENT specified, Value trait empty-burden specified). New finding: registry purity violation (Vec/HashMap forbidden by registry.md).
- **R3**: All three Modify-substantially. Major themes: Phase 5 should be DUMB / Phase 6 should ELIMINATE (split adopted); spec body vs Q7 contradiction; RL-29/30 duplicate contracts (only RL-31 novel); partial moves with Drop need restriction (forbid adopted); dual-system coexistence handshake; VF-1 burden-balance predicate definition.
- **R4**: codex + opencode Modify-substantially; gemini wrapper failed. Major themes: alias-tracking inheritance (the deepest finding — burden model inherits Phase 6's alias-tracking dependency); bug-unrepresentability claim overstated (relocates rather than dissolves); generic burden composition is architectural (pre-monomorphization adopted); decompose into two proposals offered.

This redraft accepts the R4 honest critique: the burden model dissolves the predicate-stack-emission failure mode but inherits alias-tracking elimination dependency. The §Honest Acknowledgment of Limits section addresses this directly. The §Prototype Gate criterion 3 verifies the lattice alias-tracking correctness for the BUG-04-118 shape before §05+ proceeds.
