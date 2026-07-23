//! Canonical exact aggregate-reconstruction ownership transfer recognition.
//!
//! One pass produces both outputs of the proof:
//!
//! - the caller-visible, field-sensitive contract projection when the source
//!   aggregate is a function parameter; and
//! - the immutable local witness consumed by class-ledger realization.
//!
//! A projected field may cross one effectively-owned call before entering the
//! rebuilt aggregate (the COW `xs.push(v)` shape). Every constructor position
//! must be supplied by the matching projection from one aggregate, directly or
//! through one linear relay. Cleanup authority is established here, before the
//! contract enters SCC convergence.

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{
    CleanupAuthority, ExactAggregateTransfer, ExactFieldPath, ExactFieldTransfer,
    ExactFieldTransferKind, ExactTransferState, MemoryContract, ResidualDisposition,
};
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcVarId, CtorKind};
use crate::ArcClassification;

/// One immutable field route in the local realization witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactFieldTransferWitness {
    /// Typed semantic path from the source aggregate.
    pub(crate) path: ExactFieldPath,
    /// Aggregate variable read by the projection.
    pub(crate) source: ArcVarId,
    /// Stable projection result identity.
    pub(crate) projection_dst: ArcVarId,
    /// Stable value identity entering the matching constructor position.
    pub(crate) carrier: ArcVarId,
    /// Whether the path is direct or crosses one effectively-owned relay.
    pub(crate) kind: ExactFieldTransferKind,
}

/// Immutable local proof consumed after intraprocedural convergence.
///
/// Stable block and variable identities deliberately replace instruction
/// offsets. The ledger resolves current offsets from those identities after
/// normalization, and declines if any identity no longer matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactAggregateTransferWitness {
    /// Parameter projected into the caller-visible contract, when any.
    pub(crate) param: Option<usize>,
    /// Block containing the reconstruction.
    pub(crate) block: ArcBlockId,
    /// Stable destination identity of the rebuilding constructor.
    pub(crate) construct_dst: ArcVarId,
    /// All matching projection routes, including scalar residual positions.
    pub(crate) fields: Box<[ExactFieldTransferWitness]>,
}

/// Canonical recognition output for one function.
pub(super) struct ExactAggregateTransferFacts {
    /// Per-parameter flat-lattice projection.
    pub(super) states: FxHashMap<usize, ExactTransferState>,
    /// Parameters whose boundary must transfer the whole aggregate owner.
    pub(super) consumed_params: FxHashSet<usize>,
    /// Local witnesses whose proof agrees with the published parameter state.
    pub(super) witnesses: Vec<ExactAggregateTransferWitness>,
}

/// Recognize every exact reconstruction once.
#[expect(
    clippy::too_many_arguments,
    reason = "canonical recognition consumes the complete frozen analysis authority"
)]
pub(super) fn find_exact_aggregate_transfers(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    classifier: &dyn ArcClassification,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
    type_registry: Option<&TypeRegistry>,
    context_regions_present: bool,
) -> ExactAggregateTransferFacts {
    // TRMC rewrites structural identities after contract extraction. Until a
    // rewrite-stable witness exists, any detected context region is Unproven.
    if context_regions_present {
        return ExactAggregateTransferFacts {
            states: FxHashMap::default(),
            consumed_params: FxHashSet::default(),
            witnesses: Vec::new(),
        };
    }

    let authority = ExactTransferAuthority {
        sigs,
        alias_to_param,
        classifier,
        exact_callables,
        interner,
        type_registry,
    };
    let candidates: Vec<ExactTransferCandidate> = func
        .blocks
        .iter()
        .filter_map(|block| recognize_block_candidate(func, block, &authority))
        .collect();

    let mut states = FxHashMap::default();
    let mut consumed_params = FxHashSet::default();
    for (proof, witness) in &candidates {
        let Some(param) = witness.param else {
            continue;
        };
        if !parameter_uses_confined_to_reconstruction(func, param, alias_to_param, witness) {
            continue;
        }
        consumed_params.insert(param);
        let state = ExactTransferState::exact(proof.clone());
        states
            .entry(param)
            .and_modify(|current: &mut ExactTransferState| {
                *current = current.join(&state);
            })
            .or_insert(state);
    }

    let witnesses = candidates
        .into_iter()
        .filter_map(|(proof, witness)| {
            let Some(param) = witness.param else {
                return Some(witness);
            };
            matches!(
                states.get(&param),
                Some(ExactTransferState::Exact(published)) if published.as_ref() == &proof
            )
            .then_some(witness)
        })
        .collect();

    ExactAggregateTransferFacts {
        states,
        consumed_params,
        witnesses,
    }
}

