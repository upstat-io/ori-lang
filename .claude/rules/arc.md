---
paths:
  - "**arc**"
---

# AIMS — The ARC Intelligence Layer (ori_arc)

**Relationship to `aims-rules.md`**: The ARC/AIMS subsystem uses a two-file design because `aims-rules.md` is ~900 lines of formal target-system rules (including substantial unshipped subsystems). Merging would create a single file too large for effective navigation. The split is:
- **`arc.md` (this file)**: shipped surface overview — what is implemented today, verification stack, key invariants. Use this for quick orientation and "what works now."
- **`aims-rules.md`**: complete formal target-system spec — all rules including unshipped. Use this for normative rule lookup, lattice definitions, and analysis contracts.
- **Shared facts** (invariants, pipeline steps, verification) appear in both; `aims-rules.md` is authoritative when they conflict. Other phases (parse.md, typeck.md) use inline `(target-only)` annotations because their unshipped surface is small; AIMS's unshipped surface is too large for that pattern.

## Mission — READ THIS FIRST

**ARC is the runtime substrate. AIMS is the compile-time intelligence layer.** ARC is the refcount header, the atomic inc/dec primitives, the drop functions, the uniqueness check — the machinery that ships in `ori_rt` and executes at runtime. AIMS is everything in `ori_arc/src/aims/` + the surrounding passes (`borrow/`, `drop/`, `fbip/`, `uniqueness/`, `classify/`) that decide **at compile time** when RC operations are unnecessary and elides them. Plain ARC without AIMS would be a mediocre memory model; AIMS is what makes the substrate competitive. **The goal is RC rareness in emitted code, not RC speed.** Reasoning about AIMS as "RC placement" misses the point — placement is the fallback for the leftovers after elimination.

AIMS exists to replace fragmented memory-management heuristics with a **single sound semantic framework**. RC placement, reuse, COW, FIP, contracts, TRMC, borrow inference, and locality/escape classification are **not separate features** — they are facets of one product-lattice model and must agree. Today the lattice has 7 dimensions (`AccessClass × Consumption × Cardinality × Uniqueness × Locality × ShapeClass × EffectClass`); the count is not architectural — dimensions are added, refined, or merged as analysis needs evolve (e.g. `plans/locality-representation-unification/` extends `Locality`). Complementary pre-passes like immortal-object detection (`aims/immortal/`) produce typed inputs that feed the lattice-driven analysis via `AimsStateMap::immortals` — they are part of the unified pipeline, but they are NOT lattice dimensions themselves. The goal is not partial implementation of many ideas, but one trustworthy system whose claims are enforceable in code and verification.

### What AIMS does TODAY (shipped, not roadmap)

- **Interprocedural RC elimination** via `MemoryContract` — callers skip inc/dec when callees prove non-consumption; callees skip redundant drops when callers prove uniqueness
- **Intraprocedural RC elimination** via the AimsState lattice — proves locally that ownership already exists or that a value dies unused
- **FBIP / reuse** — replaces entire alloc-copy-dealloc sequences with in-place updates when uniqueness is provable (Koka + Lean 4 style)
- **TRMC** — tail-recursion modulo cons, eliminating RC ops on the return path of tail-call chains
- **Immortal pre-pass** — heap-allocated constants are detected before backward analysis (`aims/immortal/`) and marked in a per-var `immortals: Vec<bool>` bitvector stored on `AimsStateMap`; the lattice-driven analysis consults that bitvector to skip RC/COW/reuse/drop-hint emission entirely for those vars
- **Borrow inference** — per-parameter Owned/Borrowed ABI decisions at function boundaries

### Where AIMS is HEADED (pending plans, all ARC-based extensions — none replace refcounting)

- **Escape analysis → stack promotion** (`plans/repr-opt/section-08-escape-analysis.md`) — non-escaping allocations vanish entirely; no header, no RC ops, pure stack
- **Unified locality dimension** (`plans/locality-representation-unification/`) — one canonical escape classification feeding stack promotion, header sizing, and cross-function reasoning; replaces 3+ parallel escape enums
- **RC header compression** (`plans/repr-opt/section-09-arc-header.md`) — refcount field narrowed from i64 → i8/i16/i32 based on proven sharing bounds
- **Non-atomic RC** (`plans/repr-opt/section-10-thread-local-arc.md`) — thread-local allocations use plain load/store instead of atomic CAS
- **AIMS → LLVM fact export** (`plans/semantic-optimization-pipeline/section-03-aims-export.md`) — `noalias`, `alias.scope`, `memory(none)` attributes let standard LLVM passes exploit AIMS proofs
- **Clang ARC patterns** (`plans/clang-arc-lessons/`) — KnownSafe flag, barrier analysis, RC motion, COW contraction, per-phase elimination statistics

