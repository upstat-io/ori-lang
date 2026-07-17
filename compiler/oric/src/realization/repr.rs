//! Backend-neutral function inventory and representation-plan realization.
//!
//! Contains impl-method ARC lowering for interprocedural analysis and
//! representation-plan construction over a prepared whole-program batch.

use ori_ir::canon::CanonResult;
use ori_ir::{DerivedTrait, ReprAttrKind};
use ori_repr::monomorphize::{MonoFunction, MonoFunctionOrigin};

use ori_types::{
    AcceptedDerivedImpl, DerivedCallPlan, Idx, ImplMethodId, ImplSig, Pool, Tag, TypeCheckResult,
    Visibility,
};
use oric::ir::{Name, StringInterner};
use oric::parser::ParseOutput;
use rustc_hash::{FxHashMap, FxHashSet};

/// Impl-method bodies and the receiver-qualified targets derived with them.
pub(crate) struct ImplMethodAnalysis {
    /// Impl bodies retain their owned lambdas until shared batch preparation.
    pub(crate) groups: Vec<super::ArcFunctionGroup>,
    pub(crate) targets: FxHashMap<(Idx, Name), Name>,
    /// Exact semantic user-drop roles projected by the type checker and bound
    /// to their realized bodies before ordinary dispatch precedence is applied.
    pub(crate) user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    /// Stable realized body name parallel to `TypedModule::impl_sigs`.
    /// Generic entries carry `None` because their concrete bodies enter
    /// through the monomorphized-function inventory instead.
    pub(crate) emission_names: Vec<Option<Name>>,
}

#[derive(Default)]
struct ReceiverDispatch {
    targets: FxHashMap<(Idx, Name), Name>,
    inherent: FxHashSet<(Idx, Name)>,
    ambiguous_trait: FxHashSet<(Idx, Name)>,
    ambiguous_inherent: FxHashSet<(Idx, Name)>,
}

impl ReceiverDispatch {
    fn record(
        &mut self,
        recv_key_idx: Option<Idx>,
        method_name: Name,
        qualified_name: Name,
        is_inherent: bool,
    ) {
        let Some(idx) = recv_key_idx else {
            return;
        };
        let key = (idx, method_name);
        if is_inherent {
            self.ambiguous_trait.remove(&key);
            if !self.inherent.insert(key) {
                self.targets.remove(&key);
                self.ambiguous_inherent.insert(key);
                return;
            }
            if !self.ambiguous_inherent.contains(&key) {
                self.targets.insert(key, qualified_name);
            }
            return;
        }

        if self.inherent.contains(&key)
            || self.ambiguous_inherent.contains(&key)
            || self.ambiguous_trait.contains(&key)
        {
            return;
        }
        if self.targets.insert(key, qualified_name).is_some() {
            self.targets.remove(&key);
            self.ambiguous_trait.insert(key);
        }
    }
}

struct ImplLoweringInputs<'a> {
    parse_result: &'a ParseOutput,
    interner: &'a StringInterner,
    canon: &'a CanonResult,
    pool: &'a Pool,
    sig_by_id: FxHashMap<ImplMethodId, &'a ImplSig>,
}

#[derive(Default)]
struct ImplLoweringOutputs {
    groups: Vec<super::ArcFunctionGroup>,
    emission_names: Vec<Option<Name>>,
    user_drop_bindings: Vec<ori_repr::executable::UserDropBinding>,
    dispatch: ReceiverDispatch,
    consumed_sig_ids: FxHashSet<ImplMethodId>,
    method_ordinals: FxHashMap<(Idx, Name), usize>,
    problems: Vec<ori_arc::ArcProblem>,
}

/// Compiler-generated derived bodies and their exact receiver-qualified targets.
pub(crate) struct DerivedMethodAnalysis {
    /// Non-generic generated bodies. Generic bodies enter through the shared
    /// monomorphized-function inventory.
    pub(crate) groups: Vec<super::ArcFunctionGroup>,
    /// `(concrete receiver, method) -> generated executable body`.
    pub(crate) targets: FxHashMap<(Idx, Name), Name>,
}

/// Canonical receiver identity for method-target insertion and lookup.
///
/// Newtypes retain their nominal `Named` or concrete `Applied` carrier because
/// transparent representation resolution is not semantic method identity.
/// Every other receiver keeps the existing fully-resolved key so nominal
/// structs/enums and concrete generic instantiations meet at one target.
pub(crate) fn method_receiver_key(pool: &Pool, receiver: Idx) -> Idx {
    if !pool.is_valid_idx(receiver) {
        return receiver;
    }
    let is_newtype = match pool.tag(receiver) {
        Tag::Named => pool.is_newtype_ctor(pool.named_name(receiver)),
        Tag::Applied => pool.is_newtype_ctor(pool.applied_name(receiver)),
        _ => false,
    };
    if is_newtype {
        receiver
    } else {
        pool.resolve_fully(receiver)
    }
}

