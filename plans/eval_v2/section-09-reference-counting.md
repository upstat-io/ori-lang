---
section: "09"
title: Reference Counting Integration
status: not-started
goal: Define the interpreter's RC strategy and the bridge to ori_arc's ARC IR for LLVM codegen
sections:
  - id: "09.1"
    title: Architecture Overview
    status: not-started
  - id: "09.2"
    title: Interpreter RC Strategy
    status: not-started
  - id: "09.3"
    title: "EvalIR → ARC IR Bridge"
    status: deferred
    note: "Deferred pending LLVM backend validation"
  - id: "09.4"
    title: Runtime RC Instrumentation
    status: not-started
---

# Section 09: Reference Counting Integration

**Status:** Planned
**Goal:** Define how the interpreter handles reference counting (simple, via Rust's `Arc`), how the LLVM backend leverages `ori_arc` for explicit Perceus-style RC, and the bridge between the two paths.
**Dependencies:** Section 08 (Canonical EvalIR), `ori_arc` crate (existing — provides ARC IR, borrow inference, liveness analysis, RC insertion, reset/reuse detection)

---

## Prior Art Analysis

### Roc: Perceus-Style Reference Counting
Roc implements the **Perceus** algorithm (Microsoft Research, 2021) which inserts `Inc`/`Dec` operations into the mono IR. Key innovations:
- **Reuse analysis**: When deconstructing a value and constructing a new one of the same size, the memory can be reused (`Reset`/`Reuse`)
- **Borrow analysis**: Determines which operations need to increment vs. can borrow
- **Drop specialization**: Custom destructors that skip recursion for non-reference-counted fields

### Swift: Automatic Reference Counting
Swift inserts `retain`/`release` calls during SIL (Swift Intermediate Language) optimization. Key techniques:
- **Copy-on-Write**: Shared values are copied before mutation
- **Guaranteed ownership**: Parameters can be `@guaranteed` (borrowed, no RC needed)
- **Owned vs. borrowed parameter conventions**: Callee decides whether to consume or borrow

### Lean 4: RC with Reset/Reuse
Lean's IR has explicit `RC.inc`, `RC.dec`, `reset`, and `reuse` instructions. The `ExpandResetReuse.lean` pass converts high-level reuse annotations into concrete memory operations.

### Koka: FBIP (Functional But In Place)
Koka's compiler analyzes when functional updates can be done in-place by checking if the value's reference count is 1. The `CheckFBIP.hs` pass verifies this property.

---

## Existing Infrastructure: `ori_arc`

The `compiler/ori_arc/` crate already provides complete, tested implementations of the Perceus-style ARC analysis pipeline operating on a basic-block IR. Section 09 does **not** propose building these from scratch. Instead, it defines how the interpreter and LLVM codegen each handle RC, and how they connect.

### What `ori_arc` provides

| Module | Public API | Purpose |
|--------|-----------|---------|
| `ori_arc::ir` | `ArcFunction`, `ArcBlock`, `ArcInstr`, `ArcTerminator`, `ArcVarId`, `ArcBlockId` | Basic-block IR for ARC analysis |
| `ori_arc::classify` | `ArcClassifier`, `ArcClassification` trait, `ArcClass` enum | Type classification: `Scalar` / `DefiniteRef` / `PossibleRef` |
| `ori_arc::ownership` | `Ownership` (`Owned` / `Borrowed`), `AnnotatedParam`, `AnnotatedSig` | Ownership annotations for parameters |
| `ori_arc::borrow` | `infer_borrows()`, `apply_borrows()` | Iterative fixed-point borrow inference (Lean 4 style) |
| `ori_arc::liveness` | `compute_liveness()`, `BlockLiveness`, `LiveSet` | Backward dataflow liveness analysis |
| `ori_arc::rc_insert` | `insert_rc_ops()` | Perceus RC insertion: `RcInc`/`RcDec` based on liveness |
| `ori_arc::reset_reuse` | `detect_reset_reuse()` | Reset/Reuse optimization: reuse memory of deconstructed values |
| `ori_arc::expand_reuse` | `expand_reset_reuse()` | Expand `Reset`/`Reuse` intermediates into `IsShared` + conditional fast/slow paths |
| `ori_arc::drop` | `compute_drop_info()`, `compute_closure_env_drop()`, `collect_drop_infos()`, `DropInfo`, `DropKind` | Drop descriptor generation for recursive field release |
| `ori_arc::lower` | `lower_function()`, `ArcIrBuilder`, `ArcLowerer`, `ArcScope`, `ArcProblem` | AST → ARC IR lowering (builder follows LLVM `IRBuilder` pattern) |

### What `ori_arc` does NOT provide (gaps for Section 09)

- **Interpreter RC strategy**: The interpreter uses Rust's `Arc` for implicit RC — no explicit `RcInc`/`RcDec`. Section 09.2 defines this.
- **EvalIR → ARC IR bridge**: The lowering path from tree-structured EvalIR (Section 08) to basic-block ARC IR is not yet defined. `ori_arc::lower` currently lowers from the typed AST (`ExprArena`/`ExprId`), not from EvalIR. Section 09.3 defines this bridge.
- **Runtime RC instrumentation**: Debug-mode leak detection, `EvalCounters.heap_allocations` tracking. Section 09.4 defines this.

---

## 09.1 Architecture Overview

Two compilation paths handle reference counting differently:

```
ExprArena (typed AST)
    │
    ├──→ EvalIR (Section 08) ──→ Tree-walking interpreter
    │         │                   (implicit RC via Rust Arc)
    │         │
    │         └──→ [future: EvalIR → ARC IR bridge (09.3)]
    │
    └──→ ARC IR (ori_arc::lower) ──→ ori_arc pipeline ──→ LLVM codegen
              │                       (explicit RC: RcInc/RcDec)
              ├── infer_borrows()
              ├── compute_liveness()
              ├── insert_rc_ops()
              ├── detect_reset_reuse()
              └── expand_reset_reuse()
```

**Interpreter path (EvalIR)**: Uses Rust's `Arc<T>` for heap-allocated values (`Heap<T>` wrapper). `Arc::clone()` is the increment, `Arc::drop()` is the decrement. No explicit RC operations needed — the Rust runtime handles it. This is simple, correct, and sufficient for the interpreter.

**LLVM codegen path (ARC IR)**: Uses `ori_arc`'s complete Perceus pipeline. The AST is lowered to basic-block ARC IR via `ori_arc::lower::lower_function()`. Then:
1. `ori_arc::borrow::infer_borrows()` — determines which parameters can be borrowed
2. `ori_arc::liveness::compute_liveness()` — backward dataflow liveness analysis
3. `ori_arc::rc_insert::insert_rc_ops()` — inserts `ArcInstr::RcInc`/`ArcInstr::RcDec`
4. `ori_arc::reset_reuse::detect_reset_reuse()` — replaces `RcDec`+`Construct` with `Reset`+`Reuse`
5. `ori_arc::expand_reuse::expand_reset_reuse()` — expands `Reset`/`Reuse` intermediates into `IsShared` + conditional fast/slow paths

**Key design decision**: EvalIR is optimized for tree-walking interpretation. ARC IR is optimized for LLVM codegen with explicit RC. These are **separate IRs** with a lowering step between them. Borrow analysis, liveness analysis, and RC insertion operate on ARC IR's basic blocks — they do NOT operate on EvalIR's tree structure.

### Type classification: compile-time vs runtime

`ori_arc` provides compile-time type classification via `ArcClassifier::needs_rc(Idx)`, which operates on type pool indices. The interpreter uses a simpler runtime check: only `Heap<T>`-wrapped `Value` variants need RC. These are complementary:

- **Compile-time** (`ori_arc::classify`): `ArcClassifier::needs_rc(idx: Idx) -> bool` — classifies types by pool index. Used by ARC IR passes to decide where to insert RC operations. Three-way: `Scalar` (no RC), `DefiniteRef` (always RC), `PossibleRef` (conservative, pre-mono).
- **Runtime** (interpreter): A value needs RC if it is wrapped in `Heap<T>` (i.e., backed by `Arc<T>`). Simple pattern match on `Value` variant. This is not a new implementation task — it's how the interpreter already works.

### Ownership model

`ori_arc::ownership::Ownership` has two variants:
- `Owned` — callee may retain; caller must `RcInc` before passing
- `Borrowed` — callee will not retain; no `RcInc` needed

There is no `Stack` variant. Scalar values (int, float, bool, etc.) are filtered out by `ArcClassifier::needs_rc()` — they are simply skipped by all RC passes. The classification step (`Scalar` / `DefiniteRef` / `PossibleRef`) replaces the need for a separate `Stack` ownership category.

- [ ] Document the two-path architecture (interpreter + LLVM)
- [ ] Document `ori_arc`'s existing pipeline and where it fits
- [ ] Clarify that borrow/liveness/RC analysis operates on ARC IR basic blocks, not EvalIR trees

---

## 09.2 Interpreter RC Strategy

The tree-walking interpreter handles reference counting **implicitly** through Rust's `Arc<T>`. This is deliberately simpler than `ori_arc`'s Perceus approach because the interpreter does not need the performance optimizations that explicit RC provides.

### How it works

```rust
// Values that need RC are wrapped in Heap<T> (which wraps Arc<T>):
pub struct Heap<T>(Arc<T>);

// Arc::clone() is the increment (implicit)
let shared = value.clone(); // Arc refcount: 1 → 2

// Arc::drop() is the decrement (implicit)
drop(shared); // Arc refcount: 2 → 1

// When refcount reaches 0, Arc::drop() deallocates (implicit)
```

### Which values use implicit RC

- **Need RC (Heap\<T\> wrappers)**: `Str`, `List`, `Map`, `Tuple`, `Some`, `Ok`, `Err` (Value::Err(Heap\<Value\>)), `Variant`, `MultiClauseFunction`, `Newtype`, `ModuleNamespace` — single top-level `Heap<T>` wrapping `Arc<T>`. RC operates on the outer `Arc`: one clone/drop per value.
- **Need RC (direct Arc internals)**: `Struct` (`StructValue`), `Function` (`FunctionValue`), `MemoizedFunction` (`MemoizedFunctionValue`) — inline in the `Value` enum but contain `Arc` fields internally.
- **Don't need RC**: `Int`, `Float`, `Bool`, `Char`, `Byte`, `Void`, `None`, `Duration`, `Size`, `Ordering`, `VariantConstructor`, `NewtypeConstructor`, `FunctionVal`, `TypeRef`, `Range`, `Error` — inline/stack values.
- **Special**: `Interned(ValueId)` — pool-managed, no individual RC.
  - **Depends on Section 01**: ValuePool/ValueId do not exist yet.

### Borrowing rules for the interpreter

The interpreter does not need explicit borrow analysis. Rust's ownership system handles it:
- Variable lookup → `Arc::clone()` (cheap: atomic increment)
- Function argument → move or clone (Rust decides)
- Return value → move (Rust decides)
- Field access → clone the field (or borrow via reference if temporary)
- Pattern binding → clone extracted value

### Why the interpreter doesn't need `ori_arc`

The interpreter's tree-walking evaluation naturally scopes values — when a `let` binding goes out of scope (Rust function returns), `Arc::drop()` fires automatically. There's no need for:
- Borrow inference: Rust's borrow checker handles it
- Liveness analysis: Rust's drop semantics handle it
- Reset/Reuse: The interpreter allocates via Rust's allocator; the overhead of reuse analysis outweighs the benefit

The LLVM backend needs all of these because it generates machine code that must manage memory explicitly.

- [ ] Document implicit RC via `Arc`/`Heap<T>`
- [ ] Document which `Value` variants need RC
- [ ] Document why borrow inference is unnecessary for the interpreter
- [ ] Verify `Heap<T>` wrapper usage matches current `Value` enum

---

## 09.3 EvalIR → ARC IR Bridge — **Deferred: pending LLVM backend validation**

> **Note**: This subsection defines the design for the lowering bridge but implementation is deferred until the LLVM backend (roadmap Section 21) can validate the approach. The bridge is needed when we want to apply `ori_arc`'s optimizations to code that has been through the EvalIR pipeline (e.g., after constant folding from Section 07).

Currently, `ori_arc::lower::lower_function()` lowers directly from the typed AST (`ExprArena`/`ExprId`). A future bridge would allow lowering from EvalIR to ARC IR, enabling the LLVM backend to benefit from EvalIR's constant folding and other optimizations before entering the ARC analysis pipeline.

### Bridge design (sketch)

```rust
/// Lower an EvalIR function to ARC IR for LLVM codegen.
///
/// This is the bridge between the interpreter's tree-structured IR
/// and ori_arc's basic-block IR. The output can be fed into
/// ori_arc::borrow::infer_borrows() and the rest of the pipeline.
pub fn lower_eval_ir_to_arc_ir(
    ir: &EvalIrArena,
    root: EvalIrId,
    pool: &Pool,
    interner: &StringInterner,
) -> ArcFunction {
    // Walk the EvalIR tree and produce ArcFunction with basic blocks.
    // Control flow (if/match/loop/for) becomes Branch/Switch/Jump terminators.
    // Let bindings become ArcInstr::Let.
    // Function calls become ArcInstr::Apply.
    // Collections become ArcInstr::Construct.
    todo!()
}
```

### Key transformations

| EvalIR (tree) | ARC IR (basic blocks) |
|---|---|
| `EvalIrNode::If { cond, then, else_ }` | `ArcTerminator::Branch` + 3 blocks |
| `EvalIrNode::Match { arms }` | `ArcTerminator::Switch` + N+1 blocks |
| `EvalIrNode::Loop { body }` | Back edge: `ArcTerminator::Jump` to header block |
| `EvalIrNode::For { binding, iter, body }` | Desugared to loop with iterator |
| `EvalIrNode::Block { stmts }` | Sequential instructions in a single block |
| `EvalIrNode::Call { func, args }` | `ArcInstr::Apply` |
| `EvalIrNode::Const(value)` | `ArcInstr::Let { value: ArcValue::Literal(..) }` |

### After lowering

Once we have an `ArcFunction`, the full `ori_arc` pipeline applies:
1. `infer_borrows(&[func], &classifier)` → `AnnotatedSig`
2. `apply_borrows(&mut [func], &sigs)`
3. `compute_liveness(&func, &classifier)` → `BlockLiveness`
4. `insert_rc_ops(&mut func, &classifier, &liveness)`
5. `detect_reset_reuse(&mut func, &classifier)`
6. `expand_reset_reuse(&mut func, &classifier)` — expands Reset/Reuse into IsShared + conditional fast/slow paths

No custom RC analysis needed — `ori_arc` handles everything.

- [ ] Define `lower_eval_ir_to_arc_ir()` function signature
- [ ] Map each EvalIR node kind to ARC IR instructions/terminators
- [ ] Handle scope nesting → block parameter passing
- [ ] Test: round-trip EvalIR → ARC IR → verify RC balance
- [ ] **Cross-reference**: `ori_arc::lower::lower_function()` for the existing AST → ARC IR lowering as reference implementation

---

## 09.4 Runtime RC Instrumentation

Add optional runtime tracking for RC activity in the interpreter, for debugging and profiling.

### Heap allocation tracking

The `EvalCounters.heap_allocations` counter (Section 10.4) tracks how many heap-allocating values are created. This counter is incremented at **call sites in `ori_eval`** where heap-allocating `Value` factory methods are called (e.g., `Value::list()`, `Value::str()`, `Value::map()`), not in the factory methods themselves.

**Crate boundary note**: `Value` factories live in `ori_value` (after Phase 0 extraction from `ori_patterns` — see Section 01.4). `EvalCounters` lives in `ori_eval` (via `ModeState` from Section 02). The factories cannot directly increment the counter because they have no access to `EvalCounters`. Instead, the increment happens at the call site in `ori_eval`:

```rust
// In ori_eval interpreter:
let result = Value::list(elements);
if let Some(counters) = &mut self.counters {
    counters.heap_allocations += 1;
}
```

### Debug-mode leak detection

In debug builds, optionally track live `Heap<T>` allocations to detect leaks:

```rust
/// Debug-mode allocation tracker.
/// Only active when `cfg(debug_assertions)` and explicitly enabled.
#[cfg(debug_assertions)]
pub struct LeakDetector {
    /// Number of Heap<T> allocations still alive.
    live_count: std::sync::atomic::AtomicUsize,
}

#[cfg(debug_assertions)]
impl LeakDetector {
    pub fn on_alloc(&self) {
        self.live_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn on_dealloc(&self) {
        self.live_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check for leaks. Call at end of evaluation.
    pub fn check(&self) -> Result<(), usize> {
        let live = self.live_count.load(std::sync::atomic::Ordering::Relaxed);
        if live == 0 { Ok(()) } else { Err(live) }
    }
}
```

**Note**: Leak detection is a debug aid, not a production feature. The `Arc` runtime guarantees that memory is freed when refcount reaches zero. Leaks indicate reference cycles or values that escape their expected scope — both are bugs worth detecting early.

- [ ] Define `EvalCounters.heap_allocations` increment strategy (at call sites in `ori_eval`)
- [ ] Implement debug-mode `LeakDetector` (opt-in, `cfg(debug_assertions)`)
- [ ] Test: verify `heap_allocations` count matches expected for simple programs
- [ ] Test: verify `LeakDetector` reports zero leaks for well-behaved programs

---

## 09.5 Completion Checklist

- [ ] Architecture documented: interpreter (implicit `Arc` RC) vs LLVM (`ori_arc` pipeline)
- [ ] `ori_arc` existing infrastructure acknowledged and referenced (not duplicated)
- [ ] Interpreter RC strategy documented (Rust `Arc`/`Heap<T>`)
- [ ] Type classification distinguished: compile-time (`ArcClassifier::needs_rc(Idx)`) vs runtime (`Heap<T>` variant check)
- [ ] Ownership model uses `ori_arc::ownership::Ownership` (2 variants: `Owned`, `Borrowed` — no `Stack`)
- [ ] EvalIR → ARC IR bridge — deferred: pending LLVM backend
- [ ] `EvalCounters.heap_allocations` tracked at `ori_eval` call sites (not in `Value` factories)
- [ ] Debug-mode leak detection via `LeakDetector`
- [ ] Interpreter ignores ARC IR RC operations (doesn't use them)
- [ ] LLVM backend uses `ori_arc` pipeline (existing infrastructure: borrow inference, liveness, RC insertion, reset/reuse detection, expand reset/reuse, drop descriptors)

**Exit Criteria:** The interpreter's implicit RC strategy is documented and instrumented. The architecture cleanly separates interpreter (implicit RC) from LLVM codegen (explicit RC via `ori_arc`). No analysis code is duplicated from `ori_arc`.
