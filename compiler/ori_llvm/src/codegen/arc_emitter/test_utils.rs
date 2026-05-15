//! `#[cfg(test)]` fixtures for `arc_emitter::tests`.
//!
//! Cycle 50a adds `entry_block` / `burden_dec_field_first` / `set_first`
//! constructors that hide the canonical zero defaults (`params: vec![]`,
//! `field: 0`) used by every `ArcBlock` / `ArcInstr::BurdenDecField` /
//! `ArcInstr::Set` fixture in `arc_emitter/tests.rs`. Per
//! `impl-hygiene.md §PARAM_SPRAWL cure 4 — side-table by existing key`: the
//! literal defaults move out of construction sites into named helpers.
//!
//! Sibling to `compiler/ori_arc/src/lower/test_utils.rs` (which serves
//! `ori_arc` tests); per-crate fixtures are required because the helpers
//! are `#[cfg(test)]`-scoped and not cross-crate visible.

use ori_arc::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId};

/// Canonical `ArcBlock` constructor for single-entry-block fixtures: hides the
/// `id: ArcBlockId::new(0)` and `params: vec![]` literals every fixture
/// repeats. Per `impl-hygiene.md §PARAM_SPRAWL:zero-default-proliferation`.
pub(crate) fn entry_block(body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body,
        terminator,
    }
}

/// Canonical `ArcInstr::BurdenDecField` for first-field cleanup: hides the
/// `field: 0` literal. Tests targeting a non-zero field MUST construct
/// `ArcInstr::BurdenDecField` inline.
pub(crate) fn burden_dec_field_first(base: ArcVarId) -> ArcInstr {
    ArcInstr::BurdenDecField { base, field: 0 }
}

/// Canonical `ArcInstr::Set` for first-field in-place mutation: hides the
/// `field: 0` literal. Tests mutating a non-zero field MUST construct
/// `ArcInstr::Set` inline.
pub(crate) fn set_first(base: ArcVarId, value: ArcVarId) -> ArcInstr {
    ArcInstr::Set {
        base,
        field: 0,
        value,
    }
}