/// Register exact concrete receiver targets for every monomorphized method.
///
/// `receiver_type_name` remains solely a Canon body-namespace selector. This
/// target map consumes the distinct concrete receiver carrier so same-spelled
/// methods and zero-argument associated functions cannot collapse through a
/// name/arity fallback during shared batch preparation.
pub(crate) fn extend_mono_method_targets(
    targets: &mut FxHashMap<(Idx, Name), Name>,
    mono_functions: &[MonoFunction],
    interner: &StringInterner,
    pool: &Pool,
) -> Result<(), Vec<ori_arc::ArcProblem>> {
    let mut problems = Vec::new();
    for mono in mono_functions {
        let Some(receiver) = mono.receiver_type else {
            continue;
        };
        if !pool.is_valid_idx(receiver) {
            problems.push(ori_arc::ArcProblem::InternalError {
                message: format!(
                    "monomorphized method {} carries invalid concrete receiver {receiver:?}",
                    interner.lookup(mono.mangled_name),
                ),
                span: ori_ir::Span::DUMMY,
            });
            continue;
        }
        let key = (method_receiver_key(pool, receiver), mono.original_name);
        if let Some(existing) = targets.insert(key, mono.mangled_name) {
            if existing != mono.mangled_name {
                targets.insert(key, existing);
                problems.push(ori_arc::ArcProblem::InternalError {
                    message: format!(
                        "concrete receiver method {} has conflicting realized targets {} and {}",
                        interner.lookup(mono.original_name),
                        interner.lookup(existing),
                        interner.lookup(mono.mangled_name),
                    ),
                    span: ori_ir::Span::DUMMY,
                });
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// Build shared ARC bodies for supported accepted non-generic derives.
///
/// This is deliberately downstream of type checking and upstream of the one
/// grouped preparation/AIMS seam. It consumes only accepted facts, emits no
/// ownership instructions. Generated Canon roots may eventually become the
/// producer for the same shared ARC/AIMS seam.
pub(crate) fn lower_non_generic_derived_methods_for_analysis(
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    interner: &StringInterner,
    pool: &Pool,
) -> Result<DerivedMethodAnalysis, Vec<ori_arc::ArcProblem>> {
    let mut groups = Vec::new();
    let mut targets = FxHashMap::default();
    let mut problems = Vec::new();

    for accepted in accepted_derives {
        if accepted.signature.is_generic() {
            continue;
        }
        let executable_name = derived_body_name(accepted, interner);
        let Some(plan) = unique_derived_plan(
            accepted,
            &[],
            derived_call_plans,
            executable_name,
            interner,
            &mut problems,
        ) else {
            continue;
        };
        let Some(body) = build_supported_derived_body(
            accepted,
            plan,
            executable_name,
            &accepted.signature,
            interner,
            pool,
        ) else {
            continue;
        };
        match body {
            Ok(function) => {
                let key = (
                    method_receiver_key(pool, accepted.owner_type),
                    accepted.method_name,
                );
                if targets.insert(key, executable_name).is_some() {
                    problems.push(ori_arc::ArcProblem::InternalError {
                        message: format!(
                            "duplicate accepted derived target for {}.{}",
                            interner.lookup(accepted.owner_name),
                            interner.lookup(accepted.method_name),
                        ),
                        span: accepted.span,
                    });
                } else {
                    groups.push(super::ArcFunctionGroup::new(function, Vec::new()));
                }
            }
            Err(error) => problems.push(ori_arc::ArcProblem::InternalError {
                message: format!(
                    "accepted derived {:?} {}.{} cannot form an executable body: {error}",
                    accepted.trait_kind,
                    interner.lookup(accepted.owner_name),
                    interner.lookup(accepted.method_name),
                ),
                span: accepted.span,
            }),
        }
    }

    if problems.is_empty() {
        Ok(DerivedMethodAnalysis { groups, targets })
    } else {
        Err(problems)
    }
}

/// Lower one monomorphized body from its producer-issued semantic origin.
///
/// Returning `None` for an unsupported derived origin intentionally leaves
/// that target unresolved. The closed executable validator remains the
/// fail-closed coverage gate while shared body coverage is expanded.
pub(crate) fn lower_mono_function_for_analysis(
    mono: &MonoFunction,
    accepted_derives: &[AcceptedDerivedImpl],
    derived_call_plans: &[DerivedCallPlan],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Option<super::ArcFunctionGroup> {
    match mono.origin {
        MonoFunctionOrigin::Source => {
            let lowered = match mono.receiver_type_name {
                Some(type_name) => crate::arc_lowering::lower_impl_method_to_arc(
                    mono.mangled_name,
                    &mono.sig,
                    mono.original_name,
                    type_name,
                    canon,
                    interner,
                    pool,
                    problems,
                    Some(&mono.body_type_map),
                ),
                None => crate::arc_lowering::lower_to_arc(
                    mono.mangled_name,
                    &mono.sig,
                    mono.original_name,
                    canon,
                    interner,
                    pool,
                    problems,
                    Some(&mono.body_type_map),
                ),
            };
            Some(lowered.into())
        }
        MonoFunctionOrigin::Impl(id) => {
            let lowered = crate::arc_lowering::lower_impl_method_to_arc_by_source(
                mono.mangled_name,
                &mono.sig,
                id.body(),
                canon,
                interner,
                pool,
                problems,
                Some(&mono.body_type_map),
            );
            Some(lowered.into())
        }
        MonoFunctionOrigin::Derived(id) => {
            let mut matches = accepted_derives.iter().filter(|accepted| accepted.id == id);
            let Some(accepted) = matches.next() else {
                problems.push(ori_arc::ArcProblem::InternalError {
                    message: format!(
                        "monomorphized derived body {} references absent accepted identity {:?}",
                        interner.lookup(mono.mangled_name),
                        id,
                    ),
                    span: ori_ir::Span::DUMMY,
                });
                return None;
            };
            if matches.next().is_some() {
                problems.push(ori_arc::ArcProblem::InternalError {
                    message: format!(
                        "monomorphized derived body {} has duplicate accepted identity {:?}",
                        interner.lookup(mono.mangled_name),
                        id,
                    ),
                    span: accepted.span,
                });
                return None;
            }
            let binder_substitutions =
                mono_binder_substitutions(accepted, mono, pool, interner, problems)?;
            let plan = unique_derived_plan(
                accepted,
                &binder_substitutions,
                derived_call_plans,
                mono.mangled_name,
                interner,
                problems,
            )?;
            let body = build_supported_derived_body(
                accepted,
                plan,
                mono.mangled_name,
                &mono.sig,
                interner,
                pool,
            )?;
            match body {
                Ok(function) => Some(super::ArcFunctionGroup::new(function, Vec::new())),
                Err(error) => {
                    problems.push(ori_arc::ArcProblem::InternalError {
                        message: format!(
                            "monomorphized derived {:?} {} cannot form an executable body: {error}",
                            accepted.trait_kind,
                            interner.lookup(mono.mangled_name),
                        ),
                        span: accepted.span,
                    });
                    None
                }
            }
        }
    }
}

fn build_supported_derived_body(
    accepted: &AcceptedDerivedImpl,
    plan: &DerivedCallPlan,
    executable_name: Name,
    signature: &ori_types::FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
) -> Option<Result<ori_arc::ArcFunction, String>> {
    let body = match accepted.trait_kind {
        DerivedTrait::Clone => Some(
            ori_arc::build_derived_clone_identity(executable_name, signature, pool)
                .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Eq => Some(
            ori_arc::build_derived_eq(
                executable_name,
                accepted.owner_name,
                accepted.method_name,
                signature,
                pool,
            )
            .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Default => Some(
            ori_arc::build_derived_default(
                executable_name,
                accepted.owner_name,
                accepted.method_name,
                signature,
                interner,
                pool,
            )
            .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Hashable => Some(
            ori_arc::build_derived_hash(
                executable_name,
                accepted.owner_name,
                accepted.method_name,
                interner.intern("hash_combine"),
                signature,
                pool,
            )
            .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Printable | DerivedTrait::Debug => Some(
            ori_arc::build_derived_format(
                accepted.trait_kind,
                executable_name,
                accepted.owner_name,
                accepted.method_name,
                signature,
                interner,
                pool,
            )
            .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Comparable => Some(
            ori_arc::build_derived_compare(
                executable_name,
                interner.intern("Ordering"),
                accepted.method_name,
                signature,
                pool,
            )
            .map_err(|error| error.to_string()),
        ),
    };
    let body = body?;
    Some(body.and_then(|mut function| {
        bind_derived_call_plan(&mut function, plan, pool)?;
        Ok(function)
    }))
}

fn unique_derived_plan<'a>(
    accepted: &AcceptedDerivedImpl,
    binder_substitutions: &[Idx],
    plans: &'a [DerivedCallPlan],
    executable_name: Name,
    interner: &StringInterner,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Option<&'a DerivedCallPlan> {
    let mut matches = plans.iter().filter(|plan| {
        plan.derived == accepted.id && plan.binder_substitutions == binder_substitutions
    });
    let Some(plan) = matches.next() else {
        problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "derived executable {} has no frozen call plan for identity {:?} and substitutions {:?}",
                interner.lookup(executable_name),
                accepted.id,
                binder_substitutions,
            ),
            span: accepted.span,
        });
        return None;
    };
    if matches.next().is_some() {
        problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "derived executable {} has duplicate frozen call plans for identity {:?} and substitutions {:?}",
                interner.lookup(executable_name),
                accepted.id,
                binder_substitutions,
            ),
            span: accepted.span,
        });
        return None;
    }
    Some(plan)
}

fn mono_binder_substitutions(
    accepted: &AcceptedDerivedImpl,
    mono: &MonoFunction,
    pool: &Pool,
    interner: &StringInterner,
    problems: &mut Vec<ori_arc::ArcProblem>,
) -> Option<Vec<Idx>> {
    if accepted.signature.type_params.is_empty() {
        return Some(Vec::new());
    }
    let Some(receiver) = mono.receiver_type else {
        problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "generic derived executable {} has no concrete receiver",
                interner.lookup(mono.mangled_name),
            ),
            span: accepted.span,
        });
        return None;
    };
    if !pool.is_valid_idx(receiver)
        || pool.tag(receiver) != Tag::Applied
        || pool.applied_name(receiver) != accepted.owner_name
    {
        problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "generic derived executable {} carries receiver {receiver:?}, expected concrete {}<...>",
                interner.lookup(mono.mangled_name),
                interner.lookup(accepted.owner_name),
            ),
            span: accepted.span,
        });
        return None;
    }
    let substitutions = pool.applied_args(receiver);
    if substitutions.len() != accepted.signature.type_params.len()
        || substitutions
            .iter()
            .any(|&ty| !pool.is_valid_idx(ty) || !pool.flags(ty).is_recordable())
    {
        problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "generic derived executable {} has incomplete concrete binder substitutions {:?}",
                interner.lookup(mono.mangled_name),
                substitutions,
            ),
            span: accepted.span,
        });
        return None;
    }
    Some(substitutions)
}

