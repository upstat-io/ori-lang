---
section: "01"
title: TypeInfo Enum & Core Type Implementations
status: done
goal: Centralize all type-specific code generation behind a single enum so adding new types requires adding one variant, not modifying match arms across the codebase
sections:
  - id: "01.1"
    title: TypeInfo Enum Definition
    status: done
  - id: "01.2"
    title: Primitive Type Implementations
    status: done
  - id: "01.3"
    title: Collection Type Implementations
    status: done
  - id: "01.4"
    title: User-Defined Type Implementations
    status: done
  - id: "01.5"
    title: TypeInfo Store
    status: done
  - id: "01.6"
    title: Heap Layout for RC Types
    status: complete
  - id: "01.7"
    title: Completion Checklist
    status: done
---

# Section 01: TypeInfo Enum & Core Type Implementations

**Status:** Complete (01.1–01.7 done)
**Goal:** Every Ori type gets a TypeInfo variant (in `ori_llvm`) that encapsulates its LLVM representation, memory layout, and calling convention. ARC classification lives in a separate `ArcClassification` trait (in `ori_arc`, no LLVM dependency). Together these are the foundational abstractions that make extending codegen easy.

**Reference compilers:**
- **Swift** `lib/IRGen/TypeInfo.h` -- Hierarchy: TypeInfo > FixedTypeInfo > LoadableTypeInfo > HeapTypeInfo > ScalarTypeInfo
- **Roc** `crates/compiler/gen_llvm/src/llvm/convert.rs` -- `basic_type_from_layout()` + `argument_type_from_layout()`
- **Zig** `src/codegen/llvm.zig` -- `lowerType()` with `TypeMap` cache

**Current state:** `TypeInfo` enum and `TypeInfoStore` are implemented in `ori_llvm/src/codegen/type_info.rs`. Pool flattening is complete — struct/enum field data is stored in Pool's extra array, and `TypeInfoStore` reads all type info from Pool only (no TypeRegistry dependency). Named types are resolved via `Pool::resolve()`. Remaining: calling convention integration (Section 04), heap layout for RC (01.6), integration tests.

**Key design decision:** The TypeInfo abstraction is split across two crates:
- **`ArcClassification` trait** (in `ori_arc`) — Provides `arc_class(idx) -> ArcClass` with no LLVM dependency. Used by ARC analysis passes (borrow inference, RC insertion/elimination). This trait operates on `Pool`/`Idx` only.
- **`TypeInfo` enum** (in `ori_llvm::codegen`) — LLVM-specific: type representation, layout, ABI, copy/destroy emission. Uses enum dispatch (not `dyn Trait`) per Ori coding guidelines (enum for fixed type sets).

---

## 01.1 TypeInfo Enum Definition

The core enum that encapsulates LLVM-specific code generation for every Ori type. Lives in `ori_llvm::codegen::type_info`.

**Why an enum, not a trait?** Per Ori coding guidelines: "Enum for fixed sets (exhaustiveness, static dispatch)." The set of Ori type categories is fixed and known at compile time. An enum gives exhaustive matching (compiler catches missing cases), zero-cost dispatch, and no heap allocation. A `dyn Trait` approach would add indirection, require `Arc` wrapping, and provide no benefit since the type set is closed.

