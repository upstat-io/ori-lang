//! ARC lowering and dispatch registration for declared impl methods.

use ori_ir::canon::CanonResult;
use ori_types::{Idx, ImplMethodId, ImplSig, Pool, TypeCheckResult};
use oric::ir::{Name, StringInterner};
use oric::parser::ParseOutput;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    method_receiver_key, DispatchTier, ImplLoweringInputs, ImplLoweringOutputs, ImplMethodAnalysis,
};

/// ARC-lower impl methods for closed-program representation and AIMS analysis.
///
/// The repr plan and the single closed AIMS batch both need every executable
/// impl-method body and call site, not just top-level functions. This lowers
/// each non-generic impl method (including default trait methods used in impl
/// blocks) to ARC IR with a stable type-qualified identity.
///
/// Returns the lowered `ArcFunction` values plus a
/// `(self_type_idx, method_name) -> qualified_name` map. The qualified
/// identity joins impl call sites to their bodies before the closed batch is
/// realized, so no backend or per-function repair path invents contracts.
///
/// The realized bodies become executable-artifact members and are the bodies
/// consumed by physical backends.
#[must_use = "success or failure must be handled"]
pub(crate) fn lower_impl_methods_for_analysis(
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    interner: &StringInterner,
    canon: &CanonResult,
    pool: &Pool,
) -> Result<ImplMethodAnalysis, Vec<ori_arc::ArcProblem>> {
    let mut outputs = ImplLoweringOutputs::default();
    let mut sig_by_id: FxHashMap<ImplMethodId, &ImplSig> = FxHashMap::default();
    for sig in &type_result.typed.impl_sigs {
        if sig_by_id.insert(sig.id, sig).is_some() {
            outputs.problems.push(ori_arc::ArcProblem::InternalError {
                message: format!("duplicate typed impl-method identity {:?}", sig.id),
                span: ori_ir::Span::DUMMY,
            });
        }
    }
    let inputs = ImplLoweringInputs {
        parse_result,
        interner,
        canon,
        pool,
        sig_by_id,
    };
    for (impl_index, impl_def) in parse_result.module.impls.iter().enumerate() {
        let type_name_name = impl_def.semantic_type_name(interner);
        lower_declared_impl_methods(impl_def, impl_index, type_name_name, &inputs, &mut outputs);
        lower_default_trait_methods(impl_def, impl_index, type_name_name, &inputs, &mut outputs);
    }
    for (extension_index, extension) in parse_result.module.extends.iter().enumerate() {
        lower_extension_methods(
            extension,
            parse_result.module.impls.len() + extension_index,
            &inputs,
            &mut outputs,
        );
    }
    for sig in &type_result.typed.impl_sigs {
        if !outputs.consumed_sig_ids.contains(&sig.id) {
            outputs.problems.push(ori_arc::ArcProblem::InternalError {
                message: format!("unconsumed typed impl-method identity {:?}", sig.id),
                span: ori_ir::Span::DUMMY,
            });
        }
    }
    if outputs.problems.is_empty() {
        Ok(ImplMethodAnalysis {
            groups: outputs.groups,
            targets: outputs.dispatch.targets,
            producer_targets: outputs.producer_targets,
            user_drop_bindings: outputs.user_drop_bindings,
            emission_names: outputs.emission_names,
        })
    } else {
        Err(outputs.problems)
    }
}

