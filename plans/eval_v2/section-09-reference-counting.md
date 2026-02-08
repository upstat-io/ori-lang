---
section: "09"
title: Reference Counting Integration
status: not-started
goal: Design and implement reference counting insertion for the EvalIR, preparing for AOT compilation
sections:
  - id: "09.1"
    title: RC Model Design
    status: not-started
  - id: "09.2"
    title: Borrow Analysis
    status: not-started
  - id: "09.3"
    title: RC Insertion Pass
    status: deferred
    note: "Deferred pending LLVM backend validation"
  - id: "09.4"
    title: Reuse Analysis
    status: deferred
    note: "Deferred pending LLVM backend validation"
---

# Section 09: Reference Counting Integration

**Status:** 📋 Planned
**Goal:** Design a reference counting model for the EvalIR that prepares for AOT compilation via LLVM, inspired by Roc's Perceus and Swift's ARC.
**Dependencies:** Section 08 (Canonical EvalIR — provides EvalIrNode, EvalIrId, EvalIrArena used by borrow analysis and RC insertion), Section 01 (ValuePool — provides ValueId for interned values)

---

## Prior Art Analysis

### Roc: Perceus-Style Reference Counting
Roc implements the **Perceus** algorithm (Microsoft Research, 2021) which inserts `Inc`/`Dec`/`Free` operations into the mono IR. Key innovations:
- **Reuse analysis**: When deconstructing a value and constructing a new one of the same size, the memory can be reused (`Reset`/`ResetRef`)
- **Borrow analysis**: Determines which operations need to increment vs. can borrow
- **Drop specialization**: Custom destructors that skip recursion for non-reference-counted fields

### Swift: Automatic Reference Counting
Swift inserts `retain`/`release` calls during SIL (Swift Intermediate Language) optimization. Key techniques:
- **Copy-on-Write**: Shared values are copied before mutation
- **Guaranteed ownership**: Parameters can be `@guaranteed` (borrowed, no RC needed)
- **Owned vs. borrowed parameter conventions**: Callee decides whether to consume or borrow

### Koka: FBIP (Functional But In Place)
Koka's compiler analyzes when functional updates can be done in-place by checking if the value's reference count is 1. The `CheckFBIP.hs` pass verifies this property.

### Lean 4: RC with Reset/Reuse
Lean's IR has explicit `RC.inc`, `RC.dec`, `reset`, and `reuse` instructions. The `ExpandResetReuse.lean` pass converts high-level reuse annotations into concrete memory operations.

---

## 09.1 RC Model Design

Define the reference counting operations for EvalIR:

```rust
/// Reference counting operations inserted into EvalIR.
pub enum RcOp {
    /// Increment reference count (value will be shared)
    Inc {
        value: EvalIrId,
        /// Number of additional references (usually 1)
        count: u32,
    },

    /// Decrement reference count (may trigger deallocation)
    Dec {
        value: EvalIrId,
        /// Whether to recursively decrement children
        recursive: bool,
    },

    /// Free memory unconditionally (refcount known to be 0)
    Free {
        value: EvalIrId,
    },

    /// Reset: prepare memory for reuse (decrement children but keep allocation)
    Reset {
        value: EvalIrId,
        /// Opaque token identifying the reuse opportunity
        reuse_token: ReuseToken,
    },

    /// Reuse: construct a new value in previously reset memory
    Reuse {
        token: ReuseToken,
        new_value: EvalIrId,
    },
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ReuseToken(u32);

/// Classification of a value's ownership at a point in the IR.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Ownership {
    /// We own this value — responsible for Dec when done
    Owned,
    /// We borrowed this value — no RC responsibility
    Borrowed,
    /// Value is on the stack (inline, no heap allocation) — no RC needed
    Stack,
}
```

**Which values need RC?**

`needs_rc()` returns `true` for both subcategories below, but the RC insertion strategy differs:

