# LLVM V2: Best-of-Breed Codegen Architecture

> **Description**: Replace the current monolithic `ori_llvm` codegen with a layered, modular architecture synthesized from patterns across Rust, Zig, Roc, Swift, Koka, Lean 4, Go, Gleam, and TypeScript compilers. The result is a highly extensible, ARC-aware codegen pipeline where adding new LLVM features is straightforward, each module has a single responsibility, and ARC optimization happens at the right abstraction level.
>
> **Primary Goal**: Make LLVM extension *easy*. Every new Ori language feature should map to a focused codegen module with clear inputs, outputs, and testing patterns. No more struggling with LLVM.

## Reference Compiler Analysis

Patterns were analyzed from nine production compilers and synthesized into a novel design:

| Compiler | Key Insight Adopted |
|----------|-------------------|
| **Rust** (`rustc_codegen_ssa` + `rustc_codegen_llvm`) | Two-context system (SimpleCx/FullCx); CGU partitioning; RAII builder lifetime management (trait-based backend abstraction studied but not adopted — YAGNI with single backend) |
| **Zig** (`src/codegen/llvm.zig` + `lib/std/zig/llvm/Builder.zig`) | Pure-Zig IR before LLVM; ID-based references everywhere (not pointers); bitcode serialization; deferred LLVM involvement |
| **Roc** (`crates/compiler/gen_llvm/`) | Inkwell wrapper patterns (BuilderExt); refcount-at-offset-minus-1 layout; Zig bitcode for RC runtime; Scope struct with persistent maps; monomorphized IR before codegen |
| **Swift** (`lib/SILOptimizer/ARC/` + `lib/IRGen/`) | SIL-level ARC optimization (not in codegen!); TypeInfo hierarchy for type-driven codegen; ownership lattice (None/Owned/Guaranteed/Unowned); two-pass dataflow (top-down + bottom-up); Explosion for multi-register values |
| **Koka** (`src/Backend/C/Parc.hs`) | Perceus precise RC; FBIP (Functional But In-Place) reuse analysis; borrow parameter inference; shape-based alias tracking from pattern matching |
| **Lean 4** (`src/Lean/Compiler/IR/`) | Borrow inference via iterative refinement; explicit `reset`/`reuse` operations; `isPossibleRef`/`isDefiniteRef` type classification; LCNF pipeline structure |
| **Go** (`cmd/compile/internal/ssa/`) | Dense ID allocation for values/blocks; ordered pass pipeline with logging hooks; deterministic iteration |
| **Gleam** (`compiler-core/src/codegen.rs`) | Lightweight struct-per-backend (no trait objects); multi-target codegen via separate generator structs; immutable environment with scope tracking |
| **TypeScript** (`src/compiler/transformer.ts`) | Transform pipeline factory pattern; layered transforms before emission; visitor-based tree rewriting |

## Design Philosophy

### 1. Two-Crate Architecture (Inspired by Swift + Rust)

The current `ori_llvm` mixes ARC concerns, type lowering, expression compilation, and LLVM emission in one giant `Builder`. The V2 design separates these into two crates with clear module boundaries:

```
Crate: ori_llvm (LLVM codegen — excluded from default workspace, requires LLVM)
  Module: ir_builder     — Safe, ID-based inkwell wrapper
  Module: codegen        — ARC IR → LLVM IR emission
    - TypeInfo enum         — LLVM type representation, layout, ABI
    - ExprLowerer           — Trivial ARC IR instruction emission
    - AbiComputer           — Calling conventions
  Module: emit           — Module/function lifecycle, debug info, optimization passes

Crate: ori_arc (ARC analysis — normal workspace member, NO LLVM dependency)
  - ARC IR lowering (typed AST → basic-block IR with explicit control flow)
  - Pattern compilation (decision trees compiled during AST → ARC IR lowering)
  - ArcClassification trait (Scalar/PossibleRef/DefiniteRef)
  - Borrow inference (like Lean 4)
  - RC insertion (like Koka Perceus)
  - RC elimination (like Swift ARC optimizer)
  - Reuse analysis (like Koka FBIP / Lean 4 reset-reuse)
  - Operates on ARC IR, BEFORE codegen
```

