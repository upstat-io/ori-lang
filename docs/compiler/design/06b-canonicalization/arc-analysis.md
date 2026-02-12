---
title: "ARC Analysis"
description: "Ori Compiler Design — ARC Pipeline Overview"
order: 654
section: "Canonicalization"
---

# ARC Analysis

The ARC (Automatic Reference Counting) system (`ori_arc` crate) transforms canonical IR into memory-managed code with explicit reference counting. It operates as a multi-pass pipeline that analyzes ownership, inserts RC operations, and optimizes for in-place mutation.

## Pipeline Position

```text
Canonicalize → [ARC Pipeline] → LLVM Codegen
                    │
                    ├─ 1. Lower (CanExpr → ARC IR)
                    ├─ 2. Borrow Inference
                    ├─ 3. Ownership Derivation
                    ├─ 4. Liveness Analysis
                    ├─ 5. RC Insertion
                    ├─ 6. Reset/Reuse Detection
                    ├─ 7. Reuse Expansion
                    ├─ 8. RC Elimination
                    └─ 9. FBIP Diagnostics (optional)
```

Consumers use `run_arc_pipeline_all()` for batch processing (applies borrows, then runs per-function pipeline) or `run_arc_pipeline()` for a single function — never manual pass sequencing.

## ARC IR

The ARC IR is a basic-block, SSA-form IR with explicit control flow:

| Concept | AST (CanExpr) | ARC IR |
|---------|--------------|--------|
| Control flow | Implicit (if/else as expressions) | Explicit blocks + jumps |
| Variables | Scoped names | SSA variables (VarId) |
| Mutability | Mutable let bindings | Phi-via-block-params |
| Lambdas | Inline closures | Separate ArcFunctions + PartialApply |
| Calls | May or may not unwind | Apply (nounwind) or Invoke (may-panic) |

### Instruction Set

| Instruction | Purpose |
|-------------|---------|
| `Let` | Bind value to variable |
| `Apply` / `ApplyIndirect` | Direct / indirect function call |
| `PartialApply` | Create closure (captures args) |
| `Construct` | Create struct/tuple/enum value |
| `Project` | Extract field from composite |
| `RcInc` | Increment reference count |
| `RcDec` | Decrement reference count |
| `IsShared` | Test if refcount > 1 |
| `Set` / `SetTag` | In-place field / tag mutation |
| `Reset` / `Reuse` | Memory reuse markers |

### Terminators

`Return`, `Jump`, `Branch`, `Switch`, `Invoke`, `Resume`, `Unreachable`

## Type Classification

Every type is classified into one of three categories:

| ArcClass | Meaning | RC Needed? |
|----------|---------|------------|
| `Scalar` | Primitive (int, float, bool, byte) | No |
| `DefiniteRef` | Heap-allocated (struct, enum, list, str, closure) | Yes |
| `PossibleRef` | Generic parameter (pre-monomorphization) | Conservative yes |

The `ArcClassifier` caches classification results and detects cycles.

## Pass Descriptions

### Borrow Inference