fn lower_extension_methods(
    extension: &ori_ir::ExtendDef,
    owner_index: usize,
    inputs: &ImplLoweringInputs<'_>,
    outputs: &mut ImplLoweringOutputs,
) {
    let self_kw = inputs.interner.intern("self");
    for method in &extension.methods {
        if inputs
            .parse_result
            .arena
            .get_params(method.params)
            .first()
            .is_none_or(|param| param.name != self_kw)
        {
            continue;
        }

        let method_id = ImplMethodId::new(owner_index, method.body);
        let Some(ori_types::ImplSig { receiver, sig, .. }) =
            inputs.sig_by_id.get(&method_id).copied()
        else {
            outputs.problems.push(ori_arc::ArcProblem::InternalError {
                message: format!("missing typed extension-method identity {method_id:?}"),
                span: method.span,
            });
            outputs.emission_names.push(None);
            continue;
        };
        outputs.consumed_sig_ids.insert(method_id);
        if sig.is_generic() {
            outputs.emission_names.push(None);
            continue;
        }

        let (_, qualified_name) = make_qualified_name(
            Some(method_receiver_key(inputs.pool, *receiver)),
            method.name,
            inputs.interner,
            &mut outputs.method_ordinals,
        );
        outputs.dispatch.record_extension(
            Some(method_receiver_key(inputs.pool, *receiver)),
            method.name,
            qualified_name,
        );
        record_producer_target(outputs, method_id, qualified_name, method.span);
        outputs.emission_names.push(Some(qualified_name));
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: inputs.canon,
            interner: inputs.interner,
            pool: inputs.pool,
            problems: &mut outputs.problems,
        };
        let (arc_fn, lambdas) = crate::arc_lowering::lower_impl_method_to_arc_by_source(
            qualified_name,
            sig,
            method.body,
            &mut context,
            None,
        );
        outputs
            .groups
            .push(super::super::ArcFunctionGroup::new(arc_fn, lambdas));
    }
}

fn lower_declared_impl_methods(
    impl_def: &ori_ir::ImplDef,
    impl_index: usize,
    type_name_name: Option<Name>,
    inputs: &ImplLoweringInputs<'_>,
    outputs: &mut ImplLoweringOutputs,
) {
    for method in &impl_def.methods {
        let method_id = ImplMethodId::new(impl_index, method.body);
        let Some(ori_types::ImplSig {
            receiver,
            role,
            sig,
            ..
        }) = inputs.sig_by_id.get(&method_id).copied()
        else {
            outputs.problems.push(ori_arc::ArcProblem::InternalError {
                message: format!("missing typed impl-method identity {method_id:?}"),
                span: method.span,
            });
            outputs.emission_names.push(None);
            continue;
        };
        outputs.consumed_sig_ids.insert(method_id);
        if sig.is_generic() {
            outputs.emission_names.push(None);
            continue;
        }
        let (ordinal, qualified_name) = make_qualified_name(
            Some(method_receiver_key(inputs.pool, *receiver)),
            method.name,
            inputs.interner,
            &mut outputs.method_ordinals,
        );
        let recv_key = Some(method_receiver_key(inputs.pool, *receiver));
        if let ori_types::ImplMethodRole::UserDrop { logical } = role {
            outputs
                .user_drop_bindings
                .push(ori_repr::executable::UserDropBinding::new(
                    *receiver,
                    *logical,
                    qualified_name,
                ));
        }
        let tier = if impl_def.trait_path.is_none() {
            DispatchTier::Inherent
        } else {
            DispatchTier::Trait
        };
        outputs
            .dispatch
            .record(recv_key, method.name, qualified_name, tier);
        record_producer_target(outputs, method_id, qualified_name, method.span);
        outputs.emission_names.push(Some(qualified_name));
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon: inputs.canon,
            interner: inputs.interner,
            pool: inputs.pool,
            problems: &mut outputs.problems,
        };
        let (arc_fn, lambdas) = if let Some(type_name) = type_name_name {
            crate::arc_lowering::lower_impl_method_to_arc_nth(
                qualified_name,
                sig,
                method.name,
                type_name,
                ordinal,
                &mut context,
                None,
            )
        } else {
            crate::arc_lowering::lower_to_arc(qualified_name, sig, method.name, &mut context, None)
        };
        outputs
            .groups
            .push(super::super::ArcFunctionGroup::new(arc_fn, lambdas));
    }
}

