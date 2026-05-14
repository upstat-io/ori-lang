//! User-type burden computation (typeck phase 0b).
//!
//! Computes `UserBurdenSpec` for struct / enum / newtype declarations at
//! registration time via the Tag-first ternary partition per
//! `plans/aims-burden-tracking/decisions/06-burden-field-placement.md §Phase
//! Boundary Cure`. Consumed by `register_type_decl` in `user_types.rs`.
//!
//! Partition rules:
//! - Scalar (`Int/Float/Bool/Char/Byte/Unit/Duration/Size/Ordering/Never`):
//!   OMIT from both `owned_fields` and `borrowed_fields`.
//! - `Tag::Borrowed` (target-only per `types.md §TL-9`): `borrowed_fields`.
//! - Heap-owning tags (`Str/List/Map/Set/Channel/Iterator/Function`) +
//!   `Triviality::NonTrivial` compounds: `owned_fields`.
//! - `Triviality::Unknown`: conservative `owned_fields` (PC-2 violation per
//!   `typeck.md §PC-2` if reached at typeck exit).
//!
//! Phase-purity: this module reads `ori_types::Tag` + `ori_types::triviality`
//! only — `ori_arc::ArcClass` is downstream (Phase 4) and SHALL NOT leak here
//! per `compiler.md §Phase-Specific Purity`.

use core::num::NonZeroU32;

use ori_registry::burden::{TransferKind, VariantId};

use crate::registry::burden::{
    UserBorrowedField, UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden,
};
use crate::triviality::{classify_triviality, Triviality};
use crate::{FieldDef, Idx, Pool, Tag, TypeKind, TypeRegistry, VariantDef, VariantFields};

/// Per-field burden classification under the Tag-first ternary partition.
///
/// The `Borrowed` arm of the ternary partition (per decisions/06 §Phase
/// Boundary Cure) is reserved for `Tag::Borrowed` which is target-only per
/// `types.md §TL-9` — added back when borrow-tag construction ships.
#[derive(Clone, Copy, Eq, PartialEq)]
enum FieldClass {
    /// Scalar — omit from both partitions.
    Scalar,
    /// Owned-heap — track in `owned_fields` / `transfers_on_match`.
    Owned,
}

/// Saturating `usize -> u32` for field-path indices.
///
/// Field/variant counts are bounded by source-code declarations; in practice
/// they never approach `u32::MAX`. Saturation is a conservative correctness
/// floor for pathological inputs.
fn idx_u32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

/// Saturating `usize -> u16` for variant `binding_index`.
fn idx_u16(i: usize) -> u16 {
    u16::try_from(i).unwrap_or(u16::MAX)
}

/// Classify a single field's tag for the ternary partition.
fn classify_field_for_burden(field_ty: Idx, pool: &Pool) -> FieldClass {
    let resolved = pool.resolve_fully(field_ty);
    let tag = pool.tag(resolved);
    match tag {
        // Scalar tier — omit per decisions/06 §Phase Boundary Cure.
        Tag::Int
        | Tag::Float
        | Tag::Bool
        | Tag::Char
        | Tag::Byte
        | Tag::Unit
        | Tag::Never
        | Tag::Duration
        | Tag::Size
        | Tag::Ordering
        | Tag::Error => FieldClass::Scalar,
        // Heap-owning leaf tags — owned.
        Tag::Str
        | Tag::List
        | Tag::Map
        | Tag::Set
        | Tag::Channel
        | Tag::Iterator
        | Tag::DoubleEndedIterator
        | Tag::Function => FieldClass::Owned,
        // Compound / nominal / type-var tags — classify via Triviality.
        _ => match classify_triviality(field_ty, pool) {
            Triviality::Trivial => FieldClass::Scalar,
            Triviality::NonTrivial | Triviality::Unknown => FieldClass::Owned,
        },
    }
}

/// Compute burden for a struct from its field list.
pub(crate) fn compute_struct_burden(
    field_defs: &[FieldDef],
    pool: &Pool,
) -> Option<UserBurdenSpec> {
    let mut owned = Vec::new();
    for (i, fd) in field_defs.iter().enumerate() {
        let path = vec![idx_u32(i)];
        match classify_field_for_burden(fd.ty, pool) {
            FieldClass::Scalar => {}
            FieldClass::Owned => owned.push(UserOwnedField {
                field_path: path,
                field_type: fd.ty,
            }),
        }
    }
    // borrowed_fields stays empty until Tag::Borrowed construction ships per
    // types.md §TL-9 — borrowed tier reserved by decisions/06 §Phase Boundary Cure.
    let borrowed: Vec<UserBorrowedField> = Vec::new();
    if owned.is_empty() && borrowed.is_empty() {
        return None;
    }
    Some(UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: owned,
        borrowed_fields: borrowed,
        ..UserBurdenSpec::default()
    })
}

