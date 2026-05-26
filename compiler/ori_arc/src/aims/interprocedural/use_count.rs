//! Variable use-counting for demand-propagation uniqueness analysis.
//!
//! `count_var_uses` is load-bearing for BUG-04-069: `arg_satisfies_uniqueness`
//! gates uniqueness-tightening on `count_var_uses == 1`, so every operand
//! position (including the `Burden*` family) must be counted.

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

/// Count how many times a variable appears as an operand in a function.
///
/// Counts uses in instruction operands and terminators. Does NOT count
/// the variable's definition (e.g., as `dst` in Construct or Apply).
pub(crate) fn count_var_uses(func: &ArcFunction, var: ArcVarId) -> usize {
    let mut count = 0;

    for block in &func.blocks {
        for instr in &block.body {
            count += instr_use_count(instr, var);
        }
        count += terminator_use_count(&block.terminator, var);
    }

    count
}

/// Count uses of a variable in a single instruction.
fn instr_use_count(instr: &ArcInstr, var: ArcVarId) -> usize {
    match instr {
        ArcInstr::Let { value, .. } => match value {
            crate::ir::ArcValue::Var(v) => usize::from(*v == var),
            crate::ir::ArcValue::Literal(_) => 0,
            crate::ir::ArcValue::PrimOp { args, .. } => args.iter().filter(|&&v| v == var).count(),
        },
        ArcInstr::Construct { args, .. }
        | ArcInstr::Apply { args, .. }
        | ArcInstr::PartialApply { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            usize::from(*closure == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcInstr::Project { value, .. } => usize::from(*value == var),
        ArcInstr::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            usize::from(*cond == var)
                + usize::from(*true_val == var)
                + usize::from(*false_val == var)
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            usize::from(*old_var == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcInstr::IsShared { var: v, .. }
        | ArcInstr::Reset { var: v, .. }
        | ArcInstr::RcInc { var: v, .. }
        | ArcInstr::RcDec { var: v, .. }
        | ArcInstr::BurdenInc { var: v }
        | ArcInstr::BurdenDec { var: v }
        | ArcInstr::BurdenDecPartial { var: v, .. }
        | ArcInstr::BurdenDecVariant { var: v } => usize::from(*v == var),
        ArcInstr::BurdenDecField { base, .. } | ArcInstr::SetTag { base, .. } => {
            usize::from(*base == var)
        }
        ArcInstr::Set { base, value, .. } => usize::from(*base == var) + usize::from(*value == var),
        ArcInstr::Reuse { token, args, .. } => {
            usize::from(*token == var) + args.iter().filter(|&&v| v == var).count()
        }
    }
}

/// Count uses of a variable in a terminator.
fn terminator_use_count(term: &crate::ir::ArcTerminator, var: ArcVarId) -> usize {
    use crate::ir::ArcTerminator;
    match term {
        ArcTerminator::Return { value } => usize::from(*value == var),
        ArcTerminator::Jump { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcTerminator::Branch { cond, .. } => usize::from(*cond == var),
        ArcTerminator::Switch { scrutinee, .. } => usize::from(*scrutinee == var),
        ArcTerminator::Invoke { args, .. } => args.iter().filter(|&&v| v == var).count(),
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            usize::from(*closure == var) + args.iter().filter(|&&v| v == var).count()
        }
        ArcTerminator::Resume | ArcTerminator::Unreachable => 0,
    }
}
