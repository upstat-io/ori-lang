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
    allocate_generic_binders(checker, method.generics, None)
}

/// Shared rigid-binder allocation for a `GenericParamRange` — the SSOT core of
/// `allocate_method_binders`, used for BOTH method-level (`@m<U>`) and
/// impl-level (`impl<T: Bound>`) generics. Allocating impl-level generics as
/// fresh `RigidVar`s (vs the `Tag::Named` fallback at
/// `registration/type_resolution.rs`) is what makes a body-internal
/// `receiver.method()` on an impl-level type parameter dispatch via the
/// bound-chain (impl-level bound registered) or surface a method-not-found
/// (impl-level param unbounded) — both impossible while impl-level `T` stayed
/// an unresolved `Tag::Named`.
pub(super) fn allocate_generic_binders(
    checker: &mut ModuleChecker<'_>,
    generics: ori_ir::GenericParamRange,
    prealloc: Option<&FxHashMap<Name, Idx>>,
) -> MethodBinderInfo {
    let generic_params = checker.arena().get_generic_params(generics).to_vec();
    let method_generic_params: Vec<Name> = generic_params
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();
    // `prealloc = Some(map)` REUSES `RigidVar`s allocated earlier (impl-level
    // binders allocated at `register_impls`, Pass 0c); `None` allocates fresh
    // (method-level binders, allocated here at Pass 4). Reuse is what lets a
    // method mono recorded at a Pass-3 call site see the impl `RigidVar` in
    // `var_states` — the impl binder already exists by Pass 0c.
    let method_substitutions: FxHashMap<Name, Idx> = match prealloc {
        Some(map) => map.clone(),
        None => allocate_rigid_vars_for_names(checker, &method_generic_params),
    };
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

/// Allocate one fresh `RigidVar` per name and return the `name → Idx` map. SSOT
/// for the "binder name → `RigidVar`" allocation loop shared by
/// `allocate_generic_binders` (Pass-4 method binders) and
/// `allocate_rigid_var_map` (Pass-0c impl binders).
fn allocate_rigid_vars_for_names(
    checker: &mut ModuleChecker<'_>,
    names: &[Name],
) -> FxHashMap<Name, Idx> {
    let mut map = FxHashMap::default();
    for &name in names {
        let rigid_idx = checker.pool_mut().rigid_var(name);
        map.insert(name, rigid_idx);
    }
    map
}

/// Allocate the impl-level `RigidVar` substitution map (non-const binders only)
/// for one impl block's `generics`. Called by `register_impls` (Pass 0c) so the
/// `RigidVar`s exist before any body pass records a method mono; `check_impl_block`
/// (Pass 4) reuses the stored map via `allocate_generic_binders`'s `prealloc`.
pub(crate) fn allocate_rigid_var_map(
    checker: &mut ModuleChecker<'_>,
    generics: ori_ir::GenericParamRange,
) -> FxHashMap<Name, Idx> {
    let names: Vec<Name> = checker
        .arena()
        .get_generic_params(generics)
        .iter()
        .filter(|p| !p.is_const)
        .map(|p| p.name)
        .collect();
    allocate_rigid_vars_for_names(checker, &names)
}
