---
paths:
  - "**/ori_arc/**"
  - "**/arc_emitter/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# ARC Optimization

## Design

Inspired by Lean 4's LCNF IR and three-way type classification (`Scalar`/`DefiniteRef`/`PossibleRef`). Backend-independent — `ori_arc` has no LLVM dependency. The `arc_emitter` in `ori_llvm` translates ARC IR to LLVM IR.

**Sole codegen path**: As of 2026-02-24, the ARC pipeline is the only codegen path. The previous Tier 1 (`ExprLowerer`) was removed entirely (~11K lines deleted). All LLVM code generation goes through ARC IR. See `plans/aot_codegen_pipeline/` for the full 12-section plan that unified the pipeline.

## Pipeline

Canonical pass ordering (do NOT reorder or skip passes):

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

Entry points: `run_arc_pipeline()` (single function), `run_arc_pipeline_all()` (batch with borrow application). Borrow signatures are cached per session — recompilation of unchanged function bodies reuses cached sigs.

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `ArcClass` | `classify/` | `Scalar` / `DefiniteRef` / `PossibleRef` — drives all RC decisions |
| `ArcFunction` | `ir/` | Basic-block IR: params, blocks, var_types |
| `ArcInstr` | `ir/` | Instructions: Apply, PartialApply, Construct, Project, RcInc, RcDec, Set, etc. |
| `ArcTerminator` | `ir/` | Block exits: Return, Branch, CondBranch, Switch, Unreachable |
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

### No Invisible Gaps

**Never stub with silent dummy values.** When an ARC instruction emission is not yet implemented:
- Use `todo!("emit_<instr_name>")` — NOT silent `{ null, null }` or zero values
- If a lowering pass returns data that won't be consumed, use `assert!(data.is_empty(), "feature X not yet supported")` — NOT `let _data = ...`
**Rationale**: Silent stubs produce wrong output that passes tests. `todo!()` crashes with a clear message. `_var` discards suppress compiler warnings that would signal incomplete work. Invisible gaps compound — by the time you discover them, multiple layers need fixing simultaneously.

### Vertical Slice Testing

Every ARC IR instruction that has LLVM emission code must have an AOT test exercising the **full pipeline**: Ori source → ARC lowering → borrow inference → RC insertion → LLVM emission → execution. Unit tests for individual passes are necessary but not sufficient.

### Classification Correctness

`ArcClass` determines all RC behavior. Misclassification is catastrophic:
- **Scalar classified as DefiniteRef**: Unnecessary RC ops (performance bug)
- **DefiniteRef classified as Scalar**: Missing RC ops (use-after-free / memory leak)
- After monomorphization, `PossibleRef` should never appear — it's a compiler bug

### Pipeline Ordering

