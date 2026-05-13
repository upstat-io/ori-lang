//! Pure-const burden specifications for builtin types.
//!
//! Per `plans/aims-burden-tracking/decisions/05-burdenspec-storage-model.md`:
//! builtin types use `BuiltinBurdenSpec` (pure-const, `&'static` slices) while
//! user-defined types use `UserBurdenSpec` (heap-backed, in `ori_types`).
//! Both implement the `Burden` trait (defined in `ori_arc::lower::burden`).
//!
//! # Purity Contract
//!
//! Zero heap allocation. All fields are `&'static` slices or scalar/copy types.
//! Enforced by `tests::purity_no_heap_allocation_types`.
//!
//! # Cross-Crate ID Note
//!
//! `TypeId`, `VariantId`, and `FnSym` are defined LOCALLY here as `u32`
//! newtypes (NOT imported from `ori_ir`) to honor the zero-dependency
//! invariant of this crate. Boundary translation between this local `TypeId`
//! and `ori_ir::TypeId` happens at the `BurdenRef` lookup boundary
//! (`ori_arc::lower::burden_lookup` per §01.4).

// Local Copy/Eq/Hash newtypes — defined here to honor the zero-dependency
// purity contract of this crate.
//
// `TypeId` and `FnSym` wrap `NonZeroU32` so `Option<TypeId>` /
// `Option<FnSym>` enjoy niche optimization (4 bytes vs 8). This is
// load-bearing for the `BuiltinBurdenSpec` ≤64-byte size invariant per
// decision/05. ID 0 is reserved; consumers shift indices by +1 at the
// boundary translation in `ori_arc::lower::burden_lookup`.

use core::num::NonZeroU32;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct TypeId(pub NonZeroU32);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct VariantId(pub NonZeroU32);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct FnSym(pub NonZeroU32);

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

#[derive(Copy, Clone, Debug)]
pub struct BuiltinBurdenSpec {
    pub self_heap_alloc: bool,
    pub owned_fields: &'static [OwnedField],
    pub borrowed_fields: &'static [BorrowedField],
    pub variant_burdens: &'static [VariantBurden],
    pub element_burden: Option<TypeId>,
    pub compiled_drop: Option<FnSym>,
    pub user_drop: Option<FnSym>,
}

impl BuiltinBurdenSpec {
    pub const EMPTY: BuiltinBurdenSpec = BuiltinBurdenSpec {
        self_heap_alloc: false,
        owned_fields: &[],
        borrowed_fields: &[],
        variant_burdens: &[],
        element_burden: None,
        compiled_drop: None,
        user_drop: None,
    };
}

// Compile-time size assertion per decision/05.
const _: () = {
    assert!(core::mem::size_of::<BuiltinBurdenSpec>() <= 64);
};

#[cfg(test)]
mod tests;