Determines whether function parameters are **borrowed** (callee doesn't retain) or **owned** (must increment on call).

- **Algorithm**: Fixed-point iteration — initialize all non-scalar params as Borrowed, promote to Owned when ownership-requiring patterns are found
- **Convergence**: Monotonic (Borrowed → Owned only), guaranteed in ≤N iterations

### Liveness Analysis

Backward dataflow analysis determining which variables are live at each block boundary.

- **Algorithm**: Fixed-point iteration in postorder over CFG
- **Skips**: Scalar variables (no RC needed)
- **Refined variant**: Enhanced with aliasing checks for reset/reuse safety

### RC Insertion (Perceus Algorithm)

Places `RcInc` and `RcDec` at precise points:

- **Multi-use**: Variable used more than once → `RcInc` at second use
- **Last use**: Variable's final use → `RcDec` after use
- **Borrowed parameters**: Skip all RC (caller's responsibility)

### Reset/Reuse Detection

Identifies opportunities for in-place constructor reuse:

```text
Pattern: RcDec(x) followed by Construct(T) where typeof(x) == T
         ↓
         Reset(x, token) ... Reuse(token, T, fields)
```

Constraints: same type, no aliasing between dec and construct, RC-needed type.

### Reuse Expansion

Expands Reset/Reuse pairs into conditional two-path code:

```text
IsShared(token)
├─ Fast path (unique, RC==1): Set mutations in-place
└─ Slow path (shared, RC>1): RcDec + fresh Construct
Merge: continuation receives result
```

Sub-optimizations: projection-increment erasure (skip RcInc for projected fields on fast path), self-set elimination (skip no-op field writes).

### RC Elimination

Removes redundant RcInc/RcDec pairs via bidirectional intra-block dataflow:

```text
RcInc(x); ... /* no use of x */ ...; RcDec(x)  →  removed
```

Cascading: iterates until no more pairs found.

### FBIP Diagnostics

"Functional But In-Place" — read-only diagnostic pass that reports:

- **Achieved reuse**: Reset/Reuse pairs that will use in-place mutation
- **Missed reuse**: RcDec + Construct that *could* have reused but didn't (with reason: type mismatch, intermediate use, no dominance, possibly shared)

This provides developer feedback on allocation reuse without modifying the IR.

## Drop Descriptors

The `drop` module generates declarative drop descriptors for LLVM codegen:

| DropKind | Applies To |
|----------|-----------|
| `Trivial` | Just free, no RC'd children |
| `Fields(Vec<(idx, ty)>)` | Struct/tuple: dec specific fields |
| `Enum(Vec<Vec<...>>)` | Switch on tag, per-variant drops |
| `Collection { element_type }` | Iterate & dec elements |
| `Map { key, value }` | Iterate entries, dec keys/values |
| `ClosureEnv(...)` | Dec captured variables |

## Module Structure

```
compiler/ori_arc/src/
├── lib.rs              # Pipeline entry (run_arc_pipeline, run_arc_pipeline_all), ArcClass
├── ir.rs               # ARC IR types (ArcFunction, ArcBlock, ArcInstr, ArcTerminator, ArcVarId)
├── classify.rs         # ArcClassifier: Pool → ArcClass mapping with cache + cycle detection
├── decision_tree.rs    # DecisionTree, PatternMatrix, FlatPattern, TestKind, TestValue
├── borrow.rs           # Borrow inference (fixed-point) and borrow application
├── ownership.rs        # Ownership enums (Borrowed/Owned, DerivedOwnership)
├── liveness.rs         # Backward liveness analysis (standard + refined with aliasing)
├── rc_insert.rs        # RC insertion (Perceus)
├── rc_elim.rs          # RC elimination (bidirectional)
├── reset_reuse.rs      # Reset/Reuse detection
├── expand_reuse.rs     # Reuse expansion (two-path codegen)
├── drop.rs             # Drop descriptor generation
├── fbip.rs             # FBIP diagnostics (read-only)
├── graph.rs            # CFG utilities (dominator tree, predecessors)
├── test_helpers.rs     # Shared test factories (test-only)
└── lower/              # CanExpr → ARC IR lowering
    ├── mod.rs          # ArcIrBuilder, lower_function_can entry point
    ├── expr.rs         # ArcLowerer: expression dispatch
    ├── calls.rs        # Function/method calls, lambdas, invoke/apply classification
    ├── collections.rs  # List/map/set/struct/enum construction, field access
    ├── control_flow.rs # if/else, match, try, loops, break/continue, assign
    ├── patterns.rs     # Pattern binding destructuring
    └── scope.rs        # ArcScope: name bindings, mutable tracking, SSA merge
```

## Prior Art

| Language | Reference | Approach |
|----------|-----------|----------|
| **Lean 4** | `src/Lean/Compiler/IR/{Borrow,RC,LiveVars,ResetReuse}.lean` | Perceus-style RC with borrow inference |
| **Koka** | Perceus paper (Reinking et al. 2021) | Precise reference counting with reuse |
| **Swift** | `lib/SILOptimizer/ARC/` | Bidirectional RC elimination, ownership SSA |
| **Roc** | Refcount helpers | Specialized drop per type |
