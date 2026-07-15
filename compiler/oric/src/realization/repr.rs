//! Backend-neutral function inventory and representation-plan realization.
//!
//! Contains impl-method ARC lowering for interprocedural analysis and
//! representation-plan construction over a prepared whole-program batch.

use ori_ir::canon::CanonResult;
use ori_ir::{DerivedTrait, ReprAttrKind};
use ori_repr::monomorphize::{MonoFunction, MonoFunctionOrigin};

use ori_types::{
    AcceptedDerivedImpl, Idx, ImplMethodId, ImplSig, Pool, TypeCheckResult, Visibility,
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

/// Build shared ARC bodies for supported accepted non-generic derives.
///
/// This is deliberately downstream of type checking and upstream of the one
/// grouped preparation/AIMS seam. It consumes only accepted facts, emits no
/// ownership instructions. Generated Canon roots may eventually become the
/// producer for the same shared ARC/AIMS seam.
pub(crate) fn lower_non_generic_derived_methods_for_analysis(
    accepted_derives: &[AcceptedDerivedImpl],
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
        let Some(body) = build_supported_derived_body(
            accepted.trait_kind,
            executable_name,
            &accepted.signature,
            pool,
        ) else {
            continue;
        };
        match body {
            Ok(function) => {
                let key = (
                    pool.resolve_fully(accepted.owner_type),
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
            let body = build_supported_derived_body(
                accepted.trait_kind,
                mono.mangled_name,
                &mono.sig,
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
    trait_kind: DerivedTrait,
    executable_name: Name,
    signature: &ori_types::FunctionSig,
    pool: &Pool,
) -> Option<Result<ori_arc::ArcFunction, String>> {
    match trait_kind {
        DerivedTrait::Clone => Some(
            ori_arc::build_derived_clone_identity(executable_name, signature, pool)
                .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Eq => Some(
            ori_arc::build_derived_eq(executable_name, signature, pool)
                .map_err(|error| error.to_string()),
        ),
        DerivedTrait::Hashable
        | DerivedTrait::Printable
        | DerivedTrait::Debug
        | DerivedTrait::Default
        | DerivedTrait::Comparable => None,
    }
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
        let type_name_name = impl_def.type_name();
        let self_type_idx = type_name_name.and_then(|name| {
            type_result
                .typed
                .types
                .iter()
                .find(|te| te.name == name)
                .map(|te| te.idx)
        });
        lower_declared_impl_methods(
            impl_def,
            impl_index,
            type_name_name,
            self_type_idx,
            &inputs,
            &mut outputs,
        );
        lower_default_trait_methods(
            impl_def,
            impl_index,
            type_name_name,
            self_type_idx,
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
    self_type_idx: Option<Idx>,
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
            self_type_idx,
            method.name,
            inputs.interner,
            &mut outputs.method_ordinals,
        );
        let recv_key = Some(inputs.pool.resolve_fully(*receiver));
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
    self_type_idx: Option<Idx>,
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
                    self_type_idx,
                    default.name,
                    inputs.interner,
                    &mut outputs.method_ordinals,
                );
                let recv_key = Some(inputs.pool.resolve_fully(*receiver));
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
