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

            // Channel: opaque heap-allocated handle
            Self::Channel { .. } => scx.type_ptr().into(),

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
mod tests {
    use super::*;
    use inkwell::context::Context;

    /// Helper to create a Pool with just the pre-interned primitives.
    fn test_pool() -> Pool {
        Pool::new()
    }

    // -- TypeInfo classification tests --

    #[test]
    fn primitive_triviality() {
        assert!(TypeInfo::Int.is_trivial());
        assert!(TypeInfo::Float.is_trivial());
        assert!(TypeInfo::Bool.is_trivial());
        assert!(TypeInfo::Char.is_trivial());
        assert!(TypeInfo::Byte.is_trivial());
        assert!(TypeInfo::Unit.is_trivial());
        assert!(TypeInfo::Never.is_trivial());
        assert!(TypeInfo::Duration.is_trivial());
        assert!(TypeInfo::Size.is_trivial());
        assert!(TypeInfo::Ordering.is_trivial());
        assert!(TypeInfo::Range.is_trivial());
        assert!(TypeInfo::Error.is_trivial());
    }

    #[test]
    fn heap_types_not_trivial() {
        assert!(!TypeInfo::Str.is_trivial());
        assert!(!TypeInfo::List { element: Idx::INT }.is_trivial());
        assert!(!TypeInfo::Map {
            key: Idx::STR,
            value: Idx::INT
        }
        .is_trivial());
        assert!(!TypeInfo::Set { element: Idx::INT }.is_trivial());
        assert!(!TypeInfo::Channel { element: Idx::INT }.is_trivial());
        assert!(!TypeInfo::Function {
            params: vec![Idx::INT],
            ret: Idx::INT
        }
        .is_trivial());
    }

    #[test]
    fn tagged_unions_not_trivial() {
        assert!(!TypeInfo::Option { inner: Idx::INT }.is_trivial());
        assert!(!TypeInfo::Result {
            ok: Idx::INT,
            err: Idx::STR
        }
        .is_trivial());
    }

    // -- Size tests --

    #[test]
    fn primitive_sizes() {
        assert_eq!(TypeInfo::Int.size(), Some(8));
        assert_eq!(TypeInfo::Float.size(), Some(8));
        assert_eq!(TypeInfo::Bool.size(), Some(1));
        assert_eq!(TypeInfo::Char.size(), Some(4));
        assert_eq!(TypeInfo::Byte.size(), Some(1));
        assert_eq!(TypeInfo::Unit.size(), Some(8));
        assert_eq!(TypeInfo::Never.size(), Some(8));
        assert_eq!(TypeInfo::Duration.size(), Some(8));
        assert_eq!(TypeInfo::Size.size(), Some(8));
        assert_eq!(TypeInfo::Ordering.size(), Some(1));
    }

    #[test]
    fn composite_sizes() {
        assert_eq!(TypeInfo::Str.size(), Some(16));
        assert_eq!(TypeInfo::List { element: Idx::INT }.size(), Some(24));
        assert_eq!(
            TypeInfo::Map {
                key: Idx::STR,
                value: Idx::INT
            }
            .size(),
            Some(32)
        );
        assert_eq!(TypeInfo::Range.size(), Some(24));
        assert_eq!(TypeInfo::Option { inner: Idx::INT }.size(), Some(16));
        assert_eq!(TypeInfo::Channel { element: Idx::INT }.size(), Some(8));
        assert_eq!(
            TypeInfo::Function {
                params: vec![],
                ret: Idx::UNIT
            }
            .size(),
            Some(16)
        );
    }

    #[test]
    fn dynamic_sizes_are_none() {
        assert_eq!(
            TypeInfo::Tuple {
                elements: vec![Idx::INT, Idx::STR]
            }
            .size(),
            None
        );
        assert_eq!(TypeInfo::Struct { fields: vec![] }.size(), None);
        assert_eq!(TypeInfo::Enum { variants: vec![] }.size(), None);
    }

    // -- Alignment tests --

    #[test]
    fn alignment_values() {
        assert_eq!(TypeInfo::Bool.alignment(), 1);
        assert_eq!(TypeInfo::Byte.alignment(), 1);
        assert_eq!(TypeInfo::Ordering.alignment(), 1);
        assert_eq!(TypeInfo::Char.alignment(), 4);
        assert_eq!(TypeInfo::Int.alignment(), 8);
        assert_eq!(TypeInfo::Float.alignment(), 8);
        assert_eq!(TypeInfo::Str.alignment(), 8);
    }

    // -- Loadability tests --

    #[test]
    fn loadable_types() {
        assert!(TypeInfo::Int.is_loadable());
        assert!(TypeInfo::Str.is_loadable()); // 16 bytes fits in 2 registers
        assert!(TypeInfo::Option { inner: Idx::INT }.is_loadable()); // 16 bytes
    }

