//! Realization of compiler-generated derived method bodies.

use ori_ir::canon::CanonResult;
use ori_ir::DerivedTrait;
use ori_repr::monomorphize::{MonoFunction, MonoFunctionOrigin};
use ori_types::{AcceptedDerivedImpl, DerivedCallPlan, Idx, Pool, Tag};
use oric::ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use super::derived_binding::bind_derived_call_plan;
use super::{method_receiver_key, DerivedMethodAnalysis};

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
                    groups.push(super::super::ArcFunctionGroup::new(function, Vec::new()));
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
) -> Option<super::super::ArcFunctionGroup> {
    match mono.origin {
        MonoFunctionOrigin::Source => {
            let mut context = crate::arc_lowering::ArcLoweringContext {
                canon,
                interner,
                pool,
                problems,
            };
            let lowered = crate::arc_lowering::lower_mono_to_arc(
                mono.mangled_name,
                &mono.sig,
                mono.identity.original_name(),
                mono.receiver_type_name,
                &mut context,
                &mono.body_type_map,
                mono.identity.const_bindings(),
            );
            Some(lowered.into())
        }
        MonoFunctionOrigin::Impl(id) => {
            let mut context = crate::arc_lowering::ArcLoweringContext {
                canon,
                interner,
                pool,
                problems,
            };
            let lowered = crate::arc_lowering::lower_mono_impl_method_to_arc_by_source(
                mono.mangled_name,
                &mono.sig,
                id.body(),
                &mut context,
                &mono.body_type_map,
                mono.identity.const_bindings(),
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
                Ok(function) => Some(super::super::ArcFunctionGroup::new(function, Vec::new())),
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

pub(super) fn build_supported_derived_body(
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
    let Some(receiver) = mono.identity.receiver_type() else {
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

fn derived_body_name(accepted: &AcceptedDerivedImpl, interner: &StringInterner) -> Name {
    interner.intern(&format!(
        "{}$derived${}",
        interner.lookup(accepted.method_name),
        accepted.id.raw(),
    ))
}
