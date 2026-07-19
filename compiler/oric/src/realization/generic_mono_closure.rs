//! Fixed-point closure of generic targets exposed by lambda specialization.

use ori_ir::canon::{CanonResult, MonoInstanceId};
use ori_ir::{Name, StringInterner};
use ori_repr::monomorphize::{
    collect_mono_functions, concrete_sig_for_instance, mangle_mono_instance_name, ImportSig,
    MonoFunction, MonoFunctionIdentity, MonoFunctionOrigin,
};
use ori_types::{
    build_finalized_body_type_map, extend_var_subst_with_roots, extract_var_from_types,
    register_concrete_applied_resolutions, substitute_in_pool, AcceptedDerivedImpl,
    DerivedCallPlan, FunctionSig, GenericArg, Idx, ImplSig, MonoInstance, Pool, TypeEntry,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::generic_mono_discovery::collect_generic_uses;
use super::{lower_new_mono_functions_for_analysis, ArcFunctionGroup, MonoFunctionInventory};

/// Re-interned generic source template retained even before an instance exists.
#[derive(Clone, Debug)]
pub(crate) struct ImportedGenericTemplate {
    pub(crate) local_name: Name,
    pub(crate) signature: FunctionSig,
    pub(crate) module_index: usize,
    pub(crate) source_name: Name,
    /// Generic composite declarations owned by the provider module.
    pub(crate) generic_type_params: FxHashMap<Name, Vec<Name>>,
}

/// Complete inputs for closing generic calls hidden in specialized lambda bodies.
pub(crate) struct GenericMonoClosureInput<'a> {
    pub(crate) groups: Vec<ArcFunctionGroup>,
    pub(crate) mono_functions: Vec<MonoFunction>,
    pub(crate) mono_instances: &'a [MonoInstance],
    pub(crate) function_sigs: &'a [FunctionSig],
    pub(crate) local_generic_type_params: &'a FxHashMap<Name, Vec<Name>>,
    pub(crate) impl_sigs: &'a [ImplSig],
    pub(crate) accepted_derives: &'a [AcceptedDerivedImpl],
    pub(crate) derived_call_plans: &'a [DerivedCallPlan],
    pub(crate) import_sigs: &'a [ImportSig],
    pub(crate) imported_generic_templates: &'a [ImportedGenericTemplate],
    pub(crate) re_interned_canons: &'a [CanonResult],
    pub(crate) canon: &'a CanonResult,
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a mut Pool,
}

/// Canonical unspecialized groups plus the fixed-point mono inventory.
pub(crate) struct GenericMonoClosure {
    pub(crate) groups: Vec<ArcFunctionGroup>,
    pub(crate) mono_functions: Vec<MonoFunction>,
}

/// Failure while closing the pre-AIMS generic target inventory.
#[derive(Debug, thiserror::Error)]
pub(crate) enum GenericMonoClosureError {
    #[error(
        "generic target census lambda probe failed for {count} parent/lambda group(s): {errors:?}"
    )]
    LambdaSpecialization {
        count: usize,
        errors: Vec<ori_arc::LambdaSpecializationError>,
    },
    #[error("generic target census ARC lowering produced {count} problem(s): {problems:?}")]
    ArcLowering {
        count: usize,
        problems: Vec<ori_arc::ArcProblem>,
    },
    #[error("generic target census could not close the mono source inventory: {message}")]
    MonoInventory { message: String },
    #[error(
        "generic target census did not converge after {rounds} round(s) across {generic_functions} generic function(s); polymorphic recursion must reuse an existing exact specialization"
    )]
    NonConverging {
        rounds: usize,
        generic_functions: usize,
    },
    #[error(
        "generic target census cannot lower imported callable `{callable}` because module slot {module_index} has no re-interned canonical body"
    )]
    MissingImportedBody {
        callable: String,
        module_index: usize,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct GenericUse {
    pub(super) callee: Name,
    pub(super) param_types: Vec<Idx>,
    pub(super) return_type: Idx,
}

type InstanceKey = (Name, Vec<GenericArg>, Vec<Idx>);

#[derive(Clone, Copy)]
pub(super) struct GenericSignature<'a> {
    pub(super) signature: &'a FunctionSig,
    pub(super) imported: Option<&'a ImportedGenericTemplate>,
}