```rust
/// LLVM-specific type information for code generation.
///
/// Every Ori type category gets a variant. The enum encapsulates all
/// information needed to generate LLVM IR for values of this type:
/// representation, size, ABI, copy/destroy emission.
///
/// ARC classification is NOT here — it lives in `ori_arc::ArcClassification`
/// (no LLVM dependency). This enum is purely about LLVM code generation.
///
/// Design from Swift's TypeInfo hierarchy, adapted as a Rust enum.
pub enum TypeInfo {
    /// int → i64
    Int,
    /// float → f64
    Float,
    /// bool → i1
    Bool,
    /// char → i32 (Unicode scalar value)
    Char,
    /// byte → i8
    Byte,
    /// unit → i64 (placeholder; LLVM void cannot be stored/passed/phi'd)
    Unit,
    /// never → i64 (placeholder; LLVM void cannot be stored/passed/phi'd)
    Never,
    /// str → {i64, ptr}
    Str,
    /// duration → i64 (nanoseconds)
    Duration,
    /// size → i64 (bytes)
    Size,
    /// ordering → i8
    Ordering,
    /// [T] → {i64, i64, ptr}
    List { element: Idx },
    /// {K: V} → {i64, i64, ptr, ptr}
    Map { key: Idx, value: Idx },
    /// set[T] → {i64, i64, ptr}
    Set { element: Idx },
    /// (A, B, ...) → {A, B, ...}
    Tuple { elements: Vec<Idx> },
    /// option[T] → {i8, T}
    Option { inner: Idx },
    /// result[T, E] → {i8, max(T, E)}
    Result { ok: Idx, err: Idx },
    /// range → {i64, i64, i1}
    Range,
    /// User-defined struct → {field1, field2, ...}
    Struct { fields: Vec<(Name, Idx)> },
    /// User-defined enum → {tag, max(variant payloads)}
    Enum { variants: Vec<EnumVariantInfo> },
    /// Channel type → ptr (opaque heap-allocated channel)
    Channel { element: Idx },
    /// Function type → ptr (function pointer or closure pointer)
    Function { params: Vec<Idx>, ret: Idx },
    /// Error/unknown type fallback.
    ///
    /// Also used for types that should NEVER reach codegen:
    /// Tag::Scheme, Tag::Var, Tag::BoundVar, Tag::RigidVar,
    /// Tag::Projection, Tag::ModuleNs, Tag::Infer, Tag::SelfType.
    /// If any of these are encountered, emit TypeInfo::Error with a diagnostic.
    Error,
}

impl TypeInfo {
    // === Representation ===

    /// The LLVM type used to represent values of this type in memory.
    pub fn storage_type(&self, cx: &CodegenCx) -> BasicTypeEnum { ... }

    /// Size in bytes (ABI size — used for allocation and parameter passing).
    /// None for dynamically-sized types.
    /// This is the method used by Section 04's `compute_param_passing()`
    /// and `compute_return_passing()` to determine calling conventions.
    pub fn size(&self, cx: &CodegenCx) -> Option<u64> { ... }

    /// Alignment in bytes. Returns u32 — LLVM alignment values always fit in u32,
    /// and this matches the u32 alignment fields in ParamPassing/ReturnPassing (Section 04.2).
    pub fn alignment(&self, cx: &CodegenCx) -> u32 { ... }

    // === Classification ===

    /// True if this type has no ARC semantics (no retain/release needed).
    pub fn is_trivial(&self) -> bool { ... }

    /// True if values fit in registers and can be loaded/stored directly.
    pub fn is_loadable(&self) -> bool { ... }

    // === Code Generation ===

    /// Emit a copy of a value (bitwise for trivial, retain for RC).
    pub fn emit_copy(&self, builder: &IrBuilder, dest: ValueId, src: ValueId) { ... }

    /// Emit destruction of a value (no-op for trivial, release for RC).
    pub fn emit_destroy(&self, builder: &IrBuilder, value: ValueId) { ... }

    /// Emit retain (increment reference count). No-op for trivial types.
    pub fn emit_retain(&self, builder: &IrBuilder, value: ValueId) { ... }

    /// Emit release (decrement reference count). No-op for trivial types.
    pub fn emit_release(&self, builder: &IrBuilder, value: ValueId) { ... }

    // === Calling Convention ===

    /// How this type should be passed as a function argument.
    pub fn param_passing(&self, cx: &CodegenCx) -> ParamPassing { ... }

    /// How this type should be returned from a function.
    pub fn return_passing(&self, cx: &CodegenCx) -> ReturnPassing { ... }

    // === Debug Info ===

    /// Generate DWARF debug type information.
    pub fn debug_type(&self, cx: &CodegenCx) -> DebugTypeId { ... }
}

// ParamPassing and ReturnPassing enums: see Section 04.2 for canonical definitions.
// TypeInfo::param_passing() and return_passing() return those types.
```