Every pending plan shrinks the problem space AIMS has to emit RC ops for. The endgame is emitted code where RC operations are rare enough to audit one-by-one.

### Verification Surface

The AIMS verification stack is **layered**, not a single function. Each layer catches a different class of inconsistency; a fix that passes one layer but regresses another is a correctness regression, not a partial win.

1. **Structural ARC IR verification** (`pipeline::run_verify` → `verify::check_function`) — runs 5 dedicated checks producing 5 corresponding `VerifyError` variants: `check_variable_scope` (`UseBeforeDef`), `check_block_connectivity` (`DanglingBlockRef`), `check_no_rc_on_scalar` (`RcOnScalar`), `check_no_dec_on_borrowed` (`DecOnBorrowed`), and `check_arg_ownership_len` (`ArgOwnershipLenMismatch`). It does NOT check "RC balance" or "drop placement" as holistic properties — each check targets a specific well-formedness failure. It also does NOT produce `VerifyError::FipStructural` directly; that variant exists in the shared `VerifyError` enum but is **constructed by the pipeline runner** when wrapping layer-4 FIP errors (see layer 4 below). Called at exactly two checkpoints: after AIMS emission and after the full AIMS pipeline (`pipeline/aims_pipeline/postprocess.rs:21,63`); NOT after every pipeline step.
2. **AIMS contract consistency** (`pipeline::run_aims_verify` → `verify::check_function_with_contract`) — filters the general verifier's output to AIMS-specific inconsistencies. **Currently the only variant reported is `VerifyError::AbsentParamHasUses`** (parameters declared `Cardinality::Absent` must have no live uses on any forward-reachable path). It is NOT a whole-lattice oracle.
3. **Oracle cross-check** (`aims::verify::oracle::verify_coherence`, `aims/verify/oracle.rs`) — re-derives a `MemoryContract` from the realized IR and compares it against the inferred contract along `access`, `consumption`, and `effects` dimensions; reports unsafe mismatches where analysis was more optimistic than realization needed. Runs in batch mode under `pipeline::aims_pipeline::batch` when `verify_arc` is enabled.
4. **FIP certification** (`aims::verify::fip::verify_fip_contract`, `aims/verify/fip.rs`) — proves `FipContract::Certified` functions have zero unmatched allocations/deallocations in the realized IR. When FIP verification fails, the pipeline runner **wraps** the underlying `FipVerificationError` values as `VerifyError::FipStructural` entries in the shared verification error stream (first-pass wrap at `pipeline/aims_pipeline/mod.rs:269`, second-pass batch wrap at `pipeline/aims_pipeline/batch.rs:231`). `FipStructural` originates here, not in layer 1 — the shared `VerifyError` enum is a carrier, not an attribution.

When CLAUDE.md or these docs refer to "verifying AIMS consistency," the claim is about this layered stack as a whole — not about any single function. Do NOT cite `run_aims_verify()` as the proof engine for the full AimsState model; it is one specific check in layer 2.

**Every change to ARC/AIMS code must preserve system coherence.** Fixing one subsystem while leaving another inconsistent is not a fix — it's a new bug. When you touch RC emission, ask if contracts still agree. When you touch contracts, ask if realization still matches. When you touch COW, ask if reuse and drop hints still cohere. And ask: **does this fix preserve the through-line from proof to elimination?** A change that adds RC ops without pointing at a specific proof failure is a regression, not a correctness win.

### Non-Negotiable Invariants

These hold at all times. Any change that violates one is a bug, not a tradeoff.