/// Close generic free-function targets revealed only after lambda types become concrete.
///
/// Specialization runs on cloned groups against the canonical pool read-only.
/// The canonical groups remain unspecialized until `LoweredArcBatch::prepare`;
/// every type selected by the probe was already interned by type checking.
pub(crate) fn close_generic_mono_targets(
    input: GenericMonoClosureInput<'_>,
) -> Result<GenericMonoClosure, GenericMonoClosureError> {
    let GenericMonoClosureInput {
        mut groups,
        mut mono_functions,
        mono_instances,
        function_sigs,
        local_generic_type_params,
        impl_sigs,
        accepted_derives,
        derived_call_plans,
        import_sigs,
        imported_generic_templates,
        re_interned_canons,
        canon,
        interner,
        pool,
    } = input;
    let (signatures, initial_imported_sources, mut instances, mut seen) =
        init_generic_signature_and_instances(
            function_sigs,
            imported_generic_templates,
            &mono_functions,
            mono_instances,
            interner,
        );
    let mut rounds = 0_usize;
    let mut inventory_reconciled = false;

    let inventory_sources = MonoInventorySources {
        function_sigs,
        impl_sigs,
        accepted_derives,
        import_sigs,
        signatures: &signatures,
        initial_imported_sources: &initial_imported_sources,
    };
    let body_sources = GenericBodyLoweringSources {
        accepted_derives,
        derived_call_plans,
        signatures: &signatures,
        re_interned_canons,
        canon,
        interner,
    };

    loop {
        let discovered = discover_new_instances(
            &groups,
            pool,
            interner,
            &signatures,
            local_generic_type_params,
            rounds,
        )?;
        let added = append_new_instances(&mut instances, &mut seen, discovered, interner, pool);
        tracing::debug!(
            round = rounds,
            added,
            instances = instances.len(),
            "materialized specialization-probe generic instances"
        );
        if added == 0 && inventory_reconciled {
            break;
        }
        inventory_reconciled = true;

        if added > 0 {
            rounds += 1;
            if rounds > signatures.len() {
                return Err(GenericMonoClosureError::NonConverging {
                    rounds,
                    generic_functions: signatures.len(),
                });
            }
        }

        mono_functions = recompute_mono_inventory(&instances, &inventory_sources, interner, pool)?;

        let selected = select_newly_discovered(&groups, &mono_functions, interner);
        if selected.is_empty() {
            continue;
        }
        lower_newly_discovered_bodies(
            &mut groups,
            &mono_functions,
            &selected,
            &body_sources,
            pool,
        )?;
    }

    Ok(GenericMonoClosure {
        groups,
        mono_functions,
    })
}

/// Static inputs to one `recompute_mono_inventory` call — unchanged across rounds.
struct MonoInventorySources<'a> {
    function_sigs: &'a [FunctionSig],
    impl_sigs: &'a [ImplSig],
    accepted_derives: &'a [AcceptedDerivedImpl],
    import_sigs: &'a [ImportSig],
    signatures: &'a FxHashMap<Name, GenericSignature<'a>>,
    initial_imported_sources: &'a [MonoFunction],
}

/// Static inputs to one `lower_newly_discovered_bodies` call — unchanged across rounds.
struct GenericBodyLoweringSources<'a> {
    accepted_derives: &'a [AcceptedDerivedImpl],
    derived_call_plans: &'a [DerivedCallPlan],
    signatures: &'a FxHashMap<Name, GenericSignature<'a>>,
    re_interned_canons: &'a [CanonResult],
    canon: &'a CanonResult,
    interner: &'a StringInterner,
}

