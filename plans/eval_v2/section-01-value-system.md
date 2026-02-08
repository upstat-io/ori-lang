---
section: "01"
title: Value System V2
status: not-started
goal: Redesign value representation with interning, immediate optimization, and arena allocation
sections:
  - id: "01.1"
    title: ValuePool (Interned Constants)
    status: not-started
  - id: "01.2"
    title: Optimized Value Representation
    status: not-started
  - id: "01.3"
    title: Value Creation & Factory Methods
    status: not-started
  - id: "01.4"
    title: Migration from Current Value Enum
    status: not-started
---

# Section 01: Value System V2

**Status:** Planned
**Goal:** Redesign the `Value` type with interned constants, immediate optimization, and efficient heap management — inspired by Zig's InternPool, Rust's Immediate/Scalar, and Go's representation promotion.

**Crate Location:** The `Value` enum currently lives in `ori_patterns` (not `ori_eval`). It moves to a new `ori_value` crate so that the evaluator, codegen, and pattern system can all use it directly without circular dependencies. `ori_types` remains focused on type checking and does NOT contain Value. `ValuePool` and eval-specific value logic remain in `ori_eval`. Constant values in EvalIR use `Value` directly (via `EvalIR::Const(Value)`) — there is no separate `ConstValue` type.

---

## Prior Art Analysis

### Zig: InternPool — All Values as Indices
Zig represents **all** compile-time values and types as 32-bit indices into a single `InternPool`. This gives instant equality (compare integers, not structures), deduplication, and memory efficiency. The key insight: **interning eliminates Arc overhead for common values**.

### Rust CTFE: Immediate Optimization
Rust's const evaluator has two value tiers: **Immediate** (Scalar/ScalarPair — fits in registers, no allocation) and **MPlaceTy** (memory-backed — needs allocation). 90%+ of evaluations use Immediates. The key insight: **most values are small; optimize for the common case**.

### Go: Representation Promotion
Go's `go/constant` package stores small integers as `int64Val` (fast) and promotes to `intVal` (big.Int) only when needed. Rationals stay exact until precision is lost. The key insight: **defer expensive representations until required**.

### Roc: Arena-Allocated IR
Roc uses `bumpalo::Bump` arenas for all temporary compilation structures. Values during compilation live in arenas; only final results are heap-allocated. The key insight: **arenas prevent fragmentation and enable bulk deallocation**.

---

## 01.1 ValuePool (Interned Constants)

A `ValuePool` interns frequently-used constant values so they're stored once and referenced by a compact `ValueId`:

```rust
/// Compact reference to an interned value.
/// The upper 4 bits encode the tag (inline values), the lower 28 bits are the pool index.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ValueId(u32);

/// Pool of interned values. Values stored once, referenced by ValueId.
pub struct ValuePool {
    /// Interned compound values (strings, lists, etc.)
    entries: Vec<ValueEntry>,
    /// Deduplication: hash(value) → index
    dedup: FxHashMap<u64, SmallVec<[u32; 2]>>,
    /// Pre-interned sentinel values
    pub void: ValueId,
    pub true_val: ValueId,
    pub false_val: ValueId,
    pub none: ValueId,
    pub zero: ValueId,
    pub one: ValueId,
    pub empty_string: ValueId,
}

enum ValueEntry {
    Int(i64),
    Float(f64),
    Str(Box<str>),
    // ... compound types stored here
}
```

**Design decisions:**
- **Inline small values**: Void, Bool, small integers (0-255), None encode directly in ValueId bits — zero-cost, no pool lookup
- **Dedup compound values**: Strings, lists with same content share storage
- **Pre-intern sentinels**: Common values (void, true, false, none, 0, 1, "") created at pool initialization

- [ ] Define `ValueId` with inline encoding for small values
  - [ ] Design bit layout: 4-bit tag + 28-bit payload
  - [ ] Inline tags: Void, Bool(true), Bool(false), None, SmallInt(0-255), Unit variants
  - [ ] Pool tags: everything else (index into entries Vec)
- [ ] Implement `ValuePool` struct
  - [ ] `intern(&mut self, value: &Value) -> ValueId` — intern a runtime Value
  - [ ] `get(&self, id: ValueId) -> ValueRef` — retrieve by ID (returns reference, not clone)
  - [ ] `intern_str(&mut self, s: &str) -> ValueId` — fast path for strings
  - [ ] `intern_int(&mut self, n: i64) -> ValueId` — fast path for ints
- [ ] Implement deduplication via content hashing
  - [ ] Hash function for Value (must be deterministic)
  - [ ] Collision handling (small Vec per bucket)
- [ ] Pre-intern sentinel values at pool creation
- [ ] Thread-safety for parallel const-eval
  - [ ] `ValuePool` behind `Arc<RwLock<>>` for shared access across parallel const-eval queries
  - [ ] Alternative: per-thread pools merged after parallel phases (avoids lock contention)
  - [ ] Read-heavy workload (intern once, lookup many times) favors `RwLock` over `Mutex`
  - [ ] Sentinel values (void, true, false, etc.) are immutable after init — no locking needed for reads