The pass order in `run_arc_pipeline()` is load-bearing. Passes depend on prior pass output:
- Borrow inference before RC insertion (ownership drives inc/dec placement)
- Liveness before RC insertion (dead variables don't need dec)
- Reset/reuse before expansion (detection before lowering)
- RC elimination last (removes redundancies introduced by earlier passes)

Do NOT add passes without updating `run_arc_pipeline()`. Do NOT call passes out of order.

## Reference Repos

- **Lean 4**: `lean4/src/Lean/Compiler/IR/RC.lean` (RC insertion), `Borrow.lean` (borrow inference), `ExpandResetReuse.lean` (reuse)
- **Swift**: `swift/lib/SILOptimizer/ARC/` (ARC optimization), `swift/lib/SIL/` (SIL IR)
- **Koka**: `koka/src/Core/Borrowed.hs` (borrow analysis), `koka/src/Core/CheckFBIP.hs` (FBIP)

## Debugging / Tracing

**Always use `ORI_LOG` first when debugging ARC issues.** The ARC lowering pipeline is fully instrumented with conditional `tracing` macros — zero-cost when disabled.

### Quick Reference

```bash
# ARC IR lowering (CanExpr → ARC IR)
ORI_LOG=ori_arc=debug ori build file.ori          # Function entry/exit, loops, lambdas, match, merges
ORI_LOG=ori_arc=trace ori build file.ori          # Per-expression lowering, scope bindings, assigns
ORI_LOG=ori_arc=debug ORI_LOG_TREE=1 ori build f.ori  # Hierarchical view of lowering phases

# ARC IR emission (ARC IR → LLVM IR)
ORI_LOG=ori_llvm=trace ori build file.ori         # ARC emission detail
ORI_DUMP_AFTER_LLVM=1 ori build file.ori           # Annotated LLVM IR (Ori names, RC/COW ops)
ORI_DUMP_AFTER_ARC=1 ori build file.ori            # ARC IR with RC strategy annotations

# Combined: see both lowering and emission
ORI_LOG=ori_arc=debug,ori_llvm=trace ori build file.ori

# Diagnostic scripts (USE THESE for quick RC debugging)
ORI_TRACE_RC=1 ./binary                           # Runtime RC event trace (alloc/inc/dec/free)
ORI_RT_DEBUG=1 ./binary                            # Runtime assertions (header validation, bounds checks)
ORI_CHECK_LEAKS=1 ./binary                         # Leak check with attribution on exit

# In-pipeline codegen audit (Rust-level, runs during compilation)
ORI_AUDIT_CODEGEN=1 ori build file.ori            # RC balance, COW sequencing, ABI arg counts
ORI_AUDIT_STRICT=1 ORI_AUDIT_CODEGEN=1 ori build file.ori  # Pessimistic: COW=freeing, params=RC-managed
ORI_AUDIT_FUNCTION=name ORI_AUDIT_CODEGEN=1 ori build file.ori  # Filter to specific function

# Diagnostic scripts (USE THESE for quick RC debugging)
diagnostics/rc-stats.sh file.ori                  # RC balance per function (flags over-release/leaks)
diagnostics/codegen-audit.sh file.ori             # Static RC balance, COW correctness, ABI checks (--strict)
diagnostics/diagnose-aot.sh file.ori              # All-in-one: build + leak check + RC stats + IR
diagnostics/dual-exec-debug.sh file.ori           # Interpreter vs AOT comparison (auto-dumps on mismatch)
```

### What Each Level Shows (ori_arc)

| Level | Events |
|-------|--------|
| `debug` | Function lowering enter/exit (block/var/lambda/problem counts), loop entry (header/body/exit blocks, mutable var count), for-iterator/for-option/for-yield dispatch, match entry (arm count), lambda lowering (param/capture count), break/continue jumps, mutable var merge summary |
| `trace` | Every `lower_expr` call (id, current BB), pattern bindings (name, var, mutable flag), mutable assignment rebinds (old var → new var), let-pattern bindings, per-variable merge divergence, scope bind/lookup |

### Tracing Instrumented Files

| File | What's traced |
|------|--------------|
| `lower/mod.rs` | `lower_function_can` entry/exit, per-param bindings |
| `lower/expr/mod.rs` | Every `lower_expr` dispatch (id + basic block) |
| `lower/control_flow/mod.rs` | Block, if, loop, for (range/iterator/option), break, continue, assign, let, match |
| `lower/control_flow/for_yield.rs` | For-yield dispatch (option vs iterator strategy) |
| `lower/calls/mod.rs` | Direct/indirect calls, method calls, lambda lowering with captures |
| `lower/patterns/mod.rs` | Pattern name bindings with mutability |
| `lower/scope/mod.rs` | Mutable variable merge (per-var divergence + summary) |

### Tips

- **Loop not terminating?** Use `ori_arc=debug` to see break/continue jumps and mutable var counts at loop boundaries.
- **Wrong variable value after if/match?** Use `ori_arc=trace` to see mutable var merge — which vars diverge and get block params.
- **Lambda captures wrong?** Use `ori_arc=debug` to see capture count and `ori_arc=trace` to see each captured name.
- **SSA block params wrong?** Use `ori_arc=trace` with `ORI_LOG_TREE=1` to see the full lowering tree with block parameter threading.

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