    #[test]
    fn non_loadable_types() {
        assert!(!TypeInfo::List { element: Idx::INT }.is_loadable()); // 24 bytes
        assert!(!TypeInfo::Map {
            key: Idx::STR,
            value: Idx::INT
        }
        .is_loadable()); // 32 bytes
    }

    // -- Storage type tests --

    #[test]
    fn primitive_storage_types() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");

        // i64 types
        let i64_ty: BasicTypeEnum = scx.type_i64().into();
        assert_eq!(TypeInfo::Int.storage_type(&scx), i64_ty);
        assert_eq!(TypeInfo::Duration.storage_type(&scx), i64_ty);
        assert_eq!(TypeInfo::Size.storage_type(&scx), i64_ty);
        assert_eq!(TypeInfo::Unit.storage_type(&scx), i64_ty);
        assert_eq!(TypeInfo::Never.storage_type(&scx), i64_ty);

        // Other primitives
        assert_eq!(TypeInfo::Float.storage_type(&scx), scx.type_f64().into());
        assert_eq!(TypeInfo::Bool.storage_type(&scx), scx.type_i1().into());
        assert_eq!(TypeInfo::Char.storage_type(&scx), scx.type_i32().into());
        assert_eq!(TypeInfo::Byte.storage_type(&scx), scx.type_i8().into());
        assert_eq!(TypeInfo::Ordering.storage_type(&scx), scx.type_i8().into());
    }

    #[test]
    fn channel_type_is_pointer() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");

        let ptr_ty: BasicTypeEnum = scx.type_ptr().into();
        assert_eq!(
            TypeInfo::Channel { element: Idx::INT }.storage_type(&scx),
            ptr_ty
        );
    }

    #[test]
    fn function_type_is_fat_pointer() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");

        let func_ty = TypeInfo::Function {
            params: vec![],
            ret: Idx::UNIT,
        }
        .storage_type(&scx);
        // Should be a struct { ptr, ptr }
        match func_ty {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 2, "fat pointer should have 2 fields");
                assert!(
                    st.get_field_type_at_index(0).unwrap().is_pointer_type(),
                    "first field should be ptr"
                );
                assert!(
                    st.get_field_type_at_index(1).unwrap().is_pointer_type(),
                    "second field should be ptr"
                );
            }
            other => panic!("Expected StructType for Function, got {other:?}"),
        }
    }

    // -- TypeInfoStore tests --

    #[test]
    fn store_primitive_lookup() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);

        // Primitives should be pre-populated
        assert!(matches!(store.get(Idx::INT), TypeInfo::Int));
        assert!(matches!(store.get(Idx::FLOAT), TypeInfo::Float));
        assert!(matches!(store.get(Idx::BOOL), TypeInfo::Bool));
        assert!(matches!(store.get(Idx::STR), TypeInfo::Str));
        assert!(matches!(store.get(Idx::CHAR), TypeInfo::Char));
        assert!(matches!(store.get(Idx::BYTE), TypeInfo::Byte));
        assert!(matches!(store.get(Idx::UNIT), TypeInfo::Unit));
        assert!(matches!(store.get(Idx::NEVER), TypeInfo::Never));
        assert!(matches!(store.get(Idx::DURATION), TypeInfo::Duration));
        assert!(matches!(store.get(Idx::SIZE), TypeInfo::Size));
        assert!(matches!(store.get(Idx::ORDERING), TypeInfo::Ordering));
    }

    #[test]
    fn store_none_returns_error() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);
        assert!(matches!(store.get(Idx::NONE), TypeInfo::Error));
    }

    #[test]
    fn store_reserved_slots_are_error() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);

        // Indices 12-63 are reserved padding
        assert!(matches!(store.get(Idx::from_raw(12)), TypeInfo::Error));
        assert!(matches!(store.get(Idx::from_raw(32)), TypeInfo::Error));
        assert!(matches!(store.get(Idx::from_raw(63)), TypeInfo::Error));
    }

    #[test]
    fn store_dynamic_list_type() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(list_int);
        match info {
            TypeInfo::List { element } => assert_eq!(element, Idx::INT),
            other => panic!("Expected TypeInfo::List, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_map_type() {
        let mut pool = Pool::new();
        let map_str_int = pool.map(Idx::STR, Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(map_str_int);
        match info {
            TypeInfo::Map { key, value } => {
                assert_eq!(key, Idx::STR);
                assert_eq!(value, Idx::INT);
            }
            other => panic!("Expected TypeInfo::Map, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_option_type() {
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(opt_int);
        match info {
            TypeInfo::Option { inner } => assert_eq!(inner, Idx::INT),
            other => panic!("Expected TypeInfo::Option, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_result_type() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::INT, Idx::STR);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(res);
        match info {
            TypeInfo::Result { ok, err } => {
                assert_eq!(ok, Idx::INT);
                assert_eq!(err, Idx::STR);
            }
            other => panic!("Expected TypeInfo::Result, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_tuple_type() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(tup);
        match info {
            TypeInfo::Tuple { elements } => {
                assert_eq!(elements, vec![Idx::INT, Idx::STR, Idx::BOOL]);
            }
            other => panic!("Expected TypeInfo::Tuple, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_function_type() {
        let mut pool = Pool::new();
        let func = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(func);
        match info {
            TypeInfo::Function { params, ret } => {
                assert_eq!(params, vec![Idx::INT, Idx::STR]);
                assert_eq!(ret, Idx::BOOL);
            }
            other => panic!("Expected TypeInfo::Function, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_set_type() {
        let mut pool = Pool::new();
        let set_int = pool.set(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(set_int);
        match info {
            TypeInfo::Set { element } => assert_eq!(element, Idx::INT),
            other => panic!("Expected TypeInfo::Set, got {other:?}"),
        }
    }

    #[test]
    fn store_dynamic_range_type() {
        let mut pool = Pool::new();
        let range = pool.range(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(range);
        assert!(matches!(info, TypeInfo::Range));
    }

    #[test]
    fn store_caches_on_second_access() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);

        let store = TypeInfoStore::new(&pool);

        // First access: computes and caches
        let info1 = store.get(list_int);
        // Second access: returns cached
        let info2 = store.get(list_int);

        // Both should be List with same element
        match (&info1, &info2) {
            (TypeInfo::List { element: e1 }, TypeInfo::List { element: e2 }) => {
                assert_eq!(e1, e2);
            }
            _ => panic!("Expected matching List types"),
        }
    }

    #[test]
    fn store_dynamic_channel_type() {
        let mut pool = Pool::new();
        let chan_int = pool.channel(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(chan_int);
        match info {
            TypeInfo::Channel { element } => assert_eq!(element, Idx::INT),
            other => panic!("Expected TypeInfo::Channel, got {other:?}"),
        }
    }

    #[test]
    fn store_struct_from_pool() {
        let mut pool = Pool::new();
        let name = Name::from_raw(10);
        let x_name = Name::from_raw(20);
        let y_name = Name::from_raw(21);

        let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(struct_idx);
        match info {
            TypeInfo::Struct { fields } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0], (x_name, Idx::INT));
                assert_eq!(fields[1], (y_name, Idx::FLOAT));
            }
            other => panic!("Expected TypeInfo::Struct, got {other:?}"),
        }
    }

    #[test]
    fn store_enum_from_pool() {
        use ori_types::EnumVariant;

        let mut pool = Pool::new();
        let name = Name::from_raw(30);
        let none_name = Name::from_raw(31);
        let some_name = Name::from_raw(32);

        let variants = vec![
            EnumVariant {
                name: none_name,
                field_types: vec![],
            },
            EnumVariant {
                name: some_name,
                field_types: vec![Idx::INT],
            },
        ];
        let enum_idx = pool.enum_type(name, &variants);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(enum_idx);
        match info {
            TypeInfo::Enum { variants } => {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, none_name);
                assert!(variants[0].fields.is_empty());
                assert_eq!(variants[1].name, some_name);
                assert_eq!(variants[1].fields, vec![Idx::INT]);
            }
            other => panic!("Expected TypeInfo::Enum, got {other:?}"),
        }
    }

    #[test]
    fn store_named_resolves_to_struct() {
        let mut pool = Pool::new();
        let name = Name::from_raw(40);
        let x_name = Name::from_raw(41);

        let named_idx = pool.named(name);
        let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT)]);
        pool.set_resolution(named_idx, struct_idx);

        let store = TypeInfoStore::new(&pool);
        let info = store.get(named_idx);
        match info {
            TypeInfo::Struct { fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0], (x_name, Idx::INT));
            }
            other => panic!("Expected TypeInfo::Struct via resolution, got {other:?}"),
        }
    }

    #[test]
    fn store_named_unresolved_is_error() {
        let mut pool = Pool::new();
        let name = Name::from_raw(50);
        let named_idx = pool.named(name);
        // No resolution registered

        let store = TypeInfoStore::new(&pool);
        let info = store.get(named_idx);
        assert!(matches!(info, TypeInfo::Error));
    }

    // -- Transitive triviality tests --

    #[test]
    fn trivial_primitives() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);

        assert!(store.is_trivial(Idx::INT));
        assert!(store.is_trivial(Idx::FLOAT));
        assert!(store.is_trivial(Idx::BOOL));
        assert!(store.is_trivial(Idx::CHAR));
        assert!(store.is_trivial(Idx::BYTE));
        assert!(store.is_trivial(Idx::UNIT));
        assert!(store.is_trivial(Idx::NEVER));
        assert!(store.is_trivial(Idx::DURATION));
        assert!(store.is_trivial(Idx::SIZE));
        assert!(store.is_trivial(Idx::ORDERING));
    }

    #[test]
    fn trivial_option_int() {
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(opt_int));
    }

    #[test]
    fn nontrivial_option_str() {
        let mut pool = Pool::new();
        let opt_str = pool.option(Idx::STR);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(opt_str));
    }

    #[test]
    fn trivial_tuple_scalars() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(tup));
    }

    #[test]
    fn nontrivial_tuple_with_str() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::STR]);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(tup));
    }

    #[test]
    fn trivial_result_scalars() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::INT, Idx::BOOL);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(res));
    }

    #[test]
    fn nontrivial_result_with_str() {
        let mut pool = Pool::new();
        let res = pool.result(Idx::INT, Idx::STR);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(res));
    }

    #[test]
    fn trivial_struct_all_scalars() {
        let mut pool = Pool::new();
        let name = Name::from_raw(200);
        let x_name = Name::from_raw(201);
        let y_name = Name::from_raw(202);

        let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(struct_idx));
    }

    #[test]
    fn nontrivial_struct_with_str_field() {
        let mut pool = Pool::new();
        let name = Name::from_raw(210);
        let x_name = Name::from_raw(211);

        let struct_idx = pool.struct_type(name, &[(x_name, Idx::STR)]);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(struct_idx));
    }

    #[test]
    fn trivial_nested_option_in_struct() {
        // struct Foo { x: option[int] } — trivial because option[int] is trivial
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);
        let name = Name::from_raw(220);
        let x_name = Name::from_raw(221);

        let struct_idx = pool.struct_type(name, &[(x_name, opt_int)]);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(struct_idx));
    }

    #[test]
    fn trivial_enum_all_unit_variants() {
        use ori_types::EnumVariant;

        let mut pool = Pool::new();
        let name = Name::from_raw(230);
        let a = Name::from_raw(231);
        let b = Name::from_raw(232);

        let variants = vec![
            EnumVariant {
                name: a,
                field_types: vec![],
            },
            EnumVariant {
                name: b,
                field_types: vec![],
            },
        ];
        let enum_idx = pool.enum_type(name, &variants);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(enum_idx));
    }

    #[test]
    fn trivial_enum_with_scalar_fields() {
        use ori_types::EnumVariant;

        let mut pool = Pool::new();
        let name = Name::from_raw(240);
        let a = Name::from_raw(241);
        let b = Name::from_raw(242);

        let variants = vec![
            EnumVariant {
                name: a,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: b,
                field_types: vec![Idx::FLOAT, Idx::BOOL],
            },
        ];
        let enum_idx = pool.enum_type(name, &variants);

        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(enum_idx));
    }

    #[test]
    fn nontrivial_enum_with_str_field() {
        use ori_types::EnumVariant;

        let mut pool = Pool::new();
        let name = Name::from_raw(250);
        let a = Name::from_raw(251);
        let b = Name::from_raw(252);

        let variants = vec![
            EnumVariant {
                name: a,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: b,
                field_types: vec![Idx::STR],
            },
        ];
        let enum_idx = pool.enum_type(name, &variants);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(enum_idx));
    }

    #[test]
    fn nontrivial_heap_types() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        let map_ty = pool.map(Idx::STR, Idx::INT);
        let set_int = pool.set(Idx::INT);
        let chan_int = pool.channel(Idx::INT);
        let func_ty = pool.function(&[Idx::INT], Idx::INT);

        let store = TypeInfoStore::new(&pool);
        assert!(!store.is_trivial(Idx::STR));
        assert!(!store.is_trivial(list_int));
        assert!(!store.is_trivial(map_ty));
        assert!(!store.is_trivial(set_int));
        assert!(!store.is_trivial(chan_int));
        assert!(!store.is_trivial(func_ty));
    }

    #[test]
    fn trivial_none_sentinel() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);
        assert!(store.is_trivial(Idx::NONE));
    }

    #[test]
    fn triviality_caching() {
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        // First call computes
        assert!(store.is_trivial(opt_int));
        // Second call hits cache — verify same result
        assert!(store.is_trivial(opt_int));
    }

    // -- TypeLayoutResolver tests --

    #[test]
    fn resolver_primitive_types() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
        assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
        assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
        assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());
        assert_eq!(resolver.resolve(Idx::BYTE), scx.type_i8().into());
    }

    #[test]
    fn resolver_simple_struct() {
        let mut pool = Pool::new();
        let name = Name::from_raw(300);
        let x_name = Name::from_raw(301);
        let y_name = Name::from_raw(302);

        let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty = resolver.resolve(struct_idx);
        // Should be a named struct with 2 fields
        match ty {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 2);
                assert!(st.get_name().is_some());
            }
            other => panic!("Expected StructType, got {other:?}"),
        }
    }

    #[test]
    fn resolver_nested_struct() {
        // struct Inner { x: int }
        // struct Outer { a: Inner, b: float }
        let mut pool = Pool::new();
        let inner_name = Name::from_raw(310);
        let outer_name = Name::from_raw(311);
        let x_name = Name::from_raw(312);
        let a_name = Name::from_raw(313);
        let b_name = Name::from_raw(314);

        let inner_idx = pool.struct_type(inner_name, &[(x_name, Idx::INT)]);
        let outer_idx = pool.struct_type(outer_name, &[(a_name, inner_idx), (b_name, Idx::FLOAT)]);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty = resolver.resolve(outer_idx);
        match ty {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 2);
                // First field should be a named struct (Inner)
                let field0 = st.get_field_type_at_index(0).unwrap();
                assert!(matches!(field0, BasicTypeEnum::StructType(_)));
            }
            other => panic!("Expected StructType, got {other:?}"),
        }
    }

    #[test]
    fn resolver_recursive_enum() {
        use ori_types::EnumVariant;

        // type Tree = Leaf(int) | Node(Tree, Tree)
        let mut pool = Pool::new();
        let tree_name = Name::from_raw(320);
        let leaf_name = Name::from_raw(321);
        let node_name = Name::from_raw(322);

        // Create a Named ref for Tree to use in Node's fields
        let tree_named = pool.named(tree_name);

        // Create the enum with Tree references in Node variant
        let variants = vec![
            EnumVariant {
                name: leaf_name,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: node_name,
                field_types: vec![tree_named, tree_named],
            },
        ];
        let tree_enum = pool.enum_type(tree_name, &variants);

        // Link Named -> Enum
        pool.set_resolution(tree_named, tree_enum);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        // Should not infinite loop!
        let ty = resolver.resolve(tree_enum);
        match ty {
            BasicTypeEnum::StructType(st) => {
                // Should be a named struct (tagged union)
                assert!(st.get_name().is_some());
                // Should have at least a tag field
                assert!(st.count_fields() >= 1);
            }
            other => panic!("Expected StructType for Tree enum, got {other:?}"),
        }

        // Recursive type should be non-trivial
        assert!(!store.is_trivial(tree_enum));
    }

    #[test]
    fn resolver_enum_all_unit() {
        use ori_types::EnumVariant;

        // type Color = Red | Green | Blue
        let mut pool = Pool::new();
        let name = Name::from_raw(330);
        let r = Name::from_raw(331);
        let g = Name::from_raw(332);
        let b = Name::from_raw(333);

        let variants = vec![
            EnumVariant {
                name: r,
                field_types: vec![],
            },
            EnumVariant {
                name: g,
                field_types: vec![],
            },
            EnumVariant {
                name: b,
                field_types: vec![],
            },
        ];
        let enum_idx = pool.enum_type(name, &variants);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty = resolver.resolve(enum_idx);
        match ty {
            BasicTypeEnum::StructType(st) => {
                // All-unit enum: just { i8 tag }
                assert_eq!(st.count_fields(), 1);
            }
            other => panic!("Expected StructType, got {other:?}"),
        }
    }

    #[test]
    fn resolver_option_with_recursive_resolve() {
        // option[int] should resolve correctly through the resolver
        let mut pool = Pool::new();
        let opt_int = pool.option(Idx::INT);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty = resolver.resolve(opt_int);
        match ty {
            BasicTypeEnum::StructType(st) => {
                // { i8 tag, i64 payload }
                assert_eq!(st.count_fields(), 2);
            }
            other => panic!("Expected StructType for option, got {other:?}"),
        }
    }

    #[test]
    fn resolver_tuple() {
        let mut pool = Pool::new();
        let tup = pool.tuple(&[Idx::INT, Idx::BOOL, Idx::FLOAT]);

        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty = resolver.resolve(tup);
        match ty {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 3);
            }
            other => panic!("Expected StructType for tuple, got {other:?}"),
        }
    }

    #[test]
    fn resolver_caches_results() {
        let pool = test_pool();
        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        let ty1 = resolver.resolve(Idx::INT);
        let ty2 = resolver.resolve(Idx::INT);
        assert_eq!(ty1, ty2);
    }

    // -- Benchmark: TypeInfoStore lookup performance --

    /// Benchmark TypeInfoStore lookup on a representative type workload.
    ///
    /// Constructs a Pool with primitives, collections, composites, and
    /// user-defined types, then measures lookup latency across all of them.
    /// Reports per-lookup timing for cached (hot) and first-access (cold) paths.
    #[test]
    fn benchmark_type_info_store_lookup() {
        use ori_types::EnumVariant;
        use std::hint::black_box;
        use std::time::Instant;

        // --- Build a representative type workload ---
        let mut pool = Pool::new();
        let mut all_indices: Vec<Idx> = Vec::new();

        // 1. Primitives (pre-interned, indices 0-11)
        let primitives = [
            Idx::INT,
            Idx::FLOAT,
            Idx::BOOL,
            Idx::STR,
            Idx::CHAR,
            Idx::BYTE,
            Idx::UNIT,
            Idx::NEVER,
            Idx::DURATION,
            Idx::SIZE,
            Idx::ORDERING,
        ];
        all_indices.extend_from_slice(&primitives);

        // 2. Simple collections
        let list_int = pool.list(Idx::INT);
        let list_str = pool.list(Idx::STR);
        let map_str_int = pool.map(Idx::STR, Idx::INT);
        let set_int = pool.set(Idx::INT);
        let range_int = pool.range(Idx::INT);
        let opt_int = pool.option(Idx::INT);
        let opt_str = pool.option(Idx::STR);
        let res_int_str = pool.result(Idx::INT, Idx::STR);
        let chan_int = pool.channel(Idx::INT);
        all_indices.extend_from_slice(&[
            list_int,
            list_str,
            map_str_int,
            set_int,
            range_int,
            opt_int,
            opt_str,
            res_int_str,
            chan_int,
        ]);

        // 3. Tuples and functions
        let tup2 = pool.tuple(&[Idx::INT, Idx::FLOAT]);
        let tup3 = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
        let func_simple = pool.function(&[Idx::INT], Idx::INT);
        let func_multi = pool.function(&[Idx::INT, Idx::STR, Idx::BOOL], Idx::FLOAT);
        all_indices.extend_from_slice(&[tup2, tup3, func_simple, func_multi]);

        // 4. User-defined structs
        let point_name = Name::from_raw(100);
        let x_name = Name::from_raw(101);
        let y_name = Name::from_raw(102);
        let point = pool.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);

        let person_name = Name::from_raw(110);
        let name_field = Name::from_raw(111);
        let age_field = Name::from_raw(112);
        let person = pool.struct_type(
            person_name,
            &[(name_field, Idx::STR), (age_field, Idx::INT)],
        );
        all_indices.extend_from_slice(&[point, person]);

        // 5. User-defined enums
        let color_name = Name::from_raw(120);
        let red = Name::from_raw(121);
        let green = Name::from_raw(122);
        let blue = Name::from_raw(123);
        let color = pool.enum_type(
            color_name,
            &[
                EnumVariant {
                    name: red,
                    field_types: vec![],
                },
                EnumVariant {
                    name: green,
                    field_types: vec![],
                },
                EnumVariant {
                    name: blue,
                    field_types: vec![],
                },
            ],
        );

        let shape_name = Name::from_raw(130);
        let circle = Name::from_raw(131);
        let rect = Name::from_raw(132);
        let shape = pool.enum_type(
            shape_name,
            &[
                EnumVariant {
                    name: circle,
                    field_types: vec![Idx::FLOAT],
                },
                EnumVariant {
                    name: rect,
                    field_types: vec![Idx::FLOAT, Idx::FLOAT],
                },
            ],
        );
        all_indices.extend_from_slice(&[color, shape]);

        // 6. Nested collections (list of tuples, option of struct, etc.)
        let list_of_tup = pool.list(tup2);
        let opt_point = pool.option(point);
        let res_person_str = pool.result(person, Idx::STR);
        all_indices.extend_from_slice(&[list_of_tup, opt_point, res_person_str]);

        let type_count = all_indices.len();

        // --- Cold lookups: first access (compute + cache) ---
        let store = TypeInfoStore::new(&pool);
        let iterations = 1000;

        let cold_start = Instant::now();
        for _ in 0..iterations {
            // Create a fresh store each iteration to measure cold path
            let fresh_store = TypeInfoStore::new(&pool);
            for &idx in &all_indices {
                black_box(fresh_store.get(idx));
            }
        }
        let cold_elapsed = cold_start.elapsed();
        let cold_per_lookup_ns =
            cold_elapsed.as_nanos() as f64 / (iterations as f64 * type_count as f64);

        // --- Hot lookups: cached access ---
        // Warm up the cache
        for &idx in &all_indices {
            store.get(idx);
        }

        let hot_iterations = 10_000;
        let hot_start = Instant::now();
        for _ in 0..hot_iterations {
            for &idx in &all_indices {
                black_box(store.get(idx));
            }
        }
        let hot_elapsed = hot_start.elapsed();
        let hot_per_lookup_ns =
            hot_elapsed.as_nanos() as f64 / (hot_iterations as f64 * type_count as f64);

        // --- Triviality classification ---
        let triv_iterations = 10_000;
        let triv_start = Instant::now();
        for _ in 0..triv_iterations {
            for &idx in &all_indices {
                black_box(store.is_trivial(idx));
            }
        }
        let triv_elapsed = triv_start.elapsed();
        let triv_per_lookup_ns =
            triv_elapsed.as_nanos() as f64 / (triv_iterations as f64 * type_count as f64);

        // --- Report ---
        eprintln!("\n=== TypeInfoStore Benchmark ===");
        eprintln!("Types: {type_count}");
        eprintln!("Cold lookup (compute+cache): {cold_per_lookup_ns:.1} ns/lookup");
        eprintln!("Hot lookup (cached):         {hot_per_lookup_ns:.1} ns/lookup");
        eprintln!("Triviality (cached):         {triv_per_lookup_ns:.1} ns/lookup");
        eprintln!("================================\n");

        // Sanity: hot lookups must be faster than cold
        assert!(
            hot_per_lookup_ns < cold_per_lookup_ns,
            "Hot lookups ({hot_per_lookup_ns:.1}ns) should be faster than cold ({cold_per_lookup_ns:.1}ns)"
        );
    }

    // -- Integration test: compile through new type system --

    /// End-to-end integration test: constructs a Pool with a variety of
    /// types (primitives, collections, structs, enums, recursive types),
    /// creates a `TypeInfoStore`, resolves all types through the
    /// `TypeLayoutResolver`, and verifies the resulting LLVM types.
    ///
    /// This validates the full TypeInfo pipeline:
    /// Pool → TypeInfoStore → TypeLayoutResolver → LLVM BasicTypeEnum
    #[test]
    fn integration_compile_through_type_system() {
        use ori_types::EnumVariant;
        use std::hint::black_box;

        let mut pool = Pool::new();

        // --- Primitives ---
        // Already interned; just verify they resolve.

        // --- Collections ---
        let list_int = pool.list(Idx::INT);
        let map_str_int = pool.map(Idx::STR, Idx::INT);
        let set_float = pool.set(Idx::FLOAT);
        let range_int = pool.range(Idx::INT);
        let opt_int = pool.option(Idx::INT);
        let opt_str = pool.option(Idx::STR);
        let res_int_str = pool.result(Idx::INT, Idx::STR);
        let chan_byte = pool.channel(Idx::BYTE);

        // --- Composites ---
        let tup_if = pool.tuple(&[Idx::INT, Idx::FLOAT]);
        let func_ii = pool.function(&[Idx::INT], Idx::INT);

        // --- User-defined struct: Point { x: int, y: int } ---
        let point_name = Name::from_raw(500);
        let x_name = Name::from_raw(501);
        let y_name = Name::from_raw(502);
        let point = pool.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);

        // --- User-defined enum: Color = Red | Green | Blue ---
        let color_name = Name::from_raw(510);
        let red = Name::from_raw(511);
        let green = Name::from_raw(512);
        let blue = Name::from_raw(513);
        let color = pool.enum_type(
            color_name,
            &[
                EnumVariant {
                    name: red,
                    field_types: vec![],
                },
                EnumVariant {
                    name: green,
                    field_types: vec![],
                },
                EnumVariant {
                    name: blue,
                    field_types: vec![],
                },
            ],
        );

        // --- Enum with payloads: Shape = Circle(float) | Rect(float, float) ---
        let shape_name = Name::from_raw(520);
        let circle = Name::from_raw(521);
        let rect = Name::from_raw(522);
        let shape = pool.enum_type(
            shape_name,
            &[
                EnumVariant {
                    name: circle,
                    field_types: vec![Idx::FLOAT],
                },
                EnumVariant {
                    name: rect,
                    field_types: vec![Idx::FLOAT, Idx::FLOAT],
                },
            ],
        );

        // --- Recursive enum: Tree = Leaf(int) | Node(Tree, Tree) ---
        let tree_name = Name::from_raw(530);
        let leaf = Name::from_raw(531);
        let node = Name::from_raw(532);
        let tree_named = pool.named(tree_name);
        let tree_enum = pool.enum_type(
            tree_name,
            &[
                EnumVariant {
                    name: leaf,
                    field_types: vec![Idx::INT],
                },
                EnumVariant {
                    name: node,
                    field_types: vec![tree_named, tree_named],
                },
            ],
        );
        pool.set_resolution(tree_named, tree_enum);

        // --- Named type alias: MyPoint -> Point ---
        let my_point_name = Name::from_raw(540);
        let my_point = pool.named(my_point_name);
        pool.set_resolution(my_point, point);

        // --- Nested: option[Point], [Shape], result[Tree, str] ---
        let opt_point = pool.option(point);
        let list_shape = pool.list(shape);
        let res_tree_str = pool.result(tree_named, Idx::STR);

        // === Build TypeInfoStore and TypeLayoutResolver ===
        let store = TypeInfoStore::new(&pool);
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "integration_test");
        let resolver = TypeLayoutResolver::new(&store, &scx);

        // === Verify all types resolve without panic ===
        let all_types = [
            // Primitives
            Idx::INT,
            Idx::FLOAT,
            Idx::BOOL,
            Idx::STR,
            Idx::CHAR,
            Idx::BYTE,
            Idx::UNIT,
            Idx::NEVER,
            Idx::DURATION,
            Idx::SIZE,
            Idx::ORDERING,
            // Collections
            list_int,
            map_str_int,
            set_float,
            range_int,
            opt_int,
            opt_str,
            res_int_str,
            chan_byte,
            // Composites
            tup_if,
            func_ii,
            // User-defined
            point,
            color,
            shape,
            tree_enum,
            // Named/alias
            my_point,
            tree_named,
            // Nested
            opt_point,
            list_shape,
            res_tree_str,
        ];

        for &idx in &all_types {
            // TypeInfoStore: get() must succeed
            let info = store.get(idx);
            assert!(
                !matches!(info, TypeInfo::Error),
                "TypeInfo::Error for idx {} (tag {:?})",
                idx.raw(),
                pool.tag(idx)
            );

            // TypeLayoutResolver: resolve() must produce a valid LLVM type
            let llvm_ty = resolver.resolve(idx);
            // All types should produce a non-void BasicTypeEnum
            let _ = black_box(llvm_ty);
        }

        // === Verify specific type properties ===

        // Primitives: correct storage types
        assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
        assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
        assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
        assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());

        // Point struct: 2 fields (both i64)
        match resolver.resolve(point) {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 2, "Point should have 2 fields");
                assert!(st.get_name().is_some(), "Point should be a named struct");
            }
            other => panic!("Point should be StructType, got {other:?}"),
        }

        // Color enum: all-unit → just {i8 tag}
        match resolver.resolve(color) {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(
                    st.count_fields(),
                    1,
                    "All-unit Color enum should have 1 field (tag)"
                );
            }
            other => panic!("Color should be StructType, got {other:?}"),
        }

        // Shape enum: {i8 tag, payload}
        match resolver.resolve(shape) {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(st.count_fields(), 2, "Shape enum should have tag + payload");
            }
            other => panic!("Shape should be StructType, got {other:?}"),
        }

        // Tree (recursive): should resolve without infinite loop, be a named struct
        match resolver.resolve(tree_enum) {
            BasicTypeEnum::StructType(st) => {
                assert!(st.get_name().is_some(), "Tree should be a named struct");
            }
            other => panic!("Tree should be StructType, got {other:?}"),
        }

        // Named alias: MyPoint should resolve to same shape as Point
        let my_point_ty = resolver.resolve(my_point);
        match my_point_ty {
            BasicTypeEnum::StructType(st) => {
                assert_eq!(
                    st.count_fields(),
                    2,
                    "MyPoint alias should resolve to Point's 2 fields"
                );
            }
            other => panic!("MyPoint alias should resolve to StructType, got {other:?}"),
        }

        // === Verify triviality classification ===
        assert!(store.is_trivial(Idx::INT), "int should be trivial");
        assert!(!store.is_trivial(Idx::STR), "str should NOT be trivial");
        assert!(
            store.is_trivial(point),
            "Point{{int,int}} should be trivial"
        );
        assert!(
            !store.is_trivial(tree_enum),
            "Recursive Tree should NOT be trivial"
        );
        assert!(
            store.is_trivial(color),
            "All-unit Color enum should be trivial"
        );
        assert!(store.is_trivial(opt_int), "option[int] should be trivial");
        assert!(
            !store.is_trivial(opt_str),
            "option[str] should NOT be trivial"
        );

        // === Sentinel handling ===
        assert!(matches!(store.get(Idx::NONE), TypeInfo::Error));
        assert_eq!(resolver.resolve(Idx::NONE), scx.type_i64().into());
    }
}