1. **Contracts and realization must agree.** If `MemoryContract` says `FipContract::Certified`, the realized IR must have zero unmatched allocations/deallocations. If realization disproves the contract, correct the contract — not leave it stale.
2. **Active rewrites must be sound.** `normalize_function()` transforms must produce identical observable behavior. Structural tests alone do not satisfy this — behavioral verification is required. If unverifiable, the rewrite must not run.
3. **No pass may rely on stale summaries.** If a pipeline step modifies IR or updates an effect summary, all downstream consumers must see updated values. A verifier that runs before its inputs are available is a sequencing bug.
4. **The enabled surface must be end-to-end verified.** Every active subsystem needs: implementation + invariant enforcement + verification (structural + behavioral + regression). Missing any of the three = incomplete.
5. **The unified model must stay unified.** New analysis capabilities must either (a) extend a lattice dimension, (b) extend a contract field on `MemoryContract` / `ParamContract` / `ReturnContract` / `EffectSummary`, or (c) feed the lattice-driven analysis as a typed pre-pass input that lands on `AimsStateMap` (as `immortal` detection does via the `immortals: Vec<bool>` bitvector). What they must NOT do is spawn an independent RC emission path, a parallel escape enum, or a shadow uniqueness tracker that bypasses the lattice. If a fix looks like it needs a new top-level data structure next to `AimsStateMap`, pause and ask whether the facility belongs inside one of (a)/(b)/(c) instead.

---

## Design

- Inspired by Lean 4 LCNF IR | three-way classification: `Scalar`/`DefiniteRef`/`PossibleRef`
- Backend-independent — `ori_arc` has no LLVM dependency | `arc_emitter` in `ori_llvm` translates ARC IR to LLVM IR
- **Sole codegen path** (since 2026-02-24) — previous Tier 1 `ExprLowerer` removed (~11K lines). All LLVM codegen goes through ARC IR.

## Pipeline (AIMS unified lattice)

```
CanExpr → lower → ArcFunction
  Interprocedural (once):
    1. analyze_program()         — MemoryContract per function (SCC fixpoint)
    2. apply_ownership()         — Populate ArcParam.ownership
  Per-function (steps 3–12):
    3. compute_var_reprs()       — ValueRepr per variable
   3a. normalize_function()      — TRMC context region detection
    4. analyze_function()        — Backward dataflow → converged AimsStateMap
    5. realize_rc_reuse()        — Phase 1: RC + reuse + arg_ownership (pre-merge)
   5a. verify_fip_contract()     — FIP enforcement verification
    6. verify()                  — ARC IR sanity check
    7. run_aims_verify()         — AIMS contract vs IR consistency
    8. detect/rewrite tail calls — CFG optimization
   8a. unwind_cleanup()          — Invoke-unwind RC cleanup (must precede merge)
    9. merge_blocks()            — CFG cleanup
   10. realize_annotations()     — Phase 2: COW + drop hints (post-merge)
   11. verify()                  — Final sanity check
   12. FBIP enforcement          — Read-only diagnostic
```

Key constraint: step 10 uses `AimsStateMap` via ArcVarId-keyed lookups (`var_state_at_block_entry`), not the position-keyed `entry_states`/`exit_states` maps (invalidated by merge_blocks). The state map is accessed by walking the post-merge IR block indices and using those as block IDs into the pre-merge state map — this works because merge_blocks() preserves entry block IDs.

### Fixpoint Convergence Obligations

Any fixpoint analysis (interprocedural contract computation, intraprocedural backward dataflow) must satisfy:

- **Finite lattice height**: every lattice dimension has bounded height. The count of dimensions is a choice that can evolve; the invariant is that **each active dimension** must be provably finite. Current state: 7 dimensions, each proven finite — adding a new dimension requires re-proving finiteness for it.
- **Monotone transfer functions**: `state_after >= state_before` in the lattice partial order. Non-monotone transfer = unsound analysis.
- **Deterministic worklist ordering**: iteration must be deterministic regardless of hash-map ordering. Use reverse-postorder or SCC-index-based ordering.
- **Iteration bound**: derived from domain dimensions — see `aims-rules.md` IC-7 for the authoritative formula. Practical safety cap: `max(100, derived_limit)`. Log warning at 50% of cap. Abort with diagnostic at 100%.
- **Widening**: if convergence is slow due to lattice height, apply widening at loop headers. Currently not needed (all dimensions have height <= 5).

### Entry Points