`ori_arc` is a separate crate because ARC analysis is reusable without LLVM — it can serve future backends (WASM, interpreter optimizations) and is testable independently. Codegen stays consolidated in `ori_llvm` since there is only one backend (LLVM) — no need for a separate `ori_codegen` crate (YAGNI).

**Key insight from Swift**: ARC optimization happens where you have full ownership information (`ori_arc`), NOT during LLVM IR emission (`ori_llvm`). Codegen is a "dumb emitter."

### 2. TypeInfo-Driven Code Generation (from Swift)

Two complementary abstractions split across crates:

**`ArcClassification` trait (in `ori_arc`)** — No LLVM dependency:
- Scalar / PossibleRef / DefiniteRef classification
- Used by ARC analysis to skip trivial types
- Operates on Pool/Idx, no LLVM types involved

**`TypeInfo` enum (in `ori_llvm`)** — LLVM-specific:
- LLVM type representation, size, alignment
- How to copy, destroy, retain, release
- Calling convention (by-value vs by-reference)
- Debug info generation
- Enum with variants per type category (static dispatch, no `dyn Trait`)

This centralizes all type-specific codegen in one place. Adding a new type means adding an enum variant, not modifying 20 match arms across 10 files.

### 3. ID-Based Everything (from Zig + existing Ori patterns)

Following Ori's existing `ExprId`/`Idx` patterns and Zig's approach:
- `LLVMValueId(u32)` instead of `&'ll Value`
- `LLVMTypeId(u32)` instead of `&'ll Type`
- `BlockId(u32)` instead of `BasicBlock<'ll>`

This eliminates lifetime pain, enables serialization/caching, and makes the codegen context `Send + Sync` for parallel compilation.

### 4. Modular Expression Lowering (from existing Ori + Gleam)

Each expression category gets its own focused module in `ori_llvm`:
- `lower_arithmetic.rs` -- Binary/unary ops
- `lower_control_flow.rs` -- If/match/loop/for
- `lower_functions.rs` -- Calls, lambdas, closures
- `lower_collections.rs` -- Lists, maps, tuples, structs

Pattern match compilation (decision trees) lives in `ori_arc` as part of AST-to-ARC-IR lowering. The `ori_llvm` codegen layer just emits `Switch` terminators as LLVM `switch` instructions -- no pattern logic in codegen.

Each module is independently testable with clear inputs (ARC IR instructions) and outputs (LLVM IR).

### 5. ARC as a Separate Analysis Pass (from Swift + Koka + Lean 4)

The current codegen has no ARC optimization. The V2 design introduces a pre-codegen ARC analysis pipeline operating on a dedicated **ARC IR**:

1. **Lower to ARC IR**: Convert typed AST to basic-block IR with explicit control flow (branches, joins, terminators). This is the intermediate representation that all ARC algorithms operate on — not the expression tree. The ARC IR makes control flow explicit, which is required for correct liveness analysis across branches, loops, and early exits.
2. **Classify types**: Scalar (no RC) vs PossibleRef vs DefiniteRef (from Lean 4)
3. **Infer borrows**: Parameters that don't escape can be borrowed (from Lean 4)
4. **Insert RC ops**: Precise inc/dec placement via liveness analysis on ARC IR basic blocks (from Koka Perceus)
5. **Eliminate redundant RC**: Paired retain/release elimination via dataflow (from Swift)
6. **Reuse analysis**: Dropped constructors reused for new allocations (from Koka FBIP)

This happens on the ARC IR, not on LLVM IR or the expression tree. The ARC IR design follows proven patterns from Lean 4 (LCNF) and Koka, where algorithms are known correct on basic-block form. The codegen layer just emits the pre-computed RC operations.

### 6. Builder That Wraps, Not Implements (from Roc)

