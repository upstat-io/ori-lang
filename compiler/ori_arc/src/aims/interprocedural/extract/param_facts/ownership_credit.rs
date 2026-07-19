//! Borrowed-root ownership-credit detection for parameter contracts.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership};
use crate::ArcClassification;

/// Find parameters whose borrowed-root alias crosses an ownership-taking edge.
///
/// The class ledger funds every such edge with a logical credit when contract
/// extraction leaves the parameter Borrowed. This structural fact stays
/// separate from access promotion: changing a parameter to Owned would alter
/// the call boundary, while `may_share` publishes the credit actually planned
/// for the borrowed boundary.
pub(in crate::aims::interprocedural::extract) fn find_borrowed_root_credit_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    exact_callables: &FxHashSet<Name>,
) -> FxHashSet<usize> {
    let mut credited = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::PartialApply { args, .. } => {
                    absorb_args(args, alias_to_param, &mut credited);
                }
                ArcInstr::CollectionReuse { old_var, args, .. } => {
                    absorb_arg(*old_var, alias_to_param, &mut credited);
                    absorb_args(args, alias_to_param, &mut credited);
                }
                ArcInstr::Set { value, .. } => {
                    absorb_arg(*value, alias_to_param, &mut credited);
                }
                ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::PrimOp { args, .. },
                    ..
                } => {
                    let fact = func.primitive_facts.get(*dst).unwrap_or_else(|| {
                        panic!("validated PrimOp v{} is missing its frozen fact", dst.raw())
                    });
                    for (&arg, &use_) in args.iter().zip(fact.descriptor.operand_uses) {
                        if use_ == ori_registry::PrimitiveOperandUse::Consume {
                            absorb_arg(arg, alias_to_param, &mut credited);
                        }
                    }
                }
                ArcInstr::Select { dst, .. }
                    if classifier.has_managed_ownership_obligation(func.var_types[dst.index()]) =>
                {
                    absorb_arg(*dst, alias_to_param, &mut credited);
                }
                ArcInstr::Apply {
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => absorb_call_args(
                    func,
                    *callee,
                    args,
                    arg_ownership,
                    sigs,
                    alias_to_param,
                    classifier,
                    builtins,
                    exact_callables,
                    &mut credited,
                ),
                _ => {}
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            absorb_call_args(
                func,
                *callee,
                args,
                arg_ownership,
                sigs,
                alias_to_param,
                classifier,
                builtins,
                exact_callables,
                &mut credited,
            );
        }
    }
    credited
}

#[expect(
    clippy::too_many_arguments,
    reason = "call credit detection keeps contracts, types, aliases, and callable identity explicit"
)]
fn absorb_call_args(
    func: &ArcFunction,
    callee: Name,
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    exact_callables: &FxHashSet<Name>,
    credited: &mut FxHashSet<usize>,
) {
    let arg_tags = args
        .iter()
        .map(|arg| classifier.builtin_type_tag(func.var_types[arg.index()]))
        .collect::<Vec<_>>();
    let typed_consuming = if exact_callables.contains(&callee) {
        smallvec::SmallVec::<[usize; 3]>::new()
    } else {
        builtins.type_qualified_consuming_positions(callee, &arg_tags)
    };
    for (position, &arg) in args.iter().enumerate() {
        let contract_owned = sigs.get(&callee).is_some_and(|contract| {
            contract
                .params
                .get(position)
                .is_some_and(|param| param.access == AccessClass::Owned)
        });
        let annotated_owned = arg_ownership.get(position) == Some(&ArgOwnership::Owned);
        if contract_owned || annotated_owned || typed_consuming.contains(&position) {
            absorb_arg(arg, alias_to_param, credited);
        }
    }
}

fn absorb_args(
    args: &[ArcVarId],
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    credited: &mut FxHashSet<usize>,
) {
    for &arg in args {
        absorb_arg(arg, alias_to_param, credited);
    }
}

fn absorb_arg(
    arg: ArcVarId,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    credited: &mut FxHashSet<usize>,
) {
    if let Some(params) = alias_to_param.get(&arg) {
        credited.extend(params);
    }
}
