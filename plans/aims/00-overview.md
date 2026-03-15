---
plan: "aims"
title: "AIMS — ARC Intelligent Memory System"
status: in-progress
references:
  - "docs/compiler/design/09-arc-system/index.md"
  - "docs/ori_lang/v2026/spec/21-memory-model.md"
  - ".claude/rules/arc.md"
---

# AIMS — ARC Intelligent Memory System

## 1. Mission

> AIMS exists to replace fragmented memory-management heuristics with a single
> sound semantic framework. RC placement, reuse, COW, FIP, contracts, and TRMC
> are not separate features in this design; they are facets of one model and must
> agree. The goal is not partial implementation of many ideas, but one trustworthy
> system whose claims are enforceable in code and verification.

AIMS is done when the legacy ARC pipeline is deleted and every memory decision
in the compiler flows through the unified lattice, contracts, and realization.

**Current objective:** Verify every AIMS claim against code, fix mismatches,
and only then restore complete status. Sections are in-progress until
design, implementation, and verification all agree.

## 2. Non-Negotiable Invariants

These hold at all times. Any change that violates one is a bug, not a tradeoff.

1. **Contracts and realization must agree.** If `MemoryContract` says a function
   is `FipContract::Certified`, the realized IR must have zero unmatched
   allocations and zero unmatched deallocations. If realization evidence
   disproves the contract, the contract is corrected — not left stale.

2. **Active rewrites must be sound.** If `normalize_function()` transforms an
   `ArcFunction`, the rewritten function must produce identical observable
   behavior to the original for all inputs. Structural tests alone do not
   satisfy this — behavioral verification is required. If the rewrite cannot
   be verified, it must not run.

3. **No pass may rely on stale summaries.** If a pipeline step modifies the
   IR or updates an effect summary, all downstream consumers must see the
   updated values. A verifier that runs before its inputs are available is
   a sequencing bug, not a timing optimization.

4. **The enabled surface must be end-to-end verified.** Every subsystem that
   is active in the pipeline must have: implementation, invariant enforcement
   (canonicalize rules, verifier checks), and verification (structural +
   behavioral + regression tests). A subsystem missing any of the three is
   incomplete, regardless of how many checkboxes are marked.

## 3. System Map

AIMS is one system with six subsystems. Each is described as part of the same
model — not as stages of different models.

```
ArcFunction (from lowering)
  │
  ▼
┌─────────────────────────────────────────────────────┐
│  NORMALIZATION (pre-analysis)                        │
│  Rewrites IR to expose structural opportunities.     │
│  TRMC: self-recursive constructor-context rewrites.  │
│  Must be sound — rewritten IR is behaviorally        │
│  equivalent to the original.                         │
│  Files: aims/normalize/                              │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│  ANALYSIS (backward dataflow + SCC fixpoint)         │
│  Computes AimsState per (variable, block boundary).  │
│  7 dimensions: access, consumption, cardinality,     │
│  uniqueness, locality, shape, effect.                │
│  Interprocedural: MemoryContract per function.       │
│  Intraprocedural: AimsStateMap per function.         │
│  Files: aims/lattice/, aims/transfer/,               │
│         aims/intraprocedural/, aims/interprocedural/  │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│  CONTRACTS (interprocedural summaries)               │
│  MemoryContract: param ownership, return uniqueness, │
│  effect summary, FIP classification, context         │
│  behavior. One contract per function, computed by    │
│  SCC fixpoint. Contracts are the interface between   │
│  callers and callees — all cross-function reasoning  │
│  goes through contracts.                             │
│  Files: aims/contract/, aims/interprocedural/extract │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│  REALIZATION (reads state, writes IR)                │
│  Two-phase: Phase 1 (pre-merge) emits RC ops, reuse │
│  ops, arg ownership. Phase 2 (post-merge) emits COW │
│  annotations and drop hints. All decisions read from │
│  one AimsStateMap — no side tables.                  │
│  Files: aims/realize/, aims/emit_rc/, aims/emit_reuse│
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│  VERIFICATION (post-realization checks)              │
│  ARC IR structural verification (verify pass).       │
│  FIP contract-vs-evidence verification.              │
│  TRMC post-rewrite soundness verification.           │
│  Files: aims/verify/, pipeline verify steps          │
└──────────────────────┬──────────────────────────────┘
                       ▼
┌─────────────────────────────────────────────────────┐
│  BACKEND INTEGRATION                                 │
│  ArcFunction consumed by ori_llvm ArcIrEmitter.      │
│  Contract types consumed by LLVM emitter for ABI.    │
│  COW annotations, drop hints keyed by position.      │
│  Files: ori_llvm/codegen/arc_emitter/                │
└─────────────────────────────────────────────────────┘
```