**Note:** `ArcClass` is defined in `ori_arc` (Section 05), not here. The `TypeInfo` enum does not contain ARC classification — it queries `ArcClassification` when needed for emit_retain/emit_release decisions.

- [x] Define `TypeInfo` enum in `ori_llvm/src/codegen/type_info.rs`
- [x] Import `ParamPassing`, `ReturnPassing` enums from Section 04.2 (canonical definitions)
- [x] Implement all methods on `TypeInfo` via `match self { ... }`
- [x] Define `TypeInfoStore` to look up TypeInfo by `Idx` (see 01.5)
- [x] Implement `TypeInfo::Error` variant for unknown/error types and unreachable tags
- [x] Implement `TypeInfo::Channel` variant (opaque heap pointer)
- [x] Implement `TypeInfo::Function` variant (function/closure pointer)

---

## 01.2 Primitive Type Implementations

Each primitive gets a focused `TypeInfo` impl:

| Ori Type | LLVM Type | Trivial | Size | Passing |
|----------|-----------|---------|------|---------|
| `int` | `i64` | Yes | 8 | Direct |
| `float` | `f64` | Yes | 8 | Direct |
| `bool` | `i1` | Yes | 1* | Direct |
| `char` | `i32` | Yes | 4 | Direct |
| `byte` | `i8` | Yes | 1 | Direct |
| `unit` | `i64` | Yes | 8 | Direct |
| `never` | `i64` | Yes | 8 | Direct |
| `str` | `{i64, ptr}` | No | 16 | Direct |
| `duration` | `i64` | Yes | 8 | Direct |
| `size` | `i64` | Yes | 8 | Direct |
| `ordering` | `i8` | Yes | 1 | Direct |

**\*Bool size note:** `i1` is 1 bit in LLVM IR, but when stored in memory it is padded to 1 byte. The "Size = 1" refers to the memory size (1 byte), not the IR bit width.

**Unit and Never are `i64`, not `void`:** LLVM `void` is not a `BasicTypeEnum` — it cannot be stored in variables, passed as function parameters, used in phi nodes, or returned from functions that participate in expression-based evaluation. The existing codegen uses `i64` as a zero-value placeholder for both `unit` and `never`. This matches the expression-based semantics where every expression produces a value.

- [x] Implement `IntTypeInfo`, `FloatTypeInfo`, `BoolTypeInfo`
- [x] Implement `CharTypeInfo`, `ByteTypeInfo`, `UnitTypeInfo`
- [x] Implement `StrTypeInfo` (reference-counted: {len, ptr})
- [x] Implement `DurationTypeInfo`, `SizeTypeInfo`, `OrderingTypeInfo`
- [x] Implement `NeverTypeInfo` (unreachable, void return)
- [x] Test each primitive's LLVM type, size, alignment, and passing convention

---

## 01.3 Collection Type Implementations

| Ori Type | LLVM Type | Trivial | ARC |
|----------|-----------|---------|-----|
| `[T]` (List) | `{i64, i64, ptr}` | No | Yes (heap data) |
| `{K: V}` (Map) | `{i64, i64, ptr, ptr}` | No | Yes (heap data) |
| `(A, B, ...)` (Tuple) | `{A, B, ...}` | Depends | Depends |
| `option[T]` | `{i8, T}` | Depends | Depends |
| `result[T, E]` | `{i8, max(T, E)}` | Depends | Depends |
| `range` | `{i64, i64, i1}` | Yes | No |
| `set[T]` | `{i64, i64, ptr}` | No | Yes |
| `chan<T>` (Channel) | `ptr` | No | Yes (opaque heap) |
| `(P1, ...) -> R` (Function) | `ptr` | No | Yes (closure) |