/// Build the generic-signature census plus the initial fixed-point instance
/// set from the already-closed inputs, logging the census contents.
fn init_generic_signature_and_instances<'a>(
    function_sigs: &'a [FunctionSig],
    imported_generic_templates: &'a [ImportedGenericTemplate],
    mono_functions: &[MonoFunction],
    mono_instances: &[MonoInstance],
    interner: &StringInterner,
) -> (
    FxHashMap<Name, GenericSignature<'a>>,
    Vec<MonoFunction>,
    Vec<MonoInstance>,
    FxHashSet<InstanceKey>,
) {
    let signatures = generic_signature_census(function_sigs, imported_generic_templates);
    let initial_imported_sources: Vec<_> = mono_functions
        .iter()
        .filter(|function| function.is_imported)
        .cloned()
        .collect();
    let instances = mono_instances.to_vec();
    let seen: FxHashSet<InstanceKey> = instances
        .iter()
        .filter(|instance| instance.receiver_type.is_none())
        .map(instance_key)
        .collect();
    tracing::debug!(
        generic_signatures = signatures.len(),
        names = ?signatures
            .keys()
            .map(|name| interner.lookup(*name))
            .collect::<Vec<_>>(),
        "initialized generic signature census"
    );
    (signatures, initial_imported_sources, instances, seen)
}

/// Select the mangled names of mono functions not yet represented as a group.
fn select_newly_discovered(
    groups: &[ArcFunctionGroup],
    mono_functions: &[MonoFunction],
    interner: &StringInterner,
) -> FxHashSet<Name> {
    let existing: FxHashSet<Name> = groups.iter().map(ArcFunctionGroup::parent_name).collect();
    let selected: FxHashSet<Name> = mono_functions
        .iter()
        .map(|function| function.mangled_name)
        .filter(|name| !existing.contains(name))
        .collect();
    tracing::debug!(
        selected = ?selected
            .iter()
            .map(|name| interner.lookup(*name))
            .collect::<Vec<_>>(),
        existing = groups.len(),
        "selected newly discovered mono bodies"
    );
    selected
}

/// Run one specialization probe and materialize the generic-use instances it reveals.
fn discover_new_instances(
    groups: &[ArcFunctionGroup],
    pool: &mut Pool,
    interner: &StringInterner,
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    local_generic_type_params: &FxHashMap<Name, Vec<Name>>,
    rounds: usize,
) -> Result<Vec<MonoInstance>, GenericMonoClosureError> {
    let probe_groups = specialized_probe(groups, pool, interner)?;
    let uses = collect_generic_uses(&probe_groups, signatures, pool);
    tracing::debug!(
        round = rounds,
        uses = uses.len(),
        callees = ?uses
            .iter()
            .map(|generic_use| interner.lookup(generic_use.callee))
            .collect::<Vec<_>>(),
        "scanned specialization probe"
    );
    let mut discovered = Vec::new();
    for generic_use in uses {
        let Some(source) = signatures.get(&generic_use.callee).copied() else {
            continue;
        };
        tracing::debug!(
            callee = interner.lookup(generic_use.callee),
            observed_params = generic_use.param_types.len(),
            declared_params = source.signature.param_types.len(),
            value_capabilities = source
                .signature
                .capability_params
                .iter()
                .filter(|param| param.is_value())
                .count(),
            scheme_vars = source.signature.scheme_var_ids.len(),
            "materializing specialization-probe use"
        );
        let generic_type_params = source
            .imported
            .map_or(local_generic_type_params, |template| {
                &template.generic_type_params
            });
        if let Some(instance) =
            materialize_instance(source.signature, &generic_use, pool, generic_type_params)
        {
            discovered.push(instance);
        }
    }
    Ok(discovered)
}

