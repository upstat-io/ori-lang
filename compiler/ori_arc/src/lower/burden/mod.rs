//! Burden trait + `BurdenRef` bridge unifying builtin + user burden specs.
//!
//! The `Burden` trait is the consumer-facing surface Phase 5 emission queries
//! regardless of whether the underlying burden comes from a pure-const
//! `BuiltinBurdenSpec` or a heap-backed `UserBurdenSpec`.
//!
//! Trait methods (`owned_fields` / `borrowed_fields` / `variant_burdens`)
//! return `Box<dyn Iterator>` over view types so each variant constructs the
//! top-level traversal on-demand from its native storage (static slice vs
//! `Vec`) without allocating a `Vec<View>` for the outer collection. Nested
//! per-variant structure (`VariantBurdenView`) does materialize inner
//! `Vec<TransferRuleView>` and `Vec<OwnedFieldView>` because the variant
//! shape requires the nested rows be addressable. View types use
//! `Cow<'a, [u32]>` to unify `&'static [u32]` (builtin) and `&'a [u32]`
//! (user) field paths.

use std::borrow::Cow;

use ori_registry::burden::{
    BorrowedField, BuiltinBurdenSpec, FnSym, OwnedField, TransferKind, TransferRule, TypeId,
    VariantBurden, VariantId,
};
use ori_types::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden,
};
use ori_types::Idx;

/// Unified type reference — builtin `TypeId` or user `Idx`.
///
/// Phase 5 emission consumes via `TypeRef::lookup(type_registry)` returning
/// `Option<BurdenRef<'a>>` regardless of partition side.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeRef {
    /// Builtin type — pre-interned `TypeId`; lookup hits `BURDEN_TABLE`.
    Builtin(TypeId),
    /// User-defined type — Pool `Idx`; lookup hits `TypeRegistry` burden side-table.
    User(Idx),
}

/// Read-only view of an owned field — uniform interface over `&'static [u32]`
/// (builtin path) and `&[u32]` (user path) via `Cow`.
#[derive(Clone, Debug)]
pub struct OwnedFieldView<'a> {
    pub field_path: Cow<'a, [u32]>,
    pub field_type: TypeRef,
}

/// Read-only view of a borrowed field. Same `Cow` + `TypeRef` shape as `OwnedFieldView`.
#[derive(Clone, Debug)]
pub struct BorrowedFieldView<'a> {
    pub field_path: Cow<'a, [u32]>,
    pub field_type: TypeRef,
}

/// Read-only view of a transfer rule.
#[derive(Clone, Debug)]
pub struct TransferRuleView<'a> {
    pub source_field_path: Cow<'a, [u32]>,
    pub binding_index: u16,
    pub field_type: TypeRef,
    pub transfer_kind: TransferKind,
}

/// Read-only view of a variant burden.
#[derive(Clone, Debug)]
pub struct VariantBurdenView<'a> {
    pub variant_id: VariantId,
    pub transfers_on_match: Vec<TransferRuleView<'a>>,
    pub retained_owned: Vec<OwnedFieldView<'a>>,
}

/// Bridge trait — Phase 5 emission's uniform interface over builtin + user
/// burden storage. Methods return boxed iterators so each variant constructs
/// views on-demand from its native storage shape.
pub trait Burden {
    fn self_heap_alloc(&self) -> bool;
    fn owned_fields<'a>(&'a self) -> Box<dyn Iterator<Item = OwnedFieldView<'a>> + 'a>;
    fn borrowed_fields<'a>(&'a self) -> Box<dyn Iterator<Item = BorrowedFieldView<'a>> + 'a>;
    fn variant_burdens<'a>(&'a self) -> Box<dyn Iterator<Item = VariantBurdenView<'a>> + 'a>;
    fn element_burden(&self) -> Option<TypeRef>;
    fn compiled_drop(&self) -> Option<FnSym>;
    fn user_drop(&self) -> Option<FnSym>;
}

