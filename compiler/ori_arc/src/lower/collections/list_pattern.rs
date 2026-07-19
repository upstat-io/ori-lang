//! Shared list-pattern element and rest-slice emission.

use ori_ir::{Span, StringInterner};
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue};

use super::super::ArcIrBuilder;

pub(crate) fn emit_list_element(
    builder: &mut ArcIrBuilder,
    interner: &StringInterner,
    list_value: ArcVarId,
    idx: u32,
    elem_ty: Idx,
    span: Option<Span>,
) -> ArcVarId {
    let idx_var = builder.emit_let(
        Idx::INT,
        ArcValue::Literal(LitValue::Int(i64::from(idx))),
        span,
    );
    let index_fn =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
    builder.emit_apply(elem_ty, index_fn, vec![list_value, idx_var], span, None)
}

pub(crate) fn emit_list_rest_slice(
    builder: &mut ArcIrBuilder,
    interner: &StringInterner,
    list_value: ArcVarId,
    start_idx: u32,
    list_ty: Idx,
    span: Option<Span>,
) -> ArcVarId {
    let start_var = builder.emit_let(
        Idx::INT,
        ArcValue::Literal(LitValue::Int(i64::from(start_idx))),
        span,
    );
    let slice_fn = interner.intern("ori_list_slice_drop");
    builder.emit_apply(list_ty, slice_fn, vec![list_value, start_var], span, None)
}