**Layout notes:**
- **Map** uses `{i64, i64, ptr, ptr}` (len, cap, keys_ptr, vals_ptr) — 4 fields, matching the existing implementation in `ori_llvm/src/types.rs`.
- **Range** uses `{i64, i64, i1}` (start, end, inclusive) — 3 fields, matching the existing implementation. Currently `range<int>` only (fixed layout). If/when range becomes truly generic over `T`, the `TypeInfo::Range` variant will need an `element: Idx` field and the LLVM struct will become `{T, T, i1}`. For now, keep the fixed `{i64, i64, i1}` layout but be aware of this limitation.
- **Result** uses `{i8, max(T, E)}` — this is an improvement over the current implementation which only stores the ok type (error handling is TBD). The current code has a comment acknowledging this limitation. The V2 layout correctly reserves space for whichever of `T` or `E` is larger.

- [x] Implement `TypeInfo::List` variant (always heap-allocated, RC on data pointer)
- [x] Implement `TypeInfo::Map` variant (hash table with separate key/value pointers)
- [x] Implement `TypeInfo::Tuple` variant (trivial iff all fields trivial)
- [x] Implement `TypeInfo::Option` variant (tagged union: {tag, payload})
- [x] Implement `TypeInfo::Result` variant (tagged union: {tag, max(ok, err)})
- [x] Implement `TypeInfo::Range` variant (start, end, inclusive — 3 fields)
- [x] Implement `TypeInfo::Set` variant (like Map without values)
- [x] Implement `TypeInfo::Channel` variant (opaque heap pointer)
- [x] Implement `TypeInfo::Function` variant (function/closure pointer)

---

## 01.4 User-Defined Type Implementations

User-defined types (structs, enums) require dynamic TypeInfo creation based on their definitions. Struct and enum field data must be pre-flattened into the Pool's extra array during type checking (see 01.5 for rationale).

**Newtypes** are transparent at codegen time. In the Pool, newtypes resolve to `Tag::Named` or `Tag::Applied`. After the flattening refactor (prerequisite), the Pool stores the underlying type directly. The TypeInfo for a newtype IS the TypeInfo of its underlying type — no separate variant needed.

**Aliases** are resolved during type checking. By codegen time, the Pool `Idx` points to the resolved type. Aliases never appear in codegen — no separate variant needed.

The `TypeInfo::Struct` and `TypeInfo::Enum` variants carry the data needed for layout computation:

**Structs:**
```rust
// Stored in the Struct variant of TypeInfo enum
// fields: Vec<(Name, Idx)>  — Field name + type
// LLVM struct type is computed lazily and cached in the TypeInfoStore
```

**Enums (tagged unions):**
```rust
/// Variant info stored in the Enum variant of TypeInfo enum.
pub struct EnumVariantInfo {
    pub name: Name,
    pub fields: Vec<Idx>,
    // tag_type: i8 for <=256 variants
    // payload_size: Max variant payload size
    // Niche optimization (like Rust/Swift): use invalid bit patterns for tag — future
}
```

- [x] Implement `StructTypeInfo` with field-based layout computation
- [x] Implement `EnumTypeInfo` with tag + max-payload union layout
- [x] Verify newtypes resolve to underlying type's TypeInfo (no separate variant)
- [x] Verify aliases are resolved before codegen (no separate variant)
- [x] Handle recursive types (use LLVM opaque struct + later body fill)
- [x] Compute triviality transitively (struct trivial iff all fields trivial)

---

## 01.5 TypeInfo Store

**Design decision:** No `Arc<dyn TypeInfo>`, no `RwLock<FxHashMap>`. Per Ori coding guidelines: "Enum for fixed sets (exhaustiveness, static dispatch)" and "arena + ID, not `Box<Expr>`". The `TypeInfoStore` uses indexed storage (arena-style) with `TypeInfo` enum values.

**Pool-only design (HARD prerequisite — no TypeRegistry bridge):** TypeInfoStore holds only a `&Pool` reference, NOT a `TypeRegistry`. There is NO TypeRegistry bridge — this is a firm architectural constraint, not a preference. All type information needed for codegen — including struct field data and enum variant data — must be pre-flattened into the Pool's extra array during type checking. TypeInfoStore depends on Pool only.

**HARD BLOCKING PREREQUISITE — Pool Flattening Refactor:**