fn bind_derived_call_plan(
    function: &mut ori_arc::ArcFunction,
    plan: &DerivedCallPlan,
    pool: &Pool,
) -> Result<(), String> {
    if function.method_call_facts.len() != plan.calls.len() {
        return Err(format!(
            "generated body emitted {} method calls but its frozen plan contains {}",
            function.method_call_facts.len(),
            plan.calls.len(),
        ));
    }

    let emitted_calls = emitted_direct_calls(function);
    let mut claimed = FxHashSet::default();
    for (fact, selection) in function.method_call_facts.iter_mut().zip(&plan.calls) {
        let Some((_, emitted_name)) = emitted_calls
            .iter()
            .find(|(destination, _)| *destination == fact.destination)
        else {
            return Err(format!(
                "method-call fact at {:?} has no emitted direct call",
                fact.destination,
            ));
        };
        if *emitted_name != selection.method_name {
            return Err(format!(
                "method-call fact at {:?} emits {emitted_name:?}, frozen plan selects {:?}",
                fact.destination, selection.method_name,
            ));
        }
        if fact.receiver_type != selection.receiver_type {
            return Err(format!(
                "method-call fact at {:?} records receiver {:?}, frozen plan selects {:?}",
                fact.destination, fact.receiver_type, selection.receiver_type,
            ));
        }
        let expected_form = if selection.has_self {
            ori_arc::MethodCallForm::Instance
        } else {
            ori_arc::MethodCallForm::Associated
        };
        if fact.form != expected_form {
            return Err(format!(
                "method-call fact at {:?} records {:?}, frozen plan selects {:?}",
                fact.destination, fact.form, expected_form,
            ));
        }
        if fact.producer.is_some() || fact.derived_position.is_some() {
            return Err(format!(
                "method-call fact at {:?} was bound more than once",
                fact.destination,
            ));
        }
        fact.producer = Some(selection.producer.clone());
        fact.derived_position = Some(selection.position);
        claimed.insert(fact.destination);
    }

    function.direct_call_facts.clear();
    for selection in &plan.direct_calls {
        let Some(&(destination, _)) = emitted_calls.iter().find(|(destination, name)| {
            *name == selection.function_name && !claimed.contains(destination)
        }) else {
            return Err(format!(
                "frozen direct-call position {:?} for {:?} has no emitted call",
                selection.position, selection.function_name,
            ));
        };
        claimed.insert(destination);
        function.direct_call_facts.push(ori_arc::DirectCallFact {
            destination,
            producer: selection.producer.clone(),
            derived_position: selection.position,
        });
    }

    if emitted_calls.len() != claimed.len() {
        let unbound: Vec<_> = emitted_calls
            .iter()
            .filter(|(destination, _)| !claimed.contains(destination))
            .collect();
        return Err(format!(
            "generated body contains direct calls without frozen producers: {unbound:?}"
        ));
    }

    for fact in &function.method_call_facts {
        let Some(producer) = &fact.producer else {
            return Err(format!(
                "generated method-call fact at {:?} has no producer",
                fact.destination,
            ));
        };
        if matches!(producer, ori_types::MethodProducer::Prelude(_)) {
            return Err(format!(
                "generated method-call fact at {:?} carries a free-function producer",
                fact.destination,
            ));
        }
        if !pool.is_valid_idx(fact.receiver_type) {
            return Err(format!(
                "generated method-call fact at {:?} carries invalid receiver {:?}",
                fact.destination, fact.receiver_type,
            ));
        }
    }
    Ok(())
}