/// Recompute the complete mono-function inventory from the current instance set.
fn recompute_mono_inventory(
    instances: &[MonoInstance],
    sources: &MonoInventorySources<'_>,
    interner: &StringInterner,
    pool: &mut Pool,
) -> Result<Vec<MonoFunction>, GenericMonoClosureError> {
    let collected = collect_mono_functions(
        instances,
        sources.function_sigs,
        sources.impl_sigs,
        sources.accepted_derives,
        sources.import_sigs,
        interner,
        pool,
    );
    let imported_sources = collect_imported_mono_functions(
        instances,
        sources.signatures,
        sources.initial_imported_sources,
        interner,
        pool,
    )?;
    Ok(
        MonoFunctionInventory::try_new(collected, imported_sources, interner)
            .map_err(|error| GenericMonoClosureError::MonoInventory {
                message: error.to_string(),
            })?
            .into_all(),
    )
}

/// Lower ARC bodies for every newly `selected` mono function and append them to `groups`.
fn lower_newly_discovered_bodies(
    groups: &mut Vec<ArcFunctionGroup>,
    mono_functions: &[MonoFunction],
    selected: &FxHashSet<Name>,
    sources: &GenericBodyLoweringSources<'_>,
    pool: &mut Pool,
) -> Result<(), GenericMonoClosureError> {
    let mut problems = Vec::new();
    let mut new_groups_context = crate::arc_lowering::ArcLoweringContext {
        canon: sources.canon,
        interner: sources.interner,
        pool,
        problems: &mut problems,
    };
    let mut new_groups = lower_new_mono_functions_for_analysis(
        mono_functions,
        selected,
        sources.accepted_derives,
        sources.derived_call_plans,
        &mut new_groups_context,
    );
    for mono in mono_functions
        .iter()
        .filter(|mono| mono.is_imported && selected.contains(&mono.mangled_name))
    {
        let Some(source) = sources
            .signatures
            .get(&mono.identity.original_name())
            .and_then(|entry| entry.imported)
        else {
            continue;
        };
        let Some(source_canon) = sources.re_interned_canons.get(source.module_index) else {
            return Err(GenericMonoClosureError::MissingImportedBody {
                callable: sources.interner.lookup(source.local_name).to_owned(),
                module_index: source.module_index,
            });
        };
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: source_canon,
            interner: sources.interner,
            pool,
            problems: &mut problems,
        };
        let (function, lambdas) = crate::arc_lowering::lower_to_arc(
            mono.mangled_name,
            &mono.sig,
            source.source_name,
            &mut context,
            Some(&mono.body_type_map),
        );
        new_groups.push(ArcFunctionGroup::new(function, lambdas));
    }
    if !problems.is_empty() {
        return Err(GenericMonoClosureError::ArcLowering {
            count: problems.len(),
            problems,
        });
    }
    tracing::debug!(
        new_groups = ?new_groups
            .iter()
            .map(|group| sources.interner.lookup(group.parent_name()))
            .collect::<Vec<_>>(),
        "lowered newly discovered mono bodies"
    );
    groups.extend(new_groups);
    Ok(())
}

fn generic_signature_census<'a>(
    functions: &'a [FunctionSig],
    imports: &'a [ImportedGenericTemplate],
) -> FxHashMap<Name, GenericSignature<'a>> {
    let mut signatures = FxHashMap::default();
    let local_names: FxHashSet<_> = functions.iter().map(|signature| signature.name).collect();
    for signature in functions.iter().filter(|signature| {
        signature.requires_specialization() && signature.const_params.is_empty()
    }) {
        signatures
            .entry(signature.name)
            .or_insert(GenericSignature {
                signature,
                imported: None,
            });
    }
    for import in imports.iter().filter(|import| {
        !local_names.contains(&import.local_name)
            && import.signature.requires_specialization()
            && import.signature.const_params.is_empty()
    }) {
        signatures
            .entry(import.local_name)
            .or_insert(GenericSignature {
                signature: &import.signature,
                imported: Some(import),
            });
    }
    signatures
}

