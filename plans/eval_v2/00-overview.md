# Eval V2: A Best-of-Breed Evaluator Architecture for Ori

> **Design Plan** — Synthesizes patterns from Rust (CTFE/Miri), Go, Zig, Gleam, Elm, Roc, and TypeScript into a novel, highly modular eval system for Ori.

## Motivation

The current Ori evaluator (`ori_eval`) is a functional tree-walking interpreter (~15K lines) with Arc-enforced values, chain-of-responsibility method dispatch, and Salsa integration. While it works, it has several architectural limitations that will compound as the language grows:

1. **Monolithic value type** — `Value` enum with 30 variants, all heap types wrapped in `Arc`
2. **Error-based control flow** — `break`/`continue` propagated as `EvalError` variants
3. **No constant folding** — All expressions evaluated at runtime, even compile-time-known ones
4. **No pattern compilation** — Match arms tested sequentially, not compiled to decision trees
5. **Single evaluation mode** — Same code path for `ori run`, `ori check`, and test execution
6. **Thread-unsafe scope chain** — `Rc<RefCell<T>>` in Environment prevents per-scope parallelism. Other subsystems (captures, print handlers, test runner) already use Arc-based patterns.
7. **No IR lowering** — Evaluates directly from the parse AST (ExprArena)
8. **Weak integration boundary** — Type checker provides only `Idx[]` and pattern resolutions

## Design Philosophy

> **"Each compiler taught us something different. The goal is not to copy any one, but to synthesize a design that is uniquely suited to Ori's expression-based, capability-tracked, ARC-managed paradigm."**

### Key Principles

1. **Machine-parametric evaluation** (from Rust): One core interpreter, multiple evaluation policies
2. **Interned value pool** (from Zig): Common values stored once, referenced by index
3. **Progressive lowering** (from Elm/Roc): AST → Canonical IR → Eval IR → Codegen IR
4. **Decision tree patterns** (from Gleam/Elm): Compile match arms, don't interpret them
5. **Structured control flow** (from Roc): Join points, not error-based break/continue
6. **Perceus-inspired RC** (from Roc/Swift): Reference counting with reuse analysis
7. **Fact-based narrowing** (from TypeScript): Efficient type refinement through control flow
8. **Arena-first allocation** (from Zig/Roc): Temporary structures in arenas, intern for permanence

## Prior Art Synthesis

| Pattern | Source | Ori Adaptation |
|---------|--------|----------------|
| Machine trait | Rust CTFE | `EvalMode` enum — Interpret, ConstEval, TestRun (enum dispatch) |
| InternPool | Zig | `ValuePool` — intern constants, small values, type refs |
| Progressive AST lowering | Elm (3-tier), Roc (can→mono) | `ExprArena` → `EvalIR` (new canonical IR) |
| Decision tree matching | Gleam, Elm | `PatternCompiler` producing `DecisionTree` |
| Join points | Roc mono IR | `JoinPoint` in EvalIR for break/continue/early-exit |
| Perceus RC | Roc, Koka | `RcInsertion` pass on EvalIR |
| Operand modes | Go | `EvalContext` tracking const/runtime/builtin |
| Fact-based narrowing | TypeScript | `TypeFacts` bit flags for efficient refinement |
| Lambda sets | Roc | `ClosureLayout` for closure specialization |
| CPS unification | Elm | Already in ori_types — extend for eval integration |
| Representation promotion | Go constants | `ValueRepr` auto-promotes small→big |
| Arena allocation | Zig, Roc (bumpalo) | `EvalArena` for temporary eval structures |
| Trampoline pattern | TypeScript | For deeply nested binary expressions |
| Memoized function calls | Zig InternPool | `MemoCache` keyed on (func, args) |
| Error recovery expressions | Gleam | `EvalIR::Invalid` for continued eval after errors |
| Variant inference | Gleam | `InferredVariant` on pattern match results |
| Graph-based DCE | Elm | Dead code elimination in EvalIR |
| Lazy string concat | Go | `LazyStr` for compile-time string building |
| Static init scheduling | Go | Separate constant vs. runtime initialization |

---

## Architecture Overview

### Crate Structure (Post-V2)

