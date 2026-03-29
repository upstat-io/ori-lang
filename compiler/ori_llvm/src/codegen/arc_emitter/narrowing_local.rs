//! Local range derivation for post-merge fresh variables.
//!
//! Block-merge creates fresh variables (`func.fresh_var_repr()`) for Select
//! destinations and renamed arm-local Let bindings. These variables have no
//! entry in the `ReprPlan` because range analysis ran on the pre-merge ARC IR.
//!
//! This module provides [`derive_local_range`] which recursively derives ranges
//! from the post-merge defining instructions, enabling Phase B narrowing for
//! block-merge-introduced locals (e.g., Select results from folded if/else).

use ori_arc::ir::{ArcInstr, ArcValue, ArcVarId, LitValue};
use ori_repr::ValueRange;

/// Derive a variable's range from the `ReprPlan`, falling back to local
/// instruction analysis for variables created by post-fixpoint transforms.
///
/// For pre-merge variables, returns the plan range directly.
/// For post-merge fresh variables (range is `Top` in the plan), derives
/// the range from the defining instruction:
/// - `Let { Literal(Int(n)) }` → `[n, n]`
/// - `Let { Var(src) }` → source variable's range
/// - `Select { true_val, false_val }` → `join(true_val range, false_val range)`
pub(super) fn derive_local_range(
    var: ArcVarId,
    func_name: ori_ir::Name,
    plan: &ori_repr::ReprPlan,
    def_instrs: &rustc_hash::FxHashMap<ArcVarId, &ArcInstr>,
    cache: &mut rustc_hash::FxHashMap<ArcVarId, ValueRange>,
) -> ValueRange {
    if let Some(&cached) = cache.get(&var) {
        return cached;
    }

    // Try the plan first — covers pre-merge variables.
    let plan_range = plan.var_range(func_name, var);
    if plan_range != ValueRange::Top {
        cache.insert(var, plan_range);
        return plan_range;
    }

    // Not in plan — derive from the defining instruction.
    let Some(instr) = def_instrs.get(&var) else {
        cache.insert(var, ValueRange::Top);
        return ValueRange::Top;
    };

    let result = match instr {
        ArcInstr::Let {
            value: ArcValue::Literal(LitValue::Int(n)),
            ..
        } => ValueRange::Bounded { lo: *n, hi: *n },

        ArcInstr::Let {
            value: ArcValue::Var(src),
            ..
        } => derive_local_range(*src, func_name, plan, def_instrs, cache),

        ArcInstr::Select {
            true_val,
            false_val,
            ..
        } => {
            let t = derive_local_range(*true_val, func_name, plan, def_instrs, cache);
            let f = derive_local_range(*false_val, func_name, plan, def_instrs, cache);
            t.join(f)
        }

        _ => ValueRange::Top,
    };

    cache.insert(var, result);
    result
}