This is a **hard prerequisite refactor** that must be completed before TypeInfoStore can work. There is no alternative path (no TypeRegistry bridge, no lazy loading from TypeRegistry). Currently, struct field data and enum variant data live in `TypeRegistry`, not in `Pool`. The Pool has no `struct_fields()`, `enum_variants()`, or `resolve()` methods — those are `TypeRegistry` methods. The code examples in this section call these methods on Pool, but they do not exist yet.

The Pool flattening refactor must:

1. **Add Pool constructors** for struct and enum types that store field/variant data in Pool's extra array (similar to how `Function` and `Tuple` already use extra arrays with length prefixes). Estimated: ~200-300 lines in `pool/construct.rs`.
2. **Define extra-array layouts** for struct fields (field count, then pairs of Name + Idx) and enum variants (variant count, then per-variant: name + field count + field Idx list). Estimated: ~100-150 lines of layout logic.
3. **Add Pool accessor methods**: `struct_fields(idx) -> Vec<(Name, Idx)>`, `enum_variants(idx) -> Vec<EnumVariantInfo>`, `resolve(idx) -> Idx` (follows Named/Applied/Alias indirections), and `channel_elem(idx) -> Idx` (returns the element type for a `Tag::Channel` entry). Estimated: ~150-200 lines in `pool/mod.rs`.
4. **Migrate TypeRegistry callers** to write struct/enum data into Pool during type registration, so that by codegen time all data is accessible via Pool alone. Estimated: ~200-300 lines of changes across `registry/types.rs` and `check/`.
5. **Add Pool interning constructors and accessors for `Tag::Struct` and `Tag::Enum`** — the `Tag::Struct` and `Tag::Enum` variants already exist in `tag.rs`; what's missing are Pool methods to intern struct/enum types (storing field/variant data in the extra array) and accessor methods to read them back. Ensure deduplication works correctly for structural types. Estimated: ~100-150 lines.

**Total estimated scope: ~750-1100 lines of changes across 4-6 files.** This is a substantial refactor that touches the Pool internals, the type registration pipeline, and the TypeRegistry. It should be tracked as a separate work item and completed before any TypeInfoStore implementation begins.

Until this refactor is complete, the `struct_fields()`, `enum_variants()`, and `resolve()` calls in the code below are **aspirational API** — they show the intended interface but will not compile against the current Pool.

**Idx density:** Primitive types occupy indices 0-11. Indices 12-63 are reserved (padded with `Tag::Error` entries in the Pool — they map to `TypeInfo::Error`). Dynamic types start at `Idx::FIRST_DYNAMIC` (64). Note: `Idx::is_primitive()` returns `true` for ALL indices 0-63 (not just 0-11), so it should not be used to distinguish "real" primitives from reserved slots. The `entries` Vec will have 64 initial slots (12 real primitives + 52 Error padding) — this is acceptable given the simplicity of O(1) indexed lookup.

