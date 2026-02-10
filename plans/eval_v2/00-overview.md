# Canonical IR & Evaluation Pipeline

> **Status: COMPLETE** — Closed 2026-02-10. All 7 sections done. CanExpr pipeline live in both backends. 8,490 tests passing. Remaining cleanup (ori_eval ExprKind removal, cross-block RC/ASAN) tracked in main roadmap.
>
> **Synthesizes** Roc (Can → Mono, shared backend IR, arena allocation) and Elm (Canonical → Optimized, decision trees, destructor paths) into a unified canonicalization pipeline for Ori. Both `ori_eval` and `ori_arc`/`ori_llvm` consume the same canonical form — new language features are implemented once.

## Motivation

Every new language feature requires dual implementation across `ori_eval` and `ori_llvm`/`ori_arc`. Both backends independently handle all 52 `ExprKind` variants — including 7 sugar variants that require identical non-trivial transformations. This dual implementation creates:

1. **Semantic divergence** — Subtle behavioral differences accumulate between backends
2. **Double implementation cost** — Every feature is written twice, each with its own bugs
3. **LLVM stubs** — Features work in eval but are missing in LLVM (template literals, some spreads)
4. **No shared optimizations** — Constant folding, pattern compilation, desugaring all duplicated

Both Roc and Elm solve this by introducing a canonical IR between the frontend and backends. Complex syntax is reduced to primitive operations once. Backends never see sugar.

## Prior Art Synthesis

| Compiler | Pipeline | What We Adopt |
|----------|----------|---------------|
| **Roc** | Parse → **Can** → Constrain → Solve → **Mono** → Codegen | Arena-allocated canonical IR with IDs; both dev + LLVM backends consume identical Mono IR; explicit captures; `Symbol` (resolved names) |
| **Elm** | Parse → **Canonical** → TypeCheck → Nitpick → **Optimized** → JS | Decision trees baked into Optimized form; `Destructor`/`Path` for pattern access; tail call detection; dead code via dependency graph |
| **Ori** | Parse → TypeCheck → ??? → Eval / LLVM | Ori's type checker already does what Roc's Can and Elm's Canonical do (name resolution, type inference, pattern resolution). What's missing is the **Optimized/Mono** stage. |

### What Comes From Each