fn emitted_direct_calls(function: &ori_arc::ArcFunction) -> Vec<(ori_arc::ArcVarId, Name)> {
    let mut calls = Vec::new();
    for block in &function.blocks {
        for instruction in &block.body {
            if let ori_arc::ArcInstr::Apply { dst, func, .. } = instruction {
                calls.push((*dst, *func));
            }
        }
        if let ori_arc::ArcTerminator::Invoke { dst, func, .. } = &block.terminator {
            calls.push((*dst, *func));
        }
    }
    calls
}

fn derived_body_name(accepted: &AcceptedDerivedImpl, interner: &StringInterner) -> Name {
    interner.intern(&format!(
        "{}$derived${}",
        interner.lookup(accepted.method_name),
        accepted.id.raw(),
    ))
}

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
            user_drop_bindings: outputs.user_drop_bindings,
            emission_names: outputs.emission_names,
        })
    } else {
        Err(outputs.problems)
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
        outputs.dispatch.record(
            recv_key,
            method.name,
            qualified_name,
            impl_def.trait_path.is_none(),
        );
        outputs.emission_names.push(Some(qualified_name));
        let (arc_fn, lambdas) = if let Some(type_name) = type_name_name {
            crate::arc_lowering::lower_impl_method_to_arc_nth(
                qualified_name,
                sig,
                method.name,
                type_name,
                ordinal,
                inputs.canon,
                inputs.interner,
                inputs.pool,
                &mut outputs.problems,
                None,
            )
        } else {
            crate::arc_lowering::lower_to_arc(
                qualified_name,
                sig,
                method.name,
                inputs.canon,
                inputs.interner,
                inputs.pool,
                &mut outputs.problems,
                None,
            )
        };
        outputs
            .groups
            .push(super::ArcFunctionGroup::new(arc_fn, lambdas));
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
                outputs
                    .dispatch
                    .record(recv_key, default.name, qualified_name, false);
                outputs.emission_names.push(Some(qualified_name));
                let (arc_fn, lambdas) = if let Some(tn) = type_name_name {
                    crate::arc_lowering::lower_impl_method_to_arc_nth(
                        qualified_name,
                        sig,
                        default.name,
                        tn,
                        ordinal,
                        inputs.canon,
                        inputs.interner,
                        inputs.pool,
                        &mut outputs.problems,
                        None,
                    )
                } else {
                    crate::arc_lowering::lower_to_arc(
                        qualified_name,
                        sig,
                        default.name,
                        inputs.canon,
                        inputs.interner,
                        inputs.pool,
                        &mut outputs.problems,
                        None,
                    )
                };
                outputs
                    .groups
                    .push(super::ArcFunctionGroup::new(arc_fn, lambdas));
            }
        }
    }
}