- `run_arc_pipeline()` (single fn) | `run_arc_pipeline_all()` (batch) — always use AIMS pipeline
- `compute_aims_contracts()` — interprocedural contract computation + param ownership application

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `ArcClass` | `classify/` | `Scalar` / `DefiniteRef` / `PossibleRef` — drives all RC decisions |
| `ArcFunction` | `ir/` | Basic-block IR: params, blocks, var_types |
| `ArcInstr` | `ir/` | Instructions: Apply, PartialApply, Construct, Project, RcInc, RcDec, Set, etc. |
| `ArcTerminator` | `ir/` | Block exits: Return, Jump, Branch, Switch, Invoke, Resume, Unreachable |
| `Ownership` | `ownership/` | `Owned` / `Borrowed` — borrow inference output |
| `DerivedOwnership` | `ownership/` | Per-variable ownership for all locals |
| `AnnotatedSig` | `ownership/` | Function signature with ownership annotations |
| `DropInfo` / `DropKind` | `drop/` | Per-type drop requirements: None, Scalar, RcDec, Struct, Enum, ClosureEnv |
| `ArcClassifier` | `classify/` | Pool-backed classifier with caching |
| `AimsState` | `aims/lattice/` | Product lattice over memory dimensions — today 7: AccessClass × Consumption × Cardinality × Uniqueness × Locality × ShapeClass × EffectClass (extensible) |
| `MemoryContract` | `aims/contract/` | Per-function interprocedural summary (param contracts + return info + effects) |
| `ParamContract` | `aims/contract/` | Per-parameter access, consumption, cardinality, locality_bound |
| `AimsStateMap` | `aims/intraprocedural/` | Block-boundary analysis results (block_exit_states, block_entry_states, borrow_sources, events, scalars, immortals) |
| `AimsPipelineConfig` | `pipeline/aims_pipeline` | Config bundle: classifier, contracts, pool, interner, builtins, verify flag |

## ARC IR Instructions

| Instruction | Semantics |
|-------------|-----------|
| `Apply { dst, func, args }` | Direct function call |
| `ApplyIndirect { dst, closure, args }` | Call through closure (fat pointer) |
| `PartialApply { dst, ty, func, args }` | Capture args into closure environment |
| `Construct { dst, ty, ctor, args }` | Build struct/enum/closure |
| `Project { dst, ty, src, field }` | Extract field from struct/enum |
| `RcInc { var }` | Increment reference count |
| `RcDec { var }` | Decrement reference count (+ drop if zero) |
| `Set { dst, ty, obj, field, val }` | In-place field mutation (requires IsShared check) |
| `IsShared { dst, var }` | Check if refcount > 1 |
| `Reuse { dst, ty, token, args }` | Reuse allocation (reset/reuse optimization) |

## Protocol Builtins

Compiler-internal protocol functions emitted by ARC lowering. These appear as `Apply` callees in ARC IR but are intercepted by the LLVM emitter -- they never become real function calls. Each variant carries per-argument ownership semantics so borrow inference handles them correctly.

Source: `ori_ir/src/builtin_constants/protocol/mod.rs`

| Variant | ARC IR Name | Args | Ownership | Purpose |
|---------|------------|------|-----------|---------|
| `Index` | `__index` | 2 | Borrowed, Borrowed | `receiver[index]` -- list/map indexing |
| `Iter` | `iter` | 1 | Borrowed | Iterator creation from collection |
| `IterNext` | `__iter_next` | 2 | Owned, Borrowed | Iterator advancement (iterator consumed, type marker borrowed) |
| `IterDrop` | `ori_iter_drop` | 1 | Owned | Iterator cleanup — consumes the iterator handle (TPR-07-008) |
| `CollectSet` | `__collect_set` | 1 | Owned | Set collection from iterator (iterator consumed) |

## Crate Structure

| Module | Purpose |
|--------|---------|
| `ir/` | ARC IR definitions (ArcFunction, ArcBlock, ArcInstr, ArcVarId) |
| `lower/` | CanExpr → ARC IR lowering (expressions, calls, control flow, patterns, collections) |
| `classify/` | Type classification (Scalar/DefiniteRef/PossibleRef) |
| `borrow/` | Borrow inference — determines Owned vs Borrowed for params (used by LLVM ABI) |
| `ownership/` | Ownership annotations, derived ownership for locals |
| `liveness/` | Liveness analysis (standard + refined with dominator info, used by FBIP) |
| `rc_insert/` | Insert RcInc/RcDec based on ownership + liveness |
| `drop/` | Per-type drop info computation (DropKind, ClosureEnv drops) |
| `fbip/` | Functional-but-in-place analysis (Koka-inspired) |
| `graph/` | Dominator tree construction |
| `decision_tree/` | Pattern match compilation to decision trees |
| `uniqueness/` | COW type definitions (CowAnnotations, DropHints, CowMode, Uniqueness, UniquenessSummary) |
| `aims/lattice/` | AimsState product lattice + dimension enums + join/meet/predicates (extensible; currently 7 dimensions) |
| `aims/transfer/` | Per-instruction transfer functions (backward dataflow) |
| `aims/contract/` | MemoryContract, ParamContract, ReturnContract, EffectSummary |
| `aims/intraprocedural/` | Per-function backward analysis + AimsStateMap + block-level computation |
| `aims/interprocedural` | SCC-based fixpoint loop computing contracts across call graph |
| `aims/emit_rc/` | RC emission from state map (inc/dec placement, arg ownership, COW, drop hints, coalescing) |
| `aims/emit_reuse/` | Reuse emission (detect, plan, dynamic expansion, FIP checking) |
| `aims/immortal/` | Heap-allocated constant detection (immortal objects skip RC) |
| `aims/builtins/` | Builtin function ownership contracts |
| `pipeline/aims_pipeline` | AIMS pipeline orchestration (AimsPipelineConfig, run_aims_pipeline) |