**From Roc:**
- `CanExpr` as a proper new type (not in-place mutation) — type-level guarantee that sugar is absent
- `CanArena` with arena allocation and `CanId` indices (following Ori's existing arena patterns)
- Both backends consume the **same** IR (Roc's key architectural win)
- Explicit type information attached to expressions (resolved, not inferred)
- Pattern compilation producing typed nodes shared across backends

**From Elm:**
- Decision trees baked into Match nodes (not a side table) — `Decider` with `Leaf`/`Chain`/`FanOut`
- `Destructor`/`Path` for pattern variable binding (root → field → index navigation)
- Clean phase invariants (Canonical guarantees X, Optimized guarantees Y)
- `Constant(Value)` nodes for folded expressions — backends skip evaluation/emission entirely

**Evaluation improvements:**
- `EvalMode` enum (Section 05) — Interpret/ConstEval/TestRun with match dispatch, Salsa-compatible
- Structured `EvalErrorKind` + `EvalBacktrace` (Section 06) — typed errors with call stacks
- `EvalCounters` for `--profile` instrumentation (Section 06.4)

## Architecture

### Pipeline (Post-Canon)

```
Source → Lex → Parse → Type Check → Canonicalize ─┬─→ ori_eval  (interprets CanExpr)
                                      (NEW)        └─→ ori_arc   (lowers CanExpr to ARC IR → ori_llvm)
```

### Canonical IR Type

A proper new type — not in-place `ExprArena` mutation. Sugar variants **cannot be represented** in `CanExpr`. This is the Roc/Elm approach: the type system enforces the invariant.

```rust
/// Canonical expression node — sugar-free, type-annotated, pattern-compiled.
///
/// This is NOT ExprKind with variants removed. It is a distinct type with
/// distinct semantics. Backends pattern-match on CanExpr exhaustively —
/// no unreachable!() arms, no sugar handling.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CanExpr {
    // === Literals ===
    Int(i64),
    Float(u64),
    Bool(bool),
    Str(Name),
    Char(char),
    Duration { value: u64, unit: DurationUnit },
    Size { value: u64, unit: SizeUnit },
    Unit,

    // === Compile-Time Constant (folded during canonicalization) ===
    Constant(ConstantId),  // Index into constant pool

    // === References ===
    Ident(Name),
    Const(Name),
    SelfRef,
    FunctionRef(Name),
    HashLength,

    // === Operators ===
    Binary { op: BinaryOp, left: CanId, right: CanId },
    Unary { op: UnaryOp, operand: CanId },
    Cast { expr: CanId, target: Name, fallible: bool },  // target is type name, not ParsedTypeId

    // === Calls (always positional — named args already reordered) ===
    Call { func: CanId, args: CanRange },
    MethodCall { receiver: CanId, method: Name, args: CanRange },

    // === Access ===
    Field { receiver: CanId, field: Name },
    Index { receiver: CanId, index: CanId },

    // === Control Flow ===
    If { cond: CanId, then_branch: CanId, else_branch: CanId },
    Match { scrutinee: CanId, decision_tree: DecisionTreeId, arms: CanRange },
    For { binding: Name, iter: CanId, guard: CanId, body: CanId, is_yield: bool },
    Loop { body: CanId },
    Break(CanId),
    Continue(CanId),

    // === Bindings ===
    Block { stmts: CanRange, result: CanId },
    Let { pattern: CanBindingPatternId, init: CanId, mutable: bool },  // self-contained
    Assign { target: CanId, value: CanId },

    // === Functions ===
    Lambda { params: CanParamRange, body: CanId },  // self-contained, no ParsedTypeId

    // === Collections (no spread variants — already expanded) ===
    List(CanRange),
    Tuple(CanRange),
    Map(CanMapEntryRange),
    Struct { name: Name, fields: CanFieldRange },
    Range { start: CanId, end: CanId, step: CanId, inclusive: bool },

    // === Algebraic ===
    Ok(CanId),
    Err(CanId),
    Some(CanId),
    None,

    // === Error Handling ===
    Try(CanId),
    Await(CanId),

    // === Capabilities ===
    WithCapability { capability: Name, provider: CanId, body: CanId },

    // === Special Forms (self-contained — no ExprArena back-references) ===
    FunctionExp { kind: FunctionExpKind, props: CanNamedExprRange },  // inlined, no FunctionExpId

    // === Error Recovery ===
    Error,
}
```

**What's different from ExprKind:**
- No `CallNamed` / `MethodCallNamed` — desugared to positional `Call` / `MethodCall`
- No `TemplateLiteral` / `TemplateFull` — desugared to `Str` + `.to_str()` + `.concat()` chains
- No `ListWithSpread` / `MapWithSpread` / `StructWithSpread` — desugared to `List`/`Map`/`Struct` + method calls
- No `FunctionSeq` — desugared to `Block`/`Match` during lowering
- No `ParsedTypeId` — resolved types on `CanNode.ty`, cast targets use `Name`
- No `BindingPatternId` — replaced by self-contained `CanBindingPatternId`
- No `ParamRange` — replaced by self-contained `CanParamRange`
- No `FunctionExpId` — inlined as `FunctionExp { kind, props: CanNamedExprRange }`
- Added `Constant(ConstantId)` — compile-time-folded values
- Added `DecisionTreeId` on Match — patterns pre-compiled to decision trees
- Uses `CanId` / `CanRange` (not `ExprId` / `ExprRange`) — distinct index space
- **Fully self-contained** — zero `ExprArena` back-references; evaluator and LLVM backend need only `CanonResult`

### Crate Structure

```
ori_ir (shared types — no logic, depended on by everyone)
  ast/expr.rs         — ExprKind, ExprArena, ExprId (parse AST — unchanged)
  canon/mod.rs        — CanExpr, CanArena, CanId, CanRange (canonical IR — NEW)
  canon/tree.rs       — DecisionTree, ScrutineePath, TestKind, TestValue (MOVED from ori_arc)

ori_canon (NEW crate — lowering logic)
  lower.rs            — ExprArena + TypeCheckResult → CanArena
  desugar.rs          — Sugar elimination during lowering
  patterns.rs         — Pattern → DecisionTree compilation (Maranget algorithm)
  const_fold.rs       — Constant folding during lowering
  validate.rs         — Debug assertions on canonical invariants

ori_eval (interpreter — consumes CanExpr)
  interpreter/        — Rewritten to dispatch on CanExpr (not ExprKind)
  machine.rs          — EvalMode enum (Interpret/ConstEval/TestRun)
  diagnostics.rs      — EvalErrorKind, EvalBacktrace, EvalCounters

ori_arc (ARC analysis — consumes CanExpr)
  lower/              — CanExpr → ARC IR basic blocks (instead of ExprKind → ARC IR)
  decision_tree/      — emit.rs (emit decision trees as ARC IR blocks — types moved to ori_ir)
```

**Dependency resolution:** Decision tree types (`DecisionTree`, `ScrutineePath`, `TestKind`, `TestValue`, `FlatPattern`, `PatternRow`, `PatternMatrix`) move from `ori_arc` to `ori_ir/canon/tree.rs`. This breaks the circular dependency: `ori_canon` needs decision tree types to build them, `ori_arc` needs them to emit ARC IR from them. With types in `ori_ir`, both crates depend on `ori_ir` (which they already do).

**Key architectural deviation:** `CanNode` uses `TypeId` (from `ori_ir`) instead of `Idx` (from `ori_types`), because `ori_ir` cannot depend on `ori_types`. Both share the same u32 index layout, so conversion is free.

### Salsa Integration

```rust
/// Salsa query: canonicalize a module.
/// This is the single point where both backends diverge from raw parse output.
#[salsa::tracked]
fn canonicalize(db: &dyn Db, module: Module) -> CanonResult {
    let parse_result = parse(db, module);
    let type_result = type_check(db, module);
    ori_canon::lower(parse_result, type_result)
}
```

---

## Section Overview

| Section | Title | Focus | Est. Lines | Status |
|---------|-------|-------|-----------|--------|
| 01 | Canonical IR | `CanExpr`, `CanArena`, `CanId` types + decision tree type relocation | ~600 | ✅ Complete |
| 02 | AST Lowering | `ExprArena → CanArena` transformation (all 52 variants) | ~2,000 | ✅ Complete |
| 03 | Pattern Compilation | Decision trees via Maranget, baked into canonical form | ~800 | ✅ Complete |
| 04 | Constant Folding | Compile-time evaluation during lowering | ~500 | ✅ Complete |
| 05 | Evaluation Modes | `EvalMode` enum — Interpret/ConstEval/TestRun | ~500 | ✅ Complete |
| 06 | Structured Diagnostics | `EvalErrorKind`, backtraces, `EvalCounters`, `--profile`, `ControlAction` | ~800 | ✅ Complete |
| 07 | Backend Migration | Rewrite eval + LLVM to consume `CanExpr`; delete old dispatch | ~2,500 | ✅ Complete |

**Total: ~7,700 lines**

---

## Dependency Graph

```
Section 01 (Canonical IR types)
    ↓
Section 02 (AST Lowering) ←─── Section 03 (Pattern Compilation)
    ↓                           Section 04 (Constant Folding)
    ↓
Section 05 (Evaluation Modes)  ← independent, can proceed after 01
Section 06 (Structured Diagnostics) ← independent, can proceed after 01
    ↓
Section 07 (Backend Migration) ← depends on ALL above
```

**Critical path**: 01 → 02 → 07
**Parallelizable**: 03, 04, 05, 06 can all proceed in parallel after Section 01

---

## Migration Strategy

1. **Section 01** ✅: Define types in `ori_ir`. Move decision tree types from `ori_arc` to `ori_ir`. Create `ori_canon` crate skeleton. No behavioral changes. *Completed 2026-02-09.*
2. **Section 02** ✅: Implement lowering pass. `ExprArena → CanArena` for all 52 variants (7 desugared, 44 mapped, 1 error). 14 unit tests. Sugar elimination in `desugar.rs`. Round-trip integration testing deferred to Section 07. *Completed 2026-02-09.*
3. **Sections 03-04** ✅: Pattern compilation and constant folding integrated into the lowering pass. Decision trees stored in `DecisionTreePool`. Constants stored in `ConstantPool`. Decision tree walker in `ori_eval` ready for Section 07 wiring. *Completed 2026-02-09.*
4. **Section 05** ✅: `EvalMode` enum (Interpret/ConstEval/TestRun) with policy methods, `ModeState` for budget tracking, `PrintHandlerImpl::Silent` for const-eval, unified recursion limits (removed `#[cfg]` duplication). All construction sites specify mode. Test runner uses `TestRun` mode. *Completed 2026-02-09.*
4b. **Section 06** ✅: Structured diagnostics — `EvalErrorKind` (24 variants), `CallStack` replacing `call_depth`, `EvalBacktrace` for error context, `EvalCounters` for `--profile`, `eval_error_to_diagnostic()` with E6xxx error codes. `EvalErrorSnapshot` preserves full error context at Salsa boundary. `snapshot_to_diagnostic()` enriches backtraces with file:line:col. `--profile` CLI flag wired with counter report. Counter increments at `eval_inner()`, `eval_call()`, `eval_method_call()`, `eval_match()`. `ControlAction` refactor: control flow signals (Break/Continue/Propagate) extracted from `EvalError` into first-class enum; `EvalResult = Result<Value, ControlAction>`. *Complete 2026-02-09.*
5. **Section 07.1** ✅: Evaluator migration complete. Self-contained canonical IR (zero ExprArena back-references). `eval_can()` dispatches exhaustively on `CanExpr`. `lower_module()` canonicalizes all function bodies. Multi-clause functions compiled to decision trees at lowering time (`Value::MultiClauseFunction` eliminated). `FunctionValue` carries `SharedCanonResult`. All 8434 tests pass. *Completed 2026-02-09.*
6. **Section 07.2** ✅: LLVM/ARC migration. Both backends now consume `CanExpr` exclusively (~500+ lines of sugar dispatch deleted). `invoke`/`landingpad` wired for user-defined calls with `rust_eh_personality`. All 8434 tests pass. *Completed 2026-02-10.*
7. **Section 07.3-07.5**: Dead code removal and cross-backend sync verification.

At every step, `./test-all.sh` must pass. Section 07.1 proved the canonical IR correct end-to-end in the evaluator. Section 07.2 is mechanical — `ori_arc` already consumes the same decision tree types from `ori_ir`.

---

## Success Criteria

1. **Type-level sugar guarantee** — `CanExpr` has no sugar variants. Backends exhaustively match without `unreachable!()`
2. **Shared pattern compilation** — Decision trees compiled once in `ori_canon`, consumed by both `ori_eval` and `ori_arc`
3. **Shared constant folding** — Compile-time-known expressions pre-evaluated, both backends skip them
4. **Evaluation modes** — `ori run`, `ori check`, `ori test` use distinct `EvalMode` variants
5. **Structured errors** — Runtime errors have typed kinds, backtraces, and context notes
6. **All tests pass** — `./test-all.sh` green throughout migration
7. **New feature cost halved** — Sugar features need ONE implementation (in `ori_canon/desugar.rs`)
8. **Net negative LOC in backends** — More code deleted from dual-implementation than added in `ori_canon`

---

## References

| Document | Purpose |
|----------|---------|
| `plans/eval_v2/section-*.md` | Detailed section plans |
| `plans/eval_v2/index.md` | Keyword-based section discovery |
| `plans/llvm_v2/section-10-decision-trees.md` | Decision tree types + algorithm (relocated to `ori_ir`) |
| `plans/llvm_v2/00-overview.md` | LLVM V2 / `ori_arc` architecture |
| `compiler/ori_ir/src/ast/expr.rs` | `ExprKind` (52 variants — source for lowering) |
| `compiler/ori_eval/` | Current evaluator (migration target) |
| `compiler/ori_llvm/` + `compiler/ori_arc/` | LLVM backend (migration target) |
| Reference: Roc `crates/compiler/can/src/expr.rs` | Roc canonical IR |
| Reference: Roc `crates/compiler/mono/src/ir.rs` | Roc mono IR (shared by both backends) |
| Reference: Elm `compiler/src/AST/Canonical.hs` | Elm canonical AST |
| Reference: Elm `compiler/src/AST/Optimized.hs` | Elm optimized AST (decision trees, destructors) |