```rust
/// Maps Idx → TypeInfo for all types encountered during codegen.
///
/// Indices 0-63 are pre-populated at construction (12 real primitives +
/// 52 Error padding), matching Pool's layout. Dynamic types (index >= 64)
/// are populated lazily on first access.
/// Uses indexed storage (Vec) for O(1) lookup — Idx values are dense.
/// No Arc, no dyn, no RwLock — single-threaded per codegen context.
/// For parallel codegen (Section 12), each thread has its own store.
///
/// Uses interior mutability (RefCell) to allow shared access. Matches
/// the existing RefCell<TypeCache> pattern in CodegenCx. This lets
/// ExprLowerer hold `&TypeInfoStore` (shared ref) while still supporting
/// lazy population of entries on first access.
///
/// Only depends on Pool — struct/enum field data must be pre-flattened
/// into Pool's extra array during type checking (prerequisite refactor).
pub struct TypeInfoStore<'tcx> {
    /// Idx → TypeInfo mapping. Dense indexed storage.
    /// Indices 0-63 are pre-populated at construction.
    /// None = not yet computed. Some = cached.
    /// Uses RefCell for interior mutability — get() takes &self.
    entries: RefCell<Vec<Option<TypeInfo>>>,

    /// Pool reference for type property queries.
    /// All type data (including struct fields, enum variants) is accessible
    /// through Pool after the flattening refactor.
    pool: &'tcx Pool,
}

/// Static sentinel returned for Idx::NONE lookups.
/// Avoids indexing into `entries` for a sentinel that maps to u32::MAX.
static NONE_TYPE_INFO: TypeInfo = TypeInfo::Error;

impl<'tcx> TypeInfoStore<'tcx> {
    pub fn new(pool: &'tcx Pool) -> Self {
        // Pre-populate entries 0-63 to match Pool's primitive region.
        // Indices 0-11 are real primitives; 12-63 are reserved (Error padding).
        let mut entries = Vec::with_capacity(64);
        for i in 0..64u32 {
            let idx = Idx::from_raw(i);
            let info = match pool.tag(idx) {
                Tag::Int => TypeInfo::Int,
                Tag::Float => TypeInfo::Float,
                Tag::Bool => TypeInfo::Bool,
                Tag::Str => TypeInfo::Str,
                Tag::Char => TypeInfo::Char,
                Tag::Byte => TypeInfo::Byte,
                Tag::Unit => TypeInfo::Unit,
                Tag::Never => TypeInfo::Never,
                Tag::Duration => TypeInfo::Duration,
                Tag::Size => TypeInfo::Size,
                Tag::Ordering => TypeInfo::Ordering,
                _ => TypeInfo::Error, // Reserved slots 12-63
            };
            entries.push(Some(info));
        }
        Self { entries: RefCell::new(entries), pool }
    }

    /// Get or compute TypeInfo for a type.
    ///
    /// Returns `&TypeInfo::Error` for `Idx::NONE` (sentinel value, u32::MAX).
    /// Indices 0-63 are pre-populated and never require lazy computation.
    /// Takes `&self` (not `&mut self`) thanks to interior mutability (RefCell).
    pub fn get(&self, idx: Idx) -> &TypeInfo {
        // Guard: Idx::NONE is a sentinel (u32::MAX), not a valid type.
        // Return a static Error — do NOT index into entries.
        if idx == Idx::NONE {
            return &NONE_TYPE_INFO;
        }

        let index = idx.raw() as usize;
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            entries.resize_with(index + 1, || None);
        }
        if entries[index].is_none() {
            let info = self.compute_type_info(idx);
            entries[index] = Some(info);
        }
        // SAFETY: We just ensured entries[index] is Some. The returned
        // reference is valid for the lifetime of the TypeInfoStore because
        // entries are never removed or moved — only appended.
        // In practice, this will use Ref::map or an unsafe pointer cast
        // to return a reference with the correct lifetime.
        entries[index].as_ref().unwrap()
    }

    /// Convenience method: get the LLVM type ID for a type's storage representation.
    /// Calls storage_type() on the TypeInfo, then registers the result with the
    /// IrBuilder to obtain an LLVMTypeId. This bridges the TypeInfo world (which
    /// returns BasicTypeEnum) with the ID-based builder world (which uses LLVMTypeId).
    /// Takes `&self` (not `&mut self`) thanks to interior mutability (RefCell).
    pub fn storage_type_id(&self, idx: Idx, builder: &mut IrBuilder) -> LLVMTypeId {
        let ty = self.get(idx).storage_type(builder.cx());
        builder.register_type(ty)
    }

    fn compute_type_info(&self, idx: Idx) -> TypeInfo {
        match self.pool.tag(idx) {
            // Primitives
            Tag::Int => TypeInfo::Int,
            Tag::Float => TypeInfo::Float,
            Tag::Bool => TypeInfo::Bool,
            Tag::Str => TypeInfo::Str,
            Tag::Char => TypeInfo::Char,
            Tag::Byte => TypeInfo::Byte,
            Tag::Unit => TypeInfo::Unit,
            Tag::Never => TypeInfo::Never,
            Tag::Error => TypeInfo::Error,
            Tag::Duration => TypeInfo::Duration,
            Tag::Size => TypeInfo::Size,
            Tag::Ordering => TypeInfo::Ordering,

            // Simple containers (data = child Idx)
            Tag::List => TypeInfo::List { element: self.pool.list_elem(idx) },
            Tag::Option => TypeInfo::Option { inner: self.pool.option_inner(idx) },
            Tag::Set => TypeInfo::Set { element: self.pool.set_elem(idx) },
            Tag::Range => {
                // Currently range is always range<int> with fixed {i64, i64, i1} layout.
                // When range becomes generic over T, this arm will need to read the
                // element type from Pool and pass it to the TypeInfo::Range variant.
                // Uses pool.range_elem() — same accessor pattern as list_elem(),
                // option_inner(), set_elem(), etc.
                let elem = self.pool.range_elem(idx);
                debug_assert!(
                    self.pool.tag(elem) == Tag::Int
                        || elem == Idx::NONE, // NONE for unparameterized range
                    "Range element type is not Int — generic range support not yet implemented"
                );
                TypeInfo::Range
            }
            Tag::Channel => TypeInfo::Channel {
                element: self.pool.channel_elem(idx),
            },

            // Two-child containers (data = index into extra[])
            Tag::Map => TypeInfo::Map {
                key: self.pool.map_key(idx),
                value: self.pool.map_value(idx),
            },
            Tag::Result => TypeInfo::Result {
                ok: self.pool.result_ok(idx),
                err: self.pool.result_err(idx),
            },

            // Complex types (extra[] with length prefix)
            Tag::Function => TypeInfo::Function {
                params: self.pool.function_params(idx),
                ret: self.pool.function_return(idx),
            },
            Tag::Tuple => TypeInfo::Tuple {
                elements: self.pool.tuple_elems(idx),
            },
            Tag::Struct => {
                // Requires pre-flattened struct field data in Pool's extra array.
                // After the flattening refactor, Pool will provide a struct_fields()
                // accessor that returns field names and types from the extra array.
                let fields = self.pool.struct_fields(idx);
                TypeInfo::Struct { fields }
            }
            Tag::Enum => {
                // Requires pre-flattened enum variant data in Pool's extra array.
                // Similar to struct fields, enum variant payloads must be accessible
                // from Pool directly.
                let variants = self.pool.enum_variants(idx);
                TypeInfo::Enum { variants }
            }

            // Named types: newtypes and aliases are resolved/transparent at codegen.
            // Tag::Named and Tag::Applied should resolve to the underlying type.
            // After the flattening refactor, the Pool Idx for a newtype points to
            // the underlying type directly.
            Tag::Named | Tag::Applied | Tag::Alias => {
                // Resolve to underlying type's TypeInfo.
                // This requires Pool to provide a resolve method that follows
                // Named/Applied/Alias indirections to the concrete type.
                let resolved = self.pool.resolve(idx);
                self.compute_type_info(resolved)
            }

            // Type variables and schemes should NEVER reach codegen.
            // If encountered, it means type inference/checking failed to resolve them.
            // Emit TypeInfo::Error with a diagnostic.
            Tag::Var | Tag::BoundVar | Tag::RigidVar
            | Tag::Scheme | Tag::Projection | Tag::ModuleNs
            | Tag::Infer | Tag::SelfType => {
                // ICE: these tags must be resolved before codegen
                tracing::error!(
                    "unreachable type tag {:?} at codegen — type inference bug",
                    self.pool.tag(idx)
                );
                TypeInfo::Error
            }
        }
    }
}
```