type ExactTransferCandidate = (ExactAggregateTransfer, ExactAggregateTransferWitness);

struct ExactTransferAuthority<'a> {
    sigs: &'a FxHashMap<Name, MemoryContract>,
    alias_to_param: &'a FxHashMap<ArcVarId, FxHashSet<usize>>,
    classifier: &'a dyn ArcClassification,
    exact_callables: &'a FxHashSet<Name>,
    interner: &'a ori_ir::StringInterner,
    type_registry: Option<&'a TypeRegistry>,
}

fn recognize_block_candidate(
    func: &ArcFunction,
    block: &ArcBlock,
    authority: &ExactTransferAuthority<'_>,
) -> Option<ExactTransferCandidate> {
    let mut candidates = Vec::new();
    for (construct_index, instr) in block.body.iter().enumerate() {
        let ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args: construct_args,
        } = instr
        else {
            continue;
        };
        if construct_args.is_empty() || matches!(ctor, CtorKind::EnumVariant { .. }) {
            continue;
        }
        let Some(routes) = exact_reconstruction(
            block,
            construct_index,
            construct_args,
            authority.sigs,
            authority.exact_callables,
            authority.interner,
        ) else {
            continue;
        };
        let Some(source_ty) = routes
            .first()
            .and_then(|route| func.var_types.get(route.source.index()))
            .copied()
        else {
            continue;
        };
        if source_ty != *ty
            || routes
                .iter()
                .any(|route| func.var_types.get(route.source.index()).copied() != Some(source_ty))
            || !source_uses_confined_to_routes(block, &routes)
            || !cleanup_is_supported(func, source_ty, &routes, authority.type_registry)
        {
            continue;
        }

        let managed_fields: Vec<ExactFieldTransfer> = routes
            .iter()
            .filter_map(|route| {
                let member_ty = *func.var_types.get(route.projection_dst.index())?;
                (!authority.classifier.is_scalar(member_ty)).then_some(ExactFieldTransfer {
                    path: route.path,
                    kind: route.kind,
                })
            })
            .collect();
        let proof = ExactAggregateTransfer::new(
            managed_fields,
            ResidualDisposition::FullyReconstructed,
            CleanupAuthority::OrdinaryCleanupProven,
        )?;
        let param = common_parameter(&routes, authority.alias_to_param);
        candidates.push((
            proof,
            ExactAggregateTransferWitness {
                param,
                block: block.id,
                construct_dst: *dst,
                fields: routes
                    .into_iter()
                    .map(|route| ExactFieldTransferWitness {
                        path: route.path,
                        source: route.source,
                        projection_dst: route.projection_dst,
                        carrier: route.carrier,
                        kind: route.kind,
                    })
                    .collect(),
            },
        ));
    }

    // The ledger books at most one aggregate move per block. Multiple
    // candidate reconstructions make the local residual ambiguous.
    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

#[derive(Clone)]
struct ProjectionRoute {
    path: ExactFieldPath,
    source: ArcVarId,
    projection_index: usize,
    projection_dst: ArcVarId,
    carrier: ArcVarId,
    kind: ExactFieldTransferKind,
}

/// Return exact same-position routes for one reconstruction.
fn exact_reconstruction(
    block: &ArcBlock,
    construct_index: usize,
    construct_args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
) -> Option<Vec<ProjectionRoute>> {
    let mut routes = Vec::with_capacity(construct_args.len());
    let mut projection_dsts = FxHashSet::default();
    for (position, &carrier) in construct_args.iter().enumerate() {
        let expected_field = u32::try_from(position).ok()?;
        let route = projection_for_carrier(
            block,
            construct_index,
            carrier,
            expected_field,
            sigs,
            exact_callables,
            interner,
        )?;
        if !projection_dsts.insert(route.projection_dst) {
            return None;
        }
        routes.push(route);
    }
    Some(routes)
}

