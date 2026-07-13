//! Class-ledger survivor walk for RC optimization remarks.

use rustc_hash::FxHashSet;

use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::verify::burden_balance::{var_def_kind, var_repr_kind};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};

use super::{emit_rc_survivor_remark, rc_remarks_enabled, RcSurvivorSite};

/// Emit one observability-only `missed` remark per variable carrying a planned
/// `BurdenInc`. The class-ledger plan lowers mechanically, so every planned inc
/// survives to a real RC operation.
pub(crate) fn emit_survivor_remarks_all_kept(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &ori_ir::StringInterner,
) {
    if !rc_remarks_enabled() {
        return;
    }
    let function_name = interner.lookup(func.name);
    let mut emitted: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::BurdenInc { var } = instr else {
                continue;
            };
            if !emitted.insert(*var) {
                continue;
            }
            let state = state_map.var_state_at_definition(block_id, *var);
            let span = func
                .spans
                .get(block_idx)
                .and_then(|block| block.get(instr_idx))
                .copied()
                .flatten()
                .map(|span| (span.start, span.end));
            emit_rc_survivor_remark(&RcSurvivorSite {
                var: *var,
                consumption: state.consumption,
                locality: state.locality,
                cardinality: state.cardinality,
                def_kind: var_def_kind(func, *var),
                var_repr: var_repr_kind(func, *var),
                instr,
                span,
                function_name,
            });
        }
    }
}