## 4. Realization Status

| Subsystem | Status | Evidence | Open Issues |
|-----------|--------|----------|-------------|
| **Analysis** | Realized | 7D lattice converges. 12,888 tests pass. Backward dataflow with `seq_add`/`alt_join`. All dimensions active and cross-influencing (8 canonicalize rules). | None |
| **Contracts** | Realized | `MemoryContract` computed via SCC fixpoint. `ParamContract`, `ReturnContract`, `EffectSummary`, `FipContract`, `ContextBehavior` all populated from converged state. | FIP classification uses optimistic `may_deallocate=false` at extraction time; post-emission update corrects it but does not recompute `contract.fip` (Section 12 bug). |
| **Realization** | Realized | Two-phase `realize()` replaced 4 separate emission passes. RC, reuse, COW, drop hints all derive from `AimsStateMap`. | None |
| **Verification** | Partially Realized | ARC structural verify works. FIP contract-vs-evidence verifier runs in two passes: step 5a (structural) and second pass after `may_deallocate` + `contract.fip` updates (Section 12, resolved). TRMC post-rewrite uniqueness verification implemented via `verify_trmc_soundness()` (Section 13 Bug 5, resolved). | Cross-system interaction matrix (Section 08.5a) not started — 22 interaction cells unverified. |
| **Normalization** | Complete | Detection works. All 5 structural bugs fixed (2026-03-15). Behavioral test matrix (Section 13.8) complete (2026-03-15): 56 ARC unit tests, 12 AOT behavioral tests, 3 Valgrind tests, 2 Ori spec programs. Contract refresh partially implemented — `has_unbounded_stack = false` updated, full `extract_contract()` re-extraction deferred (Bug 2). | Full contract refresh deferred (requires SCC peer data threading). |
| **Backend Integration** | Realized | `ori_llvm` ArcIrEmitter consumes all AIMS artifacts. Legacy RC insertion deleted. AIMS is the sole pipeline. Two critical RC codegen bugs fixed (2026-03-15): `emit_rc_inc_inline_enum` silent no-op and `emit_variant_via_alloca` missing sub-pointer inc for boxed fields. | `emit_variant_via_alloca` boxed field inc guard only checks `Tag::Enum` — `Tag::Result`-wrapped recursive types would be missed (theoretical; type system doesn't currently produce this pattern). Retained modules (`borrow/`, `liveness/`, `rc_insert/annotate`, `uniqueness/`, `ownership/`) are actively used — not dead legacy. |

### Legacy Deletion Status

| Module | Status | Reason retained |
|--------|--------|----------------|
| `rc_elim/` | Deleted | — |
| `rc_identity/` | Deleted | — |
| `reset_reuse/` | Deleted | — |
| `expand_reuse/` | Deleted | — |
| `aims-shadow` | Deleted | — |
| `aims` feature flag | Deleted | AIMS is the sole pipeline |
| `rc_insert/insert.rs` | Deleted | Legacy RC insertion replaced by AIMS `realize_rc_reuse()` |
| `rc_insert/block_rc.rs` | Deleted | Only used by deleted `insert_rc_ops_with_ownership()` |
| `rc_insert/edge_cleanup.rs` | Deleted | Only used by deleted RC insertion functions |
| `borrow/apply_borrows()` | Deleted | Replaced by AIMS `apply_aims_ownership()` |
| `borrow/` (rest) | Live | `BuiltinOwnershipSets`, `infer_borrows_scc`, `extract_callees` — actively called by Salsa queries, JIT runner, AIMS pipeline |
| `liveness/` | Live | `compute_refined_liveness()` used by FBIP enforcement in AIMS pipeline |
| `rc_insert/annotate.rs` | Live | `annotate_arg_ownership()` called by AIMS arg ownership emission |
| `uniqueness/` | Live | `CowAnnotations`, `DropHints` are `ArcFunction` fields; `UniquenessSummary` consumed by `ori_llvm` |
| `ownership/` | Live | `AnnotatedSig`, `Ownership`, `DerivedOwnership` — shared type vocabulary |

These modules are not "legacy" — they are actively used by the AIMS pipeline and external consumers.

## 5. Open Contradictions

These are exact mismatches preventing "one system" from being true in code.
Each must be resolved before this plan is complete.

### C1. ~~FIP contracts are assigned optimistically, then left stale~~ RESOLVED

`extract_contract()` classifies FIP using `may_deallocate=false` (optimistic
default). After realization, `may_deallocate` is updated from `FipEvidence`.
`recompute_fip_for_may_deallocate()` now downgrades `Certified`/`Bounded` to
`Never` when `may_deallocate=true`. A second FIP verification pass runs AFTER
the `may_deallocate` + `contract.fip` updates, catching any remaining
mismatches. Both the stale-contract bug and verifier-sequencing gap are
resolved.

**Resolved in:** Section 12.1 (2026-03-14)

### C2. ~~TRMC rewrite — structural bugs fixed, behavioral verification absent~~ MOSTLY RESOLVED

All 5 structural bugs have been fixed (2026-03-15):
- Bug 4 (HIGH): `may_share` gate removed — per-variable uniqueness is sole gate
- Bug 1 (HIGH): Argument threading fixed — loop-back Jump threads `rec_args`
- Bug 5 (MEDIUM): Uniqueness verification implemented — `NonUniqueContext`
  now actively constructed by `verify_trmc_soundness()`
- Bug 3 (MEDIUM): Helper block dominance documented + verified via
  `check_context_var_dominance()` using `DominatorTree`
- Bug 2 (MEDIUM): Partial contract refresh — `has_unbounded_stack = false`
  updated in second pass. Full `extract_contract()` re-extraction deferred
  (requires SCC peer data threading).

Behavioral test matrix (Section 13.8) complete (2026-03-15): 56 ARC unit
tests in `normalize/tests.rs`, 12 AOT behavioral tests in `trmc.rs`, 3
Valgrind memory tests, 2 Ori spec programs. Two critical RC codegen bugs
discovered and fixed during behavioral testing:
- `emit_rc_inc_inline_enum` was a silent no-op (shared `collect_variant_rc_fields` now used by both inc and dec)
- `emit_variant_via_alloca` stored inline enum data into boxed recursive
  fields without incrementing sub-pointers (now calls `emit_inline_enum_inc`)

**Remaining gap:**
1. Bug 2 is a partial fix only — contract fields other than
   `has_unbounded_stack` remain pre-rewrite values.

**Resolved in:** Section 13.7 (final gates) and Section 13.8 (test matrix), 2026-03-15

### C3. ~~Dead code exists as "future use" stubs~~ MOSTLY RESOLVED

4 of 5 items resolved (2026-03-15):
- `TrmcContext` — deleted; `verify.rs` uses `RewriteContext`
- `NonUniqueContext` — now actively constructed by `verify_trmc_soundness()` (Bug 5 fix)
- `build_alias_map()` — deleted from `emit_rc/helpers.rs`
- `resolve_alias_root()` — deleted from `emit_rc/helpers.rs`

Remaining:

| Dead item | Location | Reason retained |
|-----------|----------|----------------|
| `EffectPurityViolation` variant | `verify.rs:64-69` | Deferred to effect-handler implementation. Has `#[expect(dead_code)]` with documented reason. Will be constructed when Ori adds effect handlers. |

### C4. ~~Retained legacy modules have dead analysis logic~~ RESOLVED

Dead code deleted (2026-03-14): `apply_borrows()`, `insert_rc_ops_with_ownership()`,
`insert_rc_ops()`, `block_rc.rs`, `edge_cleanup.rs`, `insert.rs`. Remaining
modules (`borrow/`, `liveness/`, `rc_insert/annotate.rs`, `uniqueness/`,
`ownership/`) are actively used by the AIMS pipeline, Salsa queries, and
external consumers. They are not legacy dead code.

## 6. Remaining Work

The following items must be completed before the AIMS plan can be considered
done. They are ordered by priority (correctness blockers first, then
verification, then tooling).

1. ~~**Section 13.8 — TRMC Behavioral Test Matrix.**~~ **COMPLETE (2026-03-15).**
   56 ARC unit tests, 12 AOT behavioral tests, 3 Valgrind tests, 2 Ori
   spec programs. Two critical RC codegen bugs in `ori_llvm` backend
   discovered and fixed during behavioral testing (`emit_rc_inc_inline_enum`
   no-op, `emit_variant_via_alloca` missing sub-pointer inc). Invariant 4
   is now satisfied for the TRMC surface.

2. **Section 13 Bug 2 — Full contract refresh.** Only `has_unbounded_stack`
   is refreshed after TRMC rewrite. Other contract fields (ContextBehavior,
   FipContract, EffectSummary) remain pre-rewrite values. Full
   `extract_contract()` re-extraction requires SCC peer data threading.

3. ~~**Section 08.5a — Cross-System Interaction Test Matrix.**~~ **COMPLETE (2026-03-15).**
   All 22 interaction cells tested at 3 layers: 22 AOT behavioral tests
   in `aims_interactions.rs`, 3 Valgrind test files, ARC unit tests
   in 5+ test modules. Critical RC bug found and fixed during testing:
   `emit_inline_enum_inc` leaked for consumed (moved) values, double-freed
   for borrowed values sharing inline enum sub-pointers. Fix: conditional
   inc only for borrowed-rooted vars. Also fixed: recursive enum drop
   chain leak (Nil refcount inflation from unconditional sub-pointer inc).

4. **Section 11 — LLVM `.fold()` codegen bug.** 4 of 13 golden corpus
   synergy programs cannot build. Section 11 is blocked until this
   LLVM bug is fixed and the full corpus is validated. The `.fold()` bug
   is in `ori_llvm`, not in AIMS, but it prevents completing the
   integration verification.

5. **Section 11 — SynergyMetrics metric definition.** The
   `multi_dim_rc_decisions` metric reads 0% because it only counts
   reuse-site decisions. The actual cross-dimension evidence is in
   `canonicalize_cross_fires` (325 total). The exit criteria need a
   revised metric definition or a revised gate threshold.

6. **Backend — `emit_variant_via_alloca` boxed field inc guard scope.**
   The sub-pointer inc fix (2026-03-15) at `construction.rs:354` only
   checks `pool_tag == Tag::Enum`. A `Tag::Result`-wrapped recursive
   type (e.g., `type Tree = Leaf | Node(child: Result<Tree, Error>)`)
   would bypass the inc. Currently theoretical — `is_boxed_enum_field`
   returns false for `Result<Tree, Error>` so the boxed path isn't
   entered. But if the type system evolves to support boxed
   Result-wrapped recursion, this guard must be widened to include
   `Tag::Result` and `Tag::Option`. Low priority until such types are
   representable.

## 7. Completion Rule

A section is complete only when ALL of the following are true:

1. **Implementation exists.** The code described by the section is written,
   compiles, and runs.

2. **Invariants are enforced.** Canonicalize rules, verifier checks, and
   contract consistency are active — not stubbed, logged-only, or deferred.

3. **Verification exists.** Structural tests prove the shape. Behavioral
   tests prove correctness. Regression tests prove nothing degraded.

4. **Downstream consumers use the same truths.** If the section produces a
   contract, the verifier checks it, realization reads it, and the LLVM
   emitter consumes it. No consumer sees stale data.

A section that has checkboxes marked `[x]` but fails any of these four
conditions is **incomplete** regardless of what the checkboxes say.

---

## Architecture

AIMS operates as a single memory-intelligence system with three pipeline
phases. These are pipeline phases of ONE system, not stages of different
systems.

```
CanExpr
  │
  ▼ (lower)
ArcFunction (no RC ops, no var_reprs)
  │
  ▼ (compute_var_reprs)
ArcFunction + var_reprs + ValueRepr
  │
  ╔══════════════════════════════════════════════════════════╗
  ║  Phase A: NORMALIZATION (pre-analysis)                   ║
  ║                                                          ║
  ║  Rewrite IR to expose structural opportunities:          ║
  ║    → TRMC normalization (self-recursive constructor      ║
  ║      contexts)                                           ║
  ║    → constructor-context metadata extraction              ║
  ║  Rewrites must be sound — behavioral equivalence.        ║
  ╚══════════════════════════════════════════════════════════╝
  │
  ╔══════════════════════════════════════════════════════════╗
  ║  Phase B: ANALYSIS (backward dataflow + SCC fixpoint)    ║
  ║                                                          ║
  ║  Interprocedural: MemoryContract per function            ║
  ║  Intraprocedural: AimsState per (block-boundary, var)    ║
  ║  7 dimensions, single backward pass, convergence         ║
  ╚══════════════════════════════════════════════════════════╝
  │
  ╔══════════════════════════════════════════════════════════╗
  ║  Phase C: REALIZATION (reads state, writes IR)           ║
  ║                                                          ║
  ║  Phase 1 (pre-merge): RC ops, reuse ops, arg ownership   ║
  ║  Phase 2 (post-merge): COW annotations, drop hints       ║
  ║  FIP evidence collected during realization                ║
  ╚══════════════════════════════════════════════════════════╝
  │
  ▼
ArcFunction (with RC ops, reuse ops)
  │
  ▼ (verify → tail_call → merge_blocks → COW/drops → verify → fbip)
  │
  ▼ (ori_llvm ArcIrEmitter)
LLVM IR
```

## Design Invariants

Standing properties of the system. Violations are bugs.

1. **Analysis and emission are separate.** Analysis produces `AimsStateMap`
   and `MemoryContract` without modifying IR. Realization reads converged
   state and writes artifacts. No intermediate IR mutation.

2. **One lattice, one truth.** All memory facts about a variable at a program
   point live in one `AimsState`. No parallel data structures that might
   disagree.

3. **Formally grounded.** Lattice dimensions justified by established theory.
   See [Research Lineage](#research-lineage).

4. **Law before optimization.** Every rewrite in `aims/normalize/` must follow
   the equational approach: specification → algebraic laws → proven
   instantiation. No ad-hoc rewrites.

## Implementation Sections

The sections below are the implementation record. They are ordered by
dependency, not by importance — every section is part of the same system.

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Unified Lattice Design | `section-01-lattice.md` | Complete |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` | Complete |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` | Complete |
| 04 | RC Emission | `section-04-rc-emission.md` | Complete (superseded by Section 10) |
| 05 | Reuse Emission | `section-05-reuse-emission.md` | Complete (superseded by Section 10) |
| 06 | Pipeline Integration | `section-06-pipeline.md` | Complete |
| 07 | Advanced Optimizations | `section-07-advanced.md` | Complete |
| 08 | Verification & Validation | `section-08-verification.md` | Complete — all 22 interaction cells tested at 3 layers, critical `emit_inline_enum_inc` RC bug found and fixed |
| 09 | Dimensional Fusion | `section-09-dimensional-fusion.md` | Complete |
| 10 | Unified Realization | `section-10-unified-realization.md` | Complete |
| 11 | Integration Verification | `section-11-integration-verification.md` | Incomplete — 4 synergy programs cannot build (LLVM `.fold()` bug), SynergyMetrics metric scope mismatch |
| 12 | FIP Proof Obligations | `section-12-fip-enforcement.md` | Complete |
| 13 | TRMC Realization | `section-13-trmc-realization.md` | Complete — all structural bugs fixed, behavioral test matrix (13.8) complete (56+12+3+2 tests), two RC codegen bugs in backend found and fixed. Remaining: partial contract refresh (Bug 2). |

### Cross-Section Dependencies

```
01 Lattice ──► 02 Intraprocedural ──► 04 RC Emission ──►
01 Lattice ──► 03 Interprocedural ──► 05 Reuse Emission ──► 06 Pipeline
                                                               │
                    07 Advanced ◄──────────────────────────────┘
                    08 Verification ◄──────────────────────────┘
                                                               │
                    09 Dimensional Fusion ◄─────────────────────┘
                    10 Unified Realization ◄── 09
                    11 Integration Verification ◄── 09, 10
                    12 FIP Enforcement ◄── 09, 10, 11
                    13 TRMC Realization ◄── 09, 10, 11, 12
```

**Cross-section interactions (must be co-implemented):**
- **Section 12 + Section 13**: Both modify the second pass in
  `run_aims_pipeline_all()`. Section 12 adds `contract.fip` recomputation.
  Section 13 adds contract refresh for TRMC-rewritten functions. The combined
  second pass must apply updates in order: (1) contract refresh, (2)
  `may_deallocate` update, (3) `contract.fip` recomputation, (4) FIP
  verification. These must be implemented together.

**LLVM emitter sync points:**
- `ArcFunction.cow_annotations` — keyed by `(block_idx, instr_idx)`
- `ArcFunction.drop_hints` — keyed by `(block_idx, instr_idx)`
- `ArcFunction.var_reprs` — indexed by `ArcVarId`
- `Apply.arg_ownership` / `Invoke.arg_ownership`
- `ArcParam.ownership`
- `RcStrategy` on `RcInc`/`RcDec`

## Scope

AIMS covers memory semantics for Ori's current language surface: single-threaded
execution, ARC-managed heap, value semantics (no cycles), capability effects
(no effect handlers yet).

**In scope:** RC placement, reuse, COW, FIP certification, TRMC normalization
for self-recursive single-accumulator constructor contexts, interprocedural
contracts, all 7 lattice dimensions active and cross-influencing.

**Out of scope (not applicable to current Ori):**
- Concurrent RC strategies — Ori has no concurrent workloads. Extension points
  are preserved in `ori_rt` (function-call boundary, `drop_fn` callback,
  `MAX_REFCOUNT` sentinel). See Section 07.4.
- Frozen-cycle RC — Ori's value semantics prevent cycles in safe code. No
  `freeze` operation exists. See literature review §11.
- Effect purity gate for TRMC — Ori has no effect handlers. When effect
  handlers are added to the language, the TRMC pipeline must add a non-linear
  resumption check. This is a language feature dependency, not an AIMS deferral.
- Representation optimization (boxity, bit-stealing) — downstream of AIMS.
  AIMS provides the facts; a repr optimizer consumes them. Not an AIMS concern.
- Locality realization as stack allocation hints — requires backend work in
  `ori_llvm`, not in AIMS analysis. AIMS computes `Locality`; consuming it
  for stack allocation is a codegen concern.
- WASM backend integration — AIMS analysis is backend-independent; current
  verification only covers the `ori_llvm` ARC IR consumer. WASM-specific
  ARC consumption is out of scope for this plan.
- Dual-accumulator TRMC, mutual-recursion TRMC, cross-block TRMC — beyond
  the scope of self-recursive single-accumulator patterns.

## Module Tree

> The `aims/` module tree (current):
>
> ```
> aims/
> ├── mod.rs              — dispatch hub, pub re-exports
> ├── builtins/           — builtin function MemoryContract mappings
> │   ├── mod.rs          — seed_builtin_contracts()
> │   └── tests.rs
> ├── contract/           — MemoryContract, ParamContract, FipContract
> │   ├── mod.rs          — contract types + join + conversion helpers
> │   └── tests.rs
> ├── emit_rc/            — RC emission helpers (submodules used by realize/)
> │   ├── mod.rs          — emission dispatch hub
> │   ├── arg_ownership.rs — emit_arg_ownership()
> │   ├── cow.rs          — COW annotation helpers
> │   ├── dead_cleanup.rs — dead-at-entry RC dec helpers
> │   ├── drop_hints.rs   — drop hint helpers
> │   ├── edge_cleanup.rs — per-edge RcDec
> │   ├── forward_walk.rs — forward walk helpers
> │   ├── helpers.rs      — shared emission helpers
> │   ├── queries.rs      — state map query helpers
> │   └── coalesce/       — static RC coalescing peephole pass
> │       ├── mod.rs
> │       └── tests.rs
> ├── emit_reuse/         — reuse emission helpers (submodules used by realize/)
> │   ├── mod.rs          — reuse dispatch + ReuseOpportunity types
> │   ├── detect.rs       — find_reuse_opportunities()
> │   ├── dynamic.rs      — MaybeShared → IsShared + Branch CFG expansion
> │   ├── fip.rs          — FIP gate records
> │   ├── planner.rs      — cross-block reuse planner
> │   └── set_ops.rs      — Set/SetTag instruction emission
> ├── immortal/           — heap-allocated constant detection
> │   ├── mod.rs          — detect_immortals()
> │   └── tests.rs
> ├── interprocedural/    — SCC fixed-point loop
> │   ├── mod.rs          — analyze_program(), SCC fixpoint
> │   ├── extract.rs      — extract_contract() + return-info helpers
> │   └── tests.rs
> ├── intraprocedural/    — backward dataflow
> │   ├── mod.rs          — analyze_function() entry point
> │   ├── block.rs        — per-block backward analysis
> │   ├── fip_balance.rs  — FIP token balance computation
> │   ├── post_convergence.rs — post-convergence passes
> │   ├── state_map.rs    — AimsStateMap data structure
> │   ├── state_map/
> │   │   └── tests.rs
> │   └── tests.rs
> ├── lattice/            — AimsState (7 dimensions)
> │   ├── mod.rs          — product lattice + EffectClass + SizeClass + BorrowSource
> │   ├── dimensions.rs   — AccessClass, Consumption, Cardinality, Uniqueness, Locality, ShapeClass
> │   └── tests.rs
> ├── normalize/          — TRMC normalization
> │   ├── mod.rs          — normalize_function() entry point
> │   ├── detect.rs       — TRMC-eligible recursion detection
> │   ├── lift.rs         — verify A-normal form
> │   ├── rewrite.rs      — TRMC 4-equation rewrite
> │   ├── verify.rs       — post-rewrite structural verification
> │   └── tests.rs
> ├── realize/            — unified realization
> │   ├── mod.rs          — realize_rc_reuse() + realize_annotations()
> │   ├── decide.rs       — decide() + decide_annotations()
> │   ├── metrics.rs      — SynergyMetrics
> │   ├── walk.rs         — unified forward walk
> │   └── tests.rs
> ├── transfer/           — transfer functions per ArcInstr/ArcTerminator
> │   ├── mod.rs          — DefTransfer, UseTransfer
> │   └── tests.rs
> └── verify/             — post-realization verification
>     ├── mod.rs          — verification dispatch
>     ├── fip.rs          — FIP contract vs emission checks
>     └── fip/
>         └── tests.rs
> ```

## Research Lineage

AIMS is its own system — the unification is the contribution. These papers
informed specific dimensions:

| Paper | Contribution to AIMS |
|-------|---------------------|
| **Perceus** (Reinking et al., PLDI 2021) | RC ops = structural rules of linear logic |
| **FP²** (Lorenzen et al., ICFP 2023) | FIP certification: Theorem 2 (`\|S\|=\|S'\|`), token balance, FIP/FBIP containment. See [lit review §02](../aims-literature-review/section-02-fp2.md) |
| **Counting Immutable Beans** (Ullrich & de Moura, IFL 2019) | SCC-based borrow inference; reset/reuse |
| **Drop-Guided Reuse** (Lorenzen & Leijen, ICFP 2022) | Reuse after RC insertion |
| **GHC Demand Analysis** (Sergey et al., POPL 2014) | Backward cardinality: {Absent, Once, Many} |
| **Substructural Interpretation** (Chirimar et al., JFP 1996) | RC = linear logic interpretation |
| **Linearity ≠ Uniqueness** (Marshall et al., ESOP 2022) | Linearity (future) ≠ uniqueness (past). See [lit review §06](../aims-literature-review/section-06-linearity-uniqueness.md) |
| **Quantitative Type Theory** (Atkey, LICS 2018) | Semiring structure for `seq_add`/`alt_join`. See [lit review §07](../aims-literature-review/section-07-quantitative-type-theory.md) |
| **Oxidizing OCaml** (Lorenzen et al., ICFP 2024) | Locality as load-bearing dimension. See [lit review §01](../aims-literature-review/section-01-oxidizing-ocaml.md) |
| **FIPTree** (Lorenzen et al., PLDI 2024) | Constructor contexts for O(1) top-down algorithms |
| **TRMC** (Leijen & Lorenzen, JFP 2025) | Equational approach with context laws. See [lit review §04](../aims-literature-review/section-04-trmc.md) |
| **Perceus for OCaml** (Pinto & Leijen, ML Workshop 2023) | Same-compiler evaluation methodology |
| **Bit-Stealing** (Elsman, ICFP 2024) | Repr optimization downstream of AIMS. See [lit review §12](../aims-literature-review/section-12-bit-stealing.md) |