- [x] **BLOCKING PREREQUISITE**: Pool flattening refactor (~750-1100 lines, see description above):
  - [x] Add Pool constructors for Struct/Enum types with extra-array storage
  - [x] Define extra-array layouts for struct fields and enum variants
  - [x] Add Pool accessors: `struct_fields()`, `enum_variants()`, `resolve()`, `channel_elem()`
  - [x] Migrate TypeRegistry callers to write struct/enum data into Pool
  - [x] Add Pool interning constructors and accessors for Tag::Struct and Tag::Enum (variants already exist in tag.rs)
- [x] Implement `TypeInfoStore` with dense indexed storage (`&'tcx Pool` only)
- [x] Wire up Pool queries for type properties (tag, children, fields)
- [x] Guard against `Idx::NONE` — return `TypeInfo::Error`
- [x] Handle type variables (unresolved → `TypeInfo::Error` with diagnostic)
- [x] Handle generic instantiation (List[int] vs List[str] get different entries)
- [x] Benchmark lookup performance on representative programs ✅ (2026-02-07)

---

## 01.6 Heap Layout for Reference-Counted Types

Reference-counted types use a **Roc-style layout** where the refcount header lives at a negative offset from the data pointer:

```
Heap allocation:
  ┌───────────────────┬───────────────────────┐
  │ strong_count: i64 │ data bytes ...        │
  └───────────────────┴───────────────────────┘
  ^                   ^
  ptr - 8             ptr (data pointer, stored on stack)
```