fn push_variant_field(
    fi: usize,
    field_ty: Idx,
    pool: &Pool,
    transfers: &mut Vec<UserTransferRule>,
    retained: &mut Vec<UserOwnedField>,
) {
    let path = vec![idx_u32(fi)];
    match classify_field_for_burden(field_ty, pool) {
        FieldClass::Scalar => {}
        FieldClass::Owned => {
            transfers.push(UserTransferRule {
                source_field_path: path.clone(),
                binding_index: idx_u16(fi),
                field_type: field_ty,
                transfer_kind: TransferKind::Move,
            });
            retained.push(UserOwnedField {
                field_path: path,
                field_type: field_ty,
            });
        }
    }
}

/// Compute burden for an enum from its variant list.
pub(crate) fn compute_enum_burden(variants: &[VariantDef], pool: &Pool) -> Option<UserBurdenSpec> {
    let mut variant_burdens = Vec::new();
    for (vi, variant) in variants.iter().enumerate() {
        // Variant IDs are 1-indexed (NonZeroU32). Saturating add guards against
        // pathological variant counts; fall back to NonZeroU32::MIN if the
        // computed value ever lands on zero (which it cannot, by construction).
        let nz = NonZeroU32::new(idx_u32(vi).saturating_add(1)).unwrap_or(NonZeroU32::MIN);
        let variant_id = VariantId::new(nz);
        let mut transfers = Vec::new();
        let mut retained = Vec::new();
        match &variant.fields {
            VariantFields::Unit => {}
            VariantFields::Tuple(field_types) => {
                for (fi, ty) in field_types.iter().enumerate() {
                    push_variant_field(fi, *ty, pool, &mut transfers, &mut retained);
                }
            }
            VariantFields::Record(field_defs) => {
                for (fi, fd) in field_defs.iter().enumerate() {
                    push_variant_field(fi, fd.ty, pool, &mut transfers, &mut retained);
                }
            }
        }
        variant_burdens.push(UserVariantBurden {
            variant_id,
            transfers_on_match: transfers,
            retained_owned: retained,
        });
    }
    if variant_burdens
        .iter()
        .all(|v| v.transfers_on_match.is_empty() && v.retained_owned.is_empty())
    {
        return None;
    }
    Some(UserBurdenSpec {
        self_heap_alloc: true,
        variant_burdens,
        ..UserBurdenSpec::default()
    })
}

/// Compute burden for a newtype from its underlying type index.
pub(crate) fn compute_newtype_burden(
    underlying: Idx,
    pool: &Pool,
    type_registry: &TypeRegistry,
) -> Option<UserBurdenSpec> {
    let resolved = pool.resolve_fully(underlying);
    // If underlying resolves to a previously-registered user type, inherit
    // its burden directly. Cycle composition deferred to §02 per
    // decisions/06 line 122.
    if let Some(entry) = type_registry.get_by_idx(resolved) {
        match entry.kind {
            TypeKind::Struct(_)
            | TypeKind::Enum { .. }
            | TypeKind::Newtype { .. }
            | TypeKind::Alias { .. } => return entry.burden.clone(),
        }
    }
    // Underlying is a builtin / primitive / not-yet-registered.
    let tag = pool.tag(resolved);
    match tag {
        Tag::Int
        | Tag::Float
        | Tag::Bool
        | Tag::Char
        | Tag::Byte
        | Tag::Unit
        | Tag::Never
        | Tag::Duration
        | Tag::Size
        | Tag::Ordering
        | Tag::Error => None,
        Tag::Str
        | Tag::List
        | Tag::Map
        | Tag::Set
        | Tag::Channel
        | Tag::Iterator
        | Tag::DoubleEndedIterator
        | Tag::Function => Some(UserBurdenSpec {
            self_heap_alloc: true,
            owned_fields: vec![UserOwnedField {
                field_path: vec![0],
                field_type: underlying,
            }],
            ..UserBurdenSpec::default()
        }),
        _ => match classify_triviality(underlying, pool) {
            Triviality::Trivial => None,
            Triviality::NonTrivial | Triviality::Unknown => Some(UserBurdenSpec {
                self_heap_alloc: true,
                owned_fields: vec![UserOwnedField {
                    field_path: vec![0],
                    field_type: underlying,
                }],
                ..UserBurdenSpec::default()
            }),
        },
    }
}
