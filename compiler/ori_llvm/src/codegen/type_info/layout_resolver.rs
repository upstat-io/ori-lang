//! `TypeLayoutResolver` — recursive LLVM type resolution with cycle detection.
//!
//! Resolves `Idx` → `BasicTypeEnum` with two-phase struct creation for
//! recursive types. Extracted from `type_info/mod.rs` for file size hygiene.

use std::cell::{Cell, RefCell};

use inkwell::types::{BasicTypeEnum, StructType};
use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Tag};

use super::store::TypeInfoStore;
use super::TypeInfo;
use crate::context::SimpleCx;

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
    pub(super) store: &'a TypeInfoStore<'tcx>,
    /// LLVM simple context for type construction.
    pub(super) scx: &'a SimpleCx<'ll>,
    /// String interner for resolving `Name` → human-readable strings.
    ///
    /// When present, struct/enum types get meaningful LLVM names like `%ori.Point`.
    /// When absent (e.g., in unit tests), falls back to numeric IDs like `%ori.3`.
    interner: Option<&'a StringInterner>,
    /// Representation plan from `ori_repr` (Phase A migration).
    ///
    /// When present, type lookups consult the `ReprPlan` first for non-recursive
    /// types (primitives, fat pointers, opaque pointers). When absent (or when
    /// the plan has no entry for a type, or the type requires recursive
    /// resolution), falls back to `TypeInfoStore`.
    pub(super) repr_plan: Option<&'a ori_repr::ReprPlan>,
    /// Types currently being resolved (cycle detection).
    ///
    /// When we encounter an `Idx` already in this set, we've found a cycle
    /// and return the previously created opaque struct instead of recursing.
    pub(super) resolving: RefCell<FxHashSet<Idx>>,
    /// Resolved LLVM types cache.
    cache: RefCell<FxHashMap<Idx, BasicTypeEnum<'ll>>>,
    /// Named struct types created during resolution (for body filling).
    pub(super) named_structs: RefCell<FxHashMap<Idx, StructType<'ll>>>,
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
    ///
    /// Pass an `interner` to get human-readable LLVM type names (e.g., `%ori.Point`).
    /// Without it, types get numeric names (e.g., `%ori.3`).
    /// Access the representation plan (if available).
    ///
    /// Used by `ArcIrEmitter` (Phase B) to query per-variable ranges for
    /// local variable narrowing.
    pub fn repr_plan(&self) -> Option<&'a ori_repr::ReprPlan> {
        self.repr_plan
    }

    pub fn new(
        store: &'a TypeInfoStore<'tcx>,
        scx: &'a SimpleCx<'ll>,
        interner: Option<&'a StringInterner>,
        repr_plan: Option<&'a ori_repr::ReprPlan>,
    ) -> Self {
        Self {
            store,
            scx,
            interner,
            repr_plan,
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

        // Canonicalize: resolve through Pool links (Var chains, Applied→Struct
        // resolutions) so that multiple Idx values for the same concrete type
        // share a single LLVM struct type.  Without this, the caller's
        // `Applied(Pair, [Var→Int, Var→Int])` and the mono function's concrete
        // `Struct(Pair, [Int, Int])` would create distinct LLVM named structs
        // despite being the same type.
        let canonical = self.store.pool().resolve_fully(idx);

        // Cache hit (on the canonical Idx)
        if let Some(&cached) = self.cache.borrow().get(&canonical) {
            return cached;
        }

        // Depth guard: catch indirect cycles and prevent stack overflow.
        let current_depth = self.depth.get();
        if current_depth >= Self::MAX_RESOLVE_DEPTH {
            tracing::warn!(idx = ?canonical, depth = current_depth, "type resolution depth limit");
            return self.scx.type_i64().into();
        }
        self.depth.set(current_depth + 1);

        let resolved = ori_stack::ensure_sufficient_stack(|| self.resolve_inner(canonical));

        self.depth.set(current_depth);
        resolved
    }

    // `try_repr_to_llvm_type` and `try_lower_narrowed_aggregate` are defined in
    // `type_info/repr_lowering.rs` (same `impl TypeLayoutResolver` block).
    // Enum resolution methods (resolve_enum, resolve_enum_explicit,
    // resolve_enum_tagless, resolve_enum_niche, is_non_void_field)
    // live in `type_info/enum_layout.rs`.

    /// Inner resolve implementation, separated for depth guard.
    #[expect(
        clippy::too_many_lines,
        reason = "§07.2 niche checks on Option/Result add 30 lines to dispatch"
    )]
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

        // Phase A: consult ReprPlan first for non-recursive types.
        // When the plan has a decision and the type can be converted without
        // recursive resolution, use the ReprPlan path directly.
        if let Some(repr) = self.repr_plan.and_then(|p| p.get_repr(idx)) {
            if let Some(llvm_ty) = self.try_repr_to_llvm_type(repr) {
                return llvm_ty;
            }
            // If this is a narrowed Struct/Tuple (has int fields with width < I64
            // from integer narrowing), resolve directly using the narrowed FieldRepr
            // widths. Non-narrowed structs fall through to TypeInfoStore's named
            // struct path below.
            if let Some(llvm_ty) = self.try_lower_narrowed_aggregate(repr) {
                return llvm_ty;
            }
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
                // §07.2: Check ReprPlan for niche encoding.
                let resolved_idx = self.store.pool().resolve_fully(idx);
                if let Some(enum_repr) = self.repr_plan.and_then(|p| p.get_enum_repr(resolved_idx))
                {
                    if enum_repr.tag.is_niche() {
                        // Niche layout: struct IS the inner type (no tag field).
                        self.resolving.borrow_mut().insert(idx);
                        let payload = self.resolve(*inner);
                        self.resolving.borrow_mut().remove(&idx);
                        let name = self.type_name(idx, "Enum");
                        let named_struct = self.scx.type_named_struct(&name);
                        self.named_structs.borrow_mut().insert(idx, named_struct);
                        self.scx.set_struct_body(named_struct, &[payload], false);
                        return named_struct.into();
                    }
                }
                // Explicit tag: { i64, T }
                self.resolving.borrow_mut().insert(idx);
                let payload = self.resolve(*inner);
                self.resolving.borrow_mut().remove(&idx);
                self.scx
                    .type_struct(&[self.scx.type_i64().into(), payload], false)
                    .into()
            }
            TypeInfo::Result { ok, err } => {
                // §07.2: Check ReprPlan for niche encoding.
                let resolved_idx = self.store.pool().resolve_fully(idx);
                if let Some(enum_repr) = self.repr_plan.and_then(|p| p.get_enum_repr(resolved_idx))
                {
                    if enum_repr.tag.is_niche() {
                        self.resolving.borrow_mut().insert(idx);
                        let ok_ty = self.resolve(*ok);
                        let err_ty = self.resolve(*err);
                        self.resolving.borrow_mut().remove(&idx);
                        // Niche: data variant's payload. Use the larger type.
                        let ok_size = Self::type_store_size(ok_ty);
                        let err_size = Self::type_store_size(err_ty);
                        let payload = if ok_size >= err_size { ok_ty } else { err_ty };
                        let name = self.type_name(idx, "Enum");
                        let named_struct = self.scx.type_named_struct(&name);
                        self.named_structs.borrow_mut().insert(idx, named_struct);
                        self.scx.set_struct_body(named_struct, &[payload], false);
                        return named_struct.into();
                    }
                }
                // Explicit tag: { i64, payload }
                self.resolving.borrow_mut().insert(idx);
                let ok_ty = self.resolve(*ok);
                let err_ty = self.resolve(*err);
                self.resolving.borrow_mut().remove(&idx);
                let ok_size = Self::type_store_size(ok_ty);
                let err_size = Self::type_store_size(err_ty);
                let payload = if ok_size >= err_size { ok_ty } else { err_ty };
                self.scx
                    .type_struct(&[self.scx.type_i64().into(), payload], false)
                    .into()
            }

            // Tuple: struct of recursively-resolved element types.
            // §06: if the tuple is reordered, use memory-order from TupleRepr.
            TypeInfo::Tuple { elements } => {
                self.resolving.borrow_mut().insert(idx);
                let field_types: Vec<BasicTypeEnum<'ll>> =
                    if let Some(ori_repr::MachineRepr::Tuple(t)) =
                        self.repr_plan.and_then(|p| p.get_repr(idx))
                    {
                        if t.is_reordered() {
                            t.elements
                                .iter()
                                .map(|f| self.resolve(elements[f.original_index as usize]))
                                .collect()
                        } else {
                            elements.iter().map(|&e| self.resolve(e)).collect()
                        }
                    } else {
                        elements.iter().map(|&e| self.resolve(e)).collect()
                    };
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
    ///
    /// §06: when the struct is reordered in the `ReprPlan`, creates the LLVM
    /// type with fields in memory order (sorted by alignment) rather than
    /// declaration order. This ensures the LLVM struct layout matches the
    /// `StructRepr` that codegen's field-index remapping expects.
    fn resolve_struct(&self, idx: Idx, fields: &[(Name, Idx)]) -> BasicTypeEnum<'ll> {
        if self.resolving.borrow().contains(&idx) {
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            return self.scx.type_ptr().into();
        }

        let name = self.type_name(idx, "Struct");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // §06: if the struct is reordered, build LLVM type in memory order.
        // Match fields by NAME (not original_index) to handle Pool entries
        // where struct_fields() returns fields in a different order than
        // the canonical entry that was optimized.
        let field_types: Vec<BasicTypeEnum<'ll>> = if let Some(ori_repr::MachineRepr::Struct(s)) =
            self.repr_plan.and_then(|p| p.get_repr(idx))
        {
            if s.is_reordered() {
                s.fields
                    .iter()
                    .map(|f| {
                        // Match by field name for robustness across Pool entries.
                        let ty = fields
                            .iter()
                            .find(|(n, _)| *n == f.name)
                            .map_or(fields[f.original_index as usize].1, |(_, ty)| *ty);
                        self.resolve(ty)
                    })
                    .collect()
            } else {
                fields.iter().map(|&(_, ty)| self.resolve(ty)).collect()
            }
        } else {
            fields.iter().map(|&(_, ty)| self.resolve(ty)).collect()
        };

        self.scx.set_struct_body(named_struct, &field_types, false);
        self.resolving.borrow_mut().remove(&idx);

        named_struct.into()
    }

    // Enum resolution methods (resolve_enum, resolve_enum_explicit,
    // resolve_enum_tagless, resolve_enum_niche, is_non_void_field)
    // live in enum_layout.rs.

    /// Get a human-readable name for an LLVM named struct.
    pub(super) fn type_name(&self, idx: Idx, fallback: &str) -> String {
        let pool = self.store.pool();
        if idx.raw() as usize >= pool.len() {
            return format!("ori.{}.{}", fallback, idx.raw());
        }
        match pool.tag(idx) {
            Tag::Struct => {
                let name = pool.struct_name(idx);
                let label = self.resolve_name(name);
                format!("ori.{label}")
            }
            Tag::Enum => {
                let name = pool.enum_name(idx);
                let label = self.resolve_name(name);
                format!("ori.{label}")
            }
            _ => format!("ori.{}.{}", fallback, idx.raw()),
        }
    }

    /// Resolve a `Name` to its string representation.
    pub(super) fn resolve_name(&self, name: Name) -> String {
        if let Some(interner) = self.interner {
            if let Some(s) = interner.try_lookup(name) {
                return s.to_owned();
            }
        }
        name.raw().to_string()
    }

    /// Approximate store size of an LLVM type in bytes.
    ///
    /// Delegates to [`super::type_size::type_store_size`].
    pub(crate) fn type_store_size(ty: BasicTypeEnum<'ll>) -> u64 {
        super::type_size::type_store_size(ty)
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
