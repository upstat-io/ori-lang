//! Backward operand-demand transfer.

use smallvec::SmallVec;

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::{Cardinality, Consumption};

/// One explicit TF-11 operand demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackwardDemand {
    /// Operand receiving the demand.
    pub(crate) var: ArcVarId,
    /// Quantitative use count contributed by the instruction.
    pub(crate) cardinality: Cardinality,
    /// Ownership-consumption mode contributed by the instruction.
    pub(crate) consumption: Consumption,
}

impl BackwardDemand {
    fn linear_once(var: ArcVarId) -> Self {
        Self {
            var,
            cardinality: Cardinality::Once,
            consumption: Consumption::Linear,
        }
    }
}

/// Compute backward demand contributions for an instruction.
pub(crate) fn backward_demands(instruction: &ArcInstr) -> SmallVec<[BackwardDemand; 4]> {
    match instruction {
        ArcInstr::Let { value, .. } => match value {
            ArcValue::Var(_) | ArcValue::Literal(_) => SmallVec::new(),
            ArcValue::PrimOp { args, .. } => args
                .iter()
                .map(|&var| BackwardDemand::linear_once(var))
                .collect(),
        },
        ArcInstr::Construct { args, .. } | ArcInstr::Apply { args, .. } => args
            .iter()
            .map(|&var| BackwardDemand::linear_once(var))
            .collect(),
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            let mut demands = SmallVec::with_capacity(1 + args.len());
            demands.push(BackwardDemand::linear_once(*closure));
            demands.extend(args.iter().map(|&var| BackwardDemand::linear_once(var)));
            demands
        }
        ArcInstr::PartialApply { .. }
        | ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::RcDecPartial { .. }
        | ArcInstr::RcDecField { .. }
        | ArcInstr::RcDecVariant { .. }
        | ArcInstr::BurdenInc { .. }
        | ArcInstr::BurdenDec { .. }
        | ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. }
        | ArcInstr::Project { .. } => SmallVec::new(),
        ArcInstr::Select { cond, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*cond); 4], 1)
        }
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            let mut demands = SmallVec::with_capacity(1 + args.len());
            demands.push(BackwardDemand::linear_once(*old_var));
            demands.extend(args.iter().map(|&var| BackwardDemand::linear_once(var)));
            demands
        }
        ArcInstr::IsShared { var, .. } | ArcInstr::Reset { var, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*var); 4], 1)
        }
        ArcInstr::Set { base, value, .. } => {
            let mut demands = SmallVec::new();
            demands.push(BackwardDemand::linear_once(*base));
            demands.push(BackwardDemand::linear_once(*value));
            demands
        }
        ArcInstr::SetTag { base, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*base); 4], 1)
        }
        ArcInstr::Reuse { token, args, .. } => {
            let mut demands = SmallVec::with_capacity(1 + args.len());
            demands.push(BackwardDemand::linear_once(*token));
            demands.extend(args.iter().map(|&var| BackwardDemand::linear_once(var)));
            demands
        }
    }
}

/// Compute backward demand contributions for a terminator.
pub(crate) fn backward_terminator_demands(
    terminator: &ArcTerminator,
) -> SmallVec<[BackwardDemand; 4]> {
    match terminator {
        ArcTerminator::Return { value } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*value); 4], 1)
        }
        ArcTerminator::Jump { args, .. } => args
            .iter()
            .map(|&var| BackwardDemand::linear_once(var))
            .collect(),
        ArcTerminator::Branch { cond, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*cond); 4], 1)
        }
        ArcTerminator::Switch { scrutinee, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*scrutinee); 4], 1)
        }
        ArcTerminator::Invoke { args, .. } => args
            .iter()
            .map(|&var| BackwardDemand::linear_once(var))
            .collect(),
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            let mut demands = SmallVec::with_capacity(1 + args.len());
            demands.push(BackwardDemand::linear_once(*closure));
            demands.extend(args.iter().map(|&var| BackwardDemand::linear_once(var)));
            demands
        }
        ArcTerminator::Resume | ArcTerminator::Unreachable => SmallVec::new(),
    }
}