Roc's `BuilderExt` pattern: wrap inkwell methods to unwrap Results and add assertions. The V2 `IrBuilder` does the same but also:
- Tracks the current function context
- Manages block creation/positioning
- Handles phi node construction
- Provides scoped alloca management

### 7. Extensibility Through Composition (from Gleam)

Adding a new language feature (e.g., capability effects, async/await) means:
1. Add a `lower_*.rs` module with the lowering logic
2. Add a `TypeInfo` variant for any new types
3. Add ARC IR lowering and `ArcClassification` if the feature introduces reference-counted values
4. Write tests using the codegen test harness

No existing module needs modification. No giant match arms to extend.

## Current Pain Points Addressed

| Pain Point | Root Cause | V2 Solution |
|-----------|-----------|-------------|
| Builder.rs is ~1500 lines (and growing) | Expression codegen + type mapping + LLVM emission all in one struct | Split into TypeInfo enum + ExprLowerer + IrBuilder |
| Adding new types requires changes everywhere | No centralized type-to-LLVM mapping | TypeInfo enum: one variant per type category |
| No ARC optimization | RC ops emitted directly during codegen | Separate ARC analysis pass before codegen |
| LLVM lifetime complexity | `'ll` lifetime threads through everything | ID-based value/type references |
| Difficult to test codegen | Tests require full compilation pipeline | Modular lowering functions testable in isolation |
| Match compilation is fragile | Sequential if-else chains | Decision tree compilation in `ori_arc` (Maranget algorithm, from Roc/Elm) |
| No incremental codegen | Full module recompilation | CGU-like partitioning (from Rust) + content hashing |
| sret threshold is ad-hoc | No systematic ABI handling | TypeInfo-driven calling convention computation |

## Architecture Overview

```
                    ┌─────────────────────────────┐
                    │      ori_arc                 │  Crate 1: ARC Analysis
                    │  (separate crate, no LLVM)   │  (normal workspace member)
                    │                              │
                    │  ArcIrLowering               │  Typed AST → ARC IR (basic blocks)
                    │  PatternCompiler             │  Decision tree → ARC IR blocks
                    │  ArcClassification trait      │  Scalar / PossibleRef / DefiniteRef
                    │  BorrowInference              │  Parameter ownership inference
                    │  RcInserter                   │  Precise inc/dec placement
                    │  RcEliminator                 │  Paired retain/release removal
                    │  ReuseAnalyzer                │  Constructor memory reuse
                    └──────────────┬───────────────┘
                                   │ ArcAnnotatedIR
                    ┌──────────────▼───────────────┐
                    │      ori_llvm                 │  Crate 2: LLVM Codegen
                    │  (excluded from workspace,    │  (requires LLVM)
                    │   requires LLVM feature)      │
                    │                               │
                    │  mod codegen:                  │
                    │    TypeInfo enum + dispatch    │  Type-driven codegen (incl. size/alignment)
                    │    ExprLowerer                │  ARC IR → LLVM IR emission
                    │    AbiComputer                │  Calling conventions
                    │                               │
                    │  mod ir_builder:               │
                    │    IrBuilder                  │  inkwell wrapper + safety
                    │    ValueId / BlockId / etc.   │  ID-based references
                    │                               │
                    │  mod emit:                     │
                    │    ModuleEmitter              │  Module/function lifecycle
                    │    DebugInfoEmitter           │  DWARF generation
                    │    OptPipeline                │  LLVM pass configuration
                    │    ObjectEmitter              │  .o / .bc / .ll emission
                    └───────────────────────────────┘
```

**Coordinator hierarchy:** `ModuleEmitter` orchestrates per-module compilation (verify, optimize, emit). `FunctionCompiler` manages per-function compilation (declare, define). `ExprLowerer` handles per-expression lowering within a function body. `IrBuilder` provides the low-level LLVM instruction API.

## Implementation Tiers

