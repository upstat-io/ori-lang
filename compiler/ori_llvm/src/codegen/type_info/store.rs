//! `TypeInfoStore` — cached `Idx` → `TypeInfo` mapping for codegen.
//!
//! Maps every `Idx` from the type checker's Pool to its `TypeInfo` variant.
//! Indices 0-63 are pre-populated at construction (12 real primitives +
//! 52 Error padding). Dynamic types (index >= 64) are populated lazily
//! on first access.
//!
//! Uses indexed storage (`Vec`) for O(1) lookup — `Idx` values are dense.
//! No Arc, no dyn, no `RwLock` — single-threaded per codegen context.

use std::cell::RefCell;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::{triviality, Idx, Pool, Tag};

use super::info::{EnumVariantInfo, TypeInfo};

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
    /// When constructed via `new_with_plan()`, pre-populated from
    /// `ReprPlan::is_trivial()` for all Pool types at construction.
    /// When constructed via `new()` (test path), populated lazily from
    /// `ori_types::triviality::classify_triviality()` — the single source
    /// of truth for triviality.
    triviality_cache: RefCell<FxHashMap<Idx, bool>>,

    /// Types currently being computed in `compute_type_info()` (cycle detection).
    ///
    /// Named/Applied/Alias resolution calls `self.get(resolved)` which can
    /// re-enter `compute_type_info()` for another Named type — unbounded
    /// recursion. This set detects the cycle and returns `TypeInfo::Error`.
    computing: RefCell<FxHashSet<Idx>>,
}

impl<'tcx> TypeInfoStore<'tcx> {
    /// Create a new store without a `ReprPlan` (test path).
    ///
    /// Triviality is lazily computed via `ori_types::triviality::classify_triviality()`
    /// — the single source of truth. For production paths, prefer
    /// [`new_with_plan()`] which pre-computes triviality from `ReprPlan`.
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
            computing: RefCell::new(FxHashSet::default()),
        }
    }

    /// Create a new store with triviality pre-computed from a `ReprPlan`.
    ///
    /// Production call sites (JIT + AOT) should use this constructor.
    /// Queries `ReprPlan::is_trivial()` for all Pool types at construction
    /// time, populating the triviality cache. After construction, `is_trivial()`
    /// is O(1) cache lookup — no lazy walk needed.
    pub fn new_with_plan(pool: &'tcx Pool, repr_plan: &ori_repr::ReprPlan) -> Self {
        let mut entries = Vec::with_capacity(64);
        for i in 0..64u32 {
            let idx = Idx::from_raw(i);
            let info = Self::primitive_type_info(pool, idx);
            entries.push(Some(info));
        }
        // Pre-compute triviality for all Pool types from the plan.
        let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
        let mut cache =
            FxHashMap::with_capacity_and_hasher(pool_len as usize, rustc_hash::FxBuildHasher);
        for raw in 0..pool_len {
            let idx = Idx::from_raw(raw);
            cache.insert(idx, repr_plan.is_trivial(idx));
        }
        Self {
            entries: RefCell::new(entries),
            pool,
            triviality_cache: RefCell::new(cache),
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
    /// When a `ReprPlan` is available (production paths via `new_with_plan()`),
    /// the cache is pre-populated from `ReprPlan::is_trivial()`. Otherwise
    /// (test paths via `new()`), lazily delegates to the single source of
    /// truth: `ori_types::triviality::classify_triviality()`.
    pub fn is_trivial(&self, idx: Idx) -> bool {
        // Sentinel
        if idx == Idx::NONE {
            return true;
        }

        // Fast path: cache hit (always populated when constructed with plan).
        if let Some(&cached) = self.triviality_cache.borrow().get(&idx) {
            return cached;
        }

        // Fallback: delegate to the canonical source of truth.
        let result =
            triviality::classify_triviality(idx, self.pool) == triviality::Triviality::Trivial;
        self.triviality_cache.borrow_mut().insert(idx, result);
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
    #[expect(
        clippy::too_many_lines,
        reason = "type info dispatch table over all Tag variants"
    )]
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

            // Named types: resolve to concrete Struct/Enum via Pool resolution table.
            // Use resolve_fully() which chains: resolve() → Applied→Named fallback.
            Tag::Named | Tag::Applied | Tag::Alias => {
                let resolved = self.pool.resolve_fully(idx);
                if resolved == idx {
                    tracing::warn!(
                        ?idx,
                        tag = ?self.pool.tag(idx),
                        "Named/Applied/Alias type has no Pool resolution — \
                         may be a generic type parameter or unregistered type"
                    );
                    TypeInfo::Error
                } else {
                    self.get(resolved)
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
                // Cross-phase invariant contract (Type Checker → Codegen):
                // All type variables must be resolved before codegen. An
                // unresolved Tag::Var here indicates a type inference bug.
                // Spec: impl-hygiene.md § Cross-Phase Invariant Contracts.
                //
                // NOTE: A targeted entry-point validation (walking function
                // signatures at codegen entry) is the correct enforcement
                // mechanism. An inline debug_assert!(false) here is too
                // aggressive — it fires on types queried during lazy lookup
                // that may not be critical to emission. The tracing::error!
                // provides detection; TypeInfo::Error provides graceful
                // degradation in release builds.
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