/// Burden reference — either pure-const builtin (lifetime-free) or heap-backed
/// user spec tied to `TypeRegistry`'s lifetime.
#[derive(Clone, Copy, Debug)]
pub enum BurdenRef<'a> {
    /// Builtin path — `&'static BuiltinBurdenSpec`; lifetime widens to any `'a`.
    Builtin(&'static BuiltinBurdenSpec),
    /// User path — `&'a UserBurdenSpec` tied to `TypeRegistry`'s lifetime.
    User(&'a UserBurdenSpec),
}

// View construction helpers — builtin path.

fn view_from_builtin_owned<'a>(field: &OwnedField) -> OwnedFieldView<'a> {
    OwnedFieldView {
        field_path: Cow::Borrowed(field.field_path),
        field_type: TypeRef::Builtin(field.field_type),
    }
}

fn view_from_builtin_borrowed<'a>(field: &BorrowedField) -> BorrowedFieldView<'a> {
    BorrowedFieldView {
        field_path: Cow::Borrowed(field.field_path),
        field_type: TypeRef::Builtin(field.field_type),
    }
}

fn view_from_builtin_transfer<'a>(rule: &TransferRule) -> TransferRuleView<'a> {
    TransferRuleView {
        source_field_path: Cow::Borrowed(rule.source_field_path),
        binding_index: rule.binding_index,
        field_type: TypeRef::Builtin(rule.field_type),
        transfer_kind: rule.transfer_kind,
    }
}

fn view_from_builtin_variant<'a>(v: &VariantBurden) -> VariantBurdenView<'a> {
    VariantBurdenView {
        variant_id: v.variant_id,
        transfers_on_match: v
            .transfers_on_match
            .iter()
            .map(view_from_builtin_transfer)
            .collect(),
        retained_owned: v
            .retained_owned
            .iter()
            .map(view_from_builtin_owned)
            .collect(),
    }
}

fn view_from_user_owned(field: &UserOwnedField) -> OwnedFieldView<'_> {
    OwnedFieldView {
        field_path: Cow::Borrowed(&field.field_path),
        field_type: TypeRef::User(field.field_type),
    }
}

fn view_from_user_borrowed(field: &UserBorrowedField) -> BorrowedFieldView<'_> {
    BorrowedFieldView {
        field_path: Cow::Borrowed(&field.field_path),
        field_type: TypeRef::User(field.field_type),
    }
}

fn view_from_user_transfer(rule: &UserTransferRule) -> TransferRuleView<'_> {
    TransferRuleView {
        source_field_path: Cow::Borrowed(&rule.source_field_path),
        binding_index: rule.binding_index,
        field_type: TypeRef::User(rule.field_type),
        transfer_kind: rule.transfer_kind,
    }
}

fn view_from_user_variant(v: &UserVariantBurden) -> VariantBurdenView<'_> {
    VariantBurdenView {
        variant_id: v.variant_id,
        transfers_on_match: v
            .transfers_on_match
            .iter()
            .map(view_from_user_transfer)
            .collect(),
        retained_owned: v.retained_owned.iter().map(view_from_user_owned).collect(),
    }
}

impl Burden for BurdenRef<'_> {
    fn self_heap_alloc(&self) -> bool {
        match self {
            BurdenRef::Builtin(b) => b.self_heap_alloc,
            BurdenRef::User(u) => u.self_heap_alloc,
        }
    }

    fn owned_fields<'a>(&'a self) -> Box<dyn Iterator<Item = OwnedFieldView<'a>> + 'a> {
        match self {
            BurdenRef::Builtin(b) => Box::new(b.owned_fields.iter().map(view_from_builtin_owned)),
            BurdenRef::User(u) => Box::new(u.owned_fields.iter().map(view_from_user_owned)),
        }
    }

    fn borrowed_fields<'a>(&'a self) -> Box<dyn Iterator<Item = BorrowedFieldView<'a>> + 'a> {
        match self {
            BurdenRef::Builtin(b) => {
                Box::new(b.borrowed_fields.iter().map(view_from_builtin_borrowed))
            }
            BurdenRef::User(u) => Box::new(u.borrowed_fields.iter().map(view_from_user_borrowed)),
        }
    }

    fn variant_burdens<'a>(&'a self) -> Box<dyn Iterator<Item = VariantBurdenView<'a>> + 'a> {
        match self {
            BurdenRef::Builtin(b) => {
                Box::new(b.variant_burdens.iter().map(view_from_builtin_variant))
            }
            BurdenRef::User(u) => Box::new(u.variant_burdens.iter().map(view_from_user_variant)),
        }
    }

    fn element_burden(&self) -> Option<TypeRef> {
        match self {
            BurdenRef::Builtin(b) => b.element_burden.map(TypeRef::Builtin),
            BurdenRef::User(u) => u.element_burden.map(TypeRef::User),
        }
    }

    fn compiled_drop(&self) -> Option<FnSym> {
        match self {
            BurdenRef::Builtin(b) => b.compiled_drop,
            BurdenRef::User(u) => u.compiled_drop,
        }
    }

    fn user_drop(&self) -> Option<FnSym> {
        match self {
            BurdenRef::Builtin(b) => b.user_drop,
            BurdenRef::User(u) => u.user_drop,
        }
    }
}

#[cfg(test)]
mod tests;
