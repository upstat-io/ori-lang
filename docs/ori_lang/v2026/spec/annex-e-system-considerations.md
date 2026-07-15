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
6. Logical ownership-bearing classification is determined by semantic containment, not representation size; a counter-based compiled projection may use that fact to decide where counter operations are meaningful (see [Memory Model § Type Classification](21-memory-model.md#217-type-classification))

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

## AIMS — Backend-Neutral Ownership Calculus

NOTE  Annex E is informative. Rules in this section using `shall` / `shall not` document the AIMS algorithm and its invariants. Target subsystems documented in §11 describe design targets; implementations conforming to a given Ori build need not satisfy target rules until those subsystems ship.

NOTE  “ARC Intelligent Memory System” is a historical expansion of the AIMS
name from its first compiled counter projection. It is not normative: AIMS is
neither an LLVM analysis nor a requirement that an admitted physical plan use
reference counting.

### §1 Mission and Design Center

AIMS is Ori's AIMS memory model — one calculus over logical ownership, lifetime, cleanup, transfer, sharing, reuse, effect, and unwind obligations. Its laws — the lattice dimensions and their algebra (§3), the transfer functions (§4), the canonicalization rules (§5), the pipeline ordering (§6), the interprocedural contracts (§7), the realization rules (§8), and the verification layers (§9) — are stated and proven against the AIMS product lattice, an object no prior system defines; the calculus inherits no proof, rule, or law.

Its calculus, soundness proofs, and proof checker are Ori's own contribution; the design drew on argument-shape patterns from prior compilers (Lean 4, Koka, Swift, GHC, OxCaml, Clang, Racordon) as historical influences, not architectural dependencies.

The design center is to make surviving logical ownership events rare and individually justified, not to make a chosen physical operation faster. In the current counter-based compiled projection, that goal manifests as rare emitted reference-count operations; another satisfying physical plan may encode the same exact facts without a counter.

AIMS is backend-neutral. It freezes logical ownership, lifetime, event, cleanup, reachability, and visibility facts exactly once; it does not prescribe LLVM IR, VM bytecode, JIT machinery, WebAssembly instructions, allocation primitives, object headers, counter widths, or synchronization operations.

VM and compiled targets consume the same frozen facts, choose their own physical mechanisms, and prove those mechanisms satisfy the common contract. A target-specific spelling is never an AIMS fact and shall not be re-derived as a parallel backend calculus.

The Ori programmer never writes ownership annotations, lifetime markers, or borrow syntax. AIMS infers ownership, locality, uniqueness, and reuse opportunities from a unified product lattice and propagates the results across function boundaries through interprocedural contracts.

Memory safety holds across the entire program surface, including FFI; `unsafe` relaxes type-level guarantees but never permits memory unsafety.

### §2 Five Load-Bearing Invariants

1. **Contract and realization shall agree.** A function whose `MemoryContract` records `FipContract::Certified` shall have zero unmatched logical storage-acquisition/allocation obligations and lifetime-end cleanup/release obligations in the realized AIMS plan.
2. **Active rewrites shall be sound.** Every active rewrite (TRMC, ownership-event motion, KnownSafe elimination, COW contraction) shall preserve identical observable behavior, with structural verification at compile time and behavioral verification at test time.
3. **No pass shall rely on stale summaries.** Pipeline ordering is load-bearing: when a step modifies IR or updates an effect summary, all downstream consumers shall see the updated value.
4. **Every active subsystem shall be end-to-end verified.** Implementation, invariant enforcement, and behavioral tests are all required; the absence of any one is a spec gap.
5. **The unified model shall stay unified.** New capabilities shall extend a lattice dimension, extend a contract field on `MemoryContract` / `ParamContract` / `ReturnContract` / `EffectSummary`, or feed the lattice-driven analysis as a typed pre-pass input. Independent ownership-event emission paths, parallel escape enumerations, and shadow uniqueness trackers are forbidden.

### §3 Lattice Dimensions

The AIMS lattice is a product of finite-height lattices; product join is componentwise join followed by canonicalization (§5). Every dimension shall have finite height. Transfer functions (§4) shall be monotone. Canonicalization (§5) shall be idempotent and shall preserve join results.

NOTE  Adding a new dimension requires proving finite height, defining the join, updating canonicalization, and proving the new product lattice satisfies commutativity, associativity, idempotence, and antisymmetry.

#### §3.1 Access

Ownership versus borrowing.

| Value | Meaning | Logical ownership obligation |
|-------|---------|------------------------------|
| `Borrowed` | Temporary view of another value | No owner credit; source lifetime governs |
| `Owned` | Carries one owner credit | Transfer or discharge that credit exactly once |

Order: `Borrowed < Owned`. Join: `max`. Height: 1.

#### §3.2 Consumption

Substructural mode.

| Value | Meaning | Logical ownership implication |
|-------|---------|-------------------------------|
| `Dead` | Not live at this point | No ownership event |
| `Linear` | Consumed exactly once (moved) | Transfer once or discharge at death; no duplicate credit |
| `Affine` | May be dropped without use | At most one transfer; discharge if not transferred |
| `Unrestricted` | Freely copied and dropped | Account for every credit creation and discharge |

Order: `Dead < Linear < Affine < Unrestricted`. Join: `max`. Height: 3.

#### §3.3 Cardinality

Forward usage count.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Absent` | Never used after this point | Skip ownership events |
| `Once` | Used exactly once | Move semantics |
| `Many` | Used multiple times or in a loop | Account for every logical duplication and use |

Order: `Absent < Once < Many`. Join: `max`. Height: 2. Sequential composition (`seq_add`) follows the QTT semiring: `Absent + x = x`, `Once + Once = Many`, `Once + Many = Many`, `Many + Many = Many`.

#### §3.4 Uniqueness

Logical owner multiplicity at the realized AIMS boundary. This dimension does not
name a counter representation or require any particular runtime encoding.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `Unique` | Provably exactly one logical owner credit | Static copy-on-write fast path; reset / reuse eligibility |
| `MaybeShared` | Logical owner multiplicity is not statically known | Dynamic sharing observation required |
| `Shared` | Provably more than one logical owner credit | Static copy-before-mutation path |

Order: `Unique < MaybeShared < Shared`. Join: `max`. Height: 2.

NOTE  Uniqueness is a past guarantee ("not logically duplicated"), distinct from Linearity which is a future guarantee ("consumed once"). A VM handle flag, reference count, tagged header, side table, or compiled counter is a projection mechanism that must satisfy this fact; it is not the fact itself.

#### §3.5 Locality

Escape classification.

| Value | Meaning | Optimization |
|-------|---------|--------------|
| `BlockLocal` | Does not escape defining basic block | Lifetime bound no wider than the defining block |
| `FunctionLocal` | Does not escape defining function | Lifetime bound no wider than the defining function |
| `ArgEscaping` | Escapes through arguments within known caller uses | Exact caller-use extent when known |
| `HeapEscaping` | May escape beyond every proven block, function, or caller extent | Logical lifetime is escaping; physical realization remains target-owned |
| `Unknown` | Conservative default | Unknown lifetime; physical realization must satisfy the conservative facts |

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
| `may_alloc` | May create a logical allocation or storage-acquisition obligation | FIP certification |
| `may_share` | May create shared references | Uniqueness preservation |
| `may_throw` | May throw / panic | Cleanup-path correctness |

Three independent boolean flags. Join: componentwise OR. Height: 3.

`may_alloc` does not imply heap placement or any particular target allocation primitive. It records a backend-neutral effect obligation that each executor realizes under its own validated physical plan.

### §4 Transfer Functions

Transfer functions define how each ARC IR instruction updates the lattice state. There are two directions: forward (definition) and backward (demand). Every ARC IR instruction variant shall have an explicit forward and backward rule; adding a new instruction variant without corresponding TF rules shall be a spec gap.

| Rule | Purpose |
|------|---------|
| TF-1 | Scalar literal: `dst.state := SCALAR` |
| TF-2 | Variable binding: `dst.state := state(v)` (alias) |
| TF-2a | Scalar PrimOp: typed descriptor `result = Scalar`; `dst.state := SCALAR` |
| TF-2b | Typed non-scalar PrimOp: descriptor-directed independent-owned result or operand alias; never inferred from physical representation |
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
| TF-10a | Reset: `dst := SCALAR` (opaque logical reuse witness; no owner credit or physical-handle meaning) |
| TF-11 | Standard backward demand: `(operand, Once, Linear)` per argument; `seq_add` accumulation |
| TF-11a | Terminator backward demands (Return / Jump / Branch / Switch / Invoke / Resume / Unreachable) |
| TF-12 | PartialApply emits no standard backward demand; capture handled by TF-13 |
| TF-13 | `capture_state_update`: closure capture rule with access promotion when logical locality is `HeapEscaping` or `Unknown` |
| TF-14 | Project backward propagation: `src.locality := max(src.locality, dst.locality)`, `src.cardinality := seq_add(src.cardinality, dst.cardinality)`, `src.consumption := seq_add(src.consumption, Affine)` |
| TF-15 | `Set { base, field, value }`: in-place mutation; backward demand `(base, Once)` + `(value, Once, Linear)`; `value` access promoted to `Owned` |
| TF-15a | `SetTag { base, tag }`: in-place tag mutation; backward demand `(base, Once)` only |

A primitive operation shall carry one backend-neutral typed ownership descriptor. The descriptor classifies result ownership as `Scalar`, `IndependentOwned`, `OwnedFromConsumedOrIndependent(inputs)`, or `Alias(input)`; classifies each operand use as `Borrow` or `Consume`; and separately classifies allocation as `None`, `MayAllocate`, or `StrategyDependent`. A logical independent-owned result is not a physical-allocation claim. For a strategy-dependent dual-consuming operation, an implementation may take over storage from either descriptor-declared consumed input or allocate independently, but every admitted row shall consume both input obligations exactly once and produce exactly one result obligation.

Descriptor validation shall fail closed before analysis or execution. Missing descriptors, arity mismatch, out-of-range alias or takeover indices, an empty takeover-candidate set, a takeover candidate whose operand use is not `Consume`, and incoherent result/allocation combinations are invalid. AIMS resolves the descriptor once; every physical executor projects the resulting ownership facts and shall not reconstruct them from runtime symbols, target layout, or value representation.

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
| IC-4 | Return contract: `uniqueness` (join), `preserves_freshness` (AND), `locality` (join), `shape` (join), and path-universal provenance facts. `returns_fresh_self_alloc` uses AND and is true only when every return path yields fresh storage with no upstream alias, either directly or through a callee carrying the same proof; forwarding caller-owned or consumed storage sets it false |
| IC-5 | Effect summary: componentwise OR over `may_allocate`, `may_deallocate`, `may_share`, `may_throw`, `has_unbounded_stack`, `may_read_inaccessible` |
| IC-6 | FIP contract: `Never` absorbs all; `Conditional` absorbs `Bounded` and `Certified`; `Bounded(n) ⊔ Bounded(m) = Bounded(max(n, m))`; `Certified` ⟺ zero unmatched logical storage-acquisition/allocation obligations and lifetime-end cleanup/release obligations in the realized AIMS plan |
| IC-7 | Convergence: finite domain guarantees termination; iteration bound derived from domain heights |
| IC-8 | Reserved (former rule deriving parameter uniqueness from caller consumption was removed for soundness) |
| IC-8a | Address-taken functions and closures: parameters initialized to CONSERVATIVE when call sites cannot be fully enumerated |

**Indirect-call boundary contract (PV-4).** Explicit operands use one caller-borrowed logical call contract: the caller retains its ownership obligation and the call contributes a logical read for each operand. The closed target identity shall carry an exact frozen inward-owner demand for every explicit parameter, derived once from the full final interprocedural parameter contract. `Owned`, borrowed iterator consumption without return transfer, and borrowed COW consumption demand one whole-value owner; projected-field iterator consumption demands exactly that field's owner; a plain borrow demands none. A simultaneous whole-value and projected-field demand is contradictory and shall fail closed. Target execution shall discharge every demanded owner credit on both normal and unwind exits and shall not discharge a plain borrow. IC-8a's CONSERVATIVE contract therefore selects the whole-value row.

Each normal return site shall also freeze one stable owner relation: `Independent`, `Direct(parameter)`, `ProjectedField(parameter, field)`, or `Contained(parameter)`. Parameter and field identities are semantic contract identities, not physical slots or byte offsets. Each owned result root and each owned payload edge shall name exactly one source: `IndependentTargetBirth`, `EntryCredit`, `TargetFunded`, or `NeedsResultCredit`. `IndependentTargetBirth` requires explicit fresh-target-birth evidence and a complete owned-root plus owned-payload-edge certificate; absence of an input relation is not evidence. `EntryCredit` transfers the already-created inward owner. `TargetFunded` requires an exact target exit-summary or class-ledger fact identifying funding for the owned root and every owned payload edge. `NeedsResultCredit` is a normal-only adapter obligation admitted only for otherwise-unfunded direct or projected borrowed results. Unfunded containment shall fail closed.

Exactly one source shall fund each normal owned result; simultaneous entry and target funding shall fail closed. A normal path shall export exactly one logical owner per certified root or owned payload edge. An unwind path shall export no result, perform no result-credit action, and discharge an entry credit exactly once when that credit was the result source. Missing or malformed target facts, incomplete independent or target funding, missing or extra credits, contradictory topology, double funding, or discharge that is not path-total shall fail closed. These reads, credits, semantic identities, sources, and discharges are backend-neutral AIMS obligations. AIMS does not prescribe a retain instruction, counter, object layout, physical offset, calling convention, VM opcode, runtime helper, or unwind mechanism; each admitted executor shall project and verify its own realization from the same frozen fact identities.

### §8 Realization Rules

#### §8.1 Logical ownership-event realization

| Rule | Constraint |
|------|-----------|
| RL-1 | A logical owner-credit event shall be recorded when a value is duplicated without transferring an existing credit |
| RL-2 | A logical owner-release event shall be recorded at the last use of an owned value or at scope exit, unless the last use transfers that credit. `ApplyToIterConsumingParam` is a distinct inward-transfer kind. For `Invoke`, argument transfer occurs before the call and applies to both normal and unwind exits; only a normal return may produce result credit |
| RL-3 | Logical ownership events shall be elided when the lattice proves that the corresponding credit topology is already balanced |
| RL-4 | Edge-specific release: an owned non-scalar variable alive at block exit but dead at successor entry shall receive a logical release event on that CFG edge |
| RL-5 | Dead-at-entry cleanup: an owned non-scalar block parameter with `Cardinality = Absent` at entry shall receive an immediate logical release event |

The current compiled counter projection spells some RL-1 through RL-5 events as
`RcInc` and `RcDec`. Those names and operations are a target realization, not the
AIMS contract. A region, arena, transfer, tracing, or VM-specific plan may satisfy
the same exact event topology without materializing a reference counter.

#### §8.2 Copy-on-write

| Rule | Constraint |
|------|-----------|
| RL-6 | Static unique mutation (`Uniqueness = Unique`) shall freeze an in-place-admissible logical outcome with no sharing-observation obligation |
| RL-7 | Dynamic copy-on-write (`Uniqueness = MaybeShared`) shall freeze a sharing-observation obligation: the one-owner outcome admits in-place mutation and the shared outcome requires an independent value before mutation; physical plans choose the observation, branch, and copy mechanisms |
| RL-8 | Static shared mutation (`Uniqueness = Shared`) shall require an independent logical result before mutation; AIMS does not choose a copy helper, buffer, allocation, or instruction sequence |
| RL-9 | A logical COW decision may contract its expanded diamond into one compound obligation only when observation, ownership, mutation, cleanup, and unwind identities are preserved; physical plans choose the encoding |
| RL-10 | Disjoint field mutation shall not trigger copy-on-write when receiver is mutated at field F and all active borrows are from different fields |

#### §8.3 Allocation reuse

| Rule | Constraint |
|------|-----------|
| RL-11 | Same-block reuse: given dominance, type compatibility, and logical uniqueness, AIMS shall freeze the dying-allocation donor/recipient eligibility relation and exact owner-credit transfer |
| RL-11a | Dynamic reuse: `MaybeShared` values require a logical sharing observation; the one-owner outcome admits the Reset / Reuse transfer and the shared outcome preserves the original allocation |
| RL-12 | Cross-block reuse: under dominance, post-dominance, and no-throw constraints, AIMS shall record the cross-block donor/recipient transfer and its path obligations |
| RL-13 | Reserved (former cardinality-based reuse rule was removed for soundness) |

The selected VM or compiled layout plan chooses how an admitted RL-11/RL-12
transfer recycles storage and proves that mechanism satisfies the frozen logical
transfer. AIMS does not mandate a header, counter, allocator, arena, or instruction
sequence.

#### §8.4 Allocation and lifetime facts

Final realization freezes one backend-neutral product per logical allocation:

`AllocationFacts { site, locality, lifetime, owners, ownership_observations, cleanup, thread, visibility }`

`site` is the stable logical allocation/birth-site identity, never a target storage site. `lifetime` is `Block(block-id)`, `Function`, `CallerExtent(nonempty exact caller-use list)`, `Escaping`, or `Unknown`. Each caller use records a stable call-site identity and its `BorrowOnly`, `MayShare`, or `OwnershipTransfer` protocol. `owners` is a dynamic `OwnerBound`. `ownership_observations` is `OwnershipObservationFacts::Exact`, preserving stable additional-credit, release, and sharing-observation event identities plus whether owner multiplicity is externally observable, or it is explicitly `Unknown`. It neither selects nor requires a counter, header, side table, atomic operation, or opcode. `cleanup` records exact release events, drop-plan identity, field traversal order, normal and unwind exits, and logical lifetime-end events, or is explicitly `Unknown`. `thread` is `Confined` or `PotentiallyShared`. `visibility` is `Internal`, `CrossModule`, `ForeignOrOpaque`, or `Unknown`.

Representation extent is deliberately absent from `AllocationFacts`. `ExtentClass = StaticShape(type-id) | RuntimeSized(storage-site-id)` is representation-owned input supplied alongside the facts to a physical planner. Missing or conflicting evidence shall freeze the conservative defaults `LifetimeBound::Unknown`, `OwnerBound::Unbounded`, `OwnershipObservationFacts::Unknown`, unknown cleanup obligations, `ThreadReachability::PotentiallyShared`, and `ExternalVisibility::Unknown`.

| Rule | Constraint |
|------|-----------|
| RL-14 | Converged `Locality` shall freeze a logical `LifetimeBound` and preserve allocation-site identity. `BlockLocal`, `FunctionLocal`, `HeapEscaping`, and `Unknown` map to block, function, escaping, and unknown bounds respectively; `ArgEscaping` maps through RL-15a's exact caller-use rule. The rule shall not select stack, arena, region, managed, or heap placement |
| RL-14a | An exact `CleanupObligation` shall preserve every logical release-event identity, drop-plan identity, field traversal order, and required normal, unwind, and lifetime-end event. Missing cleanup evidence shall remain `Unknown`; no target may invent or silently discard an obligation |
| RL-15 | `ExtentClass` shall remain representation-owned projection input and shall not be stored or re-derived as an AIMS allocation fact. Changing extent input cannot change the frozen `AllocationFacts` |
| RL-15a | `ArgEscaping` with complete caller evidence shall freeze a nonempty exact `CallerExtent`, preserving every call-site identity and borrow/share/transfer protocol. Missing caller-use evidence shall map to `Unknown` |
| RL-16 | Missing, conflicting, or site-mismatched evidence shall produce the conservative `AllocationFacts` defaults above and shall not select a physical fallback mechanism |

#### §8.5 Owner bounds and physical-plan satisfaction

`OwnerBound::Bounded(extra)` means at most `extra + 1` simultaneous owners; `Unbounded` carries no finite capacity proof. A target-owned `LayoutCapabilities` advertises lifetime and representation-extent coverage, owner capacity, the exact `OwnershipObservationFacts` and cleanup protocols, unwind coverage, thread safety, visibility coverage, and stable site/contract identity. `Satisfies(facts, extent, capabilities)` is the admission relation. A `VmLayoutPlan` or `CompiledLayoutPlan` is validated only when it carries a proof of this relation. A stronger capability may replace a weaker one only while preserving site, the exact ownership-observation and cleanup contracts, and external-contract identity.

| Rule | Constraint |
|------|-----------|
| RL-17 | `OwnerBound` shall be a dynamic upper bound. With no loop/global/external-retention path, `N` exact straight-line logical credit-creation sites imply `Bounded(N)` and local uniqueness implies `Bounded(0)`. A loop or global path, external retention, or unknown evidence shall force `Unbounded`; that result takes precedence over local uniqueness |
| RL-18 | Every VM or compiled physical layout/ownership plan shall prove `Satisfies` against the same frozen `AllocationFacts` and representation-owned `ExtentClass`. AIMS shall not choose the plan's placement, counter representation, metadata layout, synchronization primitive, or instruction spelling |

#### §8.6 Projection parity and single-source facts

The exact logical trace of a validated plan is `AimsTrace { ownership_observations, cleanup }`. A target may coalesce or specialize physical operations only when its validation preserves every externally observable additional-credit, release, and sharing-observation event and every cleanup identity represented by that trace.

| Rule | Constraint |
|------|-----------|
| RL-18a | VM, native, JIT, compiled WebAssembly, and LLVM projections may select different physical mechanisms, but each shall consume the same frozen facts, prove `Satisfies`, and erase to the same exact AIMS trace. Parallel backend derivation of ownership, lifetime, additional-credit/release/sharing-observation, cleanup, or reachability facts is forbidden |

#### §8.7 Thread reachability and target capability

Thread reachability is a logical fact, not a prescribed reference-count implementation. A target may use any mechanism whose advertised capability satisfies the fact.

| Rule | Constraint |
|------|-----------|
| RL-19 | `ThreadReachability` shall be derived from converged `Locality` plus call-graph evidence for spawn, channel, foreign, and opaque thread-boundary paths. A proven boundary or `Locality::Unknown` shall yield `PotentiallyShared`; otherwise it yields `Confined` |
| RL-20 | `PotentiallyShared` shall require a shared-safe target capability. `Confined` may use a confined-specialized or conservative shared-safe capability. AIMS shall not mandate atomic reference counting, memory ordering, a lock, ownership transfer, or any other physical mechanism |
| RL-21 | A whole-program proof of no spawn, channel, foreign, or opaque thread boundary shall freeze every allocation as `Confined`; physical mechanism selection remains target-owned and subject to `Satisfies` |

#### §8.8 KnownSafe pair elimination

| Rule | Constraint |
|------|-----------|
| RL-22 | When the exact logical owner-credit balance proves at least one live ownership credit, inner credit / debit pairs on the same variable shall be eliminated. This does not require a physical reference counter |
| RL-23 | KnownSafe flag propagation at join points: `true` only if all predecessors agree |

#### §8.9 PRE-style global ownership-event motion

| Rule | Constraint |
|------|-----------|
| RL-24 | Bidirectional dataflow shall identify matching logical `(Credit, Release)` event pairs across basic blocks |
| RL-25 | A pair is eliminable when KnownSafe holds, or both forward and backward paths are safe and no CFG hazard exists |
| RL-26 | Logical ownership-event motion shall not cross ownership-observable barriers |

#### §8.10 Selective barriers

| Rule | Constraint |
|------|-----------|
| RL-27 | At call sites, pending logical ownership events shall be flushed for variables whose callee parameters are `Owned` or `Borrowed` with `may_share = true`; the latter may observe sharing or introduce an additional-credit, release, or sharing-observation obligation |
| RL-28 | Unknown callees shall trigger conservative flush of all pending logical ownership events |

#### §8.11 Backend-neutral AIMS fact export

AIMS contracts and the realized ownership plan are typed, backend-neutral compiler facts. Final realization shall compute each fact once. VM, native, compiled WebAssembly, JIT, and LLVM consumers shall transport and project those exact facts without re-running ownership analysis.

RL-29 through RL-31 define backend-neutral fact obligations. LLVM attributes and metadata are a separate target projection; their spellings neither define nor narrow the AIMS calculus.

| Rule | Constraint |
|------|-----------|
| RL-29 | Final realization shall freeze `FreshSelfAllocationFacts` from `IC-4.returns_fresh_self_alloc`. Preserved freshness or uniqueness alone is insufficient; parameter passthrough and possible takeover of consumed storage remain unproven |
| RL-30 | Final realization shall freeze `MemoryAccessFact` from `IC-5`, parameter contracts, and realized operations. `ReadOnly` proves absence of writes but permits both argument and inaccessible-memory reads. `may_throw = true` or `may_write_inaccessible = true` forces `ReadWrite`; every call, I/O, panic/TLS operation, or runtime operation without a typed write-effect descriptor shall fail closed |
| RL-31 | Final realization shall freeze parameter-disjointness facts for disjoint borrowed parameters from common type and provenance evidence |

Target projections shall apply their ABI and lowering constraints after consuming the frozen fact. LLVM may project RL-29 `proven` as return `noalias` only for a direct-pointer return. It may project RL-30 `ReadOnly` as generic `memory(read)` and shall omit a restrictive memory attribute for `ReadWrite`. The shipped conservative RL-30 subset shall not emit `memory(none)` or argument-region-only memory attributes; those require typed accessible-region and inaccessible-region read/write facts. LLVM may project RL-31 as parameter `noalias` or alias-scope metadata only where its ABI and metadata placement preserve the neutral disjointness proof.

#### §8.12 Borrow inference

| Rule | Constraint |
|------|-----------|
| RL-32 | All non-scalar parameters initialize to `Borrowed`; fixed-point iteration promotes to `Owned` based on demand |
| RL-33 | Projection propagation: if a projected field becomes `Owned`, the source variable shall be promoted to `Owned` |
| RL-34 | Tail-call preservation: a logical release shall not be inserted after a tail call; ownership shall transfer instead, restricted to `Owned` callee parameters |

### §9 Verification Layers

The verification stack is layered. Each layer catches a different class of inconsistency.

| Rule | Layer |
|------|-------|
| VF-1 | Layer 1 (Structural): ARC IR well-formedness — use-before-def, dangling block refs, ownership-credit event on a scalar, ownership-release event on a borrowed parameter, argument-ownership length mismatch |
| VF-2 | Layer 2 (AIMS Contract): independent contract-consistency checks against the realized IR, including neutral RL-29 through RL-31 fact derivation. Backend verifiers separately validate target spelling and placement fidelity |
| VF-3 | Layer 3 (Oracle): re-derives `MemoryContract` from subject-independent realized evidence and compares against the inferred contract. Every CFG path starts with zero local funding; only explicit realized credit funds non-iter transfer/release; iter transfer is independently identified by the committed terminal-use kind; Access requires `Owned` on any unfunded path. Consumption uses TF-11 sequential composition within paths and IC-3 join across alternatives, never raw visit counts |
| VF-4 | Layer 4 (FIP Certification): proves `FipContract::Certified` functions have zero unmatched logical storage-acquisition/allocation obligations and lifetime-end cleanup/release obligations |
| VF-5 | Every active subsystem shall be end-to-end verified: implementation + invariant enforcement + tests |
| VF-6 | Contracts and realization shall agree |
| VF-7 | Active rewrites shall be sound: identical observable behavior; structural verification + behavioral tests + documented proof sketch |
| VF-8 | The verification stack applies to all rules in this section, including target rules; an unimplemented rule without a planned verification layer is a spec gap |

### §10 Active AIMS Fact Producers

The following backend-neutral AIMS fact producers and transformations are shipped
and end-to-end verified:

- Logical ownership-event realization and elision (RL-1..RL-5)
- Copy-on-write static and dynamic paths (RL-6..RL-10)
- Same-block and cross-block allocation reuse (RL-11..RL-12)
- TRMC tail-recursion-modulo-cons rewrite (PL-7..PL-11)
- KnownSafe pair elimination and PRE-style global ownership-event motion (RL-22..RL-26)
- Selective barriers at call sites (RL-27..RL-28)
- Exact final-contract, RL-29, RL-30, and RL-31 fact transport at the executable realization boundary; conservative facts only
- Borrow inference (RL-32..RL-34)
- Immortal pre-pass and FBIP certification (IC-6, VF-4)

### §11 Target Projections and Conformance

The following producer completions and physical projection/conformance targets are
designed but not yet shipped. The VM and compiled layout planners consume AIMS
facts; they are not AIMS subsystems and may not re-derive its policy. Annex E's
informative status accommodates target-only rules without imposing pre-shipping
conformance.

- Production `AllocationFacts` freezing, representation-owned `ExtentClass` transport, and conservative unknown defaults (RL-14..RL-17, RL-19, RL-21)
- Production VM and compiled `LayoutCapabilities` planners, `Satisfies` validation, and exact cross-target AIMS trace parity (RL-18, RL-18a, RL-20)
- Target-owned physical optimizations driven by those facts, including frame / stack / arena / region placement, metadata omission or narrowing, and confined-specialized ownership operations; these mechanisms are not themselves AIMS rules
- Complete RL-29/RL-31 provenance precision, typed inaccessible-region effects for RL-30, and production backend projection/binding of RL-29 through RL-31
- Provenance-partition ledger emission (§12)

### §12 Provenance-Partition Ledger

The provenance-partition ledger is the machine-checked foundation for logical
ownership-event placement over value provenance. Physical planners consume that
frozen topology after AIMS; the underlying calculus — six theorem families and
their composition extensions — is proven independently of any counter, object
layout, instruction set, runtime helper, or backend. Two objects define the model:

- **Partition** — per-(variable, field-path) classes keyed by allocation birth site, built as a union-find over semantic alias edges.
- **Ledger** — per-class event sequences (birth, credit, consume, read, mutate) derived over the CFG — normal, back-edge, and unwind edges, plus TRMC regions — from a fixed terminal-use classification table, never re-classified per emission site.

The proven theorem families:

1. **Partition soundness.** Two nodes shall share a partition class only when they share an allocation birth site. Phi / select merges shall be admitted only under a singleton-birth-site witness; a merge over distinct birth sites is inadmissible.
2. **Compositional placement.** A placement satisfying three clauses shall be safe on every CFG walk, including unwind-fed merges and TRMC back-edge loops of any iteration count: per-path owner-credit net zero (no leak); logical live-owner balance at least one at every read (no use-after-free); logical live-owner balance at least one plus the live same-class sibling balance at every mutation (no copy-on-write corruption). The three clauses are equivalent to ledger safety. Relocating a release past an unwind-fed merge is rejected. Filling a tail-recursion constructor-context hole transfers the filled value's owner credit into the aggregate's interior — a consume on the filled value's class, with the in-place hole write carrying the mutation floor on the context's class; the fill is the filled value's release, and a release placed after the fill nets negative one (double release) and is rejected.
3. **Keep-alive whole-pair elision.** A keep-alive owner-credit / release pair shall be elided only as a whole, and only when a live same-class sibling keeps the interior logical live-owner balance at or above one; eliding the credit alone permits premature cleanup or a negative balance. Same-class sibling liveness is the dominating-credit evidence that KnownSafe pair elimination (§8.8) consumes.
4. **Contract-boundary composition.** Boundary events at a call shall be classified through the callee's parameter contract (§7) via the fixed terminal-use table: an owned parameter is a birth on the callee side; an owned argument is a consume; an argument to an iterator-consuming parameter is a consume; a borrowed argument is a read; a transfer-through-return pairs a consume at the call with a credit at the return (net zero); a sharing-view producer is a credit. Given caller-clause satisfaction, callee conformance, and liveness at the call, the composed ledger satisfies the placement clauses without re-deriving the callee body. Classifying an owned argument as borrowed produces a double release and is rejected.
5. **Frame-limited robustness.** Introducing an alias edge that merges two partition classes shall leave every other class's derived ledger verbatim. The merged class's net is unconditionally additive; the merged class preserves all three placement clauses only when both prior classes have nonnegative logical owner-credit balance and are mutation-free. Unconditional preservation of the read and mutation clauses under merges does not hold.
6. **Per-field release decomposition.** A container release decomposed per named owned field (a partial release skipping fields whose payload ownership transferred out) shall derive its skip set from the partition's consume marks and from nothing else: a field is consume-marked exactly when its extracted view carries an ownership-transferring terminal use at a site other than the container's own construction. The consume-mark skip set is the unique clause-preserving skip set — skipping a moved field and releasing an unmoved one each preserve the payload class's net; failing to skip a moved field nets negative one (double release), and skipping an unmoved field nets positive one (leak). A merely-read view is never consume-marked and therefore never skipped. The container's own class books the same single whole-value consume as an undecomposed release; classes disjoint from the container derive verbatim-empty ledgers from the decomposition. When the payload's move is path-dependent (extracted on some control-flow paths and kept on others), the decomposition refines per release site: a site's skip verdict holds exactly when every path through it moved the payload — skip sites are extraction-dominated, whole-value sites are extraction-free — and each path's stream then balances on its own verdict agreement.

The composition extensions integrate the partition as a side table without weakening the elimination calculus. Class-grain refinement gated to a subset of the lattice's elimination verdict preserves single elimination and analysis-state immutability; an eliminator outside the lattice's verdict set provably breaks that guarantee — the machine-checked form of invariant 5 (§2). The partition pre-pass sits between analysis and realization (§6) and flows without stale summaries; an appended partition verification layer (§9) only rejects more, and the class assignment remains a complete, distinct partition.
