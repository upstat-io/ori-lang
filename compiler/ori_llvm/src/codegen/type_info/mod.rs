//! `TypeInfo` enum, `TypeInfoStore`, and `TypeLayoutResolver` for V2 codegen.
//!
//! Every Ori type category gets a [`TypeInfo`] variant that encapsulates its
//! LLVM representation, memory layout, and calling convention. Adding a new
//! type means adding one enum variant — not modifying match arms across the
//! codebase.
//!
//! Design from Swift's `TypeInfo` hierarchy, adapted as a Rust enum per Ori
//! coding guidelines ("enum for fixed sets — exhaustiveness, static dispatch").
//!
//! # Module Layout
//!
//! - **[`info`]** — `TypeInfo` enum + methods (`storage_type`, `size`, `alignment`, triviality)
//! - **[`store`]** — `TypeInfoStore` cached `Idx` → `TypeInfo` mapping
//! - **`TypeLayoutResolver`** (this file) — recursive LLVM type resolution with cycle detection
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

mod info;
mod store;

pub use info::{EnumVariantInfo, TypeInfo};
pub use store::TypeInfoStore;

use std::cell::{Cell, RefCell};

use inkwell::types::{BasicTypeEnum, StructType};
use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::{Name, StringInterner};
#[cfg(test)]
use ori_types::Pool;
use ori_types::{Idx, Tag};

use crate::context::SimpleCx;

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
    /// String interner for resolving `Name` → human-readable strings.
    ///
    /// When present, struct/enum types get meaningful LLVM names like `%ori.Point`.
    /// When absent (e.g., in unit tests), falls back to numeric IDs like `%ori.3`.
    interner: Option<&'a StringInterner>,
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
    pub fn new(
        store: &'a TypeInfoStore<'tcx>,
        scx: &'a SimpleCx<'ll>,
        interner: Option<&'a StringInterner>,
    ) -> Self {
        Self {
            store,
            scx,
            interner,
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

        let resolved = self.resolve_inner(canonical);

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
        let tag_ty = self.scx.type_i64();
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
    /// When the interner is available, resolves `Name` → string for readable
    /// type names like `%ori.Point`. Falls back to numeric IDs otherwise.
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
    ///
    /// Uses the interner when available; falls back to the raw numeric ID
    /// (for test `Name::from_raw()` values or missing interner).
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
    /// Uses LLVM's type system to determine sizes. For types where we
    /// can't easily determine the size, falls back to 8 bytes (i64-sized).
    ///
    /// **Sync point**: `ForYieldLowerer::type_store_size()` in `ori_arc` mirrors
    /// this logic at the Pool level. Both must agree on sizes for all types,
    /// otherwise for-yield element buffers will be mis-sized.
    /// See `compiler/ori_arc/src/lower/control_flow/for_yield.rs`.
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
