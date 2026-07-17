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
#[derive(Clone, Copy)]
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
