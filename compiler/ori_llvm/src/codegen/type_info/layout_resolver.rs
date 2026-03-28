//! `TypeLayoutResolver` — recursive LLVM type resolution with cycle detection.
//!
//! Resolves `Idx` → `BasicTypeEnum` with two-phase struct creation for
//! recursive types. Extracted from `type_info/mod.rs` for file size hygiene
//! (§04 HYGIENE-04-001).

use std::cell::{Cell, RefCell};

use inkwell::types::{BasicTypeEnum, StructType};
use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Tag};

use super::info::EnumVariantInfo;
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
    store: &'a TypeInfoStore<'tcx>,
    /// LLVM simple context for type construction.
    scx: &'a SimpleCx<'ll>,
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
    repr_plan: Option<&'a ori_repr::ReprPlan>,
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

    /// Try to convert a `MachineRepr` to an LLVM type for non-recursive types.
    ///
    /// Returns `Some` for primitives, fat pointers, opaque pointers, closures,
    /// and ranges — these have fixed LLVM layouts independent of child type
    /// resolution. Returns `None` for recursive types (Struct, Enum, Tuple,
    /// `StackPromoted`) that require `TypeInfoStore`'s two-phase creation pattern.
    fn try_repr_to_llvm_type(&self, repr: &ori_repr::MachineRepr) -> Option<BasicTypeEnum<'ll>> {
        use ori_repr::{FloatWidth, IntWidth, MachineRepr};

        match repr {
            MachineRepr::Int { width, .. } => Some(match width {
                IntWidth::I8 => self.scx.type_i8().into(),
                IntWidth::I16 => self.scx.type_i16().into(),
                IntWidth::I32 => self.scx.type_i32().into(),
                IntWidth::I64 => self.scx.type_i64().into(),
            }),
            MachineRepr::Float { width } => Some(match width {
                FloatWidth::F32 => self.scx.type_f32().into(),
                FloatWidth::F64 => self.scx.type_f64().into(),
            }),
            MachineRepr::Bool => Some(self.scx.type_i1().into()),
            MachineRepr::Char => Some(self.scx.type_i32().into()),
            MachineRepr::Byte | MachineRepr::Ordering => Some(self.scx.type_i8().into()),
            MachineRepr::Duration | MachineRepr::Size | MachineRepr::Unit | MachineRepr::Never => {
                Some(self.scx.type_i64().into())
            }
            MachineRepr::Range => Some(
                self.scx
                    .type_struct(
                        &[
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                        ],
                        false,
                    )
                    .into(),
            ),
            MachineRepr::FatPointer(_) => Some(
                self.scx
                    .type_struct(
                        &[
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_ptr().into(),
                        ],
                        false,
                    )
                    .into(),
            ),
            MachineRepr::OpaquePtr | MachineRepr::UnmanagedPtr | MachineRepr::RcPointer(_) => {
                Some(self.scx.type_ptr().into())
            }
            MachineRepr::Closure(_) => Some(
                self.scx
                    .type_struct(
                        &[self.scx.type_ptr().into(), self.scx.type_ptr().into()],
                        false,
                    )
                    .into(),
            ),
            // Recursive types — require two-phase creation via TypeInfoStore.
            // Narrowed Struct/Tuple types are handled in resolve_inner() by
            // try_lower_narrowed_aggregate(), not here — that ensures only
            // actually-narrowed structs bypass the named struct path.
            MachineRepr::Struct(_)
            | MachineRepr::Enum(_)
            | MachineRepr::Tuple(_)
            | MachineRepr::StackPromoted { .. } => None,
        }
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

        // Phase A: consult ReprPlan first for non-recursive types.
        // When the plan has a decision and the type can be converted without
        // recursive resolution, use the ReprPlan path directly.
        if let Some(repr) = self.repr_plan.and_then(|p| p.get_repr(idx)) {
            if let Some(llvm_ty) = self.try_repr_to_llvm_type(repr) {
                return llvm_ty;
            }
            // §04.4: If this is a narrowed Struct/Tuple (has int fields with
            // width < I64 from integer narrowing), resolve directly using the
            // narrowed FieldRepr widths. Non-narrowed structs fall through to
            // TypeInfoStore's named struct path below.
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
                self.resolving.borrow_mut().insert(idx);
                let payload = self.resolve(*inner);
                self.resolving.borrow_mut().remove(&idx);
                self.scx
                    .type_struct(&[self.scx.type_i64().into(), payload], false)
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
                    .type_struct(&[self.scx.type_i64().into(), payload], false)
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

    /// Try to lower a narrowed Struct/Tuple from the `ReprPlan` to an LLVM type.
    ///
    /// Returns `Some` only if the aggregate has at least one narrowed int field
    /// (`MachineRepr::Int { width != I64 }`) and all field types can be resolved
    /// without recursive type resolution. Non-narrowed aggregates return `None`
    /// to preserve the existing named-struct creation path.
    fn try_lower_narrowed_aggregate(
        &self,
        repr: &ori_repr::MachineRepr,
    ) -> Option<BasicTypeEnum<'ll>> {
        use ori_repr::MachineRepr;

        let fields = match repr {
            MachineRepr::Struct(s) => &s.fields[..],
            MachineRepr::Tuple(t) => &t.elements[..],
            _ => return None,
        };

        // Only enter the narrowed path if at least one field was actually
        // narrowed. The canonical (non-narrowed) path for all int fields is
        // Int { width: I64, signed: true }. Any Int with width < I64 must
        // have come from the §04 narrowing pass — natural narrow types
        // (Byte, Char, Bool) use their own MachineRepr variants.
        let has_narrowed = fields.iter().any(|f| {
            matches!(
                f.repr,
                MachineRepr::Int {
                    width,
                    ..
                } if width != ori_repr::IntWidth::I64
            )
        });
        if !has_narrowed {
            return None;
        }

        // Safety: only create narrowed LLVM types when ALL fields are
        // scalar int types. Mixed structs (e.g., { str, int }) change
        // the overall struct size, which breaks element_store_size() and
        // elem_dec_fn assumptions in collection codegen (list element
        // GEP stride, RC cleanup offsets). Mixed-field narrowing requires
        // Phase C element_store_size integration (§04.4).
        let all_scalar_int = fields.iter().all(|f| {
            matches!(
                f.repr,
                MachineRepr::Int { .. }
                    | MachineRepr::Float { .. }
                    | MachineRepr::Bool
                    | MachineRepr::Char
                    | MachineRepr::Byte
                    | MachineRepr::Unit
            )
        });
        if !all_scalar_int {
            return None;
        }

        // Resolve all field types. If any field can't be resolved (nested
        // Struct/Enum), return None to fall through to TypeInfoStore.
        let field_types: Option<Vec<BasicTypeEnum<'ll>>> = fields
            .iter()
            .map(|f| self.try_repr_to_llvm_type(&f.repr))
            .collect();
        field_types.map(|ft| self.scx.type_struct(&ft, false).into())
    }

    /// Resolve a struct type with two-phase creation for cycle safety.
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

        let field_types: Vec<BasicTypeEnum<'ll>> =
            fields.iter().map(|&(_, ty)| self.resolve(ty)).collect();

        self.scx.set_struct_body(named_struct, &field_types, false);
        self.resolving.borrow_mut().remove(&idx);

        named_struct.into()
    }

    /// Resolve an enum type with two-phase creation for cycle safety.
    ///
    /// Layout: `{ i64 tag, [M x i64] payload }` where M is enough i64s to
    /// hold the largest variant's fields. All-unit enums omit the payload.
    fn resolve_enum(&self, idx: Idx, variants: &[EnumVariantInfo]) -> BasicTypeEnum<'ll> {
        if self.resolving.borrow().contains(&idx) {
            if let Some(&named) = self.named_structs.borrow().get(&idx) {
                return named.into();
            }
            return self.scx.type_ptr().into();
        }

        let name = self.type_name(idx, "Enum");
        let named_struct = self.scx.type_named_struct(&name);
        self.named_structs.borrow_mut().insert(idx, named_struct);
        self.resolving.borrow_mut().insert(idx);

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

        let tag_ty = self.scx.type_i64();
        if max_payload_bytes == 0 {
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into()], false);
        } else {
            let payload_i64_count = max_payload_bytes.div_ceil(8);
            let payload_ty = self.scx.type_i64().array_type(payload_i64_count as u32);
            self.scx
                .set_struct_body(named_struct, &[tag_ty.into(), payload_ty.into()], false);
        }

        self.resolving.borrow_mut().remove(&idx);
        named_struct.into()
    }

    /// Get a human-readable name for an LLVM named struct.
    fn type_name(&self, idx: Idx, fallback: &str) -> String {
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
    fn resolve_name(&self, name: Name) -> String {
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