/// Trace a constructor carrier to a same-position projection, directly or
/// through one effectively-owned direct-call relay.
fn projection_for_carrier(
    block: &ArcBlock,
    construct_index: usize,
    carrier: ArcVarId,
    expected_field: u32,
    sigs: &FxHashMap<Name, MemoryContract>,
    exact_callables: &FxHashSet<Name>,
    interner: &ori_ir::StringInterner,
) -> Option<ProjectionRoute> {
    if let Some((index, source, dst)) =
        direct_projection(block, construct_index, carrier, expected_field)
    {
        return projection_has_only_use(block, index, dst, construct_index).then(|| {
            ProjectionRoute {
                path: ExactFieldPath::single(expected_field),
                source,
                projection_index: index,
                projection_dst: dst,
                carrier,
                kind: ExactFieldTransferKind::DirectMove,
            }
        });
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
            direct_projection(block, relay_index, arg, expected_field).map(
                |(index, source, dst)| ProjectionRoute {
                    path: ExactFieldPath::single(expected_field),
                    source,
                    projection_index: index,
                    projection_dst: dst,
                    carrier,
                    kind: ExactFieldTransferKind::EffectiveOwnedRelay,
                },
            )
        });
    let route = sources.next()?;
    if sources.next().is_some()
        || !projection_has_only_use(
            block,
            route.projection_index,
            route.projection_dst,
            relay_index,
        )
        || !var_has_only_use(block, carrier, construct_index)
    {
        return None;
    }
    Some(route)
}

fn direct_projection(
    block: &ArcBlock,
    before: usize,
    dst: ArcVarId,
    expected_field: u32,
) -> Option<(usize, ArcVarId, ArcVarId)> {
    block
        .body
        .iter()
        .take(before)
        .enumerate()
        .find_map(|(index, instr)| match instr {
            ArcInstr::Project {
                dst: projection_dst,
                value,
                field,
                ..
            } if *projection_dst == dst && *field == expected_field => {
                Some((index, *value, *projection_dst))
            }
            _ => None,
        })
}

fn projection_has_only_use(
    block: &ArcBlock,
    projection_index: usize,
    projection: ArcVarId,
    expected_use: usize,
) -> bool {
    projection_index < expected_use && var_has_only_use(block, projection, expected_use)
}

fn var_has_only_use(block: &ArcBlock, var: ArcVarId, expected_use: usize) -> bool {
    block
        .body
        .iter()
        .enumerate()
        .filter(|(_, instr)| instr.uses_var(var))
        .map(|(index, _)| index)
        .eq(std::iter::once(expected_use))
        && !block.terminator.uses_var(var)
}

fn source_uses_confined_to_routes(block: &ArcBlock, routes: &[ProjectionRoute]) -> bool {
    let sources: FxHashSet<ArcVarId> = routes.iter().map(|route| route.source).collect();
    for (index, instr) in block.body.iter().enumerate() {
        for &source in &sources {
            if !instr.uses_var(source) {
                continue;
            }
            let permitted = matches!(
                instr,
                ArcInstr::Project { dst, value, .. }
                    if *value == source
                        && routes.iter().any(|route| {
                            route.projection_dst == *dst
                                && route.projection_index == index
                        })
            );
            if !permitted {
                return false;
            }
        }
    }
    sources
        .iter()
        .all(|&source| !block.terminator.uses_var(source))
}

fn cleanup_is_supported(
    func: &ArcFunction,
    source_ty: ori_types::Idx,
    routes: &[ProjectionRoute],
    type_registry: Option<&TypeRegistry>,
) -> bool {
    let Some(type_registry) = type_registry else {
        // Test-only extraction helpers predate registry authority. Production
        // always supplies it through the whole-program pipeline.
        return cfg!(test);
    };
    if type_registry.enum_variants(source_ty).is_some()
        || crate::lower::burden_lookup::type_has_user_drop(source_ty, type_registry)
    {
        return false;
    }
    routes.iter().all(|route| {
        func.var_types
            .get(route.projection_dst.index())
            .is_some_and(|&member_ty| {
                !crate::lower::burden_lookup::type_has_user_drop(member_ty, type_registry)
            })
    })
}

fn common_parameter(
    routes: &[ProjectionRoute],
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> Option<usize> {
    let mut common = None;
    for route in routes {
        let params = alias_to_param.get(&route.source)?;
        if params.len() != 1 {
            return None;
        }
        let param = *params.iter().next()?;
        if common.is_some_and(|known| known != param) {
            return None;
        }
        common = Some(param);
    }
    common
}

/// No parameter alias may be used outside the selected projections.
///
/// This contract-side guard is intentionally stricter than the local ledger
/// validation. It rejects a surviving parent, real alias, or second
/// reconstruction before caller ownership is changed.
fn parameter_uses_confined_to_reconstruction(
    func: &ArcFunction,
    param: usize,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    witness: &ExactAggregateTransferWitness,
) -> bool {
    let aliases: FxHashSet<ArcVarId> = alias_to_param
        .iter()
        .filter(|(_, params)| params.len() == 1 && params.contains(&param))
        .map(|(&var, _)| var)
        .collect();
    let selected: FxHashSet<ArcVarId> = witness
        .fields
        .iter()
        .map(|field| field.projection_dst)
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
                        if *value == alias && selected.contains(dst)
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
