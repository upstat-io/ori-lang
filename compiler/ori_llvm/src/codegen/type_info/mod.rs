//! `TypeInfo` enum and `TypeInfoStore` for V2 codegen.
//!
//! Every Ori type category gets a [`TypeInfo`] variant that encapsulates its
//! LLVM representation, memory layout, and calling convention. Adding a new
//! type means adding one enum variant — not modifying match arms across the
//! codebase.
//!
//! Design from Swift's `TypeInfo` hierarchy, adapted as a Rust enum per Ori
//! coding guidelines ("enum for fixed sets — exhaustiveness, static dispatch").
//!
//! # Crate Split
//!
//! - **`TypeInfo`** (here, `ori_llvm`) — LLVM-specific: types, layout, ABI
//! - **`ArcClassification`** (future `ori_arc`) — No LLVM dependency: Scalar/Ref
//!
//! # References
//!
//! - Swift `lib/IRGen/TypeInfo.h` (hierarchy: `TypeInfo` > `FixedTypeInfo` > `LoadableTypeInfo`)
//! - Roc `gen_llvm/src/llvm/convert.rs` (`basic_type_from_layout`)
//! - Zig `src/codegen/llvm.zig` (`lowerType` with `TypeMap` cache)

use std::cell::{Cell, RefCell};

use inkwell::types::{BasicTypeEnum, StructType};
use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};

use crate::context::SimpleCx;

// ---------------------------------------------------------------------------
// TypeInfo enum
// ---------------------------------------------------------------------------

/// Variant info for a single enum variant, stored in `TypeInfo::Enum`.
#[derive(Clone, Debug)]
pub struct EnumVariantInfo {
    /// Variant name (interned).
    pub name: Name,
    /// Field types (empty for unit variants, one element for tuple variants).
    pub fields: Vec<Idx>,
}

/// LLVM-specific type information for code generation.
///
/// Every Ori type category gets a variant. The enum encapsulates all
/// information needed to generate LLVM IR for values of this type:
/// representation, size, ABI, copy/destroy emission.
///
/// ARC classification is NOT here — it lives in `ori_arc::ArcClassification`
/// (no LLVM dependency). This enum is purely about LLVM code generation.
#[derive(Clone, Debug)]
pub enum TypeInfo {
    /// `int` -> i64
    Int,
    /// `float` -> f64
    Float,
    /// `bool` -> i1
    Bool,
    /// `char` -> i32 (Unicode scalar value)
    Char,
    /// `byte` -> i8
    Byte,
    /// `unit` -> i64 (LLVM void cannot be stored/passed/phi'd)
    Unit,
    /// `never` -> i64 (LLVM void cannot be stored/passed/phi'd)
    Never,
    /// `str` -> {i64 len, ptr data}
    Str,
    /// `duration` -> i64 (nanoseconds)
    Duration,
    /// `size` -> i64 (bytes)
    Size,
    /// `ordering` -> i8 (Less=0, Equal=1, Greater=2)
    Ordering,
    /// `[T]` -> {i64 len, i64 cap, ptr data}
    List { element: Idx },
    /// `{K: V}` -> {i64 len, i64 cap, ptr keys, ptr vals}
    Map { key: Idx, value: Idx },
    /// `set[T]` -> {i64 len, i64 cap, ptr data}
    Set { element: Idx },
    /// `(A, B, ...)` -> {A, B, ...}
    Tuple { elements: Vec<Idx> },
    /// `option[T]` -> {i8 tag, T payload}
    Option { inner: Idx },
    /// `result[T, E]` -> {i8 tag, max(T, E) payload}
    Result { ok: Idx, err: Idx },
    /// `range` -> {i64 start, i64 end, i1 inclusive}
    Range,
    /// User-defined struct -> {field1, field2, ...}
    Struct { fields: Vec<(Name, Idx)> },
    /// User-defined enum -> {tag, max(variant payloads)}
    Enum { variants: Vec<EnumVariantInfo> },
    /// `Iterator<T>` -> ptr (opaque heap-allocated iterator handle)
    Iterator { element: Idx },
    /// `chan<T>` -> ptr (opaque heap-allocated channel)
    Channel { element: Idx },
    /// `(P1, ...) -> R` -> ptr (function pointer or closure pointer)
    Function { params: Vec<Idx>, ret: Idx },
    /// Error/unknown type fallback.
    ///
    /// Used for types that should never reach codegen:
    /// `Var`, `BoundVar`, `RigidVar`, `Scheme`, `Projection`,
    /// `ModuleNs`, `Infer`, `SelfType`.
    Error,
}