```
oric (CLI, Salsa queries, module loading)
  ↓
ori_eval (evaluation core — enum-dispatch EvalMode)
  ├── machine.rs          — EvalMode enum (Interpret, ConstEval, TestRun) + match dispatch
  ├── interpreter.rs      — Tree-walking eval
  ├── const_eval.rs       — Compile-time evaluation
  ├── value/
  │   ├── pool.rs         — ValuePool (interned constants)
  │   └── heap.rs         — Heap<T> (Arc-enforced, unchanged API)
  ├── env/
  │   ├── scope.rs        — ScopeStack (replaces Environment)
  │   ├── binding.rs      — Binding types with mutability
  │   └── capture.rs      — Closure capture logic
  ├── pattern/
  │   ├── compiler.rs     — Decision tree compiler
  │   ├── decision.rs     — DecisionTree types
  │   └── exhaustive.rs   — Integration with exhaustiveness checker
  ├── control/
  │   ├── flow.rs         — Structured control flow (no error-based)
  │   ├── join.rs         — Join points for break/continue
  │   └── try_op.rs       — Try operator (`?`) handling
  ├── method/
  │   ├── resolver.rs     — Hash-based method resolution
  │   ├── builtin.rs      — Builtin method registry
  │   └── dispatch.rs     — Dispatch logic (replaces chain)
  ├── exec/
  │   ├── expr.rs         — Expression evaluation
  │   ├── call.rs         — Function call logic
  │   ├── operators.rs    — Binary/unary operators
  │   └── collections.rs  — List/map/tuple operations
  ├── ir/                    — Canonical evaluation IR (sub-module, not separate crate)
  │   ├── ir.rs              — EvalIR enum (lowered from ExprArena)
  │   ├── lower.rs           — ExprArena → EvalIR lowering
  │   ├── optimize.rs        — Constant folding, dead code elimination
  │   ├── rc.rs              — Reference counting insertion
  │   └── arena.rs           — Arena for EvalIR nodes
  └── diag/
      ├── backtrace.rs    — Eval backtrace capture
      └── tracing.rs      — Tracing integration
      (Note: EvalError/EvalResult remain in ori_patterns to avoid circular deps)
  ↓
ori_patterns (pattern traits, implementations, and eval error types — references ori_value)
  ├── registry.rs         — PatternRegistry (unchanged)
  ├── errors.rs           — EvalError, EvalResult (remain here to avoid circular deps)
  └── patterns.rs         — Pattern-specific logic
  ↓
ori_value (Value enum, heap, composite types — used by eval, codegen, patterns)
  ├── value/mod.rs        — Value enum (canonical location)
  ├── value/repr.rs       — Value representation
  ├── value/heap.rs       — Heap<T> type
  ↓
ori_types (type checking infrastructure — no Value dependency)
  ↓
ori_ir / ori_diagnostic (unchanged)
```

### Evaluation Pipeline (Post-V2)

```
Source → Lexer → Parser → Type Checker ──→ EvalIR Lowering ──→ Evaluation
                              ↓                   ↓                ↓
                         TypeCheckResult     EvalIR + Opts    Value/EvalError
                         (types, patterns)   (const-folded,   (result)
                                              RC-annotated)
```

### Phase Boundaries

```
Phase 1: Parse          │ ExprArena (52 ExprKind variants), MatchPatterns, FunctionSeq/Exp
Phase 2: Type Check     │ TypeCheckResult (expr_types, pattern_resolutions, FunctionSig, capabilities)
Phase 3: IR Lower       │ EvalIR (desugared, patterns compiled, type info consumed)    ← NEW
Phase 4: Optimize       │ EvalIR (constant-folded, dead code removed)                  ← NEW
Phase 5: RC Insert      │ EvalIR (with Rc(RcOp) annotations — Inc/Dec/Free/Reset/Reuse) ← NEW
Phase 6: Evaluate       │ Value (using enum-dispatch EvalMode interpreter on EvalIR only)
```

### Upstream Feature Coverage

The lowerer must handle **every** feature the parser and type checker produce:
- All 52 ExprKind variants (lowered to EvalIR nodes or desugared)
- All 10 MatchPattern variants (compiled to decision trees)
- FunctionSeq patterns (run, try, match, for)
- FunctionExp patterns (cache, parallel, spawn, timeout, recurse, with, print, panic, catch, todo, unreachable)
- Template literals with interpolation and format specs
- Named arguments (reordered using FunctionSig)
- Spread operators (desugared to collection operations)
- Pipeline operator (desugared to nested calls)
- Cast expressions (using type Idx from type checker)
- Per-expression types from `expr_types` (for casts, type-dependent dispatch)
- Pattern resolutions from type checker (variant vs variable disambiguation)
- Capability declarations from FunctionSig (for effect checking)

---

## Section Overview

### Tier 0: Foundation (Sections 1-3)

Core infrastructure that everything else depends on.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 1 | Value System V2 | Zig InternPool, Rust Immediate, Go promotion |
| 2 | Machine Abstraction | Rust Machine trait, Go operand modes |
| 3 | Environment V2 | Current + thread-safety + RAII improvements |

### Tier 1: Evaluation Core (Sections 4-6)

The heart of the new evaluator.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 4 | Pattern Compilation | Gleam/Elm decision trees |
| 5 | Structured Control Flow | Roc join points, Go static scheduling |
| 6 | Method Resolution V2 | Current chain → hash-based dispatch |

### Tier 2: Optimization (Sections 7-8)