fn specialized_probe(
    groups: &[ArcFunctionGroup],
    pool: &Pool,
    interner: &StringInterner,
) -> Result<Vec<ArcFunctionGroup>, GenericMonoClosureError> {
    let mut specialized = Vec::with_capacity(groups.len());
    let mut errors = Vec::new();
    for group in groups.iter().cloned() {
        let (mut parent, mut lambdas) = group.into_parts();
        if let Err(error) =
            ori_arc::specialize_polymorphic_lambdas(&mut parent, &mut lambdas, pool, interner)
        {
            errors.push(error);
        }
        specialized.push(ArcFunctionGroup::new(parent, lambdas));
    }
    if errors.is_empty() {
        Ok(specialized)
    } else {
        Err(GenericMonoClosureError::LambdaSpecialization {
            count: errors.len(),
            errors,
        })
    }
}

fn collect_imported_mono_functions(
    instances: &[MonoInstance],
    signatures: &FxHashMap<Name, GenericSignature<'_>>,
    initial: &[MonoFunction],
    interner: &StringInterner,
    pool: &Pool,
) -> Result<Vec<MonoFunction>, GenericMonoClosureError> {
    let mut functions = initial.to_vec();
    let mut by_name: FxHashMap<Name, usize> = functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.mangled_name, index))
        .collect();

    for (index, instance) in instances.iter().enumerate() {
        if instance.receiver_type.is_some() || instance.method_producer.is_some() {
            continue;
        }
        let Some(source) = signatures
            .get(&instance.fn_name)
            .filter(|source| source.imported.is_some())
        else {
            continue;
        };
        let Ok(raw_instance_id) = u32::try_from(index) else {
            unreachable!("mono-instance table exceeds the u32 dispatch-ID domain");
        };
        let instance_id = MonoInstanceId::new(raw_instance_id);
        let mangled_name = mangle_mono_instance_name(instance, interner, pool);
        let concrete_sig =
            concrete_sig_for_instance(instance, source.signature, pool, mangled_name);
        let candidate = MonoFunction {
            mangled_name,
            origin: MonoFunctionOrigin::Source,
            identity: MonoFunctionIdentity::new(instance, instance_id),
            sig: concrete_sig,
            body_type_map: instance.body_type_map.iter().copied().collect(),
            is_imported: true,
            receiver_type_name: None,
        };
        if let Some(&existing_index) = by_name.get(&mangled_name) {
            let existing = &mut functions[existing_index];
            if existing.identity.original_name() != candidate.identity.original_name()
                || !concrete_signatures_match(&existing.sig, &candidate.sig, pool)
            {
                return Err(GenericMonoClosureError::MonoInventory {
                    message: format!(
                        "imported template for `{}` disagrees with its existing concrete identity",
                        interner.lookup(instance.fn_name)
                    ),
                });
            }
            if !existing.identity.instance_ids().contains(&instance_id) {
                existing.identity.push_instance_id(instance_id);
            }
        } else {
            by_name.insert(mangled_name, functions.len());
            functions.push(candidate);
        }
    }
    Ok(functions)
}

/// Compare one final executable signature while treating raw pool coordinates
/// and their Merkle hashes as derived fields. The mangled callable identity is
/// already equal at this seam; every other signature field remains exact.
fn concrete_signatures_match(existing: &FunctionSig, candidate: &FunctionSig, pool: &Pool) -> bool {
    if existing.param_types.len() != candidate.param_types.len()
        || !existing
            .param_types
            .iter()
            .zip(&candidate.param_types)
            .all(|(&left, &right)| pool.structural_eq(left, right))
        || !pool.structural_eq(existing.return_type, candidate.return_type)
    {
        return false;
    }

    let mut candidate = candidate.clone();
    candidate.param_types.clone_from(&existing.param_types);
    candidate.return_type = existing.return_type;
    candidate.param_hashes.clone_from(&existing.param_hashes);
    candidate.return_hash = existing.return_hash;
    existing == &candidate
}