### Tier 1: Foundation (Must Have) -- Complete
- Section 01: TypeInfo enum and core type implementations (in `ori_llvm::codegen`) -- **Complete**
- Section 02: IrBuilder (inkwell wrapper with ID-based references, in `ori_llvm::ir_builder`) -- **Complete**
- Section 03: Expression lowering modules (split from current Builder, in `ori_llvm::codegen`) -- **Complete**
- Section 04: Function declaration and calling conventions -- **Complete**

### Tier 2: ARC (Critical for Correctness) — all in `ori_arc` -- Complete
- Section 05: Type classification and ArcClassification trait (Scalar/Ref) -- **Complete**
- Section 06: ARC IR lowering + borrow inference -- **Complete** (06.0-06.3 all complete, including LLVM wiring)
- Section 07: RC insertion via liveness analysis on ARC IR -- **Complete** (07.1-07.6)
- Section 09: Constructor reuse expansion (expands Reset/Reuse into IsShared + fast/slow paths) -- **Complete** (09.1-09.5)
- Section 08: RC elimination via dataflow on ARC IR -- **Complete** (08.1-08.3)

**ARC pipeline execution order:** 05 → 06 → 07 → 09 → 08. Section numbers indicate topic grouping, not execution order. Section 09 runs before 08 following Lean 4's pipeline design: RC insertion (07) produces Reset/Reuse intermediate operations, constructor reuse expansion (09) expands those into concrete IsShared + Branch + RcInc/RcDec instructions, and then RC elimination (08) runs over the complete set of RcInc/RcDec instructions from both passes.

### Tier 3: Optimization (Performance) -- Complete
- Section 10: Pattern match decision trees -- **Complete** (decision trees in `ori_arc`)
- Section 11: LLVM optimization pass configuration -- **Complete** (ARC-safe attributes)
- Section 12: Incremental/parallel codegen -- **Complete** (ARC IR caching, parallel codegen)

### Tier 4: Polish (Quality of Life) -- Complete
- Section 13: Debug info generation -- **Complete** (DILocalVariable support)
- Section 14: Codegen test harness -- **Complete**
- Section 15: Diagnostics and error reporting -- **Complete** (codegen diagnostics pipeline)

## Implementation Progress

> Last updated: 2026-02-09. ori_arc crate: 279 tests passing. Full test suite: 8,434 passed, 0 failed.

| Section | Status | Implementation |
|---------|--------|----------------|
| 01 | Complete | `ori_llvm` TypeInfo enum |
| 02 | Complete | `ori_llvm` IrBuilder |
| 03 | Complete | `ori_llvm` expression lowering modules |
| 04 | Complete | `ori_llvm` function declarations, ABI, mangling |
| 05 | Complete | `ori_arc/src/classify.rs` — ArcClassifier with caching and cycle detection |
| 06 | Complete | `ori_arc/src/lower/` (06.0), `ori_arc/src/borrow.rs` (06.2), `ori_llvm/src/codegen/abi.rs` + `compile_common.rs` (06.3 — `ParamPassing::Reference`, borrow-aware ABI, pipeline wiring) |
| 07 | Complete | `ori_arc/src/liveness.rs` (07.1), `ori_arc/src/rc_insert.rs` (07.2 + 07.5), `ori_rt` V2 API (07.3), `ori_arc/src/drop.rs` (07.4), `ori_arc/src/reset_reuse.rs` (07.6) |
| 08 | Complete | `ori_arc/src/rc_elim.rs` — bidirectional intra-block dataflow, cascading elimination, 27 tests |
| 09 | Complete | `ori_arc/src/expand_reuse.rs` — two-path expansion, projection-increment erasure, self-set elimination |
| 10 | Complete | `ori_arc/src/lower/` — pattern match decision trees |
| 11 | Complete | `ori_llvm` — LLVM pass pipeline v2 with ARC-safe attributes |
| 12 | Complete | `ori_arc` — incremental ARC IR caching and parallel codegen |
| 13 | Complete | `ori_llvm` — debug info pipeline wiring and DILocalVariable support |
| 14 | Complete | Codegen test harness |
| 15 | Complete | `ori_llvm` — codegen diagnostics pipeline |