/// Build the representation plan from a type-checked module.
///
/// Extracts `#repr` attributes and public type indices from the typed module,
/// then runs the repr plan computation pipeline (canonical reprs, range analysis,
/// integer narrowing, float narrowing).
///
/// `imported_type_metadata` carries repr/pub metadata from imported modules,
/// enabling the repr plan to correctly exempt imported `pub` and `#repr(...)`
/// types from integer narrowing.
///
/// Must run AFTER borrow inference (accepts `ArcFunction`s for range analysis)
/// and BEFORE codegen (`TypeLayoutResolver` and `TypeInfoStore` read the plan).
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter carries distinct metadata from different compiler phases"
)]
pub(crate) fn compute_module_repr_plan(
    pool: &Pool,
    all_arc_funcs: &[ori_arc::ArcFunction],
    narrowing_policy: ori_repr::NarrowingPolicy,
    type_result: &TypeCheckResult,
    interner: Option<&StringInterner>,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    has_analysis_only_functions: bool,
) -> ori_repr::ReprPlan {
    // Extract #repr attributes from typed module for the repr plan.
    let repr_attrs: Vec<(Idx, ReprAttrKind)> = type_result
        .typed
        .types
        .iter()
        .filter_map(|te| te.repr.map(|r| (te.idx, r)))
        .collect();

    // Extract public type indices — their field layout is an ABI contract
    // that integer narrowing must not violate.
    let mut pub_type_indices: Vec<Idx> = type_result
        .typed
        .types
        .iter()
        .filter(|te| te.visibility == Visibility::Public)
        .map(|te| te.idx)
        .collect();

    // Also mark collection wrapper types from public function signatures as
    // public. A public `@f (xs: [int]) -> [int]` means the `[int]` type's
    // element layout is an ABI surface — callers construct lists with canonical
    // element widths. Without this, integer narrowing Phase C could narrow the
    // element repr while callers still use 8-byte strides.
    ori_types::collect_public_collection_types(
        pool,
        &type_result.typed.functions,
        &mut pub_type_indices,
    );

    // Collect unconstrained function names (pub + trait impl) for
    // interprocedural range analysis. The qualified-name algorithm is a
    // cross-phase contract feeding compute_repr_plan_with_interner — the
    // canonical implementation lives in ori_llvm.
    let unconstrained_fn_names = ori_repr::collect_unconstrained_fn_names(
        &type_result.typed.functions,
        &type_result.typed.trait_impl_fn_names,
        interner,
    );

    ori_repr::compute_repr_plan_with_interner(
        pool,
        all_arc_funcs,
        narrowing_policy,
        &repr_attrs,
        interner,
        &pub_type_indices,
        imported_type_metadata,
        imported_collection_surfaces,
        &unconstrained_fn_names,
        has_analysis_only_functions,
    )
}

