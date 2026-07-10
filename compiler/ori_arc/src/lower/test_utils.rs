//! Shared `#[cfg(test)]` fixtures for `lower/` submodules.
//!
//! Lifts `test_name` and `registered_struct_with_burden` out of
//! `burden_lookup/tests.rs` so `burden_lower/tests.rs` can consume the same
//! user-defined-type fixture for full-move-suppression pins.
//!
//! adds `entry_block` / `project_first` / `set_first` constructors
//! that hide canonical zero defaults (`params: Vec::new`, `field: 0`) used
//! by every `ArcBlock` / `ArcInstr::Project` / `ArcInstr::Set` fixture in
//! `burden_lower/tests.rs`. Per `_SPRAWL cure 4 —
//! side-table by existing key`: the literal defaults move out of construction
//! sites into named helpers.
//!
//! Single source of truth; never duplicate.

use ori_ir::{Name, Span};
use ori_types::burden::{UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden};
use ori_types::{FieldDef, Idx, TypeRegistry, Visibility};

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId};

/// Construct a `Name` from a literal string via a deterministic byte-sum hash.
///
/// Test-only helper; not a real interner. Suitable for synthetic identifiers
/// in fixture structs where uniqueness within the test scope is enough.
pub(crate) fn test_name(s: &str) -> Name {
    Name::from_raw(
        s.as_bytes()
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b))),
    )
}

/// Register a single-field user-defined struct (`{ payload: int }`) into the
/// `TypeRegistry` under `name` at `idx`, optionally bound to a `UserBurdenSpec`.
///
/// Mirrors the original site at
/// `lower/burden_lookup/tests.rs::registered_struct_with_burden` (pre-);
/// callers in both `burden_lookup/tests.rs` and `burden_lower/tests.rs` consume
/// this fixture via `crate::lower::test_utils::registered_struct_with_burden`.
pub(crate) fn registered_struct_with_burden(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
    burden: Option<UserBurdenSpec>,
) {
    let fields = vec![FieldDef {
        name: test_name("payload"),
        ty: Idx::INT,
        span: Span::DUMMY,
        visibility: Visibility::Public,
    }];
    registry.register_struct(
        test_name(name),
        idx,
        vec![],
        fields,
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        burden,
    );
}

/// Register a user ENUM burden whose single payload-carrying variant is
/// `variant` (0-based ordinal; payload at variant-local field 0, typed
/// `payload_ty`) — the explicit-slot sum shape the field-decomposition sum
/// admission requires.
pub(crate) fn registered_enum_with_single_payload_variant(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
    variant: u32,
    payload_ty: Idx,
) {
    let nz = match core::num::NonZeroU32::new(variant.saturating_add(1)) {
        Some(n) => n,
        None => core::num::NonZeroU32::MIN,
    };
    let burden = UserBurdenSpec {
        variant_burdens: vec![UserVariantBurden {
            variant_id: ori_registry::burden::VariantId::new(nz),
            transfers_on_match: vec![UserTransferRule {
                source_field_path: vec![0],
                binding_index: 0,
                field_type: payload_ty,
                transfer_kind: ori_registry::burden::TransferKind::Move,
            }],
            retained_owned: vec![],
        }],
        ..UserBurdenSpec::default()
    };
    registry.register_enum(
        test_name(name),
        idx,
        vec![],
        vec![],
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(burden),
    );
}

/// Register a user ENUM burden in the COMPUTE shape (`build_enum_burden`
/// parity): `self_heap_alloc: true`, the unique payload-bearing variant
/// carrying BOTH a transfer-on-match rule AND a retained owned field —
/// the shape the constructless type-derived variant skip requires.
pub(crate) fn registered_tagged_enum_with_unique_payload_variant(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
    variant: u32,
    payload_ty: Idx,
) {
    let nz = match core::num::NonZeroU32::new(variant.saturating_add(1)) {
        Some(n) => n,
        None => core::num::NonZeroU32::MIN,
    };
    let burden = UserBurdenSpec {
        self_heap_alloc: true,
        variant_burdens: vec![UserVariantBurden {
            variant_id: ori_registry::burden::VariantId::new(nz),
            transfers_on_match: vec![UserTransferRule {
                source_field_path: vec![0],
                binding_index: 0,
                field_type: payload_ty,
                transfer_kind: ori_registry::burden::TransferKind::Move,
            }],
            retained_owned: vec![UserOwnedField {
                field_path: vec![0],
                field_type: payload_ty,
            }],
        }],
        ..UserBurdenSpec::default()
    };
    registry.register_enum(
        test_name(name),
        idx,
        vec![],
        vec![],
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(burden),
    );
}