> **0.1-alpha** uses an 8-byte header with `strong_count` only. Weak references (adding `weak_count` for a 16-byte header) are deferred to a future version (see Section 07.3).

**Key properties:**
- The data pointer (`ptr`) points directly to the user data, NOT to the refcount header
- The header is 8 bytes: `{ strong_count: i64 }`
- `strong_count` is at `ptr - 8`
- This enables direct C FFI pass-through: the data pointer can be handed to C functions without adjustment
- Allocation: `ori_rc_alloc(size, align)` allocates `size + 8` bytes, returns `base + 8`
- `emit_retain`: loads strong_count from `ptr - 8`, increments, stores back
- `emit_release`: loads strong_count from `ptr - 8`, decrements, if zero then free from `ptr - 8`

**Stack representations with heap data:**

| Type | Stack Layout | Heap Data |
|------|-------------|-----------|
| `str` | `{i64, ptr}` (len, data_ptr) | `[strong_count \| utf8_bytes...]` |
| `[T]` | `{i64, i64, ptr}` (len, cap, data_ptr) | `[strong_count \| elements...]` |
| `{K: V}` | `{i64, i64, ptr, ptr}` (len, cap, keys_ptr, vals_ptr) | `[strong_count \| keys...]`, `[strong_count \| vals...]` |
| `set[T]` | `{i64, i64, ptr}` (len, cap, data_ptr) | `[strong_count \| elements...]` |
| `chan<T>` | `ptr` (opaque channel_ptr) | `[strong_count \| channel_state...]` |
| `(P) -> R` | `ptr` (closure_ptr) | `[strong_count \| fn_ptr, captures...]` |

This layout is important context for RC insertion (Section 07) and RC elimination (Section 08).

---

## 01.7 Completion Checklist

- [x] `TypeInfo` enum defined with all variants and methods
- [x] All primitive type variants implemented (unit/never use `i64`, not `void`)
- [x] All collection type variants implemented (with corrected layouts)
- [x] Channel and Function variants implemented
- [x] User-defined type support (struct, enum — newtypes/aliases resolved before codegen)
- [x] **BLOCKING PREREQUISITE**: Pool flattening refactor complete (~750-1100 lines across pool/, registry/, check/)
- [x] `TypeInfoStore` with indexed storage (`&'tcx Pool` only, no TypeRegistry)
- [x] `Idx::NONE` guard returns `TypeInfo::Error`
- [x] Unreachable tags (Var, BoundVar, etc.) emit `TypeInfo::Error` with diagnostic
- [x] Calling convention computation correct for all types ✅ (2026-02-08, Section 04.1–04.3)
- [x] Tests for each TypeInfo variant
- [x] Integration test: compile simple program through new type system ✅ (2026-02-07)

**Exit Criteria:** Every `Idx` in the Pool can produce a correct `TypeInfo` with proper LLVM type, size, alignment, and calling convention. TypeInfoStore depends on Pool only (no TypeRegistry). ARC classification is handled separately by `ori_arc::ArcClassification` (Section 05).