#[cfg(test)]
mod tests {
    use ori_ir::{DerivedImplId, DerivedTrait, Name, Span, StringInterner};
    use ori_repr::monomorphize::{MonoFunction, MonoFunctionOrigin};
    use ori_types::{
        AcceptedDerivedImpl, DerivedCallPlan, DerivedCallPosition, DerivedCallSelection,
        DerivedDirectCallSelection, EnumVariant, FunctionSig, Idx, MethodProducer, Pool,
        RegistryMethodIdentity, RegistryPreludeIdentity,
    };
    use rustc_hash::FxHashMap;

    use super::{
        build_supported_derived_body, extend_mono_method_targets,
        lower_non_generic_derived_methods_for_analysis, method_receiver_key,
    };

    fn mono_method(
        mangled_name: Name,
        method_name: Name,
        receiver_type: Idx,
        derived_id: u32,
    ) -> MonoFunction {
        MonoFunction {
            mangled_name,
            original_name: method_name,
            origin: MonoFunctionOrigin::Derived(DerivedImplId::new(derived_id)),
            sig: FunctionSig::synthetic(mangled_name, Vec::new(), Vec::new(), receiver_type),
            body_type_map: FxHashMap::default(),
            instance_ids: Vec::new(),
            is_imported: false,
            receiver_type: Some(receiver_type),
            receiver_type_name: None,
        }
    }

    fn accepted_hashable(
        id: u32,
        owner_name: Name,
        owner_type: Idx,
        method_name: Name,
        self_name: Name,
        trait_type: Idx,
    ) -> AcceptedDerivedImpl {
        AcceptedDerivedImpl {
            id: DerivedImplId::new(id),
            owner_name,
            owner_type,
            trait_type,
            trait_kind: DerivedTrait::Hashable,
            method_name,
            signature: FunctionSig::synthetic(
                method_name,
                vec![self_name],
                vec![owner_type],
                Idx::INT,
            ),
            span: Span::DUMMY,
        }
    }