- **Need RC (Heap\<T\> wrappers)**: `Str`, `List`, `Map`, `Tuple`, `Some`, `Ok`, `Err` (Value::Err(Heap\<Value\>), the Result error variant), `Variant`, `MultiClauseFunction`, `Newtype`, `ModuleNamespace` — single top-level `Heap<T>` wrapping `Arc<T>`. RC operates on the outer `Arc`: one `Inc`/`Dec` per value.
- **Need RC (direct Arc internals)**: `Struct` (`StructValue`), `Function` (`FunctionValue`), `MemoizedFunction` (`MemoizedFunctionValue`) — these are inline in the `Value` enum but contain `Arc` fields internally. RC targets the inner `Arc` references, not a top-level wrapper. The LLVM backend must emit `Inc`/`Dec` for each `Arc` field individually. Per-type breakdown:
    - **Struct** (`StructValue`): `fields: Arc<Vec<Value>>` + `layout: Arc<StructLayout>` (2 Arcs)
    - **Function** (`FunctionValue`): `captures: Arc<FxHashMap<Name, Value>>` + `arena: SharedArena(Arc<ExprArena>)` (2 Arcs)
    - **MemoizedFunction** (`MemoizedFunctionValue`): all of Function's Arcs plus `cache: Arc<RwLock<...>>` + `insertion_order: Arc<RwLock<...>>` (4 Arcs total). **Caveat:** `cache` and `insertion_order` use `Arc<RwLock<...>>` for interior mutability. The LLVM AOT path may not support memoized functions, or may use thread-local caches without `RwLock`.
- **Don't need RC**: `Int`, `Float`, `Bool`, `Char`, `Byte`, `Void`, `None`, `Duration`, `Size`, `Ordering`, `VariantConstructor`, `NewtypeConstructor`, `FunctionVal`, `TypeRef`, `Range` (inline/stack values)
- **Don't need RC (with caveat)**: `Error(String)` (Value::Error, the error recovery sentinel — distinct from Value::Err) — contains heap-allocated `String` but is excluded from RC because error values propagate up and are consumed immediately. The LLVM backend handles cleanup via scope-based deallocation, not reference counting.
- **Special**: `Interned(ValueId)` — pool-managed, no individual RC
  - **Depends on Section 01**: ValuePool/ValueId do not exist yet; this category applies after Section 01 implementation

- [ ] Define `RcOp` enum
  - [ ] `Inc`, `Dec`, `Free`, `Reset`, `Reuse`
- [ ] Define `Ownership` enum
  - [ ] `Owned`, `Borrowed`, `Stack`
- [ ] Define `ReuseToken` for tracking reuse opportunities
- [ ] Add `EvalIrNode::Rc(RcOp)` variant to EvalIR
  - [ ] RC operations are explicit nodes in the IR
  - [ ] Inserted by the RC pass (not by lowering)
- [ ] Define `needs_rc(value: &Value) -> bool`
  - [ ] Returns true for heap-allocated values
  - [ ] Returns false for inline/stack values

---

## 09.2 Borrow Analysis

Determine which operations need to increment refcount vs. can borrow:

