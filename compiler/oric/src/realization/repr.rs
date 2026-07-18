//! Backend-neutral function inventory and representation-plan realization.
//!
//! Contains impl-method ARC lowering for interprocedural analysis and
//! representation-plan construction over a prepared whole-program batch.

mod derived_binding;
mod derived_methods;
mod impl_methods;

pub(crate) use derived_methods::{
    lower_mono_function_for_analysis, lower_non_generic_derived_methods_for_analysis,
};
pub(crate) use impl_methods::lower_impl_methods_for_analysis;

#[cfg(test)]
use derived_methods::build_supported_derived_body;

use ori_ir::canon::CanonResult;
use ori_ir::ReprAttrKind;
use ori_repr::monomorphize::MonoFunction;

use ori_types::{Idx, ImplMethodId, ImplSig, Pool, Tag, TypeCheckResult, Visibility};
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

// Lowered ARC groups have a separate executable diagnostic owner. Report the
// semantic inventory dimensions and exact target map here.
impl std::fmt::Debug for ImplMethodAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplMethodAnalysis")
            .field("group_count", &self.groups.len())
            .field("targets", &self.targets)
            .field("user_drop_binding_count", &self.user_drop_bindings.len())
            .field("emission_names", &self.emission_names)
            .finish_non_exhaustive()
    }
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

// Generated ARC groups remain opaque; derived-method diagnostics expose the
// realized group count and exact receiver-qualified targets.
impl std::fmt::Debug for DerivedMethodAnalysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedMethodAnalysis")
            .field("group_count", &self.groups.len())
            .field("targets", &self.targets)
            .finish_non_exhaustive()
    }
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
        let Some(receiver) = mono.identity.receiver_type() else {
            continue;
        };
        if !mono.identity.method_args().is_empty() {
            continue;
        }
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
        let key = (
            method_receiver_key(pool, receiver),
            mono.identity.original_name(),
        );
        if let Some(existing) = targets.insert(key, mono.mangled_name) {
            if existing != mono.mangled_name {
                targets.insert(key, existing);
                problems.push(ori_arc::ArcProblem::InternalError {
                    message: format!(
                        "concrete receiver method {} has conflicting realized targets {} and {}",
                        interner.lookup(mono.identity.original_name()),
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

/// Inputs for representation planning after borrow inference and before codegen.
/// Imported metadata preserves public and explicit representation exemptions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModuleReprInput<'a> {
    pub(crate) pool: &'a Pool,
    pub(crate) arc_functions: &'a [ori_arc::ArcFunction],
    pub(crate) narrowing_policy: ori_repr::NarrowingPolicy,
    pub(crate) type_result: &'a TypeCheckResult,
    pub(crate) interner: Option<&'a StringInterner>,
    pub(crate) imported_type_metadata: &'a [ori_types::ExportedTypeMetadata],
    pub(crate) imported_collection_surfaces: &'a [u64],
    pub(crate) has_analysis_only_functions: bool,
}

pub(crate) fn compute_module_repr_plan(input: ModuleReprInput<'_>) -> ori_repr::ReprPlan {
    let ModuleReprInput {
        pool,
        arc_functions,
        narrowing_policy,
        type_result,
        interner,
        imported_type_metadata,
        imported_collection_surfaces,
        has_analysis_only_functions,
    } = input;
    let repr_attrs: Vec<(Idx, ReprAttrKind)> = type_result
        .typed
        .types
        .iter()
        .filter_map(|te| te.repr.map(|r| (te.idx, r)))
        .collect();

    // INVARIANT: public field layouts cannot be integer-narrowed.
    let mut pub_type_indices: Vec<Idx> = type_result
        .typed
        .types
        .iter()
        .filter(|te| te.visibility == Visibility::Public)
        .map(|te| te.idx)
        .collect();

    // INVARIANT: public collection element layouts retain their caller-visible widths.
    ori_types::collect_public_collection_types(
        pool,
        &type_result.typed.functions,
        &mut pub_type_indices,
    );

    // INVARIANT: range analysis consumes qualified public and trait-impl names.
    let unconstrained_fn_names = ori_repr::collect_unconstrained_fn_names(
        &type_result.typed.functions,
        &type_result.typed.trait_impl_fn_names,
        interner,
    );

    ori_repr::compute_repr_plan_with_interner(
        pool,
        arc_functions,
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
mod tests;
