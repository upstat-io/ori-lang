//! Exact aggregate-reconstruction ownership transfer.
//!
//! A projected field may cross one owned call before entering the rebuilt
//! aggregate (the COW `xs.push(v)` shape). That call consumes the extracted
//! field credit and returns the carrier stored at the same constructor
//! position. When every constructor position has one such linear projection
//! lineage from the same parameter, and the rebuilt value is returned, the
//! function consumes the complete aggregate parameter.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::ArcClassification;

/// Parameters transferred by an exact all-field reconstruction.
pub(super) fn find_exact_aggregate_transfer_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    classifier: &dyn ArcClassification,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<usize> {
    let mut transferred = FxHashSet::default();
    for block in &func.blocks {
        let ArcTerminator::Return { value: returned } = block.terminator else {
            continue;
        };
        for (construct_index, instr) in block.body.iter().enumerate() {
            let ArcInstr::Construct {
                dst,
                ty,
                args: construct_args,
                ..
            } = instr
            else {
                continue;
            };
            if *dst != returned || construct_args.is_empty() {
                continue;
            }
            let Some((param, projections)) = exact_reconstruction(
                block,
                construct_index,
                construct_args,
                sigs,
                alias_to_param,
                exact_callables,
                interner,
            ) else {
                continue;
            };
            let Some(param_ty) = func.params.get(param).map(|param| param.ty) else {
                continue;
            };
            if param_ty != *ty
                || !projections.iter().any(|projection| {
                    func.var_types
                        .get(projection.index())
                        .is_some_and(|&ty| !classifier.is_scalar(ty))
                })
                || !parameter_uses_confined_to_projections(
                    func,
                    param,
                    alias_to_param,
                    &projections,
                )
            {
                continue;
            }
            transferred.insert(param);
        }
    }
    transferred
}

/// Return the common parameter and the exact Project destinations for one
/// reconstruction, when every constructor position is sourced once.
fn exact_reconstruction(
    block: &crate::ir::ArcBlock,
    construct_index: usize,
    construct_args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
) -> Option<(usize, FxHashSet<ArcVarId>)> {
    let mut common_param = None;
    let mut projections = FxHashSet::default();
    for (position, &carrier) in construct_args.iter().enumerate() {
        let expected_field = u32::try_from(position).ok()?;
        let projection = projection_for_carrier(
            block,
            construct_index,
            carrier,
            expected_field,
            sigs,
            exact_callables,
            interner,
        )?;
        let ArcInstr::Project { value, .. } = &block.body[projection.index] else {
            unreachable!("projection_for_carrier returns a Project index");
        };
        let params = alias_to_param.get(value)?;
        if params.len() != 1 {
            return None;
        }
        let param = *params.iter().next()?;
        if common_param.is_some_and(|common| common != param) {
            return None;
        }
        common_param = Some(param);
        if !projections.insert(projection.dst) {
            return None;
        }
    }
    Some((common_param?, projections))
}

#[derive(Clone, Copy)]
struct Projection {
    index: usize,
    dst: ArcVarId,
}

/// Trace a constructor carrier to a same-position Project, directly or
/// through one owned direct-call relay.
fn projection_for_carrier(
    block: &crate::ir::ArcBlock,
    construct_index: usize,
    carrier: ArcVarId,
    expected_field: u32,
    sigs: &FxHashMap<Name, MemoryContract>,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
) -> Option<Projection> {
    if let Some((index, dst)) = direct_projection(block, construct_index, carrier, expected_field) {
        return projection_has_only_use(block, index, dst, construct_index)
            .then_some(Projection { index, dst });
    }

    let (relay_index, relay) = block
        .body
        .iter()
        .enumerate()
        .find(|(_, instr)| instr.defined_var() == Some(carrier))?;
    if relay_index >= construct_index {
        return None;
    }
    let ArcInstr::Apply {
        func: callee,
        args,
        arg_ownership,
        ..
    } = relay
    else {
        return None;
    };
    let mut sources = args
        .iter()
        .enumerate()
        .filter(|&(position, _)| {
            arg_ownership.get(position).is_some_and(|&ownership| {
                crate::aims::builtins::effective_consuming_provenance(
                    *callee,
                    position,
                    ownership,
                    exact_callables.contains(callee),
                    sigs,
                    interner,
                )
            })
        })
        .filter_map(|(_, &arg)| {
            direct_projection(block, relay_index, arg, expected_field)
                .map(|(index, dst)| Projection { index, dst })
        });
    let projection = sources.next()?;
    if sources.next().is_some()
        || !projection_has_only_use(block, projection.index, projection.dst, relay_index)
        || !var_has_only_use(block, carrier, construct_index)
    {
        return None;
    }
    Some(projection)
}

fn direct_projection(
    block: &crate::ir::ArcBlock,
    before: usize,
    dst: ArcVarId,
    expected_field: u32,
) -> Option<(usize, ArcVarId)> {
    block
        .body
        .iter()
        .take(before)
        .enumerate()
        .find_map(|(index, instr)| match instr {
            ArcInstr::Project {
                dst: projection_dst,
                field,
                ..
            } if *projection_dst == dst && *field == expected_field => {
                Some((index, *projection_dst))
            }
            _ => None,
        })
}

fn projection_has_only_use(
    block: &crate::ir::ArcBlock,
    projection_index: usize,
    projection: ArcVarId,
    expected_use: usize,
) -> bool {
    projection_index < expected_use && var_has_only_use(block, projection, expected_use)
}

fn var_has_only_use(block: &crate::ir::ArcBlock, var: ArcVarId, expected_use: usize) -> bool {
    block
        .body
        .iter()
        .enumerate()
        .filter(|(_, instr)| instr.uses_var(var))
        .map(|(index, _)| index)
        .eq(std::iter::once(expected_use))
        && !block.terminator.uses_var(var)
}

/// No parameter alias may be used outside the selected Projects. This rejects
/// real aliases, incomplete reconstructions, and side observations.
fn parameter_uses_confined_to_projections(
    func: &ArcFunction,
    param: usize,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    projections: &FxHashSet<ArcVarId>,
) -> bool {
    let aliases: FxHashSet<ArcVarId> = alias_to_param
        .iter()
        .filter(|(_, params)| params.len() == 1 && params.contains(&param))
        .map(|(&var, _)| var)
        .collect();
    for block in &func.blocks {
        for instr in &block.body {
            for &alias in &aliases {
                if !instr.uses_var(alias) {
                    continue;
                }
                let permitted = matches!(
                    instr,
                    ArcInstr::Project { dst, value, .. }
                        if *value == alias && projections.contains(dst)
                );
                if !permitted {
                    return false;
                }
            }
        }
        if aliases
            .iter()
            .any(|&alias| block.terminator.uses_var(alias))
        {
            return false;
        }
    }
    true
}