    fn call_plan(
        accepted: &AcceptedDerivedImpl,
        interner: &StringInterner,
        pool: &Pool,
    ) -> DerivedCallPlan {
        let resolved = pool.resolve_fully(accepted.owner_type);
        let (position, receiver_type) = if pool.is_newtype_ctor(accepted.owner_name) {
            (DerivedCallPosition::Newtype, resolved)
        } else {
            let fields = pool.struct_fields(resolved);
            let Some((_, receiver_type)) = fields.first().copied() else {
                panic!("test derive fixture must have one generated field call")
            };
            (DerivedCallPosition::Field(0), receiver_type)
        };
        let Some(receiver_tag) = pool.builtin_type_tag(pool.resolve_fully(receiver_type)) else {
            panic!("test derive fixture must select a builtin nested producer")
        };
        let method_text = interner.lookup(accepted.method_name);
        let Some(method_identity) = ori_registry::find_method_id(receiver_tag, method_text) else {
            panic!("test derive fixture must resolve {receiver_tag:?}.{method_text}")
        };
        let calls = vec![DerivedCallSelection {
            position,
            receiver_type,
            trait_type: accepted.trait_type,
            method_name: accepted.method_name,
            has_self: true,
            producer: MethodProducer::Registry(RegistryMethodIdentity::from_registered(
                method_identity,
            )),
        }];
        let direct_calls = if accepted.trait_kind == DerivedTrait::Hashable
            && !pool.is_newtype_ctor(accepted.owner_name)
        {
            let function_name = interner.intern("hash_combine");
            let Some(identity) = ori_registry::find_prelude_function_id("hash_combine") else {
                panic!("hash_combine must remain in the prelude registry")
            };
            vec![DerivedDirectCallSelection {
                position: DerivedCallPosition::FieldCombine(0),
                function_name,
                producer: MethodProducer::Prelude(RegistryPreludeIdentity::from_registered(
                    identity,
                )),
            }]
        } else {
            Vec::new()
        };
        DerivedCallPlan {
            derived: accepted.id,
            binder_substitutions: Vec::new(),
            calls,
            direct_calls,
        }
    }

    #[test]
    fn same_named_associated_monos_are_keyed_by_concrete_receiver() {
        let interner = StringInterner::new();
        let default = interner.intern("default");
        let mut pool = Pool::new();
        let left = pool.struct_type(interner.intern("Left"), &[]);
        let right = pool.struct_type(interner.intern("Right"), &[]);
        let left_target = interner.intern("default$m$Left");
        let right_target = interner.intern("default$m$Right");
        let monos = vec![
            mono_method(left_target, default, left, 1),
            mono_method(right_target, default, right, 2),
        ];
        let mut targets = FxHashMap::default();

        if let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) {
            panic!("distinct concrete receivers must retain distinct targets: {problems:?}");
        }

