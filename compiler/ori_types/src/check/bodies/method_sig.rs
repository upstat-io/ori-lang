//! Shared method-binder allocation and `FunctionSig` construction for the
//! impl and def-impl body passes.

use ori_ir::{ExprId, ImplMethod, Name, Param, TraitBound};
use rustc_hash::FxHashMap;

use crate::check::signatures::resolve_const_param_type;
use crate::check::ModuleChecker;
use crate::output::ConstParamInfo;
use crate::{FunctionSig, Idx, Pool};

/// Result of `allocate_method_binders` — bundles the four method-binder data
/// products: the substitution map (binder name → fresh `RigidVar` Idx), the
/// type-generic param-name list, per-const-generic info, and inline trait-bound
/// assumptions per non-const binder. Tuple-typedef keeps the helper signature
/// readable; clippy `type_complexity` flagged the bare 4-tuple.
pub(super) type MethodBinderInfo = (
    FxHashMap<Name, Idx>,
    Vec<Name>,
    Vec<ConstParamInfo>,
    Vec<(Idx, Vec<Name>)>,
);

/// Build a `FunctionSig` from the resolved method parameters and return type.
///
/// Used by both `check_impl_method` and `check_def_impl_method` to construct the
/// signature eagerly enough for `PC-2` validator coverage of param/return
/// `Tag::Var` positions. The method body is not yet checked when this helper runs —
/// `param_types` and `return_type` are the types resolved from the AST annotations
/// (with `Tag::Var` for unannotated positions).
///
/// The `type_params: &[Name]` parameter type matches `FunctionSig.type_params:
/// Vec<Name>` (`compiler/ori_types/src/output/mod.rs`) and the caller's in-scope
/// generic-parameter vocabulary (`check_impl_method` already receives
/// `type_params: &[Name]`). The AST-level `TypeParam` node is deliberately NOT
/// accepted here — it is the declaration-site form, not the signature-level
/// identifier form.
pub(super) fn build_method_sig(
    method_name: Name,
    params: &[Param],
    param_types: Vec<Idx>,
    return_type: Idx,
    type_params: &[Name],
    const_params: Vec<ConstParamInfo>,
    pool: &Pool,
) -> FunctionSig {
    let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();
    let param_defaults: Vec<Option<ExprId>> = params.iter().map(|p| p.default).collect();
    let required_params = param_defaults.iter().filter(|d| d.is_none()).count();
    let param_hashes: Vec<u64> = param_types.iter().map(|&idx| pool.hash(idx)).collect();
    let return_hash = pool.hash(return_type);
    FunctionSig {
        name: method_name,
        type_params: type_params.to_vec(),
        const_params,
        param_names,
        param_types,
        return_type,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params,
        param_defaults,
        param_hashes,
        return_hash,
    }
}

/// Allocate fresh `RigidVar`s for a method's type-level generics.
///
/// `pool.rigid_var(name)` allocates a fresh `var_id` per call, so the
/// resulting `Idx` is distinct from any other `Tag::RigidVar` or
/// `Tag::Named` with the same name. Binder-identity guarantee:
/// method-level `T@method` and impl-level `T@impl` resolve to distinct
/// pool entries even when names collide.
///
/// Returns `(method_substitutions, method_generic_param_names)`. Callers use
/// the substitution map as the overlay for `resolve_type_with_method_generics`
/// and the param-name vec to build the combined resolver scope (impl-level +
/// method-level) and to bind the `RigidVar`s in `param_env` for body-level
/// type-annotation lookups.
pub(super) fn allocate_method_binders(
    checker: &mut ModuleChecker<'_>,
    method: &ImplMethod,
) -> MethodBinderInfo {
    let generic_params = checker.arena().get_generic_params(method.generics).to_vec();
    let method_generic_params: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();
    let mut method_substitutions: FxHashMap<Name, Idx> = FxHashMap::default();
    for &mname in &method_generic_params {
        let rigid_idx = checker.pool_mut().rigid_var(mname);
        method_substitutions.insert(mname, rigid_idx);
    }
    // Phase B-Residual-2 (a) — collect method-level const generics. Their names are
    // bound into `param_env` at body-check entry as their declared type (`int` /
    // `bool`) so the body can reference them as values, mirroring the top-level
    // function pattern in `check/bodies/functions.rs`.
    let const_params: Vec<ConstParamInfo> = generic_params
        .iter()
        .filter(|p| p.is_const)
        .map(|p| ConstParamInfo {
            name: p.name,
            const_type: resolve_const_param_type(checker, p),
            default_value: p.default_value,
        })
        .collect();
    // Phase B-Residual-2 (c) — collect inline `<T: Bound>` trait-bound assumptions
    // per non-const binder. Each entry pairs the binder's allocated RigidVar Idx
    // with the simple Names of every declared trait bound. The body-check entry
    // registers these on the InferEngine so body-internal trait dispatch (e.g.,
    // `Printable.to_str(prefix)` in string interpolation) treats `prefix : T` as
    // satisfying the bound. Empty-bounds entries are skipped to keep the table
    // tight.
    let inline_bounds: Vec<(Idx, Vec<Name>)> = generic_params
        .iter()
        .filter(|p| !p.is_const && !p.bounds.is_empty())
        .map(|p| {
            let rigid_idx = method_substitutions[&p.name];
            let trait_names: Vec<Name> = p.bounds.iter().map(TraitBound::name).collect();
            (rigid_idx, trait_names)
        })
        .collect();
    (
        method_substitutions,
        method_generic_params,
        const_params,
        inline_bounds,
    )
}
