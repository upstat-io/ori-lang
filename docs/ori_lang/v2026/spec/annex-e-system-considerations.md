---
title: "Annex E — System considerations"
description: "Ori Language Specification — Annex E (informative)"
order: 104
section: "Annexes"
---

# Annex E (informative) — System considerations

This annex describes implementation considerations for different target platforms and optimization levels.

This section specifies implementation-level requirements and platform considerations.

## Numeric Types

### Integers

The `int` type is a signed integer with the following semantic range:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Minimum | -9,223,372,036,854,775,808 (-2⁶³) |
| Maximum | 9,223,372,036,854,775,807 (2⁶³ - 1) |
| Overflow | Panics (see [Error Codes](https://ori-lang.com/docs/compiler-design/appendices/c-error-codes)) |

The canonical size defines the semantic range. The compiler may use a narrower machine representation (see [§ Representation Optimization](#representation-optimization)).

There is no separate unsigned integer type. Bitwise operations treat the value as unsigned bits.

### Floats

The `float` type is an IEEE 754 double-precision floating-point number:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Precision | ~15-17 significant decimal digits |
| Range | ±1.7976931348623157 × 10³⁰⁸ |

The canonical size defines the semantic precision. The compiler may use a narrower machine representation when it can prove no precision loss (see [§ Representation Optimization](#representation-optimization)).

Special values `inf`, `-inf`, and `nan` are supported.

## Strings

### Encoding

All strings are UTF-8 encoded. There is no separate ASCII or byte-string type.

```ori
let greeting = "Hello, 世界";  // UTF-8
let emoji = "🎉";              // UTF-8
```

### Indexing

String indexing returns a single Unicode codepoint as a `str`:

```ori
let s = "héllo";
s[0];  // "h"
s[1]  // "é" (single codepoint)
```

The index refers to codepoint position, not byte position. Out-of-bounds indexing panics.

### Grapheme Clusters

Some visual characters consist of multiple codepoints:

```ori
let astronaut = "🧑‍🚀";  // 3 codepoints: person + ZWJ + rocket
len(astronaut);        // 3
astronaut[0]          // "🧑"
```

For grapheme-aware operations, use standard library functions.

### Length

`len(str)` returns the number of bytes, not codepoints. Use `.chars().count()` for codepoint count.

```ori
len("hello")  // 5 (5 bytes)
len("世界")    // 6 (each character is 3 UTF-8 bytes)
len("🧑‍🚀")    // 11 (multi-byte emoji ZWJ sequence: 4+3+4)
```

## Collections

### Limits

Collections have no fixed size limits. Maximum size is bounded by available memory.

| Collection | Limit |
|------------|-------|
| List | Memory |
| Map | Memory |
| String | Memory |

### Capacity

Implementations may pre-allocate capacity for performance. This is not observable behavior.

## Recursion

### Tail Call Optimization

Tail calls are guaranteed to be optimized. A tail call does not consume stack space:

```ori
@countdown (n: int) -> void =
    if n <= 0 then void else countdown(n: n - 1);  // tail call

countdown(n: 1000000)  // does not overflow stack
```

A call is in tail position if it is the last operation before the function returns.

### Non-Tail Recursion

Non-tail recursive calls consume stack space. Deep recursion may cause stack overflow:

```ori
@sum_to (n: int) -> int =
    if n <= 0 then 0 else n + sum_to(n: n - 1);  // not tail call

sum_to(n: 1000000)  // may overflow stack
```

For deep recursion, use the `recurse` pattern with `memo: true` or convert to tail recursion.

## Platform Support

### Target Platforms

Conforming implementations should support:

- Linux (x86-64, ARM64)
- macOS (x86-64, ARM64)
- Windows (x86-64)
- WebAssembly (WASM)

### Endianness

Byte order is implementation-defined. Programs should not depend on endianness unless using platform-specific byte manipulation.

### Path Separators

File paths use the platform-native separator. The standard library provides cross-platform path operations.

## Implementation Limits

Implementations may impose limits on:

| Aspect | Minimum Required |
|--------|------------------|
| Identifier length | 1024 characters |
| Nesting depth | 256 levels |
| Function parameters | 255 |
| Generic parameters | 64 |

Exceeding these limits is a compile-time error.

## Representation Optimization

The compiler may optimize the machine representation of any type, provided the optimization preserves _semantic equivalence_. An optimization is semantically equivalent if no conforming program can distinguish the optimized representation from the canonical one through any language-level operation.

### Canonical Representations

| Type | Canonical | Semantic Range |
|------|-----------|----------------|
| `int` | 64-bit signed two's complement | [-2⁶³, 2⁶³ - 1] |
| `float` | 64-bit IEEE 754 binary64 | ±1.8 × 10³⁰⁸, ~15-17 digits |
| `bool` | 1-bit | `true` or `false` |
| `byte` | 8-bit unsigned | [0, 255] |
| `char` | 32-bit Unicode scalar | U+0000–U+10FFFF excluding surrogates |
| `Ordering` | Tri-state | `Less`, `Equal`, `Greater` |

### Permitted Optimizations

Permitted optimizations include but are not limited to:

- Narrowing primitive machine types (`bool` → `i1`, `byte` → `i8`, `char` → `i32`, `Ordering` → `i8`)
- Enum discriminant narrowing (`i8` for ≤256 variants)
- All-unit enum payload elimination
- Sum type shared payload slots (`Result<T, E>` uses `max(sizeof(T), sizeof(E))`)
- ARC operation elision for transitively trivial types
- Newtype representation erasure
- Struct field reordering for alignment
- Integer narrowing based on value range analysis
- Float narrowing when precision loss is provably zero

### Guarantees

1. The semantic range of every type is always preserved
2. Overflow behavior is determined by the semantic type, not the machine representation
3. Values stored and retrieved through any language operation are identical
4. `debug()` and `print()` display semantic values
5. `x == y` and `hash(x) == hash(y)` relationships are representation-independent
6. Type classification for reference counting is determined by type containment, not representation size (see [Memory Model § Type Classification](21-memory-model.md#217-type-classification))

### Non-Guarantees

1. The exact machine representation of any type is unspecified
2. Memory layout may differ between compiler versions and target platforms
3. Struct field order in memory may differ from declaration order

NOTE  For the full specification including optimization tiers, cross-cutting invariants, and interaction with `#repr` attributes, see [Representation Optimization Proposal](../../proposals/approved/representation-optimization-proposal.md).

## ARC Runtime

This section specifies the runtime support for reference-counted heap objects in AOT-compiled programs.

NOTE  The ARC runtime ABI is not stable. Heap object layout and runtime function signatures may change between compiler versions. This section applies to the AOT compilation target only; the interpreter and JIT may use different representations.

### Heap Object Layout

A reference-counted heap object has the following layout:

```
+──────────────────+───────────────────────────+
| strong_count: i64 | data bytes ...           |
+──────────────────+───────────────────────────+
^                    ^
base (data_ptr - 8)  data_ptr
```

The `data_ptr` returned by allocation points to the data area, not to the header. The strong count is stored at `data_ptr - 8`. Minimum alignment is 8 bytes.

The data pointer may be passed to foreign functions without adjustment.

### Runtime Functions

All runtime functions use the C calling convention (`extern "C"`).

| Function | Signature | Description |
|----------|-----------|-------------|
| `ori_rc_alloc` | `(size: usize, align: usize) -> *mut u8` | Allocate `size + 8` bytes, initialize strong count to 1, return data pointer |
| `ori_rc_inc` | `(data_ptr: *mut u8)` | Increment the strong count |
| `ori_rc_dec` | `(data_ptr: *mut u8, drop_fn: fn(*mut u8))` | Decrement the strong count; if zero, call `drop_fn` |
| `ori_rc_free` | `(data_ptr: *mut u8, size: usize, align: usize)` | Deallocate from `data_ptr - 8` with total size `size + 8` |
| `ori_rc_count` | `(data_ptr: *const u8) -> i64` | Return the current strong count (diagnostic use only) |

### Drop Functions

Each reference type has a compiler-generated _drop function_ with signature `extern "C" fn(*mut u8)`. The drop function:

1. Decrements reference counts of any reference-typed child fields (calling `ori_rc_dec` for each)
2. Calls `ori_rc_free(data_ptr, size, align)` to release the allocation

If the type implements the `Drop` trait, `Drop.drop` is called before step 1.

### Built-in Type Representations

| Type | Representation |
|------|----------------|
| `str` | `{ len: i64, data: *const u8 }` |
| `[T]` | `{ len: i64, cap: i64, data: *mut u8 }` |
| `Option<T>` | `{ tag: i8, value: T }` (tag 0 = `None`, 1 = `Some`) |
| `Result<T, E>` | `{ tag: i8, value: max(T, E) }` (tag 0 = `Ok`, 1 = `Err`) |

## AIMS — ARC Intelligent Memory System

NOTE  Annex E is informative. Rules in this section using `shall` / `shall not` document the AIMS algorithm and its invariants. Target subsystems documented in §11 describe design targets; implementations conforming to a given Ori build need not satisfy target rules until those subsystems ship.

### §1 Mission and Design Center

AIMS is Ori's AIMS memory model — one calculus over one object, the compile-time intelligence layer that operates on top of ARC. Its laws — the lattice dimensions and their algebra (§3), the transfer functions (§4), the canonicalization rules (§5), the pipeline ordering (§6), the interprocedural contracts (§7), the realization rules (§8), and the verification layers (§9) — are stated and proven against the AIMS product lattice, an object no prior system defines; the calculus inherits no proof, rule, or law. Its calculus, soundness proofs, and proof checker are Ori's own contribution; the design drew on argument-shape patterns from prior compilers (Lean 4, Koka, Swift, GHC, OxCaml, Clang, Racordon) as historical influences, not architectural dependencies. The design center is to make reference-count operations rare in emitted code, not to make individual reference-count operations faster. Every reference-count operation that survives to emitted IR points at a specific proof failure: the lattice could not prove the operation was redundant.

The Ori programmer never writes ownership annotations, lifetime markers, or borrow syntax. AIMS infers ownership, locality, uniqueness, and reuse opportunities from a unified product lattice and propagates the results across function boundaries through interprocedural contracts. Memory safety holds across the entire program surface, including FFI; `unsafe` relaxes type-level guarantees but never permits memory unsafety.

### §2 Five Load-Bearing Invariants

1. **Contract and realization shall agree.** A function whose `MemoryContract` records `FipContract::Certified` shall have zero unmatched allocations and deallocations in the realized IR.
2. **Active rewrites shall be sound.** Every active rewrite (TRMC, RC motion, KnownSafe elimination, COW contraction) shall preserve identical observable behavior, with structural verification at compile time and behavioral verification at test time.
3. **No pass shall rely on stale summaries.** Pipeline ordering is load-bearing: when a step modifies IR or updates an effect summary, all downstream consumers shall see the updated value.
4. **Every active subsystem shall be end-to-end verified.** Implementation, invariant enforcement, and behavioral tests are all required; the absence of any one is a spec gap.
5. **The unified model shall stay unified.** New capabilities shall extend a lattice dimension, extend a contract field on `MemoryContract` / `ParamContract` / `ReturnContract` / `EffectSummary`, or feed the lattice-driven analysis as a typed pre-pass input. Independent reference-count emission paths, parallel escape enumerations, and shadow uniqueness trackers are forbidden.

### §3 Lattice Dimensions

The AIMS lattice is a product of finite-height lattices; product join is componentwise join followed by canonicalization (§5). Every dimension shall have finite height. Transfer functions (§4) shall be monotone. Canonicalization (§5) shall be idempotent and shall preserve join results.

NOTE  Adding a new dimension requires proving finite height, defining the join, updating canonicalization, and proving the new product lattice satisfies commutativity, associativity, idempotence, and antisymmetry.

#### §3.1 Access

Ownership versus borrowing.

| Value | Meaning | Reference-count obligation |
|-------|---------|---------------------------|
| `Borrowed` | Temporary view of another value | None — caller manages |
| `Owned` | Value owns its allocation | Full reference-count responsibility |

Order: `Borrowed < Owned`. Join: `max`. Height: 1.

#### §3.2 Consumption

Substructural mode.

| Value | Meaning | Reference-count implication |
|-------|---------|----------------------------|
| `Dead` | Not live at this point | No reference-count operations |
| `Linear` | Consumed exactly once (moved) | No increment needed; decrement at death |
| `Affine` | May be dropped without use | Decrement may be needed; no increment |
| `Unrestricted` | Freely copied and dropped | Full reference-count (increment on copy, decrement on drop) |

Order: `Dead < Linear < Affine < Unrestricted`. Join: `max`. Height: 3.

#### §3.3 Cardinality

Forward usage count.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Absent` | Never used after this point | Skip all reference-count |
| `Once` | Used exactly once | Move semantics |
| `Many` | Used multiple times or in a loop | Full reference-count |

Order: `Absent < Once < Many`. Join: `max`. Height: 2. Sequential composition (`seq_add`) follows the QTT semiring: `Absent + x = x`, `Once + Once = Many`, `Once + Many = Many`, `Many + Many = Many`.

#### §3.4 Uniqueness

Runtime reference-count knowledge.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Unique` | Provably reference-count == 1 | Copy-on-write fast path; reset / reuse |
| `MaybeShared` | Unknown reference-count | Runtime check needed |
| `Shared` | Provably reference-count > 1 | Always copy on write |

Order: `Unique < MaybeShared < Shared`. Join: `max`. Height: 2.

NOTE  Uniqueness is a past guarantee ("not duplicated"), distinct from Linearity which is a future guarantee ("consumed once").

#### §3.5 Locality

Escape classification.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `BlockLocal` | Does not escape defining basic block | Stack candidate |
| `FunctionLocal` | Does not escape defining function | Stack candidate |
| `ArgEscaping` | Escapes via argument but not to heap | Caller-stack lifetime |
| `HeapEscaping` | May escape to the heap | Requires heap allocation |
| `Unknown` | Conservative default | No optimization |

Order: `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown`. Join: `max`. Height: 4.

#### §3.6 Shape

Structural classification for reuse.

| Value | Meaning |
|-------|---------|
| `NonReusable` | Not a candidate for allocation reuse (top) |
| `ReusableCtor(Struct)` | Struct constructor — reuse-eligible |
| `ReusableCtor(EnumVariant)` | Enum-variant constructor — reuse-eligible |
| `CollectionBuffer` | Collection backing buffer (list, map, set) |
| `ContextHole` | TRMC constructor-context hole |

Flat lattice — equal values stay; unequal values join to `NonReusable`. Height: 1.

#### §3.7 Effect

Memory-effect classification.

| Flag | Meaning | Blocks |
|------|---------|--------|
| `may_alloc` | May allocate heap memory | FIP certification |
| `may_share` | May create shared references | Uniqueness preservation |
| `may_throw` | May throw / panic | Cleanup-path correctness |

Three independent boolean flags. Join: componentwise OR. Height: 3.

### §4 Transfer Functions

Transfer functions define how each ARC IR instruction updates the lattice state. There are two directions: forward (definition) and backward (demand). Every ARC IR instruction variant shall have an explicit forward and backward rule; adding a new instruction variant without corresponding TF rules shall be a spec gap.

| Rule | Purpose |
|------|---------|
| TF-1 | Scalar literal: `dst.state := SCALAR` |
| TF-2 | Variable binding: `dst.state := state(v)` (alias) |
| TF-2a | PrimOp: `dst.state := SCALAR` |
| TF-3 | Construct allocation: `dst := FRESH(shape_from_ctor(ctor))` |
| TF-4 | Field projection: `dst := (Borrowed, Linear, Once, source.uniqueness, source.locality, NonReusable, NONE)` |
| TF-5 | Direct call without contract: `dst := CONSERVATIVE` |
| TF-5a | Indirect call: `dst := CONSERVATIVE` |
| TF-6 | Direct call with contract: `dst := refine(CONSERVATIVE, callee.return_contract)` |
| TF-6a | Invoke with contract: same as TF-6 |
| TF-6b | Invoke without contract: same as TF-5 |
| TF-6c | Indirect invoke: same as TF-5a |
| TF-7 | Closure capture (PartialApply): `dst := FRESH(NonReusable)` |
| TF-8 | Conditional selection (Select): scalar-aware merge of branch states |
| TF-9 | Reuse: `dst := FRESH(shape)` from token |
| TF-9a | CollectionReuse: `dst := FRESH(CollectionBuffer)` |
| TF-10 | IsShared: `dst := SCALAR` |
| TF-10a | Reset: `dst := SCALAR` (reuse token) |
| TF-11 | Standard backward demand: `(operand, Once, Linear)` per argument; `seq_add` accumulation |
| TF-11a | Terminator backward demands (Return / Jump / Branch / Switch / Invoke / Resume / Unreachable) |
| TF-12 | PartialApply emits no standard backward demand; capture handled by TF-13 |
| TF-13 | `capture_state_update`: closure capture rule with access-promotion on heap-escaping closures |
| TF-14 | Project backward propagation: `src.locality := max(src.locality, dst.locality)`, `src.cardinality := seq_add(src.cardinality, dst.cardinality)`, `src.consumption := seq_add(src.consumption, Affine)` |
| TF-15 | `Set { base, field, value }`: in-place mutation; backward demand `(base, Once)` + `(value, Once, Linear)`; `value` access promoted to `Owned` |
| TF-15a | `SetTag { base, tag }`: in-place tag mutation; backward demand `(base, Once)` only |

### §5 Canonicalization Rules

Canonicalization runs after every join and every transfer function, applied in a bounded loop until a fixed point is reached.

| Rule | Effect |
|------|--------|
| CN-1 | Dead ↔ Absent bidirectional: `Consumption = Dead ⟹ Cardinality := Absent` and `Cardinality = Absent ⟹ Consumption := Dead` |
| CN-2 | Linear + Absent infeasible: `Consumption = Linear ∧ Cardinality = Absent ⟹ Consumption := Dead` |
| CN-3 | Shared blocks reuse: `Uniqueness = Shared ∧ Shape ≠ NonReusable ⟹ Shape := NonReusable` |
| CN-4 | Reserved (former optimistic uniqueness promotion was removed for monotonicity) |
| CN-5 | Unique + Dead preserves reusable shape: no rule shall collapse shape for `Unique + Dead` states |
| CN-6 | Wide-locality uniqueness ceiling: `Locality ≥ HeapEscaping ∧ Uniqueness = Unique ⟹ Uniqueness := MaybeShared` |
| CN-7 | Reserved (former Shared+CollectionBuffer COW-mode rule was removed; canonicalization shall mutate lattice dimensions only, not decision predicates) |
| CN-8 | Borrowed locality ceiling: `Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal` |

NOTE  CN-8 fires before CN-6 to ensure locality is precise when CN-6 evaluates.

### §6 Pipeline Ordering

Each ordering constraint is load-bearing.

| Rule | Constraint |
|------|-----------|
| PL-1 | Steps 1-2 (interprocedural) shall run once across all functions before any per-function step |
| PL-1a | Per-function pipeline (Steps 3-12) shall process functions in SCC topological order (callees before callers) |
| PL-2 | Step 4 (analysis) shall precede Step 5 (realization) |
| PL-3 | Step 5 (realization phase 1) shall precede Step 9 (block merge) |
| PL-4 | Step 10 (realization phase 2) shall follow Step 9 (block merge) |
| PL-4a | Step 8a (unwind cleanup) shall precede Step 9 (block merge) |
| PL-5 | No pass shall rely on stale summaries |
| PL-6 | Adding a new pass requires updating the pipeline ordering and proving non-violation of existing constraints |
| PL-7 | TRMC normalization (Step 3a) shall detect tail-recursive functions returning constructor applications |
| PL-8 | TRMC candidate predicate: self-recursive, recursive call in tail position of constructor argument, constructor in return path |
| PL-9 | TRMC rewrite: candidate function internally normalized to accept a `ContextHole` parameter; external arity preserved via wrapper thunk |
| PL-10 | TRMC structural verification: shall confirm context-hole threading, no allocation introduced, well-formed CFG, arity preserved, evaluation order unchanged |
| PL-11 | TRMC verification failure shall roll back to pre-TRMC IR and re-run Steps 3-4 |

### §7 Interprocedural Contracts

| Rule | Constraint |
|------|-----------|
| IC-1 | Call graph shall be decomposed into SCCs and processed in topological order (callees before callers) |
| IC-2 | Each parameter initializes to most optimistic: `(Borrowed, Dead, Absent, BlockLocal, Unique, may_share=false)` |
| IC-3 | Parameter contract join is componentwise max: `access`, `consumption`, `cardinality`, `locality`, `uniqueness` use `max`; `may_share` uses OR |
| IC-4 | Return contract: `uniqueness` (join), `preserves_freshness` (AND), `locality` (join), `shape` (join) |
| IC-5 | Effect summary: componentwise OR over `may_allocate`, `may_deallocate`, `may_share`, `may_throw`, `has_unbounded_stack`, `may_read_inaccessible` |
| IC-6 | FIP contract: `Never` absorbs all; `Conditional` absorbs `Bounded` and `Certified`; `Bounded(n) ⊔ Bounded(m) = Bounded(max(n, m))`; `Certified` ⟺ zero unmatched allocations / deallocations in realized IR |
| IC-7 | Convergence: finite domain guarantees termination; iteration bound derived from domain heights |
| IC-8 | Reserved (former rule deriving parameter uniqueness from caller consumption was removed for soundness) |
| IC-8a | Address-taken functions and closures: parameters initialized to CONSERVATIVE when call sites cannot be fully enumerated |

### §8 Realization Rules

#### §8.1 Reference-count emission

| Rule | Constraint |
|------|-----------|
| RL-1 | Reference-count increment shall be emitted when a value is duplicated |
| RL-2 | Reference-count decrement shall be emitted at the last use of an owned value or at scope exit, unless the last use is an ownership-transferring instruction |
| RL-3 | Reference-count operations shall be elided when the lattice proves them unnecessary |
| RL-4 | Edge-specific decrements: an owned non-scalar variable alive at block exit but dead at successor entry shall receive a decrement on that CFG edge |
| RL-5 | Dead-at-entry cleanup: an owned non-scalar block parameter with `Cardinality = Absent` at entry shall receive an immediate decrement |

#### §8.2 Copy-on-write

| Rule | Constraint |
|------|-----------|
| RL-6 | Static unique mutation (`Uniqueness = Unique`): emit in-place mutation; no IsShared check |
| RL-7 | Dynamic copy-on-write (`Uniqueness = MaybeShared`): emit IsShared check, branch to in-place or copy paths |
| RL-8 | Static shared mutation (`Uniqueness = Shared`): emit unconditional copy before mutation |
| RL-9 | Copy-on-write compound contraction: diamond CFG patterns shall be contracted into a single compound instruction |
| RL-10 | Disjoint field mutation shall not trigger copy-on-write when receiver is mutated at field F and all active borrows are from different fields |

#### §8.3 Allocation reuse

| Rule | Constraint |
|------|-----------|
| RL-11 | Same-block reuse: dying value's allocation shall be reused for a fresh allocation of the same type, given dominance and uniqueness |
| RL-11a | Dynamic reuse: `MaybeShared` values use IsShared check; unique at runtime takes Reset / Reuse fast path |
| RL-12 | Cross-block reuse: dying value's allocation shall be reused across blocks under dominance, post-dominance, and no-throw constraints |
| RL-13 | Reserved (former cardinality-based reuse rule was removed for soundness) |

#### §8.4 Stack promotion

| Rule | Constraint |
|------|-----------|
| RL-14 | Non-escaping fixed-size unique allocations (`Locality ≤ FunctionLocal ∧ Uniqueness = Unique`) shall be stack-allocated via `alloca` with no reference-count header |
| RL-14a | Non-escaping fixed-size non-unique allocations shall be stack-allocated with reference-count header initialized to `MAX_REFCOUNT` (immortal) |
| RL-15 | Non-escaping dynamic-size allocations shall use a function-local bump allocator |
| RL-15a | ArgEscaping allocations shall be stack-allocated in the caller; header strategy depends on callee parameter contract |
| RL-16 | Escaping allocations (`Locality ≥ HeapEscaping`) shall be heap-allocated with full reference-count header |

#### §8.5 Reference-count header compression

| Rule | Constraint |
|------|-----------|
| RL-17 | Sharing-bound analysis shall determine maximum simultaneous reference count (none, `Bounded(N)`, or `Unbounded`) |
| RL-18 | Reference-count header width shall be narrowed based on RL-17's result: none / `i8` / `i16` / `i32` / `i64`; ABI-visible types shall use full-width headers |

#### §8.6 Unified representation constraint

| Rule | Constraint |
|------|-----------|
| RL-18a | All escape-driven decisions shall consume the `Locality` dimension as primary input; parallel per-variable escape enumerations are forbidden |

#### §8.7 Non-atomic reference count

| Rule | Constraint |
|------|-----------|
| RL-19 | Thread-local values shall use non-atomic reference-count operations (plain load / store) |
| RL-20 | Thread-shared values shall use atomic reference-count operations |
| RL-21 | Programs with no spawn / channel / FFI export shall use non-atomic reference-count operations for all values |

#### §8.8 KnownSafe pair elimination

| Rule | Constraint |
|------|-----------|
| RL-22 | When the physical reference-count is provably positive, inner increment / decrement pairs on the same variable shall be eliminated |
| RL-23 | KnownSafe flag propagation at join points: `true` only if all predecessors agree |

#### §8.9 PRE-style global reference-count motion

| Rule | Constraint |
|------|-----------|
| RL-24 | Bidirectional dataflow shall identify matching `(Inc, Dec)` pairs across basic blocks |
| RL-25 | A pair is eliminable when KnownSafe holds, or both forward and backward paths are safe and no CFG hazard exists |
| RL-26 | Reference-count motion shall not move operations across reference-count-observable barriers |

#### §8.10 Selective barriers

| Rule | Constraint |
|------|-----------|
| RL-27 | At call sites, reference-count operations shall be flushed for variables whose callee parameters are `Owned` or `Borrowed` with `may_share = true` |
| RL-28 | Unknown callees shall trigger conservative flush of all pending reference-count operations |

#### §8.11 LLVM fact export

| Rule | Constraint |
|------|-----------|
| RL-29 | Fresh allocation returns shall be marked with LLVM `noalias` |
| RL-30 | Effect-based call annotations shall derive LLVM `memory(...)` attributes from `IC-5` and parameter contracts |
| RL-31 | Disjoint borrowed parameters shall receive `!alias.scope` and `!noalias` metadata |

#### §8.12 Borrow inference

| Rule | Constraint |
|------|-----------|
| RL-32 | All non-scalar parameters initialize to `Borrowed`; fixed-point iteration promotes to `Owned` based on demand |
| RL-33 | Projection propagation: if a projected field becomes `Owned`, the source variable shall be promoted to `Owned` |
| RL-34 | Tail-call preservation: reference-count decrement shall not be inserted after a tail call; ownership shall transfer instead, restricted to `Owned` callee parameters |

### §9 Verification Layers

The verification stack is layered. Each layer catches a different class of inconsistency.

| Rule | Layer |
|------|-------|
| VF-1 | Layer 1 (Structural): ARC IR well-formedness — use-before-def, dangling block refs, reference-count on scalar, decrement on borrowed parameter, argument-ownership length mismatch |
| VF-2 | Layer 2 (AIMS Contract): independent contract-consistency checks against the realized IR |
| VF-3 | Layer 3 (Oracle): re-derives `MemoryContract` from realized IR and compares against inferred contract |
| VF-4 | Layer 4 (FIP Certification): proves `FipContract::Certified` functions have zero unmatched allocations / deallocations |
| VF-5 | Every active subsystem shall be end-to-end verified: implementation + invariant enforcement + tests |
| VF-6 | Contracts and realization shall agree |
| VF-7 | Active rewrites shall be sound: identical observable behavior; structural verification + behavioral tests + documented proof sketch |
| VF-8 | The verification stack applies to all rules in this section, including target rules; an unimplemented rule without a planned verification layer is a spec gap |

### §10 Active Subsystems

The following AIMS subsystems are shipped and end-to-end verified:

- Reference-count emission and elision (RL-1..RL-5)
- Copy-on-write static and dynamic paths (RL-6..RL-10)
- Same-block and cross-block allocation reuse (RL-11..RL-12)
- TRMC tail-recursion-modulo-cons rewrite (PL-7..PL-11)
- KnownSafe pair elimination and PRE-style global reference-count motion (RL-22..RL-26)
- Selective barriers at call sites (RL-27..RL-28)
- Borrow inference (RL-32..RL-34)
- Immortal pre-pass and FBIP certification (IC-6, VF-4)

### §11 Target Subsystems

The following AIMS subsystems are designed but not yet shipped. Annex E's informative status accommodates target-only rules without imposing pre-shipping conformance.

- Stack promotion for `BlockLocal` / `FunctionLocal` allocations (RL-14, RL-14a, RL-15)
- ArgEscaping caller-stack allocation (RL-15a)
- Reference-count header compression (RL-17, RL-18)
- Non-atomic reference count for thread-local programs (RL-19, RL-21)
- AIMS-to-LLVM fact export: `noalias`, `memory(...)`, alias-scope metadata (RL-29, RL-30, RL-31)
- Provenance-partition ledger emission (§12)

### §12 Provenance-Partition Ledger

The provenance-partition ledger is the machine-checked foundation for reference-count placement over value provenance. The emission pass that consumes it is a target subsystem (§11); the underlying calculus — five theorem families and their composition extensions — is proven. Two objects define the model:

- **Partition** — per-(variable, field-path) classes keyed by allocation birth site, built as a union-find over semantic alias edges.
- **Ledger** — per-class event sequences (birth, credit, consume, read, mutate) derived over the CFG — normal, back-edge, and unwind edges, plus TRMC regions — from a fixed terminal-use classification table, never re-classified per emission site.

The proven theorem families:

1. **Partition soundness.** Two nodes shall share a partition class only when they share an allocation birth site. Phi / select merges shall be admitted only under a singleton-birth-site witness; a merge over distinct birth sites is inadmissible.
2. **Compositional placement.** A placement satisfying three clauses shall be safe on every CFG walk, including unwind-fed merges and TRMC back-edge loops of any iteration count: per-path net zero (no leak); running count at least one at every read (no use-after-free); running count at least one plus the live same-class sibling count at every mutation (no copy-on-write corruption). The three clauses are equivalent to ledger safety. Relocating a release past an unwind-fed merge is rejected.
3. **Keep-alive whole-pair elision.** A keep-alive increment / decrement pair shall be elided only as a whole, and only when a live same-class sibling keeps the interior running count balanced and never below one; eliding the increment alone provably frees early or nets negative. Same-class sibling liveness is the dominating-increment evidence that KnownSafe pair elimination (§8.8) consumes.
4. **Contract-boundary composition.** Boundary events at a call shall be classified through the callee's parameter contract (§7) via the fixed terminal-use table: an owned parameter is a birth on the callee side; an owned argument is a consume; an argument to an iterator-consuming parameter is a consume; a borrowed argument is a read; a transfer-through-return pairs a consume at the call with a credit at the return (net zero); a sharing-view producer is a credit. Given caller-clause satisfaction, callee conformance, and liveness at the call, the composed ledger satisfies the placement clauses without re-deriving the callee body. Classifying an owned argument as borrowed produces a double release and is rejected.
5. **Frame-limited robustness.** Introducing an alias edge that merges two partition classes shall leave every other class's derived ledger verbatim. The merged class's net is unconditionally additive; the merged class preserves all three placement clauses only when both prior classes are count-nonnegative and mutation-free. Unconditional preservation of the read and mutation clauses under merges does not hold.

The composition extensions integrate the partition as a side table without weakening the elimination calculus. Class-grain refinement gated to a subset of the lattice's elimination verdict preserves single elimination and analysis-state immutability; an eliminator outside the lattice's verdict set provably breaks that guarantee — the machine-checked form of invariant 5 (§2). The partition pre-pass sits between analysis and realization (§6) and flows without stale summaries; an appended partition verification layer (§9) only rejects more, and the class assignment remains a complete, distinct partition.