Compile-time evaluation and IR lowering.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 7 | Constant Evaluation | Zig comptime, Go constants, Rust CTFE |
| 8 | Canonical Eval IR | Roc can→mono, Elm 3-tier AST |

### Tier 3: Memory & Diagnostics (Sections 9-10)

Reference counting and observability.

| Section | Focus | Inspired By |
|---------|-------|-------------|
| 9 | Reference Counting Integration | Roc Perceus, Swift ARC, Koka FBIP |
| 10 | Tracing & Diagnostics V2 | Rust backtrace, Elm error reporting |

---

## Dependency Graph

```
Section 1 (Values) ──→ Section 2 (Machine) ──→ Section 3 (Environment)
    ↓                      ↓                        ↓
    │                      │          ┌──────────────┼──────────────┐
    │                      │          ↓              ↓              ↓
    │                      │   Section 4 (Patterns) Section 5 (Control Flow) Section 6 (Methods)
    │                      │          │              │
    │                      │          │              ↓
    │                      │          │   Section 7 (Const Eval)
    │                      │          │              │
    │                      │          ↓              ↓
    │                      └────→ Section 8 (Eval IR) ←── Section 4, 5, 7
    │                                    │
    │                          ┌─────────┼─────────┐
    │                          ↓                   ↓
    │                   Section 9 (RC)      Section 10 (Diagnostics)
    │                          ↑
    └──────────────────────────┘
```

**Edges:**
- 1 → 2 → 3 (foundation chain)
- 3 → 4, 3 → 5, 3 → 6 (tier 1 depends on environment)
- 5 → 7 (const eval uses control flow types)
- 4 → 8, 5 → 8, 7 → 8 (IR lowering consumes patterns, control flow, const eval)
- 2 → 8 (IR lowering uses EvalMode)
- 8 → 9 (RC insertion operates on EvalIR)
- 8 → 10 (diagnostics uses EvalIR types)
- 1 → 9 (RC needs ValuePool/ValueId for interned value classification)
- 5 → 8 (EvalFlow/FlowOrError used by interpreter on EvalIR)

**Critical Path**: 1 → 2 → 3 → 5 → 7 → 8 (must be sequential)
**Parallelizable**: 4 and 6 can proceed independently after 3; 9 and 10 after 8

---

## Migration Strategy

The eval_v2 is a **complete replacement** of the current eval system. Each phase delivers working code, but the end goal is full transition — no compatibility layers, no dual paths, no legacy code left behind.

1. **Phase 0** (Pre-requisite): Decompose `ori_patterns` crate. Value, Heap, and composite types move to new `ori_value` crate. EvalError and EvalResult remain in `ori_patterns` (moving them to `ori_eval` would create a circular dependency since `ori_eval` depends on `ori_patterns`). Pattern traits/implementations remain in `ori_patterns`, updated to reference `ori_value` for value types. All downstream imports updated.
2. **Phase 1** (Sections 1-3): New value pool and machine abstraction replace current internals. Old factory methods and unparameterized interpreter removed.
3. **Phase 2** (Sections 4-6): Pattern compiler and method resolver fully replace current implementations. Chain-of-responsibility dispatcher deleted.
4. **Phase 3** (Sections 7-8): EvalIR becomes the sole evaluation path. ExprArena direct evaluation removed from interpreter. All evaluation goes through lowering.
5. **Phase 4** (Sections 9-10): RC insertion and diagnostics complete the system.

At each phase, `./test-all.sh` must pass. Old code is deleted as each replacement is validated — not kept as fallback.

---

## Success Criteria

The eval_v2 plan is complete when:

1. **All tests pass** — `./test-all.sh` green throughout migration
2. **Machine modes** — `ori run`, `ori check --no-test`, and `ori test` use distinct EvalMode enum variants
3. **Constant folding** — Compile-time-known expressions evaluated during IR lowering
4. **Decision trees** — Match expressions compiled, not sequentially interpreted
5. **Structured control flow** — No `EvalError` variants for break/continue
6. **Thread-safe option** — Environment can optionally use `Arc<Mutex<T>>`
7. **Performance** — No regression on existing benchmarks; match expressions measurably faster
8. **Diagnostics** — Eval errors include backtraces and structured span information
9. **No legacy code** — Old eval_inner dispatch, chain-of-responsibility dispatcher, and ExprArena direct evaluation are fully removed

---

## References

| Document | Purpose |
|----------|---------|
| `plans/eval_v2/section-*.md` | Detailed section plans |
| `plans/eval_v2/index.md` | Keyword-based section discovery |
| `compiler/ori_eval/` | Current evaluator (migration source) |
| `compiler/ori_patterns/` | Current value types and patterns (Value, Heap move to ori_value in Phase 0) |
| `docs/ori_lang/0.1-alpha/spec/` | Language specification (authoritative) |
| `~/projects/reference_repos/lang_repos/` | Reference compiler implementations |