// ---------------------------------------------------------------------------
// TypeInfo — representation methods
// ---------------------------------------------------------------------------

impl TypeInfo {
    /// The LLVM type used to represent values of this type in memory.
    ///
    /// This is the canonical type mapping. Every `TypeInfo` variant knows
    /// exactly how it maps to LLVM IR, making extension straightforward.
    pub fn storage_type<'ll>(&self, scx: &SimpleCx<'ll>) -> BasicTypeEnum<'ll> {
        match self {
            // Primitives
            Self::Int | Self::Duration | Self::Size | Self::Unit | Self::Never => {
                scx.type_i64().into()
            }
            Self::Float => scx.type_f64().into(),
            Self::Bool => scx.type_i1().into(),
            Self::Char => scx.type_i32().into(),
            Self::Byte | Self::Ordering => scx.type_i8().into(),

            // Str: {i64 len, ptr data}
            Self::Str => scx
                .type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false)
                .into(),

            // Collections
            Self::List { .. } | Self::Set { .. } => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_ptr().into(),
                    ],
                    false,
                )
                .into(),

            Self::Map { .. } => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_ptr().into(),
                        scx.type_ptr().into(),
                    ],
                    false,
                )
                .into(),

            Self::Range => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_i1().into(),
                    ],
                    false,
                )
                .into(),

            // Tagged unions: {i8 tag, payload}
            // Option uses the inner type directly as payload.
            // Result uses the larger of ok/err — for now, uses ok type.
            // TODO: Result should use max(ok, err) size for correct layout.
            Self::Option { inner } => {
                // Payload type depends on inner type. Since we don't have
                // the store here, use i64 as a uniform payload representation.
                // The actual payload coercion happens at emit time.
                let _ = inner;
                scx.type_struct(&[scx.type_i8().into(), scx.type_i64().into()], false)
                    .into()
            }
            Self::Result { ok, err } => {
                let _ = (ok, err);
                scx.type_struct(&[scx.type_i8().into(), scx.type_i64().into()], false)
                    .into()
            }

            // Tuple: struct of element types. Without the store, we can't
            // resolve element types here. This returns an empty struct as
            // placeholder — actual tuple lowering uses TypeInfoStore which
            // has access to resolve element types via the Pool.
            Self::Tuple { elements } => {
                // Placeholder: tuple of N i64s. Real lowering via store.
                let fields: Vec<BasicTypeEnum<'ll>> =
                    elements.iter().map(|_| scx.type_i64().into()).collect();
                scx.type_struct(&fields, false).into()
            }

            // Iterator / Channel: opaque heap-allocated handles
            Self::Iterator { .. } | Self::Channel { .. } => scx.type_ptr().into(),

            // Function: fat-pointer closure { fn_ptr: ptr, env_ptr: ptr }
            // All function-typed values use this two-pointer representation,
            // even non-closures (which have env_ptr = null). This uniform
            // representation avoids branching at call sites.
            Self::Function { .. } => scx
                .type_struct(&[scx.type_ptr().into(), scx.type_ptr().into()], false)
                .into(),

            // User-defined types (placeholder — resolved via TypeInfoStore)
            Self::Struct { fields } => {
                let field_types: Vec<BasicTypeEnum<'ll>> =
                    fields.iter().map(|_| scx.type_i64().into()).collect();
                scx.type_struct(&field_types, false).into()
            }
            Self::Enum { .. } => {
                // Default: {i8 tag, i64 payload} — real layout computed by store
                scx.type_struct(&[scx.type_i8().into(), scx.type_i64().into()], false)
                    .into()
            }

            // Error fallback
            Self::Error => scx.type_i64().into(),
        }
    }

    /// Size in bytes (ABI size).
    ///
    /// Returns `None` for types whose size depends on element types and
    /// can only be computed with a `TypeInfoStore` (which has Pool access).
    ///
    /// Used by Section 04's `compute_param_passing()` and `compute_return_passing()`.
    pub fn size(&self) -> Option<u64> {
        match self {
            // 8-byte types: scalars, handles, error fallback
            Self::Int
            | Self::Float
            | Self::Duration
            | Self::Size
            | Self::Unit
            | Self::Never
            | Self::Iterator { .. }
            | Self::Channel { .. }
            | Self::Error => Some(8),

            // 1-byte types
            Self::Bool | Self::Byte | Self::Ordering => Some(1),

            // 4-byte types
            Self::Char => Some(4),

            // 16-byte types:
            // Function: fat-pointer closure { ptr, ptr }
            // Str: {i64, ptr}
            // Option/Result: {i8, i64} — LLVM pads to 16 bytes
            Self::Function { .. } | Self::Str | Self::Option { .. } | Self::Result { .. } => {
                Some(16)
            }

            // List/Set: {i64, i64, ptr} = 24 bytes
            // Range: {i64, i64, i1} — LLVM pads to 24 bytes (8+8+8 with alignment)
            Self::List { .. } | Self::Set { .. } | Self::Range => Some(24),

            // Map: {i64, i64, ptr, ptr} = 32 bytes
            Self::Map { .. } => Some(32),

            // Dynamic-size types: depend on element/field types
            Self::Tuple { .. } | Self::Struct { .. } | Self::Enum { .. } => None,
        }
    }

    /// Alignment in bytes.
    ///
    /// Returns the required alignment for this type. On x86-64, all types
    /// align to at most 8 bytes.
    pub fn alignment(&self) -> u32 {
        match self {
            Self::Bool | Self::Byte | Self::Ordering => 1,
            Self::Char => 4,
            // Everything else aligns to 8 on x86-64
            _ => 8,
        }
    }

    /// True if this type has no ARC semantics (no retain/release needed).
    ///
    /// Trivial types are passed by value and don't participate in
    /// reference counting. This is the codegen-level triviality check;
    /// the ARC-level classification lives in `ori_arc`.
    pub fn is_trivial(&self) -> bool {
        match self {
            // Scalar primitives and error fallback are trivial
            Self::Int
            | Self::Float
            | Self::Bool
            | Self::Char
            | Self::Byte
            | Self::Unit
            | Self::Never
            | Self::Duration
            | Self::Size
            | Self::Ordering
            | Self::Range
            | Self::Error => true,

            // Everything else has heap data or may contain heap data.
            // Tagged unions (Option/Result) and composites (Tuple/Struct/Enum)
            // are conservatively non-trivial — precise classification requires
            // transitive field analysis (future: ori_arc ArcClassification).
            Self::Str
            | Self::List { .. }
            | Self::Map { .. }
            | Self::Set { .. }
            | Self::Iterator { .. }
            | Self::Channel { .. }
            | Self::Function { .. }
            | Self::Option { .. }
            | Self::Result { .. }
            | Self::Tuple { .. }
            | Self::Struct { .. }
            | Self::Enum { .. } => false,
        }
    }

    /// True if values fit in registers and can be loaded/stored directly.
    ///
    /// Non-loadable types must be passed by reference (sret ABI).
    pub fn is_loadable(&self) -> bool {
        match self.size() {
            Some(size) => size <= 16,
            // Unknown size — conservatively not loadable
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// TypeInfoStore
// ---------------------------------------------------------------------------

/// Static sentinel returned for `Idx::NONE` lookups.
static NONE_TYPE_INFO: TypeInfo = TypeInfo::Error;

/// Maps `Idx` -> `TypeInfo` for all types encountered during codegen.
///
/// Indices 0-63 are pre-populated at construction (12 real primitives +
/// 52 Error padding), matching Pool's layout. Dynamic types (index >= 64)
/// are populated lazily on first access.
///
/// Uses indexed storage (`Vec`) for O(1) lookup — `Idx` values are dense.
/// No Arc, no dyn, no `RwLock` — single-threaded per codegen context.
///
/// Uses interior mutability (`RefCell`) to allow shared access while
/// supporting lazy population on first access.
///
/// Only depends on Pool — struct/enum field data must be pre-flattened
/// into Pool's extra array during type checking (prerequisite refactor
/// required for full Struct/Enum support).
pub struct TypeInfoStore<'tcx> {
    /// `Idx` -> `TypeInfo` mapping. Dense indexed storage.
    /// Indices 0-63 are pre-populated at construction.
    /// `None` = not yet computed. `Some` = cached.
    entries: RefCell<Vec<Option<TypeInfo>>>,

    /// Pool reference for type property queries.
    pool: &'tcx Pool,

    /// Cache for transitive triviality classification.
    ///
    /// `true` = type has no ARC semantics (all fields transitively trivial).
    triviality_cache: RefCell<FxHashMap<Idx, bool>>,

    /// Types currently being classified for triviality (cycle detection).
    ///
    /// Recursive types are conservatively non-trivial since they require
    /// heap indirection (pointers).
    classifying_trivial: RefCell<FxHashSet<Idx>>,

    /// Types currently being computed in `compute_type_info()` (cycle detection).
    ///
    /// Named/Applied/Alias resolution calls `self.get(resolved)` which can
    /// re-enter `compute_type_info()` for another Named type — unbounded
    /// recursion. This set detects the cycle and returns `TypeInfo::Error`.
    computing: RefCell<FxHashSet<Idx>>,
}

impl<'tcx> TypeInfoStore<'tcx> {
    /// Create a new store, pre-populating primitive type entries.
    ///
    /// Indices 0-11 are real primitives; 12-63 are reserved (Error padding).
    pub fn new(pool: &'tcx Pool) -> Self {
        let mut entries = Vec::with_capacity(64);
        for i in 0..64u32 {
            let idx = Idx::from_raw(i);
            let info = Self::primitive_type_info(pool, idx);
            entries.push(Some(info));
        }
        Self {
            entries: RefCell::new(entries),
            pool,
            triviality_cache: RefCell::new(FxHashMap::default()),
            classifying_trivial: RefCell::new(FxHashSet::default()),
            computing: RefCell::new(FxHashSet::default()),
        }
    }

    /// Resolve primitive type info for pre-interned indices.
    fn primitive_type_info(pool: &Pool, idx: Idx) -> TypeInfo {
        // Only the first 12 indices are real primitives; the rest are padding.
        if idx.raw() >= 64 {
            return TypeInfo::Error;
        }
        match pool.tag(idx) {
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
        }
    }

    /// Get the `TypeInfo` for a type, computing lazily if needed.
    ///
    /// Returns `TypeInfo::Error` for `Idx::NONE` (sentinel, `u32::MAX`).
    pub fn get(&self, idx: Idx) -> TypeInfo {
        // Guard: Idx::NONE is a sentinel (u32::MAX) — not a valid index.
        if idx == Idx::NONE {
            return NONE_TYPE_INFO.clone();
        }

        let index = idx.raw() as usize;

        // Guard: reject indices beyond the pool — these are unresolved
        // generic types or stale indices from a different compilation unit.
        if index >= self.pool.len() {
            tracing::warn!(idx = ?idx, pool_len = self.pool.len(), "type index out of pool bounds");
            return TypeInfo::Error;
        }

        // Fast path: already computed
        {
            let entries = self.entries.borrow();
            if index < entries.len() {
                if let Some(ref info) = entries[index] {
                    return info.clone();
                }
            }
        }

        // Slow path: compute and cache
        let info = self.compute_type_info(idx);
        let mut entries = self.entries.borrow_mut();
        if index >= entries.len() {
            entries.resize_with(index + 1, || None);
        }
        entries[index] = Some(info.clone());
        info
    }

    /// Access the underlying Pool.
    pub fn pool(&self) -> &'tcx Pool {
        self.pool
    }

    /// Transitive triviality check: true if this type (and all its children)
    /// have no ARC semantics.
    ///
    /// Unlike `TypeInfo::is_trivial()` which is conservative (all compound
    /// types are non-trivial), this method walks child types transitively:
    /// - `option[int]` → trivial (inner is scalar)
    /// - `option[str]` → non-trivial (str has heap data)
    /// - `(int, float)` → trivial (all elements scalar)
    /// - `struct Point { x: int, y: int }` → trivial (all fields scalar)
    /// - Recursive types → non-trivial (require heap indirection)
    pub fn is_trivial(&self, idx: Idx) -> bool {
        // Sentinel
        if idx == Idx::NONE {
            return true;
        }

        // Fast path: cache hit
        if let Some(&cached) = self.triviality_cache.borrow().get(&idx) {
            return cached;
        }

        let result = self.classify_trivial(idx);
        self.triviality_cache.borrow_mut().insert(idx, result);
        result
    }

    /// Recursive triviality classification with cycle detection.
    fn classify_trivial(&self, idx: Idx) -> bool {
        // Cycle detection: if we're already classifying this type, it's
        // recursive, which means it needs heap indirection → non-trivial.
        if !self.classifying_trivial.borrow_mut().insert(idx) {
            return false;
        }

        let info = self.get(idx);
        let result = match &info {
            // Scalar primitives are always trivial.
            TypeInfo::Int
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Unit
            | TypeInfo::Never
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Ordering
            | TypeInfo::Range
            | TypeInfo::Error => true,

            // Heap-backed types are always non-trivial.
            TypeInfo::Str
            | TypeInfo::List { .. }
            | TypeInfo::Map { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. }
            | TypeInfo::Function { .. } => false,

            // Compound types: trivial iff all children are trivial.
            TypeInfo::Option { inner } => self.is_trivial(*inner),
            TypeInfo::Result { ok, err } => self.is_trivial(*ok) && self.is_trivial(*err),
            TypeInfo::Tuple { elements } => elements.iter().all(|&e| self.is_trivial(e)),
            TypeInfo::Struct { fields } => fields.iter().all(|&(_, ty)| self.is_trivial(ty)),
            TypeInfo::Enum { variants } => variants
                .iter()
                .all(|v| v.fields.iter().all(|&f| self.is_trivial(f))),
        };

        self.classifying_trivial.borrow_mut().remove(&idx);
        result
    }

    /// Compute `TypeInfo` from Pool tags.
    ///
    /// Dispatches on `pool.tag(idx)` to determine the type category and
    /// extract child type information from the Pool.
    fn compute_type_info(&self, idx: Idx) -> TypeInfo {
        // Cycle detection: Named/Applied/Alias resolution calls self.get()
        // which re-enters compute_type_info(). Detect and break the cycle.
        if !self.computing.borrow_mut().insert(idx) {
            tracing::warn!(idx = ?idx, "recursive type in compute_type_info");
            return TypeInfo::Error;
        }

        let result = self.compute_type_info_inner(idx);

        self.computing.borrow_mut().remove(&idx);
        result
    }

    /// Inner implementation of type info computation, separated for cycle guard.
    fn compute_type_info_inner(&self, idx: Idx) -> TypeInfo {
        match self.pool.tag(idx) {
            // Primitives (should already be pre-populated, but handle gracefully)
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

            // Simple containers (data = child Idx directly)
            Tag::List => TypeInfo::List {
                element: self.pool.list_elem(idx),
            },
            Tag::Option => TypeInfo::Option {
                inner: self.pool.option_inner(idx),
            },
            Tag::Set => TypeInfo::Set {
                element: self.pool.set_elem(idx),
            },
            Tag::Range => {
                // Currently range is always range<int> with fixed layout.
                // Verify element type is Int (or NONE for unparameterized).
                let elem = self.pool.range_elem(idx);
                debug_assert!(
                    self.pool.tag(elem) == Tag::Int || elem == Idx::NONE,
                    "Range element type is not Int — generic range not yet implemented"
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

            // Struct: read field data from Pool's extra array
            Tag::Struct => {
                let fields = self.pool.struct_fields(idx);
                TypeInfo::Struct { fields }
            }

            // Enum: read variant data from Pool's extra array
            Tag::Enum => {
                let pool_variants = self.pool.enum_variants(idx);
                let variants = pool_variants
                    .into_iter()
                    .map(|(name, field_types)| EnumVariantInfo {
                        name,
                        fields: field_types,
                    })
                    .collect();
                TypeInfo::Enum { variants }
            }

            // Named types: resolve to concrete Struct/Enum via Pool resolution table
            Tag::Named | Tag::Applied | Tag::Alias => {
                if let Some(resolved) = self.pool.resolve(idx) {
                    self.get(resolved)
                } else {
                    tracing::warn!(
                        tag = ?self.pool.tag(idx),
                        "Named/Applied/Alias type has no Pool resolution — \
                         may be a generic type parameter or unregistered type"
                    );
                    TypeInfo::Error
                }
            }

            // Type variables: follow unification link chains to the resolved type.
            //
            // Type inference creates fresh variables (e.g., `Ok(42)` gets type
            // `Result<int, ?E>`). Unification resolves `?E = str` via
            // `VarState::Link`, but the canonical IR may store the pre-resolution
            // Idx. Follow the link chain here to find the concrete type.
            Tag::Var => {
                let resolved = self.pool.resolve_fully(idx);
                if resolved != idx {
                    return self.get(resolved);
                }
                tracing::error!(
                    ?idx,
                    "unresolved type variable at codegen — type inference bug"
                );
                TypeInfo::Error
            }

            // Iterator: opaque heap-allocated handle (runtime pointer).
            Tag::Iterator | Tag::DoubleEndedIterator => TypeInfo::Iterator {
                element: self.pool.iterator_elem(idx),
            },

            // These tags should genuinely never reach codegen.
            Tag::BoundVar
            | Tag::RigidVar
            | Tag::Borrowed
            | Tag::Scheme
            | Tag::Projection
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => {
                tracing::error!(
                    tag = ?self.pool.tag(idx),
                    "unreachable type tag at codegen — type inference bug"
                );
                TypeInfo::Error
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TypeLayoutResolver — recursive LLVM type resolution
// ---------------------------------------------------------------------------

/// Resolves `Idx` → `BasicTypeEnum` with cycle-safe two-phase struct creation.
///
/// For recursive types like `type Tree = Leaf(int) | Node(Tree, Tree)`, LLVM
/// requires a two-phase approach:
/// 1. Create an opaque named struct (`%Tree = type opaque`)
/// 2. Recursively resolve field types (which may reference `%Tree`)
/// 3. Fill the struct body (`%Tree = type { i8, [2 x i64] }`)
///
/// This follows the same pattern used by:
/// - Rust's `rustc_codegen_llvm` (`declare_type` → `define_type`)
/// - Zig's `codegen/llvm.zig` (`lowerType` with `TypeMap`)
/// - Roc's `gen_llvm/src/llvm/convert.rs` (`basic_type_from_layout`)
pub struct TypeLayoutResolver<'a, 'll, 'tcx> {
    /// Type info store for looking up `TypeInfo` by `Idx`.
    store: &'a TypeInfoStore<'tcx>,
    /// LLVM simple context for type construction.
    scx: &'a SimpleCx<'ll>,
    /// Types currently being resolved (cycle detection).
    ///
    /// When we encounter an `Idx` already in this set, we've found a cycle
    /// and return the previously created opaque struct instead of recursing.
    resolving: RefCell<FxHashSet<Idx>>,
    /// Resolved LLVM types cache.
    cache: RefCell<FxHashMap<Idx, BasicTypeEnum<'ll>>>,
    /// Named struct types created during resolution (for body filling).
    named_structs: RefCell<FxHashMap<Idx, StructType<'ll>>>,
    /// Recursion depth counter for indirect cycle detection.
    ///
    /// The `resolving` set catches direct cycles (same `Idx`), but misses
    /// indirect cycles where `Named(A)` → `Idx(B)` → `Named(C)` → back to
    /// a type containing `A` — all different Idx values. The depth counter
    /// catches these and also prevents stack overflow from deeply nested types.
    depth: Cell<u32>,
}

impl<'a, 'll, 'tcx> TypeLayoutResolver<'a, 'll, 'tcx> {
    /// Create a new resolver.
    pub fn new(store: &'a TypeInfoStore<'tcx>, scx: &'a SimpleCx<'ll>) -> Self {
        Self {
            store,
            scx,
            resolving: RefCell::new(FxHashSet::default()),
            cache: RefCell::new(FxHashMap::default()),
            named_structs: RefCell::new(FxHashMap::default()),
            depth: Cell::new(0),
        }
    }

    /// Resolve an `Idx` to its LLVM type, handling recursive types correctly.
    ///
    /// For non-recursive types this delegates to `TypeInfo::storage_type()`.
    /// For structs and enums it uses two-phase creation with cycle detection.
    /// Maximum recursion depth for type resolution.
    ///
    /// Catches indirect cycles (different Idx values for the same conceptual
    /// type) and prevents stack overflow from deeply nested types.
    const MAX_RESOLVE_DEPTH: u32 = 32;

    pub fn resolve(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        // Sentinel
        if idx == Idx::NONE {
            return self.scx.type_i64().into();
        }

        // Cache hit
        if let Some(&cached) = self.cache.borrow().get(&idx) {
            return cached;
        }

        // Depth guard: catch indirect cycles and prevent stack overflow.
        let current_depth = self.depth.get();
        if current_depth >= Self::MAX_RESOLVE_DEPTH {
            tracing::warn!(idx = ?idx, depth = current_depth, "type resolution depth limit");
            return self.scx.type_i64().into();
        }
        self.depth.set(current_depth + 1);

        let resolved = self.resolve_inner(idx);

        self.depth.set(current_depth);
        resolved
    }

    /// Inner resolve implementation, separated for depth guard.
    fn resolve_inner(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        // Cycle detection: if we're already resolving this type, we've
        // found a recursive reference. For Struct/Enum this is handled by
        // the two-phase named struct pattern. For other types (Option,
        // Result, Tuple), fall back to i64 to break the cycle.
        if self.resolving.borrow().contains(&idx) {
            // Check if a named struct was already created (Struct/Enum path)
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            // For non-Struct/Enum cycles, fall back to i64
            return self.scx.type_i64().into();
        }

        let info = self.store.get(idx);
        let result = match &info {
            // Primitives, collections, handles: no recursion possible.
            // Delegate to the standalone storage_type() method.
            TypeInfo::Int
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte
            | TypeInfo::Unit
            | TypeInfo::Never
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Ordering
            | TypeInfo::Range
            | TypeInfo::Str
            | TypeInfo::List { .. }
            | TypeInfo::Map { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. }
            | TypeInfo::Function { .. }
            | TypeInfo::Error => info.storage_type(self.scx),

            // Tagged unions with possible recursive payloads.
            TypeInfo::Option { inner } => {
                self.resolving.borrow_mut().insert(idx);
                let payload = self.resolve(*inner);
                self.resolving.borrow_mut().remove(&idx);
                self.scx
                    .type_struct(&[self.scx.type_i8().into(), payload], false)
                    .into()
            }
            TypeInfo::Result { ok, err } => {
                self.resolving.borrow_mut().insert(idx);
                let ok_ty = self.resolve(*ok);
                let err_ty = self.resolve(*err);
                self.resolving.borrow_mut().remove(&idx);
                // Use the larger of the two as the payload type.
                let ok_size = Self::type_store_size(ok_ty);
                let err_size = Self::type_store_size(err_ty);
                let payload = if ok_size >= err_size { ok_ty } else { err_ty };
                self.scx
                    .type_struct(&[self.scx.type_i8().into(), payload], false)
                    .into()
            }

            // Tuple: struct of recursively-resolved element types.
            TypeInfo::Tuple { elements } => {
                self.resolving.borrow_mut().insert(idx);
                let field_types: Vec<BasicTypeEnum<'ll>> =
                    elements.iter().map(|&e| self.resolve(e)).collect();
                self.resolving.borrow_mut().remove(&idx);
                self.scx.type_struct(&field_types, false).into()
            }

            // User-defined struct: two-phase creation.
            TypeInfo::Struct { fields } => self.resolve_struct(idx, fields),

            // User-defined enum: two-phase creation.
            TypeInfo::Enum { variants } => self.resolve_enum(idx, variants),
        };

        self.cache.borrow_mut().insert(idx, result);
        result
    }

    /// Resolve a struct type with two-phase creation for cycle safety.
    fn resolve_struct(&self, idx: Idx, fields: &[(Name, Idx)]) -> BasicTypeEnum<'ll> {
        // Cycle detection: if already resolving this type, return the
        // opaque struct created by the outer call.
        if self.resolving.borrow().contains(&idx) {
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            // Fallback: shouldn't happen, but if the named struct wasn't
            // created yet, use a pointer (recursive types are boxed).
            return self.scx.type_ptr().into();
        }

        // Phase 1: Create opaque named struct.
        let name = self.type_name(idx, "Struct");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);

        // Mark as resolving (cycle detection guard).
        self.resolving.borrow_mut().insert(idx);

        // Phase 2: Recursively resolve field types.
        let field_types: Vec<BasicTypeEnum<'ll>> =
            fields.iter().map(|&(_, ty)| self.resolve(ty)).collect();

        // Phase 3: Fill struct body.
        self.scx.set_struct_body(named_struct, &field_types, false);

        // Unmark resolving.
        self.resolving.borrow_mut().remove(&idx);

        named_struct.into()
    }

    /// Resolve an enum type with two-phase creation for cycle safety.
    ///
    /// Layout: `{ i8 tag, [M x i64] payload }` where M is enough i64s to
    /// hold the largest variant's fields.
    fn resolve_enum(&self, idx: Idx, variants: &[EnumVariantInfo]) -> BasicTypeEnum<'ll> {
        // Cycle detection
        if self.resolving.borrow().contains(&idx) {
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            return self.scx.type_ptr().into();
        }

        // Phase 1: Create opaque named struct.
        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);

        // Mark as resolving.
        self.resolving.borrow_mut().insert(idx);

        // Phase 2: Compute max payload size across all variants.
        let mut max_payload_bytes: u64 = 0;
        for variant in variants {
            let variant_bytes: u64 = variant
                .fields
                .iter()
                .map(|&f| {
                    let ty = self.resolve(f);
                    Self::type_store_size(ty)
                })
                .sum();
            max_payload_bytes = max_payload_bytes.max(variant_bytes);
        }

        // Phase 3: Fill enum body.
        let tag_ty = self.scx.type_i8();
        if max_payload_bytes == 0 {
            // All-unit enum: just a tag.
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into()], false);
        } else {
            // Payload as i64 array for natural alignment.
            let payload_i64_count = max_payload_bytes.div_ceil(8);
            let payload_ty = self.scx.type_i64().array_type(payload_i64_count as u32);
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into(), payload_ty.into()], false);
        }

        // Unmark resolving.
        self.resolving.borrow_mut().remove(&idx);

        named_struct.into()
    }

    /// Get a human-readable name for an LLVM named struct.
    ///
    /// Tries to look up the type name from the Pool. Falls back to
    /// `"{fallback}.{raw_index}"` if the name isn't available.
    fn type_name(&self, idx: Idx, fallback: &str) -> String {
        let pool = self.store.pool();
        if idx.raw() as usize >= pool.len() {
            return format!("ori.{}.{}", fallback, idx.raw());
        }
        match pool.tag(idx) {
            Tag::Struct => {
                let name = pool.struct_name(idx);
                format!("ori.{}", name.raw())
            }
            Tag::Enum => {
                let name = pool.enum_name(idx);
                format!("ori.{}", name.raw())
            }
            _ => format!("ori.{}.{}", fallback, idx.raw()),
        }
    }

    /// Approximate store size of an LLVM type in bytes.
    ///
    /// Uses LLVM's type system to determine sizes. For types where we
    /// can't easily determine the size, falls back to 8 bytes (i64-sized).
    pub(crate) fn type_store_size(ty: BasicTypeEnum<'ll>) -> u64 {
        Self::type_store_size_inner(ty, 0)
    }

    /// Inner implementation with depth tracking for recursive struct types.
    fn type_store_size_inner(ty: BasicTypeEnum<'ll>, depth: u32) -> u64 {
        if depth > 16 {
            return 8; // Fall back to pointer size
        }
        match ty {
            BasicTypeEnum::IntType(t) => {
                let bits = t.get_bit_width();
                u64::from(bits).div_ceil(8)
            }
            BasicTypeEnum::StructType(st) => {
                // Opaque structs have no body yet (two-phase creation).
                if st.is_opaque() {
                    return 8; // Pointer-sized fallback
                }
                // Sum of field sizes (approximation — ignores padding).
                // For our purposes this is sufficient: we only use this to
                // compare variant payload sizes and pick the max.
                let mut total = 0u64;
                for i in 0..st.count_fields() {
                    if let Some(field) = st.get_field_type_at_index(i) {
                        total += Self::type_store_size_inner(field, depth + 1);
                    }
                }
                total
            }
            BasicTypeEnum::ArrayType(at) => {
                let elem_size = Self::type_store_size_inner(at.get_element_type(), depth + 1);
                elem_size * u64::from(at.len())
            }
            // Float (f64), Pointer, Vector, ScalableVector: all 8 bytes
            _ => 8,
        }
    }

    /// Access the underlying `TypeInfoStore`.
    pub fn store(&self) -> &'a TypeInfoStore<'tcx> {
        self.store
    }

    /// Look up a resolved named struct for a given `Idx`.
    pub fn get_named_struct(&self, idx: Idx) -> Option<StructType<'ll>> {
        self.named_structs.borrow().get(&idx).copied()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::uninlined_format_args,
    clippy::doc_markdown,
    reason = "benchmark/test code — precision loss acceptable for display, style relaxed"
)]
mod tests;