/// Register a 2-field user-defined struct (`{ data: str, name: str }`) with
/// BOTH fields heap-burden-typed and a `UserBurdenSpec` naming BOTH as owned.
///
/// Sibling to `registered_struct_with_burden` (preserved single-field variant
/// for `burden_lookup/tests.rs`); introduces this multi-field shape so
/// `burden_lower/tests.rs` can exercise the partial-move emission path — a
/// single-field fixture cannot distinguish full-move from partial-move because
/// moving the lone owned field equals "all owned fields moved".
///
/// `UserBurdenSpec.owned_fields` carries `field_path: vec![0]` (data) and
/// `field_path: vec![1]` (name); both `Idx::STR`. The struct's `burden_carries_rc`
/// returns true via the non-empty `owned_fields`, so the struct's SSA value
/// enters `owned_vars_needing_rc` and downstream `compute_partial_move_vars`
/// emits `BurdenDecPartial { var, skip_fields }` when exactly one field is
/// transferred at a project-then-consume site.
pub(crate) fn registered_struct_with_two_owned_str_fields(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
) {
    let fields = vec![
        FieldDef {
            name: test_name("data"),
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: test_name("name"),
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
    ];
    let burden = UserBurdenSpec {
        self_heap_alloc: false,
        owned_fields: vec![
            UserOwnedField {
                field_path: vec![0],
                field_type: Idx::STR,
            },
            UserOwnedField {
                field_path: vec![1],
                field_type: Idx::STR,
            },
        ],
        ..UserBurdenSpec::default()
    };
    registry.register_struct(
        test_name(name),
        idx,
        vec![],
        fields,
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(burden),
    );
}

/// Register a 2-field Value/HeapType-mixed struct (`{ tag: int, payload: str }`)
/// whose `UserBurdenSpec.owned_fields` names ONLY the heap field (`payload`,
/// `field_path: vec![1]`, `Idx::STR`); the `Value` field (`tag`, `Idx::INT`,
/// field 0) is OMITTED from `owned_fields`.
///
/// `burden_carries_rc` returns true via the non-empty `owned_fields`, so the
/// struct's SSA value enters the owned-burden walk, but the whole-var
/// `BurdenDec` covers only the `str` field through drop-glue — the `Value`
/// field drives no burden op (no per-field inc, no `BurdenDecField`). A mixed
/// fixture is the only shape that distinguishes "only the `HeapType` field is
/// burden-tracked" from "every field is owned".
pub(crate) fn registered_struct_value_heap_mixed(
    registry: &mut TypeRegistry,
    name: &str,
    idx: Idx,
) {
    let fields = vec![
        FieldDef {
            name: test_name("tag"),
            ty: Idx::INT,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
        FieldDef {
            name: test_name("payload"),
            ty: Idx::STR,
            span: Span::DUMMY,
            visibility: Visibility::Public,
        },
    ];
    let burden = UserBurdenSpec {
        self_heap_alloc: false,
        owned_fields: vec![UserOwnedField {
            field_path: vec![1],
            field_type: Idx::STR,
        }],
        ..UserBurdenSpec::default()
    };
    registry.register_struct(
        test_name(name),
        idx,
        vec![],
        fields,
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        Some(burden),
    );
}

/// Canonical `ArcBlock` constructor for single-entry-block fixtures: hides the
/// `id: ArcBlockId::new(0)` and `params: Vec::new` literals every fixture
/// repeats..
pub(crate) fn entry_block(body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body,
        terminator,
    }
}

/// Canonical `ArcInstr::Project` for the first-field projection used by every
/// two-stage / TF-4 pin: hides the `field: 0` literal. Tests projecting a
/// non-zero field MUST construct `ArcInstr::Project` inline (helper is scoped
/// to the canonical first-field case).
pub(crate) fn project_first(dst: ArcVarId, ty: Idx, value: ArcVarId) -> ArcInstr {
    ArcInstr::Project {
        dst,
        ty,
        value,
        field: 0,
    }
}

/// Canonical `ArcInstr::Set` for first-field in-place mutation: hides the
/// `field: 0` literal. Tests mutating a non-zero field MUST construct
/// `ArcInstr::Set` inline (helper is scoped to the canonical first-field case).
pub(crate) fn set_first(base: ArcVarId, value: ArcVarId) -> ArcInstr {
    ArcInstr::Set {
        base,
        field: 0,
        value,
    }
}