## ARC Emitter (ori_llvm)

`codegen/arc_emitter/` translates ARC IR → LLVM IR:
- `mod.rs` — `ArcIrEmitter`: main emission loop, instruction dispatch
- `drop_gen.rs` — `DropFunctionGenerator`: per-type LLVM drop functions (cached by mangled name)
- `tests.rs` — AOT tests for ARC codegen

### Emission Patterns

- **RcInc/RcDec**: Closure-aware — extracts env_ptr from field 1, null-checks, then inc/dec. RcDec loads drop_fn from field 0.
- **IsShared**: Inline GEP+load+icmp (no function call) — `refcount > 1`
- **Set/SetTag**: GEP+store for field mutation, with IsShared guard for COW
- **Reuse**: Fast path (in-place via token) + slow path (Dec old + fresh alloc)
- **Drop functions**: Cached by `_ori_drop$<mangled_type>`. Struct drops call field drops. Enum drops switch on tag.

## Critical Rules

- **No invisible gaps** — never stub with silent dummy values. Use `todo!("emit_<instr_name>")`, not silent `{ null, null }`. Use `assert!(data.is_empty(), "feature X not yet supported")`, not `let _data = ...`
- **Vertical slice testing** — every ARC IR instruction with LLVM emission must have an AOT test: Ori source → ARC lowering → AIMS analysis → LLVM emission → execution
- **Matrix testing for RC changes** — any change to RC emission, elem_dec_fn, or iterator cleanup must be tested across ALL relevant element types (str, [int], Option<str>, closures, structs, maps, sets) AND all relevant iteration patterns (full, break, yield, guard, nested, two-call). A fix that works for `str` but isn't tested with `Option<str>` and maps is incomplete.
- **Narrow the front** — complete one RC fix fully (fix + matrix tests + semantic pin + plan update) before starting another. RC + control-flow + lowering interactions multiply failure surfaces; working on elem_dec_fn and for-yield scoping simultaneously compounds risk.
- **Classification correctness** — `ArcClass` drives all RC behavior. Misclassification is catastrophic:
  - Scalar as DefiniteRef → unnecessary RC ops (perf bug)
  - DefiniteRef as Scalar → missing RC ops (use-after-free / leak)
  - `PossibleRef` after monomorphization → compiler bug
- **Pipeline ordering** — AIMS pipeline step order is load-bearing:
  - `analyze_program()` (interprocedural) must run before any per-function steps
  - `analyze_function()` (step 4) must run before `realize_rc_reuse()` (step 5) — state map drives emission
  - `realize_annotations()` (step 10) must run AFTER `merge_blocks()` (step 9) — it accesses `AimsStateMap` via ArcVarId-keyed lookups; position-keyed state map fields are stale after merge
  - Position-keyed state maps (`entry_states`, `exit_states`, `instr_states`) are invalid after `merge_blocks()` — never query them post-merge
  - Do NOT add passes without updating the pipeline. Do NOT call out of order.

## Debugging