## Dependencies

```
ori_arc depends on:  ori_ir (ExprArena, ExprKind, CanExpr, CanArena), ori_types (Pool, Idx)
ori_llvm depends on: ori_arc, ori_ir, ori_types, ori_rt (RC runtime: ori_rc_inc, ori_rc_dec, ori_rc_alloc, ori_rc_free)
ori_canon depends on: ori_ir, ori_types, ori_arc (decision tree compilation)

Tier 1 (ori_llvm foundation) depends on: ori_ir, ori_types
Tier 2 (ori_arc)             depends on: ori_ir, ori_types
Tier 3 (optimization)        depends on: Tier 1 + Tier 2
Tier 4 (polish)              depends on: Tier 1
```

**Note:** `ori_rt` provides the ARC runtime functions (`ori_rc_inc`, `ori_rc_dec`, `ori_rc_alloc`, `ori_rc_free`) that `ori_llvm` emits calls to. The codegen layer references these as external symbols; the runtime library is linked at the final binary stage. The `IsShared` check (refcount > 1) is emitted inline as a load + compare sequence, not as a runtime function call.

## Migration Strategy

The V2 architecture can be built incrementally alongside the existing `ori_llvm`:

1. **Phase 1**: Build `TypeInfo` enum (including size/alignment methods) as a new module inside `ori_llvm` alongside existing code
2. **Phase 2**: Build `ori_arc` crate with ARC IR, classification, and analysis passes (testable independently, no LLVM dependency)
3. **Phase 3**: Build new `IrBuilder` and expression lowering modules inside `ori_llvm`
4. **Phase 4**: Introduce a `codegen_v2` feature flag (compile-time switch). Run test suite against BOTH old and new pipelines in CI. Progressively migrate expression kinds to the new pipeline. Only deprecate the old `Builder` once full test parity is verified across all expression kinds and optimizations.
5. **Phase 5**: Remove old `Builder` code, the `codegen_v2` feature flag, and the existing backend trait abstraction (`BackendTypes`, `TypeMethods`, `BuilderMethods`, `CodegenMethods`) — these are replaced by direct LLVM implementation without the indirection layer (YAGNI — only one backend).

Each phase is independently testable and doesn't break existing functionality.

### eval_v2 Section 07.1 (Complete — 2026-02-09)

The evaluator migration (eval_v2 Section 07.1) made `CanExpr` fully self-contained with zero `ExprArena` back-references. Key changes relevant to LLVM/ARC migration:

- **`CanBindingPattern`**, **`CanParam`**, **`CanNamedExpr`** replace ExprArena-indexed types
- **`FunctionSeq`** desugared to `Block`/`Match` during lowering (no `FunctionSeqId`)
- **`Cast { target: Name }`** replaces `ParsedTypeId` (LLVM backend uses `CanNode.ty` for resolved type)
- **`lower_module()`** canonicalizes all function bodies into one `CanArena` with named roots
- **Multi-clause functions** compiled to synthesized `CanExpr::Match` with decision trees — `Value::MultiClauseFunction` eliminated
- **Decision tree guards** use `CanId` (not `ExprId`)

The LLVM/ARC migration (eval_v2 Section 07.2) can now consume `CanonResult` directly — `ori_arc` needs to update its lowering to read from `CanArena` instead of `ExprArena`.

### Backend Trait Removal (Phase 5)

The existing `ori_llvm` has a trait-based backend abstraction (`BackendTypes`, `TypeMethods`, `BuilderMethods`, `CodegenMethods`) designed for hypothetical multiple backends. Since Ori only targets LLVM for the foreseeable future, V2 removes this indirection entirely. The `ori_llvm` codegen calls inkwell directly through the `IrBuilder` wrapper. If a second backend is ever needed, the abstraction can be reintroduced at that point with actual requirements to guide its design.