```rust
/// Analyze a function body to determine ownership of each value reference.
pub fn analyze_ownership(
    ir: &EvalIrArena,
    root: EvalIrId,
) -> OwnershipMap {
    let mut map = OwnershipMap::new();
    let mut analyzer = OwnershipAnalyzer::new(ir);
    analyzer.analyze(root, &mut map);
    map
}

pub struct OwnershipMap {
    /// For each EvalIrId, the ownership of its result
    entries: FxHashMap<EvalIrId, Ownership>,
}

struct OwnershipAnalyzer<'a> {
    ir: &'a EvalIrArena,
    /// Last use of each variable (for determining when to Dec)
    last_use: FxHashMap<Name, EvalIrId>,
}

impl<'a> OwnershipAnalyzer<'a> {
    fn analyze(&mut self, id: EvalIrId, map: &mut OwnershipMap) {
        let node = self.ir.get(id);
        match node {
            EvalIrNode::Var { name, .. } => {
                // Variable reference: borrowed (we don't consume the binding)
                map.set(id, Ownership::Borrowed);
                self.last_use.insert(*name, id);
            }

            EvalIrNode::Call { func, extra } => {
                // Function call: arguments are owned (passed by value).
                // Args stored in extra array: [count, arg0, arg1, ...]
                let children = self.ir.get_children(*extra);
                for &arg_raw in children {
                    let arg = EvalIrId(arg_raw);
                    self.analyze(arg, map);
                    // Arguments to calls need Inc (unless last use)
                    map.set(arg, Ownership::Owned);
                }
                self.analyze(*func, map);
            }

            EvalIrNode::Let { name, init, .. } => {
                self.analyze(*init, map);
                // Init value is owned by the binding
                map.set(*init, Ownership::Owned);
            }

            EvalIrNode::Block { extra } => {
                // Block children stored in extra array: [count, id0, id1, ...]
                let children = self.ir.get_children(*extra);
                for &stmt_raw in children {
                    self.analyze(EvalIrId(stmt_raw), map);
                }
            }

            EvalIrNode::Struct { name: _, extra } => {
                // Struct fields stored in extra array: [count, fname0, fval0, ...]
                let count = self.ir.field_count(*extra);
                for i in 0..count {
                    let fval = self.ir.field_value(*extra, i);
                    self.analyze(fval, map);
                    map.set(fval, Ownership::Owned);
                }
            }

            EvalIrNode::Const(_) => {
                map.set(id, Ownership::Stack);
            }

            EvalIrNode::PoolRef(_) => {
                // Pool-managed value — no individual RC responsibility
                map.set(id, Ownership::Borrowed);
            }

            // ... other nodes — see ownership classification table below
            _ => {}
        }
    }
}
```

**Ownership classification summary** — default ownership for each `EvalIrNode` variant's operands and result:

| EvalIrNode Variant | Operand Ownership | Result Ownership |
|---|---|---|
| `Const` / `PoolRef` | (none) | Stack / Borrowed |
| `Var` | (none) | Borrowed |
| `Global` | (none) | Borrowed |
| `Call` / `MethodCall` | args: **Owned** (consumed by callee) | Owned (caller receives) |
| `Let` | init: **Owned** (binding takes ownership) | Stack (void) |
| `Assign` | value: **Owned** | Stack (void) |
| `BinaryOp` / `UnaryOp` / `Cast` | operands: **Borrowed** (read-only) | Owned (new value produced) |
| `List` / `Tuple` / `Map` / `Struct` / `Construct` | elements/fields: **Owned** (moved into collection) | Owned |
| `Some` / `Ok` / `Err` | inner: **Owned** | Owned |
| `If` / `Match` / `Block` | branches/body: **inherit** (result ownership follows branch) | inherit |
| `Loop` / `For` | body: **inherit** | inherit |
| `Lambda` | (captures cloned at creation) | Owned |
| `FieldAccess` / `IndexAccess` / `TupleAccess` | receiver: **Borrowed** | Borrowed |
| `Try` | inner: **Borrowed** (unwrap or propagate) | Owned (unwrapped) |
| `WithCapability` | body: **inherit** | inherit |
| `Break` / `Continue` | value: **Owned** (moved to loop exit) | N/A (control flow) |
| `Panic` | message: **Borrowed** | N/A (terminates) |
| `Join` / `Jump` | args: **Owned** | inherit |
| `Rc` | (RC operation metadata) | Stack |
| `Invalid` | (none) | N/A (error) |

**Borrowing rules for the interpreter**:
- Variable lookup → Borrowed (clone the Arc, not the value)
- Function argument → Owned (caller provides, callee consumes)
- Return value → Owned (callee provides, caller receives)
- Field access → Borrowed (reference into parent)
- Pattern binding → Owned (extracted value)

