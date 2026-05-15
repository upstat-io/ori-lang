//! Shared `#[cfg(test)]` fixtures for `lower/` submodules.
//!
//! Lifts `test_name` and `registered_struct_with_burden` out of
//! `burden_lookup/tests.rs` so `burden_lower/tests.rs` can consume the same
//! user-defined-type fixture for §03.4 cycle-43 full-move-suppression pins.
//!
//! Cycle 50a adds `entry_block` / `project_first` / `set_first` constructors
//! that hide canonical zero defaults (`params: Vec::new()`, `field: 0`) used
//! by every `ArcBlock` / `ArcInstr::Project` / `ArcInstr::Set` fixture in
//! `burden_lower/tests.rs`. Per `impl-hygiene.md §PARAM_SPRAWL cure 4 —
//! side-table by existing key`: the literal defaults move out of construction
//! sites into named helpers.
//!
//! Single source of truth per `impl-hygiene.md §SSOT`; never duplicate.

use ori_ir::{Name, Span};
use ori_types::burden::{UserBurdenSpec, UserOwnedField};
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
/// `lower/burden_lookup/tests.rs::registered_struct_with_burden` (pre-cycle-44a);
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

/// Register a 2-field user-defined struct (`{ data: str, name: str }`) with
/// BOTH fields heap-burden-typed and a `UserBurdenSpec` naming BOTH as owned.
///
/// Sibling to `registered_struct_with_burden` (preserved single-field variant
/// for `burden_lookup/tests.rs`); cycle 46 introduces this multi-field shape so
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

/// Canonical `ArcBlock` constructor for single-entry-block fixtures: hides the
/// `id: ArcBlockId::new(0)` and `params: Vec::new()` literals every fixture
/// repeats. Per `impl-hygiene.md §PARAM_SPRAWL:zero-default-proliferation`.
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