        assert_eq!(targets.get(&(left, default)), Some(&left_target));
        assert_eq!(targets.get(&(right, default)), Some(&right_target));
    }

    #[test]
    fn non_newtype_receiver_keys_keep_concrete_resolution_matching() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();

        let record_name = interner.intern("Record");
        let record_nominal = pool.named(record_name);
        let record = pool.struct_type(record_name, &[(interner.intern("value"), Idx::INT)]);
        pool.set_resolution(record_nominal, record);

        let choice_name = interner.intern("Choice");
        let choice_nominal = pool.named(choice_name);
        let choice = pool.enum_type(
            choice_name,
            &[EnumVariant {
                name: interner.intern("Only"),
                field_types: vec![Idx::STR],
            }],
        );
        pool.set_resolution(choice_nominal, choice);

        let box_name = interner.intern("Box");
        let concrete_box = pool.applied(box_name, &[Idx::INT]);
        let box_body = pool.struct_type(box_name, &[(interner.intern("item"), Idx::INT)]);
        pool.set_resolution(concrete_box, box_body);

        assert_eq!(method_receiver_key(&pool, record_nominal), record);
        assert_eq!(method_receiver_key(&pool, record), record);
        assert_eq!(method_receiver_key(&pool, choice_nominal), choice);
        assert_eq!(method_receiver_key(&pool, choice), choice);
        assert_eq!(method_receiver_key(&pool, concrete_box), box_body);
        assert_eq!(method_receiver_key(&pool, box_body), box_body);
    }

    #[test]
    fn conflicting_target_for_one_concrete_receiver_fails_closed() {
        let interner = StringInterner::new();
        let default = interner.intern("default");
        let mut pool = Pool::new();
        let receiver = pool.struct_type(interner.intern("Box"), &[]);
        let first = interner.intern("default$m$Box$first");
        let second = interner.intern("default$m$Box$second");
        let monos = vec![
            mono_method(first, default, receiver, 1),
            mono_method(second, default, receiver, 2),
        ];
        let mut targets = FxHashMap::default();

        let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool)
        else {
            panic!("one concrete receiver/method identity accepted conflicting bodies");
        };

        assert_eq!(targets.get(&(receiver, default)), Some(&first));
        assert_eq!(problems.len(), 1);
        let message = format!("{:?}", problems[0]);
        assert!(message.contains("conflicting realized targets"));
        assert!(message.contains("default$m$Box$first"));
        assert!(message.contains("default$m$Box$second"));
    }

    #[test]
    fn distinct_newtype_derives_keep_distinct_receiver_targets() {
        let interner = StringInterner::new();
        let left_name = interner.intern("LeftKey");
        let right_name = interner.intern("RightKey");
        let hash = interner.intern("hash");
        let self_name = interner.intern("self");
        let mut pool = Pool::new();
        let hashable = pool.named(interner.intern("Hashable"));
        let left = pool.named(left_name);
        let right = pool.named(right_name);
        pool.register_newtype_ctor(left_name, Idx::INT);
        pool.register_newtype_ctor(right_name, Idx::INT);
        pool.set_resolution(left, Idx::INT);
        pool.set_resolution(right, Idx::INT);
        let accepted = vec![
            accepted_hashable(1, left_name, left, hash, self_name, hashable),
            accepted_hashable(2, right_name, right, hash, self_name, hashable),
        ];

        let plans: Vec<_> = accepted
            .iter()
            .map(|item| call_plan(item, &interner, &pool))
            .collect();
        let analysis =
            lower_non_generic_derived_methods_for_analysis(&accepted, &plans, &interner, &pool)
                .unwrap_or_else(|problems| {
                    panic!(
                        "distinct newtype derives must not collide by representation: {problems:?}"
                    )
                });

        assert_eq!(analysis.groups.len(), 2);
        assert_eq!(analysis.targets.len(), 2);
        let left_target = analysis.targets.get(&(left, hash));
        let right_target = analysis.targets.get(&(right, hash));
        assert!(left_target.is_some());
        assert!(right_target.is_some());
        assert_ne!(left_target, right_target);
    }

    #[test]
    fn accepted_hashable_builds_a_shared_arc_body() {
        let interner = StringInterner::new();
        let owner_name = interner.intern("Key");
        let method_name = interner.intern("hash");
        let self_name = interner.intern("self");
        let executable_name = interner.intern("hash$derived$0");
        let mut pool = Pool::new();
        let owner_type = pool.struct_type(owner_name, &[(interner.intern("value"), Idx::INT)]);
        let signature =
            FunctionSig::synthetic(method_name, vec![self_name], vec![owner_type], Idx::INT);
        let accepted = AcceptedDerivedImpl {
            id: DerivedImplId::new(0),
            owner_name,
            owner_type,
            trait_type: pool.named(interner.intern("Hashable")),
            trait_kind: DerivedTrait::Hashable,
            method_name,
            signature: signature.clone(),
            span: Span::DUMMY,
        };

        let plan = call_plan(&accepted, &interner, &pool);
        let Some(result) = build_supported_derived_body(
            &accepted,
            &plan,
            executable_name,
            &signature,
            &interner,
            &pool,
        ) else {
            panic!("accepted Hashable must not remain outside the shared body inventory")
        };
        let body = result.unwrap_or_else(|error| {
            panic!("accepted concrete Hashable must build a shared ARC body: {error}")
        });

        assert_eq!(body.name, executable_name);
        assert_eq!(body.params[0].ty, owner_type);
        assert_eq!(body.return_type, Idx::INT);
        assert_eq!(body.method_call_facts.len(), 1);
        assert_eq!(body.method_call_facts[0].receiver_type, Idx::INT);
    }

    #[test]
    fn accepted_newtype_eq_delegates_with_nominal_target_identity() {
        let interner = StringInterner::new();
        let owner_name = interner.intern("UserId");
        let method_name = interner.intern("equals");
        let self_name = interner.intern("self");
        let other_name = interner.intern("other");
        let executable_name = interner.intern("equals$derived$0");
        let mut pool = Pool::new();
        let owner_type = pool.named(owner_name);
        pool.register_newtype_ctor(owner_name, Idx::STR);
        pool.set_resolution(owner_type, Idx::STR);
        let signature = FunctionSig::synthetic(
            method_name,
            vec![self_name, other_name],
            vec![owner_type, owner_type],
            Idx::BOOL,
        );
        let accepted = AcceptedDerivedImpl {
            id: DerivedImplId::new(0),
            owner_name,
            owner_type,
            trait_type: pool.named(interner.intern("Eq")),
            trait_kind: DerivedTrait::Eq,
            method_name,
            signature: signature.clone(),
            span: Span::DUMMY,
        };

        let plan = call_plan(&accepted, &interner, &pool);
        let Some(result) = build_supported_derived_body(
            &accepted,
            &plan,
            executable_name,
            &signature,
            &interner,
            &pool,
        ) else {
            panic!("accepted newtype Eq must remain in the shared body inventory")
        };
        let body = result.unwrap_or_else(|error| {
            panic!("accepted newtype Eq must delegate to its underlying target: {error}")
        });

        assert_eq!(body.name, executable_name);
        assert!(body
            .params
            .iter()
            .all(|parameter| parameter.ty == owner_type));
        assert_eq!(body.return_type, Idx::BOOL);
        assert_eq!(body.method_call_facts.len(), 1);
        assert_eq!(body.method_call_facts[0].receiver_type, Idx::STR);
    }
}