---

## 01.2 Optimized Value Representation

Redesign the `Value` enum to separate inline (no-alloc) from heap (Arc-wrapped) values:

```rust
pub enum Value {
    // === Tier 1: Inline (no allocation, Copy-able) ===
    Int(ScalarInt),     // KEEP ScalarInt — #[repr(transparent)] around i64 costs zero bytes
                        // but makes unchecked arithmetic a compile error. Overflow detected
                        // via ScalarInt's checked_* methods which return EvalError.
    Float(f64),
    Bool(bool),
    Char(char),
    Byte(u8),
    Void,
    None,
    Duration(i64),      // Nanoseconds
    Size(u64),          // Bytes
    Ordering(OrderingValue),  // Already uses OrderingValue in V1 (avoids std::cmp::Ordering name collision)

    // === Tier 2: Interned (pool reference, Copy-able) ===
    Interned(ValueId),  // Reference to ValuePool — for constants, shared values

    // === Tier 3: Heap (Arc-wrapped, Clone-able) ===
    Str(Heap<Cow<'static, str>>),  // Current type; evaluate CompactStr as future optimization
    List(Heap<Vec<Value>>),
    Map(Heap<BTreeMap<String, Value>>),
    Tuple(Heap<Vec<Value>>),
    Some(Heap<Value>),
    Ok(Heap<Value>),
    Err(Heap<Value>),
    Struct(StructValue),
    Function(FunctionValue),
    MemoizedFunction(MemoizedFunctionValue),
    MultiClauseFunction(Heap<Vec<FunctionValue>>),
    Variant { type_name: Name, variant_name: Name, fields: Heap<Vec<Value>> },
    VariantConstructor { type_name: Name, variant_name: Name, field_count: u16 },
    Newtype { type_name: Name, inner: Heap<Value> },
    NewtypeConstructor { type_name: Name },
    Range(RangeValue),
    ModuleNamespace(Heap<BTreeMap<Name, Value>>),

    // === Tier 4: Special ===
    FunctionVal(FunctionValFn, &'static str),
    TypeRef { type_name: Name },
    Error(String),
}
```

**Key changes from current:**
- **KEEP `ScalarInt`** — `#[repr(transparent)]` around `i64` costs zero bytes at runtime but provides compile-time safety: unchecked arithmetic (raw `+`, `-`, `*`, `/` on the inner `i64`) becomes a compile error. All arithmetic must go through `ScalarInt::checked_add`, `checked_mul`, etc., which return `EvalError::IntegerOverflow` on overflow. This is a net positive over using bare `i64` where nothing prevents accidental unchecked operations.
- Add `Interned(ValueId)` variant for pool-backed constants
- Narrow `VariantConstructor.field_count` from `usize` to `u16` — reduces Value enum size; 65,535 fields per variant is more than sufficient. **Factory methods must use `u16::try_from(count).expect("variant field count exceeds u16::MAX")` for checked conversion** at construction time to catch overflows early rather than silently truncating.
- **Caller audit required**: All callers of `variant_constructor()` and any code constructing `VariantConstructor` must be audited for the `usize` -> `u16` type narrowing. Search for `field_count` usage, `VariantConstructor` construction, and any code that passes field counts as `usize` to ensure no silent truncation occurs.
- Keep `Cow<'static, str>` (current type); evaluate `CompactStr` as a future optimization after profiling
- Future: if `Value` exceeds 2 words (128 bits), consider pointer-tagging or NaN-boxing

- [ ] Audit current `Value` variants for size optimization
  - [ ] Run `std::mem::size_of::<Value>()` — target ≤ 3 words (24 bytes on 64-bit)
  - [ ] Identify variants that inflate the enum (largest payload determines size)
  - [ ] Consider boxing large variants (Variant, Struct) if they inflate the enum
- [ ] Implement `Interned(ValueId)` variant
  - [ ] `Value::from_pool(pool: &ValuePool, id: ValueId) -> Value` — materialize from pool
  - [ ] `Value::try_intern(&self, pool: &mut ValuePool) -> Option<ValueId>` — try to intern
- [ ] Evaluate `CompactStr` vs `Cow<'static, str>`
  - [ ] Benchmark string creation/access patterns in eval
  - [ ] Decide based on profiling data
- [ ] Ensure `Value: Clone` remains cheap (Arc clone for heap types)
- [ ] Add `Value::is_inline(&self) -> bool` for optimization hints

---

## 01.3 Value Creation & Factory Methods

Maintain the current factory-method pattern but add pool-aware creation:

```rust
impl Value {
    /// Create string, interning if it's a common/short string.
    /// Strings are heap types — interning provides deduplication and sharing benefits.
    pub fn string_pooled(pool: &mut ValuePool, s: String) -> Self {
        if s.len() <= 64 {  // Short strings worth interning
            Value::Interned(pool.intern_str(&s))
        } else {
            Value::Str(Heap::new(Cow::Owned(s)))
        }
    }

    // NOTE: No int_pooled() factory. Int(ScalarInt) is already inline (zero allocation,
    // Copy-able). Routing integers through Interned(ValueId) would add indirection for
    // no benefit — the inline representation is already optimal. Pool-aware factories
    // are only worthwhile for heap types (strings, lists, maps, etc.) where interning
    // avoids allocation and enables deduplication.
}
```