**Note**: For the tree-walking interpreter, RC is implicit (Arc handles it). This analysis is primarily for the **LLVM codegen path** (roadmap Section 21) where explicit RC is needed. We design it now so the EvalIR can serve both paths.

- [ ] Implement `OwnershipAnalyzer`
  - [ ] Track last-use of each variable
  - [ ] Classify each node's ownership
  - [ ] Handle control flow (loops, branches) conservatively
  - [ ] **Dependency note**: The `EvalIrNode::PoolRef` case is conditional on Section 01's ValuePool implementation. If ValuePool is deferred, omit the PoolRef arm from the analyzer.
- [ ] Define borrowing rules for each EvalIR node kind
  - [ ] Document which arguments are borrowed vs. owned
  - [ ] Document which results are owned vs. borrowed
- [ ] **Cross-reference**: `get_children()`, `field_count()`, `field_value()` used in the analyzer are `EvalIrArena` accessor methods to be defined as part of Section 08 implementation.
- [ ] Output `OwnershipMap` for use by RC insertion pass

---

## Relationship to Heap\<T\>

The current interpreter and the future LLVM codegen path handle reference counting differently. Both systems coexist:

- **Interpreter path**: Continues using `Heap<T>` (which wraps `Arc<T>`) for implicit reference counting. `Arc::clone()` is the increment, and `Arc::drop()` is the decrement. No explicit `RcOp` nodes are needed — the interpreter ignores them.
- **LLVM codegen path**: Uses explicit `RcOp` nodes (`Inc`, `Dec`, `Free`, `Reset`, `Reuse`) inserted into the EvalIR. The LLVM backend translates these to actual reference count manipulation instructions. This path does **not** use `Arc`.
- **`needs_rc()` mapping**: The `needs_rc(value)` function answers the question "is this a `Heap<T>` variant?" — i.e., would the interpreter wrap this in an `Arc`? If yes, the LLVM path needs explicit RC operations for it.
- **Design principle**: The EvalIR is the shared representation. RC annotations in the IR are metadata that the interpreter skips and the LLVM backend consumes.
- **Cross-reference**: See Section 10 for `EvalCounters.heap_allocations` which tracks allocations for values where `needs_rc()` returns true.

---

## 09.3 RC Insertion Pass — **Deferred: pending LLVM backend validation**

> **Note**: This subsection defines the design for RC insertion but implementation is deferred until the LLVM backend (roadmap Section 21) can validate the approach. The types and model from 09.1-09.2 should be implemented first.

Insert `Inc`/`Dec`/`Free` operations into the EvalIR:

```rust
pub fn insert_rc(ir: &mut EvalIrArena, ownership: &OwnershipMap) {
    // For each owned value that's used multiple times: insert Inc before secondary uses
    // For each owned value at its last use: no Dec needed (consumed)
    // For each owned value that goes out of scope: insert Dec

    let mut inserter = RcInserter::new(ir, ownership);
    inserter.process();
}

struct RcInserter<'a> {
    ir: &'a mut EvalIrArena,
    ownership: &'a OwnershipMap,
    /// Values that need Dec when the current scope ends
    pending_decs: Vec<EvalIrId>,
}
```

**Insertion rules**:
1. When a value is **shared** (used more than once), insert `Inc` before the second use
2. When a value goes **out of scope** (let binding scope ends), insert `Dec`
3. When a function **returns**, the return value is not Dec'd (ownership transferred to caller)
4. When a match arm **extracts** fields, the parent value may need `Dec` (if not reused)

- [ ] Implement `RcInserter` pass
  - [ ] Insert `Inc` before secondary uses of shared values
  - [ ] Insert `Dec` at scope exits
  - [ ] Handle loops (Dec at continue, Dec at break)
  - [ ] Handle try operator (Dec on propagation path)
- [ ] Optimize: elide `Inc`/`Dec` for stack values
  - [ ] `needs_rc()` check before inserting any RC op
