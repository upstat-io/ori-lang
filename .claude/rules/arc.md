---
paths:
  - "**arc**"
---

# ARC Optimization

## Design

- Inspired by Lean 4 LCNF IR | three-way classification: `Scalar`/`DefiniteRef`/`PossibleRef`
- Backend-independent — `ori_arc` has no LLVM dependency | `arc_emitter` in `ori_llvm` translates ARC IR to LLVM IR
- **Sole codegen path** (since 2026-02-24) — previous Tier 1 `ExprLowerer` removed (~11K lines). All LLVM codegen goes through ARC IR. See `plans/aot_codegen_pipeline/`

## Pipeline

Canonical pass ordering (do NOT reorder or skip):

```
CanExpr → lower → ArcFunction
  → borrow inference (param ownership: Owned/Borrowed)
  → derived ownership (all locals, not just params)
  → dominator tree
  → liveness + refined liveness
  → RC insertion (RcInc/RcDec)
  → reset/reuse detection
  → expand reset/reuse
  → RC elimination (dataflow-based dead RC removal)
  → cross-block RC elimination (inc/dec pairs across basic blocks)
```

- Entry: `run_arc_pipeline()` (single fn) | `run_arc_pipeline_all()` (batch with borrow application)
- Borrow sigs cached per session — unchanged function bodies reuse cached sigs

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
| `borrow/` | Borrow inference — determines Owned vs Borrowed for params |
| `ownership/` | Ownership annotations, derived ownership for locals |
| `liveness/` | Liveness analysis (standard + refined with dominator info) |
| `rc_insert/` | Insert RcInc/RcDec based on ownership + liveness |
| `rc_elim/` | Remove redundant RC operations via dataflow analysis |
| `reset_reuse/` | Detect constructor reuse opportunities (Lean 4 pattern) |
| `expand_reuse/` | Expand reuse tokens into concrete reuse instructions |
| `drop/` | Per-type drop info computation (DropKind, ClosureEnv drops) |
| `fbip/` | Functional-but-in-place analysis (Koka-inspired) |
| `graph/` | Dominator tree construction |
| `decision_tree/` | Pattern match compilation to decision trees |

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
- **Vertical slice testing** — every ARC IR instruction with LLVM emission must have an AOT test: Ori source → ARC lowering → borrow inference → RC insertion → LLVM emission → execution
- **Classification correctness** — `ArcClass` drives all RC behavior. Misclassification is catastrophic:
  - Scalar as DefiniteRef → unnecessary RC ops (perf bug)
  - DefiniteRef as Scalar → missing RC ops (use-after-free / leak)
  - `PossibleRef` after monomorphization → compiler bug
- **Pipeline ordering** — pass order in `run_arc_pipeline()` is load-bearing:
  - Borrow inference before RC insertion (ownership drives placement)
  - Liveness before RC insertion (dead vars skip dec)
  - Reset/reuse before expansion (detection before lowering)
  - RC elimination last (removes redundancies from earlier passes)
  - Do NOT add passes without updating `run_arc_pipeline()`. Do NOT call out of order.

## Debugging

- **Tracing**: `ORI_LOG=ori_arc=debug` (function entry/exit, loops, lambdas, match, merges) | `ori_arc=trace` (per-expression lowering, scope bindings) | add `ORI_LOG_TREE=1` for hierarchical view
- **Phase dump**: `ORI_DUMP_AFTER_ARC=1 ori build file.ori` — ARC IR with RC strategy annotations
- **Runtime RC**: `ORI_TRACE_RC=1 ./binary` | `ORI_RT_DEBUG=1 ./binary` | `ORI_CHECK_LEAKS=1 ./binary`
- **Codegen audit**: `ORI_AUDIT_CODEGEN=1 ori build file.ori` (add `ORI_AUDIT_STRICT=1` | `ORI_AUDIT_FUNCTION=name`)
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
| `ori_arc/src/borrow/mod.rs` | Borrow inference |
| `ori_arc/src/rc_insert/mod.rs` | RC operation insertion |
| `ori_arc/src/drop/mod.rs` | Drop info computation |
| `ori_llvm/src/codegen/arc_emitter/mod.rs` | ARC IR → LLVM emission |
| `ori_llvm/src/codegen/arc_emitter/drop_gen.rs` | LLVM drop function generation |
| `ori_llvm/tests/aot/arc.rs` | AOT integration tests for ARC |