- [ ] Design pool-aware factory methods (heap types only)
  - [ ] `Value::string_pooled(pool, s)` — intern short strings, heap-allocate long ones
  - [ ] `Value::list_pooled(pool, items)` — intern small constant lists
  - [ ] `Value::bool(b)` — always inline (no change)
  - [ ] `Value::void()` — always inline (no change)
  - [ ] `Value::int(n)` — always inline via `Int(ScalarInt)` (no pool needed)
- [ ] Migrate heap-type factory methods to pool-aware versions
  - [ ] Replace `Value::string(s)` with `Value::string_pooled(pool, s)` at all call sites
  - [ ] Remove non-pool factory methods for heap types after migration is complete
  - [ ] Keep inline factory methods unchanged (int, float, bool, void, etc.)
- [ ] Add `Value::display_value(&self, pool: &ValuePool) -> String`
  - [ ] Resolves `Interned(id)` to display string via pool lookup

---

## 01.4 Migration from Current Value Enum

### Phase 0: ori_patterns Decomposition

Before any Value changes, decompose the `ori_patterns` crate which currently holds Value, Heap, composite types, EvalError, and pattern logic in a single crate:

- [ ] Step 1: Create new `ori_value` crate
  - [ ] Move `Value` enum, `Heap<T>`, composite value types (StructValue, FunctionValue, etc.) from `ori_patterns` to `ori_value`
  - [ ] Move `ScalarInt` to `ori_value` (it is part of the value representation)
  - [ ] Ensure `ori_value` has no upward dependencies (it sits below ori_eval, ori_patterns, ori_llvm)
- [ ] Step 2: EvalError and EvalResult remain in `ori_patterns`
  - [ ] Do NOT move EvalError/EvalResult to `ori_eval` — this would create a circular dependency (`ori_eval` -> `ori_patterns` -> `ori_eval`)
  - [ ] EvalError and EvalResult stay in `ori_patterns`, which sits below `ori_eval` in the dependency graph
- [ ] Step 3: Update `ori_patterns` to reference new locations
  - [ ] Pattern traits and implementations remain in `ori_patterns`
  - [ ] Update imports to reference `ori_value::Value`; EvalError stays in place
- [ ] Step 4: Update all downstream imports
  - [ ] `ori_eval` depends on `ori_value` (for Value) and `ori_patterns` (for EvalError/EvalResult)
  - [ ] `ori_patterns` depends on `ori_value` (for Value) — no upward dependency on `ori_eval`
  - [ ] `ori_llvm` depends on `ori_value` (for Value)
  - [ ] `oric` imports updated to new crate paths
- [ ] Step 5: Verify `./test-all.sh` passes after decomposition (no behavioral changes)

### Phase 1: Value Pool and Interning

- [ ] Phase 1: Add `Interned(ValueId)` variant to Value enum and build `ValuePool`
  - [ ] `ValuePool` created at interpreter startup
  - [ ] Sentinel values pre-interned
  - [ ] Begin migrating call sites to pool-aware factory methods
- [ ] Phase 2: Verify ScalarInt integration (KEEP ScalarInt — do NOT replace with bare i64)
  - [ ] Audit all `ScalarInt::checked_*` calls — ensure they are used consistently
  - [ ] Verify no raw i64 arithmetic bypasses ScalarInt's checked methods
  - [ ] Ensure `EvalError::IntegerOverflow` is returned when checked operations return `None`
  - [ ] ScalarInt is `#[repr(transparent)]` around i64 — zero runtime cost, compile-time safety
- [ ] Phase 3: Intern constants during EvalIR lowering (Section 08)
  - [ ] Literal expressions produce `Interned(id)` values
  - [ ] Pattern match literals compared via ValueId (instant equality)
- [ ] Phase 4: Profile and optimize
  - [ ] Measure `size_of::<Value>()` — must not regress
  - [ ] Benchmark eval throughput — must not regress
  - [ ] Measure memory usage on large programs

---

## 01.5 Completion Checklist

- [ ] `ValuePool` implemented with interning and deduplication
- [ ] `ValueId` with inline encoding for small values
- [ ] `Value::Interned(ValueId)` variant integrated
- [ ] All factory methods use pool (no non-pool fallbacks)
- [ ] All existing tests pass (updated to use pool-aware API)
- [ ] `size_of::<Value>()` documented and within target
- [ ] Performance benchmarked — no regressions

**Migration note:** `EvalResult` becomes generic `EvalResult<T = Value>` per Section 05's changes (i.e., `pub type EvalResult<T = Value> = Result<T, EvalError>`). This allows `EvalResult<()>` for void-returning operations while keeping `EvalResult` (without explicit type parameter) as shorthand for `Result<Value, EvalError>`. Value system changes should use the generic form where appropriate.

**Exit Criteria:** Value system fully uses interned constants alongside heap values. All code uses pool-aware factory methods; non-pool methods are deleted.
