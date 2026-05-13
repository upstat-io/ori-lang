//! Heap-backed burden specifications for user-defined types.
//!
//! Per `plans/aims-burden-tracking/decisions/05-burdenspec-storage-model.md`:
//! mirrors `ori_registry::burden::BuiltinBurdenSpec` but uses `Vec<...>` and
//! `Idx` (instead of `&'static [...]` and the registry-local `TypeId`).
//!
//! `TransferKind`, `FnSym`, and `VariantId` are re-exported from
//! `ori_registry::burden` — shared between builtin and user paths.

// Burden data structures consumed by §01.3 (TypeEntry.burden field) and §01.4
// (lookup_burden helper). Types defined here so §01.1 can land independently of
// the consumer-side wiring per plan ordering. §01.3 will remove the allow.
#![allow(
    dead_code,
    reason = "consumed by §01.3 TypeEntry.burden field; staging types ahead of consumer per plan ordering"
)]

use ori_registry::burden::{FnSym, TransferKind, VariantId};

use crate::Idx;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OwnedFieldOwned {
    pub field_path: Vec<u32>,
    pub field_type: Idx,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BorrowedFieldOwned {
    pub field_path: Vec<u32>,
    pub field_type: Idx,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransferRuleOwned {
    pub source_field_path: Vec<u32>,
    pub binding_index: u16,
    pub field_type: Idx,
    pub transfer_kind: TransferKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct VariantBurdenOwned {
    pub variant_id: VariantId,
    pub transfers_on_match: Vec<TransferRuleOwned>,
    pub retained_owned: Vec<OwnedFieldOwned>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct UserBurdenSpec {
    pub self_heap_alloc: bool,
    pub owned_fields: Vec<OwnedFieldOwned>,
    pub borrowed_fields: Vec<BorrowedFieldOwned>,
    pub variant_burdens: Vec<VariantBurdenOwned>,
    pub element_burden: Option<Idx>,
    pub compiled_drop: Option<FnSym>,
    pub user_drop: Option<FnSym>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU32;

    #[test]
    fn test_user_burden_spec_constructible_default() {
        let spec = UserBurdenSpec::default();
        assert!(!spec.self_heap_alloc);
        assert!(spec.owned_fields.is_empty());
        assert!(spec.borrowed_fields.is_empty());
        assert!(spec.variant_burdens.is_empty());
        assert!(spec.element_burden.is_none());
        assert!(spec.compiled_drop.is_none());
        assert!(spec.user_drop.is_none());
    }

    #[test]
    fn test_user_burden_spec_with_data() {
        let one = NonZeroU32::MIN;
        let spec = UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields: vec![OwnedFieldOwned {
                field_path: vec![0],
                field_type: Idx::STR,
            }],
            element_burden: Some(Idx::INT),
            compiled_drop: Some(FnSym(one)),
            ..UserBurdenSpec::default()
        };
        assert!(spec.self_heap_alloc);
        assert_eq!(spec.owned_fields.len(), 1);
        assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
        assert_eq!(spec.element_burden, Some(Idx::INT));
    }

    #[test]
    fn test_transfer_kind_reexport() {
        // Confirm the re-export path works.
        let move_kind = TransferKind::Move;
        let borrow_kind = TransferKind::BorrowOnly;
        assert_ne!(move_kind, borrow_kind);
    }
}
