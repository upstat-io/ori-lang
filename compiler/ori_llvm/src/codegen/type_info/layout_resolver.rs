//! `TypeLayoutResolver` — recursive LLVM type resolution with cycle detection.
//!
//! Resolves `Idx` → `BasicTypeEnum` with two-phase struct creation for
//! recursive types.

use std::cell::{Cell, RefCell};

use inkwell::types::{BasicTypeEnum, StructType};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::context::SimpleCx;

use super::store::TypeInfoStore;
use super::type_size::{select_shared_payload_arm, SharedPayloadArm};
use super::TypeInfo;

/// Resolves `Idx` → `BasicTypeEnum` with cycle-safe two-phase struct creation.
///
/// For recursive types like `type Tree = Leaf(int) | Node(Tree, Tree)`, LLVM
/// requires a two-phase approach:
/// 1. Create an opaque named struct (`%Tree = type opaque`)
/// 2. Recursively resolve field types (which may reference `%Tree`)
/// 3. Fill the struct body (`%Tree = type { i8, [2 x i64] }`)
///
#[derive(Debug)]
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
    /// Representation plan from `ori_repr`.
    ///
    /// When present, type lookups consult the `ReprPlan` first for non-recursive
    /// types (primitives, fat pointers, opaque pointers). When absent (or when
    /// the plan has no entry for a type, or the type requires recursive
    /// resolution), falls back to `TypeInfoStore`.
    pub(super) repr_plan: Option<&'a ori_repr::ReprPlan>,
    /// Active type-resolution stack for cycle detection.
    ///
    /// A repeated `Idx` denotes a cycle and resolves to its opaque struct
    /// rather than recurring.
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
    /// Maximum recursion depth for type resolution.
    ///
    /// Catches indirect cycles (different Idx values for the same conceptual
    /// type) and prevents stack overflow from deeply nested types.
    const MAX_RESOLVE_DEPTH: u32 = 32;

    /// Create a new resolver.
    ///
    /// Pass an `interner` to get human-readable LLVM type names (e.g., `%ori.Point`).
    /// Without it, types get numeric names (e.g., `%ori.3`).
    #[must_use]
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

    /// Access the representation plan, when one was supplied.
    #[must_use = "the absence of a value must be handled"]
    pub fn repr_plan(&self) -> Option<&'a ori_repr::ReprPlan> {
        self.repr_plan
    }

    /// Access the underlying `TypeInfoStore`.
    pub fn store(&self) -> &'a TypeInfoStore<'tcx> {
        self.store
    }

    /// Look up a resolved named struct for a given `Idx`.
    #[must_use = "the absence of a value must be handled"]
    pub fn get_named_struct(&self, idx: Idx) -> Option<StructType<'ll>> {
        self.named_structs.borrow().get(&idx).copied()
    }

    /// Whether the position inside `owner_idx` is an RC-boxed recursive edge.
    pub(super) fn position_is_rc_boxed(&self, owner_idx: Idx, field_ty: Idx) -> bool {
        super::repr_box_oracle::position_is_rc_boxed(self.store.pool(), owner_idx, field_ty)
    }

    /// Whether the `Option` payload is an RC-boxed recursive edge.
    pub(super) fn option_payload_is_rc_boxed(&self, idx: Idx) -> bool {
        super::repr_box_oracle::option_payload_is_rc_boxed(self.store.pool(), idx)
    }

    /// Whether the `Result` success payload is an RC-boxed recursive edge.
    pub(super) fn result_ok_is_rc_boxed(&self, idx: Idx) -> bool {
        super::repr_box_oracle::result_ok_is_rc_boxed(self.store.pool(), idx)
    }

    /// Whether the `Result` error payload is an RC-boxed recursive edge.
    pub(super) fn result_err_is_rc_boxed(&self, idx: Idx) -> bool {
        super::repr_box_oracle::result_err_is_rc_boxed(self.store.pool(), idx)
    }

    /// Resolve an `Idx` to its LLVM type, handling recursive types correctly.
    ///
    /// For non-recursive types this delegates to `TypeInfo::storage_type`.
    /// For structs and enums it uses two-phase creation with cycle detection.
    pub fn resolve(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        // Sentinel
        if idx == Idx::NONE {
            return self.scx.type_i64().into();
        }

        // Canonicalize: resolve through Pool links (Var chains, Applied→Struct
        // resolutions) so that multiple Idx values for the same concrete type
        // share a single LLVM struct type. Without this, the caller's
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

    /// Approximate store size of an LLVM type in bytes.
    ///
    /// Delegates to [`super::type_size::type_store_size`].
    pub(crate) fn type_store_size(ty: BasicTypeEnum<'ll>) -> u64 {
        super::type_size::type_store_size(ty)
    }

    /// Inner resolve implementation, separated for depth guard.
    fn resolve_inner(&self, idx: Idx) -> BasicTypeEnum<'ll> {
        if let Some(cycle_break) = self.resolve_cycle_back_edge(idx) {
            return cycle_break;
        }

        if let Some(repr_type) = self.resolve_from_repr_plan(idx) {
            return repr_type;
        }

        let info = self.store.get(idx);
        let result = match &info {
            // Primitives, collections, handles: no recursion possible.
            // Delegate to the standalone storage_type method.
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
            TypeInfo::Option { inner } => self.resolve_option(idx, *inner),
            TypeInfo::Result { ok, err } => self.resolve_result(idx, *ok, *err),

            // Tuple: struct of recursively-resolved element types.
            // If the tuple is reordered, use memory-order from TupleRepr.
            // An element the ReprPlan marks RcPointer is a boxed `ptr`
            // (recursive back-edge), decided order-independently from the
            // marker rather than the resolving-stack.
            TypeInfo::Tuple { elements } => self.resolve_tuple(idx, elements),

            // User-defined struct: two-phase creation.
            TypeInfo::Struct { fields } => self.resolve_struct(idx, fields),

            // User-defined enum: two-phase creation.
            TypeInfo::Enum { variants } => self.resolve_enum(idx, variants),
        };

        self.cache.borrow_mut().insert(idx, result);
        result
    }

    fn resolve_cycle_back_edge(&self, idx: Idx) -> Option<BasicTypeEnum<'ll>> {
        if !self.resolving.borrow().contains(&idx) {
            return None;
        }

        // Struct/Enum cycles are heap-boxed; other recursive aggregates use
        // the i64 sentinel to terminate resolution.
        Some(if self.named_structs.borrow().contains_key(&idx) {
            self.scx.type_ptr().into()
        } else {
            self.scx.type_i64().into()
        })
    }

    fn resolve_from_repr_plan(&self, idx: Idx) -> Option<BasicTypeEnum<'ll>> {
        let repr = self.repr_plan.and_then(|plan| plan.get_repr(idx))?;
        self.try_repr_to_llvm_type(repr)
            .or_else(|| self.try_lower_narrowed_aggregate(repr))
    }

    fn resolve_option(&self, idx: Idx, inner: Idx) -> BasicTypeEnum<'ll> {
        let resolved_idx = self.store.pool().resolve_fully(idx);
        let uses_niche = self
            .repr_plan
            .and_then(|plan| plan.enum_repr_with_fallback(self.store.pool(), idx))
            .is_some_and(|repr| repr.tag.is_niche());

        self.resolving.borrow_mut().insert(idx);
        let payload = if uses_niche {
            self.resolve(inner)
        } else if self.option_payload_is_rc_boxed(resolved_idx) {
            self.scx.type_ptr().into()
        } else {
            self.resolve(inner)
        };
        self.resolving.borrow_mut().remove(&idx);

        if uses_niche {
            payload
        } else {
            self.scx
                .type_struct(&[self.scx.type_i64().into(), payload], false)
                .into()
        }
    }

    fn resolve_result(&self, idx: Idx, ok: Idx, err: Idx) -> BasicTypeEnum<'ll> {
        let resolved_idx = self.store.pool().resolve_fully(idx);
        let uses_niche = self
            .repr_plan
            .and_then(|plan| plan.enum_repr_with_fallback(self.store.pool(), idx))
            .is_some_and(|repr| repr.tag.is_niche());

        self.resolving.borrow_mut().insert(idx);
        let ok_type = if !uses_niche && self.result_ok_is_rc_boxed(resolved_idx) {
            self.scx.type_ptr().into()
        } else {
            self.resolve(ok)
        };
        let err_type = if !uses_niche && self.result_err_is_rc_boxed(resolved_idx) {
            self.scx.type_ptr().into()
        } else {
            self.resolve(err)
        };
        self.resolving.borrow_mut().remove(&idx);

        let payload = match select_shared_payload_arm(ok_type, err_type) {
            SharedPayloadArm::First => ok_type,
            SharedPayloadArm::Second => err_type,
        };
        if uses_niche {
            payload
        } else {
            self.scx
                .type_struct(&[self.scx.type_i64().into(), payload], false)
                .into()
        }
    }

    fn resolve_tuple(&self, idx: Idx, elements: &[Idx]) -> BasicTypeEnum<'ll> {
        self.resolving.borrow_mut().insert(idx);
        let field_types: Vec<BasicTypeEnum<'ll>> =
            if let Some(ori_repr::MachineRepr::Tuple(tuple_repr)) =
                self.repr_plan.and_then(|plan| plan.get_repr(idx))
            {
                let ordered_elements: Vec<Idx> = if tuple_repr.is_reordered() {
                    tuple_repr
                        .elements
                        .iter()
                        .map(|field| elements[field.original_index as usize])
                        .collect()
                } else {
                    elements.to_vec()
                };
                ordered_elements
                    .into_iter()
                    .map(|element| self.resolve_tuple_element(idx, element))
                    .collect()
            } else {
                elements
                    .iter()
                    .map(|&element| self.resolve_tuple_element(idx, element))
                    .collect()
            };
        self.resolving.borrow_mut().remove(&idx);
        self.scx.type_struct(&field_types, false).into()
    }

    /// Resolve a struct type with two-phase creation for cycle safety.
    ///
    /// When the struct is reordered in the `ReprPlan`, creates the LLVM
    /// type with fields in memory order (sorted by alignment) rather than
    /// declaration order. This ensures the LLVM struct layout matches the
    /// `StructRepr` that codegen's field-index remapping expects.
    fn resolve_struct(&self, idx: Idx, fields: &[(Name, Idx)]) -> BasicTypeEnum<'ll> {
        if self.resolving.borrow().contains(&idx) {
            // Recursive back-edge: always a heap-boxed pointer, never the
            // named struct by-value (which would be infinitely sized).
            return self.scx.type_ptr().into();
        }

        let name = self.type_name(idx, "Struct");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

        // If the struct is reordered, build LLVM type in memory order.
        // Match fields by NAME (not original_index) to handle Pool entries
        // where struct_fields returns fields in a different order than
        // the canonical entry that was optimized. A field the ReprPlan marks
        // RcPointer is a boxed `ptr` (recursive back-edge), decided
        // order-independently from the marker rather than the resolving-stack.
        let field_types: Vec<BasicTypeEnum<'ll>> = if let Some(ori_repr::MachineRepr::Struct(s)) =
            self.repr_plan.and_then(|p| p.get_repr(idx))
        {
            if s.is_reordered() {
                s.fields
                    .iter()
                    .map(|f| {
                        // Why: Pool entries may store the same fields in different orders.
                        let ty = fields
                            .iter()
                            .find(|(n, _)| *n == f.name)
                            .map_or(fields[f.original_index as usize].1, |(_, ty)| *ty);
                        self.resolve_struct_field(idx, ty)
                    })
                    .collect()
            } else {
                fields
                    .iter()
                    .map(|&(_, ty)| self.resolve_struct_field(idx, ty))
                    .collect()
            }
        } else {
            fields
                .iter()
                .map(|&(_, ty)| self.resolve_struct_field(idx, ty))
                .collect()
        };

        self.scx.set_struct_body(named_struct, &field_types, false);
        self.resolving.borrow_mut().remove(&idx);

        named_struct.into()
    }

    /// Resolve a struct field, boxing it to `ptr` when the field is a recursive
    /// back-edge per the boxing SSOT; else resolve the field type normally.
    fn resolve_struct_field(&self, owner_idx: Idx, field_ty: Idx) -> BasicTypeEnum<'ll> {
        if self.position_is_rc_boxed(owner_idx, field_ty) {
            return self.scx.type_ptr().into();
        }
        self.resolve(field_ty)
    }

    /// Resolve a tuple element, boxing it to `ptr` when the element is a
    /// recursive back-edge per the boxing SSOT.
    fn resolve_tuple_element(&self, owner_idx: Idx, element_ty: Idx) -> BasicTypeEnum<'ll> {
        if self.position_is_rc_boxed(owner_idx, element_ty) {
            return self.scx.type_ptr().into();
        }
        self.resolve(element_ty)
    }

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
}
