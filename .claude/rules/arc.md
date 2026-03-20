---
paths:
  - "**arc**"
---

# ARC Optimization — AIMS (ARC Intelligent Memory System)

## Mission — READ THIS FIRST

AIMS exists to replace fragmented memory-management heuristics with a **single sound semantic framework**. RC placement, reuse, COW, FIP, contracts, and TRMC are **not separate features** — they are facets of one model and must agree. The goal is not partial implementation of many ideas, but one trustworthy system whose claims are enforceable in code and verification.

**Every change to ARC/AIMS code must preserve system coherence.** Fixing one subsystem while leaving another inconsistent is not a fix — it's a new bug. When you touch RC emission, ask if contracts still agree. When you touch contracts, ask if realization still matches. When you touch COW, ask if reuse and drop hints still cohere.

### Non-Negotiable Invariants

These hold at all times. Any change that violates one is a bug, not a tradeoff.

1. **Contracts and realization must agree.** If `MemoryContract` says `FipContract::Certified`, the realized IR must have zero unmatched allocations/deallocations. If realization disproves the contract, correct the contract — not leave it stale.
2. **Active rewrites must be sound.** `normalize_function()` transforms must produce identical observable behavior. Structural tests alone do not satisfy this — behavioral verification is required. If unverifiable, the rewrite must not run.
3. **No pass may rely on stale summaries.** If a pipeline step modifies IR or updates an effect summary, all downstream consumers must see updated values. A verifier that runs before its inputs are available is a sequencing bug.
4. **The enabled surface must be end-to-end verified.** Every active subsystem needs: implementation + invariant enforcement + verification (structural + behavioral + regression). Missing any of the three = incomplete.

---

## Design

- Inspired by Lean 4 LCNF IR | three-way classification: `Scalar`/`DefiniteRef`/`PossibleRef`
- Backend-independent — `ori_arc` has no LLVM dependency | `arc_emitter` in `ori_llvm` translates ARC IR to LLVM IR
- **Sole codegen path** (since 2026-02-24) — previous Tier 1 `ExprLowerer` removed (~11K lines). All LLVM codegen goes through ARC IR. See `plans/aot_codegen_pipeline/`

## Pipeline (AIMS unified lattice)

```
CanExpr → lower → ArcFunction
  Interprocedural (once):
    1. analyze_program()         — MemoryContract per function (SCC fixpoint)
    2. apply_ownership()         — Populate ArcParam.ownership
  Per-function (steps 3–14):
    3. compute_var_reprs()       — ValueRepr per variable
    4. emit_arg_ownership()      — Apply/Invoke arg_ownership
    5. analyze_function()        — Backward dataflow → AimsStateMap
    6. emit_rc_ops()             — RcInc/RcDec from state map
    7. emit_reuse()              — Reset/Reuse from state map
    9. verify()                  — ARC IR sanity check
   9a. run_aims_verify()         — AIMS-specific: contract vs IR consistency
   10. detect/rewrite tail calls — CFG optimization
   11. merge_blocks()            — CFG cleanup
  11a. compute_aims_cow_annotations() — COW from state map (post-merge, ArcVarId lookup)
   12. compute_aims_drop_hints()      — Drop hints from state map (post-merge, ArcVarId lookup)
   13. verify()                       — Final sanity check
   14. check_fbip_enforcement()       — Read-only diagnostic
```

Key constraint: steps 11a–12 use `AimsStateMap` via ArcVarId-keyed lookups (`var_state_at_block_entry`), not the position-keyed `entry_states`/`exit_states` maps (invalidated by merge_blocks). The state map is accessed by walking the post-merge IR block indices and using those as block IDs into the pre-merge state map — this works because merge_blocks() preserves entry block IDs.

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
| `AimsState` | `aims/lattice/` | 7D product lattice: AccessClass × Consumption × Cardinality × Uniqueness × Locality × ShapeClass × EffectClass |
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
| `aims/lattice/` | 7D product lattice (AimsState) + dimension enums + join/meet/predicates |
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
  - `analyze_function()` (step 5) must run before `emit_rc_ops()` (step 6) — state map drives emission
  - `compute_aims_cow_annotations()` and `compute_aims_drop_hints()` must run AFTER `merge_blocks()` — they access `AimsStateMap` via ArcVarId-keyed lookups; position-keyed state map fields are stale after merge
  - Position-keyed state maps (`entry_states`, `exit_states`, `instr_states`) are invalid after `merge_blocks()` — never query them post-merge
  - Do NOT add passes without updating the pipeline. Do NOT call out of order.

## Debugging

- **Tracing**: `ORI_LOG=ori_arc=debug` (function entry/exit, loops, lambdas, match, merges) | `ori_arc=trace` (per-expression lowering, scope bindings) | add `ORI_LOG_TREE=1` for hierarchical view
- **Phase dump**: `ORI_DUMP_AFTER_ARC=1 ori build file.ori` — ARC IR with RC strategy annotations
- **Runtime RC**: `ORI_TRACE_RC=1 ./binary` | `ORI_RT_DEBUG=1 ./binary` | `ORI_CHECK_LEAKS=1 ./binary`
- **Codegen audit**: `ORI_AUDIT_CODEGEN=1 ori build file.ori` (add `ORI_AUDIT_STRICT=1` | `ORI_AUDIT_FUNCTION=name`)
- **AIMS comparison**: `diagnostics/aims-compare.sh [--behavioral-only|--rc-only] [--verbose] [--release]` — compares output + RC counts
- **Diagnostic scripts**: `diagnostics/rc-stats.sh` | `codegen-audit.sh` | `diagnose-aot.sh` | `dual-exec-debug.sh` (see compiler.md for full list)
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
| `ori_arc/src/aims/mod.rs` | AIMS module root (7D lattice framework) |
| `ori_arc/src/aims/lattice/mod.rs` | AimsState product lattice + operations |
| `ori_arc/src/aims/interprocedural.rs` | SCC fixpoint for MemoryContract |
| `ori_arc/src/aims/intraprocedural/mod.rs` | Per-function backward dataflow |
| `ori_arc/src/aims/emit_rc/mod.rs` | RC emission from state map |
| `ori_arc/src/aims/emit_reuse/mod.rs` | Reuse emission (detect + plan + expand) |
| `ori_arc/src/pipeline/aims_pipeline.rs` | AIMS pipeline orchestration |
| `ori_llvm/src/codegen/arc_emitter/mod.rs` | ARC IR → LLVM emission |
| `ori_llvm/src/codegen/arc_emitter/drop_gen.rs` | LLVM drop function generation |
| `ori_llvm/tests/aot/arc.rs` | AOT integration tests for ARC |
