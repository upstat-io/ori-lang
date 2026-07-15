//! Pure-const burden specifications for builtin types.
//!
//! Builtin types use `BuiltinBurdenSpec` (pure-const, `&'static` slices) while
//! user-defined types use `UserBurdenSpec` (owned-vector storage in
//! `ori_types`).
//! Both implement the `Burden` trait (defined in `ori_arc::lower::burden`).
//!
//! # Purity Contract
//!
//! Zero host allocation. All fields are `&'static` slices or scalar/copy types.
//! Enforced by `tests::purity_no_heap_allocation_types`.
//!
//! # Cross-Crate ID Note
//!
//! `TypeId`, `VariantId`, and `FnSym` are defined LOCALLY here as `u32`
//! newtypes (NOT imported from `ori_ir`) to honor the zero-dependency
//! invariant of this crate. Boundary translation between this local `TypeId`
//! and `ori_ir::TypeId` happens at the `ori_arc::lower` burden-lookup
//! boundary.
//
// Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding the
// lattice-driven analysis.

pub mod ffi;
pub mod table;

pub use ffi::EMPTY_BURDEN_SPEC;

// Local Copy/Eq/Hash newtypes — defined here to honor the zero-dependency
// purity contract of this crate.
//
// `TypeId` and `FnSym` wrap `NonZeroU32` so `Option<TypeId>` /
// `Option<FnSym>` enjoy niche optimization (4 bytes vs 8). This is
// load-bearing for the `BuiltinBurdenSpec` ≤64-byte size invariant.
// ID 0 is reserved; consumers shift indices by +1 at the boundary
// translation in `ori_arc::lower::burden_lookup`.

use core::num::NonZeroU32;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct TypeId(NonZeroU32);

impl TypeId {
    #[must_use]
    pub const fn new(n: NonZeroU32) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct VariantId(NonZeroU32);

impl VariantId {
    #[must_use]
    pub const fn new(n: NonZeroU32) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct FnSym(NonZeroU32);

impl FnSym {
    #[must_use]
    pub const fn new(n: NonZeroU32) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn get(self) -> NonZeroU32 {
        self.0
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum TransferKind {
    Move,
    BorrowOnly,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct OwnedField {
    pub field_path: &'static [u32],
    pub field_type: TypeId,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BorrowedField {
    pub field_path: &'static [u32],
    pub field_type: TypeId,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TransferRule {
    pub source_field_path: &'static [u32],
    pub binding_index: u16,
    pub field_type: TypeId,
    pub transfer_kind: TransferKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct VariantBurden {
    pub variant_id: VariantId,
    pub transfers_on_match: &'static [TransferRule],
    pub retained_owned: &'static [OwnedField],
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BuiltinBurdenSpec {
    /// The value introduces a distinct logical ownership identity of its own,
    /// separate from any identities reachable through fields or elements.
    /// This does not select heap, stack, arena, header, or counter storage.
    pub self_owned_identity: bool,
    pub owned_fields: &'static [OwnedField],
    pub borrowed_fields: &'static [BorrowedField],
    pub variant_burdens: &'static [VariantBurden],
    pub element_burden: Option<TypeId>,
    /// Stable logical cleanup-operation identity for recursive or user-drop
    /// composition. Physical projections choose any helper symbol or inline
    /// implementation after consuming this identity.
    pub drop_operation: Option<FnSym>,
    pub user_drop: Option<FnSym>,
}

impl BuiltinBurdenSpec {
    pub const EMPTY: BuiltinBurdenSpec = BuiltinBurdenSpec {
        self_owned_identity: false,
        owned_fields: &[],
        borrowed_fields: &[],
        variant_burdens: &[],
        element_burden: None,
        drop_operation: None,
        user_drop: None,
    };
}

// Compile-time size assertion — `BuiltinBurdenSpec` must fit in a single
// cache line so it can ride alongside `BUILTIN_TYPES` entries cheaply; the
// niche-optimized `Option<TypeId>` layout in `NonZeroU32` is load-bearing
// for this bound.
const _: () = {
    assert!(core::mem::size_of::<BuiltinBurdenSpec>() <= 64);
};

#[cfg(test)]
mod tests;