fn materialize_instance(
    signature: &FunctionSig,
    generic_use: &GenericUse,
    pool: &mut Pool,
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) -> Option<MonoInstance> {
    let value_capabilities: Vec<_> = signature
        .capability_params
        .iter()
        .copied()
        .filter_map(|param| match param {
            ori_types::CapabilityParam::Value {
                provider_type,
                provider_var_id,
                ..
            } => Some((provider_type, provider_var_id)),
            ori_types::CapabilityParam::Marker { .. } => None,
        })
        .collect();
    let source_param_count = signature.param_types.len();
    if signature.scheme_var_ids.len() != signature.type_params.len()
        || source_param_count + value_capabilities.len() != generic_use.param_types.len()
    {
        return None;
    }
    let (actual_params, capability_args, actual_return) =
        canonical_actual_use_types(pool, generic_use, source_param_count)?;

    let var_subst = unify_scheme_var_substitution(
        pool,
        signature,
        &value_capabilities,
        &actual_params,
        &capability_args,
        actual_return,
    )?;

    let (concrete_params, concrete_return) = concretize_and_verify_signature(
        pool,
        signature,
        &var_subst,
        &actual_params,
        actual_return,
    )?;
    let generic_args = signature
        .scheme_var_ids
        .iter()
        .map(|var_id| GenericArg::Type(var_subst[var_id]))
        .collect();
    let body_type_map = build_finalized_body_type_map(pool, &var_subst, &[]);
    register_concrete_applied_resolutions(pool, &body_type_map, generic_type_params);
    Some(MonoInstance::new_top_level_with_capabilities(
        generic_use.callee,
        generic_args,
        capability_args,
        concrete_params,
        concrete_return,
        body_type_map,
    ))
}

/// Read a specialization-probe use's canonical param/capability/return types,
/// rejecting the use if any type is not recordable in a mono instance.
fn canonical_actual_use_types(
    pool: &Pool,
    generic_use: &GenericUse,
    source_param_count: usize,
) -> Option<(Vec<Idx>, Vec<Idx>, Idx)> {
    let actual_params: Vec<_> = generic_use
        .param_types
        .iter()
        .take(source_param_count)
        .copied()
        .collect();
    let capability_args: Vec<_> = generic_use
        .param_types
        .iter()
        .skip(source_param_count)
        .copied()
        .collect();
    let actual_return = generic_use.return_type;
    let has_unrecordable_type = actual_params
        .iter()
        .chain(&capability_args)
        .chain(std::iter::once(&actual_return))
        .any(|&ty| !pool.flags(ty).is_recordable());
    if has_unrecordable_type {
        tracing::debug!(
            parameter_flags = ?actual_params
                .iter()
                .map(|&ty| pool.flags(ty))
                .collect::<Vec<_>>(),
            capability_flags = ?capability_args
                .iter()
                .map(|&ty| pool.flags(ty))
                .collect::<Vec<_>>(),
            return_flags = ?pool.flags(actual_return),
            "rejected specialization-probe use with non-recordable types"
        );
        return None;
    }
    Some((actual_params, capability_args, actual_return))
}

/// Unify the signature's scheme variables against the actual param/return
/// types the probe observed, extending the substitution with value-capability
/// providers and their dominance roots.
fn unify_scheme_var_substitution(
    pool: &mut Pool,
    signature: &FunctionSig,
    value_capabilities: &[(Idx, u32)],
    actual_params: &[Idx],
    capability_args: &[Idx],
    actual_return: Idx,
) -> Option<FxHashMap<u32, Idx>> {
    let mut var_subst = FxHashMap::default();
    for &var_id in &signature.scheme_var_ids {
        let mut bindings = signature
            .param_types
            .iter()
            .zip(actual_params)
            .filter_map(|(&schema, &actual)| extract_var_from_types(pool, schema, actual, var_id));
        let first = bindings.next().or_else(|| {
            extract_var_from_types(pool, signature.return_type, actual_return, var_id)
        })?;
        if bindings.any(|binding| !pool.structural_eq(first, binding)) {
            return None;
        }
        var_subst.insert(var_id, pool.resolve_fully(first));
    }
    for ((_, provider_var_id), &actual) in value_capabilities.iter().zip(capability_args) {
        var_subst.insert(*provider_var_id, pool.resolve_fully(actual));
    }
    let retained_var_ids: Vec<_> = signature
        .scheme_var_ids
        .iter()
        .copied()
        .chain(
            value_capabilities
                .iter()
                .map(|(_, provider_var_id)| *provider_var_id),
        )
        .collect();
    extend_var_subst_with_roots(pool, &retained_var_ids, &mut var_subst);
    Some(var_subst)
}