- [ ] Optimize: cancel `Inc` immediately followed by `Dec`
  - [ ] Identify patterns where a value is shared then immediately dropped
- [ ] Test: verify RC operations are correctly balanced
  - [ ] Every `Inc` has a corresponding `Dec` or `Free`
  - [ ] No double-free (no two `Dec`s for the same use)

---

## 09.4 Reuse Analysis — **Deferred: pending LLVM backend validation**

> **Note**: This subsection defines the design for reuse analysis but implementation is deferred until the LLVM backend (roadmap Section 21) can validate the approach. The types and model from 09.1-09.2 should be implemented first.

Identify opportunities for memory reuse (Perceus pattern):

```rust
/// Find reuse opportunities: where a deconstructed value's memory
/// can be reused for a new construction of the same size.
pub fn analyze_reuse(ir: &EvalIrArena) -> Vec<ReuseOpportunity> {
    let mut opportunities = Vec::new();

    // Pattern: match value { Variant(fields) => NewVariant(new_fields) }
    // If the deconstructed value's refcount is 1 AND the new variant's allocation
    // size is compatible with (does not exceed) the old allocation, the memory can
    // be reused via Reset(value) then Reuse(token, NewVariant(new_fields))

    // Walk the IR looking for match → construct patterns
    // ...

    opportunities
}

pub struct ReuseOpportunity {
    /// The value being deconstructed
    pub source: EvalIrId,
    /// The value being constructed (that could reuse source's memory)
    pub target: EvalIrId,
    /// Whether the layouts are compatible
    pub compatible: bool,
}
```

**When reuse is possible**:
- A match arm destructures a variant and constructs a new variant of the same enum type
- The destructured value's refcount is exactly 1 (sole owner)
- The new variant's allocation size is compatible with the old allocation (new does not exceed old)

**Representation note**: The reuse compatibility check is backend-agnostic at the EvalIR level. Specific layout depends on the backend:
- **Interpreter**: `Heap<Vec<Value>>` — reuse is about Vec capacity (new field count does not exceed old allocation capacity)
- **LLVM**: Fixed-size tagged union — reuse is about byte-level size compatibility
- **EvalIR `Reset`/`Reuse` operations are backend-agnostic**: They express the *intent* to reuse; the backend determines whether the sizes are actually compatible at lowering time.

**Note**: Reuse analysis is primarily for AOT compilation. For the interpreter, Arc handles memory automatically. This pass is designed now to ensure the EvalIR can express reuse for the LLVM backend.

- [ ] Implement reuse opportunity detection
  - [ ] Identify match → construct patterns
  - [ ] Check allocation size compatibility (backend-agnostic: new allocation does not exceed old)
  - [ ] Track source refcount (must be sole owner)
- [ ] Insert `Reset`/`Reuse` operations when opportunities found
- [ ] Document when reuse is NOT safe
  - [ ] Source captured by a closure
  - [ ] Source shared with another thread
  - [ ] Source has finalizer/destructor with side effects

---

## 09.5 Completion Checklist

- [ ] `RcOp` enum defined (Inc, Dec, Free, Reset, Reuse)
- [ ] `Ownership` classification (Owned, Borrowed, Stack)
- [ ] Ownership analysis pass produces `OwnershipMap`
- ~~RC insertion pass adds Inc/Dec/Free to EvalIR~~ (deferred: pending LLVM backend)
- ~~Inc/Dec cancelation optimization~~ (deferred: pending LLVM backend)
- ~~Stack value RC elision~~ (deferred: pending LLVM backend)
- ~~Reuse analysis identifies opportunities~~ (deferred: pending LLVM backend)
- [ ] RC operations are balanced (verified by test — deferred with insertion pass)
- [ ] Interpreter ignores RC operations (Arc handles it)
- [ ] LLVM backend can consume RC operations (future)

**Exit Criteria:** The EvalIR can express reference counting operations for AOT compilation, with ownership analysis and reuse optimization. The interpreter path ignores these annotations.
