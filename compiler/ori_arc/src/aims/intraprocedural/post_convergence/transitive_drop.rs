//! Singleton-class materialization helpers for transitive-drop payloads.

use crate::ir::{ArcFunction, ArcVarId, RcStrategy, ValueRepr};

use super::super::state_map::AimsStateMap;

/// Ensure both endpoints of a payload edge have materialized class entries.
pub(super) fn materialize_payload_edge_classes(
    arg: ArcVarId,
    dst: ArcVarId,
    func: &ArcFunction,
    state_map: &mut AimsStateMap,
) {
    if matches!(func.var_reprs.get(arg.index()), Some(&ValueRepr::Scalar)) {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            "materialize_payload_edge: skip — arg is scalar"
        );
        return;
    }
    let arg_class = state_map.class_id_of(arg);
    let dst_class = state_map.class_id_of(dst);
    if arg_class == dst_class {
        tracing::trace!(
            func = ?func.name,
            arg_var = arg.raw(),
            dst_var = dst.raw(),
            class = arg_class,
            "materialize_payload_edge: skip — self-loop"
        );
        return;
    }
    state_map.ensure_singleton_class(arg_class);
    state_map.ensure_singleton_class(dst_class);
}

/// Return the non-scalar destination's RC strategy when one exists.
pub(super) fn dst_strategy_of(func: &ArcFunction, dst: ArcVarId) -> Option<RcStrategy> {
    *func.var_rc_strategies.get(dst.index())?
}
