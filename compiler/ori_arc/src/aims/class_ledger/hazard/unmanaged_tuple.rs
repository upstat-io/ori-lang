//! Multi-container tuple cures require every participating type to lack recursive cleanup.

use crate::aims::intraprocedural::birth_site_partition::FieldPath;
use crate::ir::ArcInstr;
use crate::lower::burden_lookup::type_has_user_drop;

use super::{increment_is_materializable, FieldViewHazard, HazardCureInputs, HazardCureState};

/// Accept an unmanaged tuple chain only when neither containers nor projected members clean up.
pub(super) fn chain_has_no_recursive_field_release(
    inputs: &HazardCureInputs<'_>,
    state: &mut HazardCureState<'_>,
    hazards: &[&FieldViewHazard],
) -> bool {
    if hazards.len() < 2
        || hazards.iter().any(|hazard| {
            hazard.is_sum_container()
                || state
                    .partition
                    .node_key(hazard.container)
                    .is_none_or(|(var, _)| {
                        inputs
                            .func
                            .var_types
                            .get(var.index())
                            .is_some_and(|&ty| type_has_user_drop(ty, inputs.type_registry))
                    })
        })
    {
        return false;
    }

    let view = hazards[0].view;
    let mut found = false;
    for block in &inputs.func.blocks {
        for instr in &block.body {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = state.partition.register_node(*dst, FieldPath::whole_var());
            if state.partition.rep_of(node) != view {
                continue;
            }
            found = true;
            let Some(&member_ty) = inputs.func.var_types.get(dst.index()) else {
                return false;
            };
            if type_has_user_drop(member_ty, inputs.type_registry)
                || increment_is_materializable(inputs.func, *dst, inputs.type_registry)
            {
                return false;
            }
        }
    }
    // A surviving multi-container hazard cannot be constructor-only: detection
    // handles rename-lineage stores as funded or released hand-offs, so the
    // extra lineage must originate at a matching `Project`.
    assert!(
        found,
        "an unmanaged multi-container hazard must have a matching Project"
    );
    true
}