/// Substitute the concrete param/return types and verify they structurally
/// agree with what the probe observed.
fn concretize_and_verify_signature(
    pool: &mut Pool,
    signature: &FunctionSig,
    var_subst: &FxHashMap<u32, Idx>,
    actual_params: &[Idx],
    actual_return: Idx,
) -> Option<(Vec<Idx>, Idx)> {
    let concrete_params: Vec<_> = signature
        .param_types
        .iter()
        .map(|&ty| substitute_in_pool(pool, ty, var_subst))
        .collect();
    let concrete_return = if signature.return_projection.is_some() {
        actual_return
    } else {
        substitute_in_pool(pool, signature.return_type, var_subst)
    };
    let parameters_match = concrete_params
        .iter()
        .zip(actual_params)
        .all(|(&expected, &actual)| pool.structural_eq(expected, actual));
    let return_matches = pool.structural_eq(concrete_return, actual_return);
    if !parameters_match || !return_matches {
        tracing::debug!(
            parameters_match,
            return_matches,
            expected_return_hash = pool.hash(concrete_return),
            actual_return_hash = pool.hash(actual_return),
            "rejected specialization-probe use whose concrete signature disagrees"
        );
        return None;
    }
    Some((concrete_params, concrete_return))
}

/// Collect generic-composite binders needed to materialize concrete `Applied`
/// bodies discovered after the checker has finished.
pub(crate) fn generic_type_param_map(types: &[TypeEntry]) -> FxHashMap<Name, Vec<Name>> {
    types
        .iter()
        .filter(|entry| !entry.type_params.is_empty())
        .map(|entry| (entry.name, entry.type_params.clone()))
        .collect()
}

fn append_new_instances(
    instances: &mut Vec<MonoInstance>,
    seen: &mut FxHashSet<InstanceKey>,
    mut discovered: Vec<MonoInstance>,
    interner: &StringInterner,
    pool: &Pool,
) -> usize {
    tracing::debug!(
        discovered = discovered.len(),
        names = ?discovered
            .iter()
            .map(|instance| interner.lookup(instance.fn_name))
            .collect::<Vec<_>>(),
        "deduplicating specialization-probe instances"
    );
    discovered.sort_by(|left, right| {
        instance_sort_key(left, interner, pool).cmp(&instance_sort_key(right, interner, pool))
    });
    let before = instances.len();
    for instance in discovered {
        if seen.insert(instance_key(&instance)) {
            instances.push(instance);
        }
    }
    instances.len() - before
}

fn instance_key(instance: &MonoInstance) -> InstanceKey {
    (
        instance.fn_name,
        instance.generic_args.clone(),
        instance.capability_args.clone(),
    )
}

fn instance_sort_key(
    instance: &MonoInstance,
    interner: &StringInterner,
    pool: &Pool,
) -> (String, Vec<(u64, u32)>) {
    let arguments = instance
        .generic_args
        .iter()
        .filter_map(|argument| match argument {
            GenericArg::Type(ty) => Some((pool.hash(*ty), ty.raw())),
            GenericArg::Const(_) => None,
        })
        .chain(
            instance
                .capability_args
                .iter()
                .map(|&ty| (pool.hash(ty), ty.raw())),
        )
        .collect();
    (interner.lookup(instance.fn_name).to_owned(), arguments)
}

#[cfg(test)]
#[path = "generic_mono_closure_tests.rs"]
mod tests;