/// Compute the ordinal-qualified name for an impl method.
///
/// Same-type same-name methods (e.g., two `impl Index<...>`) get ordinal
/// suffixes (`__impl_{idx}_{method}_{ordinal}`) for disambiguation.
fn make_qualified_name(
    self_type_idx: Option<Idx>,
    method_name: Name,
    interner: &StringInterner,
    method_ordinals: &mut FxHashMap<(Idx, Name), usize>,
) -> (usize, Name) {
    let ordinal = if let Some(idx) = self_type_idx {
        let entry = method_ordinals.entry((idx, method_name)).or_insert(0);
        let ord = *entry;
        *entry += 1;
        ord
    } else {
        0
    };
    let qualified = if let Some(idx) = self_type_idx {
        let method_str = interner.lookup(method_name);
        if ordinal == 0 {
            interner.intern(&format!("__impl_{}_{method_str}", idx.raw()))
        } else {
            interner.intern(&format!("__impl_{}_{}_{ordinal}", idx.raw(), method_str))
        }
    } else {
        method_name
    };
    (ordinal, qualified)
}

/// Lower default trait methods that appear in an impl block's sig list
/// but have no parse-level method definition (using the trait's default body).
fn lower_default_trait_methods(
    impl_def: &ori_ir::ImplDef,
    impl_index: usize,
    type_name_name: Option<Name>,
    inputs: &ImplLoweringInputs<'_>,
    outputs: &mut ImplLoweringOutputs,
) {
    let Some(trait_path) = &impl_def.trait_path else {
        return;
    };
    let Some(&trait_name) = trait_path.last() else {
        return;
    };
    let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();
    let Some(trait_def) = inputs
        .parse_result
        .module
        .traits
        .iter()
        .find(|t| t.name == trait_name)
    else {
        return;
    };

    for item in &trait_def.items {
        if let ori_ir::TraitItem::DefaultMethod(default) = item {
            if !overridden.contains(&default.name) {
                let method_id = ImplMethodId::new(impl_index, default.body);
                let Some(ori_types::ImplSig {
                    receiver,
                    role,
                    sig,
                    ..
                }) = inputs.sig_by_id.get(&method_id).copied()
                else {
                    outputs.problems.push(ori_arc::ArcProblem::InternalError {
                        message: format!("missing typed default-method identity {method_id:?}"),
                        span: default.span,
                    });
                    outputs.emission_names.push(None);
                    continue;
                };
                outputs.consumed_sig_ids.insert(method_id);
                if sig.is_generic() {
                    outputs.emission_names.push(None);
                    continue;
                }
                let (ordinal, qualified_name) = make_qualified_name(
                    Some(method_receiver_key(inputs.pool, *receiver)),
                    default.name,
                    inputs.interner,
                    &mut outputs.method_ordinals,
                );
                let recv_key = Some(method_receiver_key(inputs.pool, *receiver));
                if let ori_types::ImplMethodRole::UserDrop { logical } = role {
                    outputs
                        .user_drop_bindings
                        .push(ori_repr::executable::UserDropBinding::new(
                            *receiver,
                            *logical,
                            qualified_name,
                        ));
                }
                outputs.dispatch.record(
                    recv_key,
                    default.name,
                    qualified_name,
                    DispatchTier::Trait,
                );
                record_producer_target(outputs, method_id, qualified_name, default.span);
                outputs.emission_names.push(Some(qualified_name));
                let mut context = crate::arc_lowering::ArcLoweringContext {
                    canon: inputs.canon,
                    interner: inputs.interner,
                    pool: inputs.pool,
                    problems: &mut outputs.problems,
                };
                let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                    crate::arc_lowering::lower_impl_method_to_arc_nth(
                        qualified_name,
                        sig,
                        default.name,
                        tn,
                        ordinal,
                        &mut context,
                        None,
                    )
                } else {
                    crate::arc_lowering::lower_to_arc(
                        qualified_name,
                        sig,
                        default.name,
                        &mut context,
                        None,
                    )
                };
                outputs
                    .groups
                    .push(super::super::ArcFunctionGroup::new(arc_fn, lambdas));
            }
        }
    }
}

fn record_producer_target(
    outputs: &mut ImplLoweringOutputs,
    method: ImplMethodId,
    target: Name,
    span: ori_ir::Span,
) {
    let producer = ori_types::MethodProducer::Impl(method);
    if let Some(existing) = outputs.producer_targets.insert(producer, target) {
        if existing != target {
            outputs.problems.push(ori_arc::ArcProblem::InternalError {
                message: format!(
                    "impl-method producer {method:?} has conflicting realized targets {existing:?} and {target:?}"
                ),
                span,
            });
        }
    }
}