- **Tracing**: `ORI_LOG=ori_arc=debug` (function entry/exit, loops, lambdas, match, merges) | `ori_arc=trace` (per-expression lowering, scope bindings) | add `ORI_LOG_TREE=1` for hierarchical view
- **Per-phase RC snapshot** (post-walk bisection): `ORI_LOG=ori_arc::aims::realize=trace ori build file.ori` — emits one event per `(phase, function, block)` summarising every `RcInc`/`RcDec` by `ArcVarId`. Phases: `after_phase_1_walk`, `after_phase_1_5_dead_invoke`, `after_phase_2_edge_cleanup`, `after_phase_2_1_escape_incs`, `after_phase_3_coalesce`. Use to bisect which post-walk pass modified a specific block's RC ops without inline `tracing::debug!` insertions. Zero overhead when disabled (gated behind `tracing::enabled!`). See `compiler/ori_arc/src/aims/realize/emit_unified.rs::trace_phase_snapshot`.
- **Pipeline phase bisection**: `ORI_LOG=ori_arc::aims::pipeline=info cargo run -- build file.ori` — emits one checkpoint per pipeline step per function with RC counts + structural metrics (blocks, vars). Use `diagnostics/bisect-passes.sh file.ori` for automated table display with divergence detection. Coarser-grained than the `realize` snapshots above — answers "which pipeline step changed RC balance" without per-block detail.
- **Phase dump**: `ORI_DUMP_AFTER_ARC=1 ori build file.ori` — ARC IR with RC strategy annotations
- **Runtime RC / codegen audit / diagnostic scripts**: See `runtime.md` for `ORI_TRACE_RC`/`ORI_CHECK_LEAKS`, `diagnostic.md` §Diagnostic Scripts for full script table and flags
- **Loop not terminating?** `ori_arc=debug` → break/continue jumps + mutable var counts
- **Wrong var after if/match?** `ori_arc=trace` → mutable var merge divergence
- **Lambda captures wrong?** `ori_arc=debug` → capture count, `trace` → each captured name

## Advanced Optimization Patterns (from prior art)

### RC Identity Through Projections
`retain(struct.field)` may equal `retain(struct)` when field is the only RC-tracked component. Track RC identity equivalence through `Project` instructions to eliminate redundant inc/dec pairs. Pattern from Swift (`~rc` equivalence relation), Lean 4 (`DerivedValInfo`).

### Lattice-Based RC State
RC elimination should be formal dataflow analysis with lattice semantics, not ad-hoc:
```
None → Decremented → MightBeUsed → MightBeDecremented
```
Meet operation at control flow joins. `KnownSafe` flag when nested retains guarantee safety. Pattern from Swift ARC optimizer.

### Tail Call Preservation
Never insert `RcDec` after a tail call — breaks TCO. Transfer ownership instead: mark callee param as `Owned` when call-site arg is owned, eliminating the inc/dec pair. Pattern from Lean 4 (`ownParamsUsingArgs`).

## Reference Repos

- **Lean 4**: `lean4/src/Lean/Compiler/IR/RC.lean` (RC insertion), `Borrow.lean` (borrow inference), `ExpandResetReuse.lean` (reuse)
- **Swift**: `swift/lib/SILOptimizer/ARC/` (ARC optimization), `swift/lib/SIL/` (SIL IR), `ARCOptimization.md` (lattice docs)
- **Koka**: `koka/src/Core/Borrowed.hs` (borrow analysis), `koka/src/Core/CheckFBIP.hs` (FBIP)

## Key Files

| File | Purpose |
|------|---------|
| `ori_arc/src/lib.rs` | Pipeline entry, ArcClass, ArcClassification trait |
| `ori_arc/src/ir/mod.rs` | ARC IR definitions |
| `ori_arc/src/lower/mod.rs` | ArcLowerer + ArcIrBuilder |
| `ori_arc/src/lower/calls/mod.rs` | Function call + lambda lowering |
| `ori_arc/src/borrow/mod.rs` | Borrow inference (LLVM ABI decisions) |
| `ori_arc/src/drop/mod.rs` | Drop info computation |
| `ori_arc/src/aims/mod.rs` | AIMS module root (lattice framework) |
| `ori_arc/src/aims/lattice/mod.rs` | AimsState product lattice + operations |
| `ori_arc/src/aims/interprocedural.rs` | SCC fixpoint for MemoryContract |
| `ori_arc/src/aims/intraprocedural/mod.rs` | Per-function backward dataflow |
| `ori_arc/src/aims/emit_rc/mod.rs` | RC emission from state map |
| `ori_arc/src/aims/emit_reuse/mod.rs` | Reuse emission (detect + plan + expand) |
| `ori_arc/src/pipeline/aims_pipeline/` | AIMS pipeline orchestration (mod.rs: config+run, trmc.rs: TRMC, postprocess.rs: verify+FBIP, batch.rs: batch+second pass) |
| `ori_llvm/src/codegen/arc_emitter/mod.rs` | ARC IR → LLVM emission |
| `ori_llvm/src/codegen/arc_emitter/drop_gen.rs` | LLVM drop function generation |
| `ori_llvm/tests/aot/arc.rs` | AOT integration tests for ARC |
