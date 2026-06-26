//! Monomorphization collection pass.
//!
//! Transforms [`MonoInstance`] records (from the type checker) into
//! [`MonoFunction`] values ready for the LLVM pipeline. Each `MonoFunction`
//! carries a mangled name and a fully-concrete [`FunctionSig`] — existing
//! `declare_function()` / `define_function_body()` work unchanged.

use rustc_hash::{FxBuildHasher, FxHashMap};

use ori_ir::canon::{CanId, CanonResult, MonoInstanceId};
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, Idx, MonoInstance, Pool, Tag};

mod mangle;
pub use mangle::mangle_mono_name;

// MonoFunction

/// A monomorphized function ready for LLVM codegen.
///
/// Produced by [`collect_mono_functions`] from type-checker `MonoInstance` records.
/// The `mangled_name` is unique per (function, type args) combination.
#[derive(Clone, Debug)]
pub struct MonoFunction {
    /// Mangled name for the specialization (e.g., `identity$m$int`).
    pub mangled_name: Name,
    /// Original generic function name (for canonical IR body lookup).
    pub original_name: Name,
    /// Concrete function signature (non-generic: empty `type_params`).
    pub sig: FunctionSig,
    /// Generic `Idx` → concrete `Idx` map for ARC lowering.
    pub body_type_map: FxHashMap<Idx, Idx>,
    /// Source `MonoInstance` indices that dedup to this entry.
    ///
    /// Multiple `MonoInstance` records may collapse to one `MonoFunction` when
    /// they share the same mangled name (e.g., two call sites instantiating
    /// `identity` at `int`). Each id is the position of a `MonoInstance` in
    /// the slice passed to [`collect_mono_functions`]. Consumed by
    /// `declare_mono_functions` to populate `CodegenContext.mono_dispatch_by_id`
    /// for the abstract-index dispatch path.
    pub instance_ids: Vec<MonoInstanceId>,
    /// True when this instance was resolved via the `import_sigs` lookup chain
    /// (i.e., the generic function is defined in another module).
    ///
    /// Test-observable provenance marker: no production consumer branches on
    /// this field, and the body-import path does not branch on it — local and
    /// imported mono functions flow through identical `declare_mono_functions`
    /// + `prepare_mono_cached` plumbing.
    pub is_imported: bool,
    /// Nominal type name of the receiver for an inherent-method specialization
    /// (`Some("Box")` for `impl<T> Box<T> { @unwrap }`), else `None` for a
    /// free-function specialization. Selects the canon-body namespace: method
    /// bodies are keyed by `(type_name, method_name)` via `method_root_for`,
    /// free-function bodies by `root_for`.
    pub receiver_type_name: Option<Name>,
}

impl MonoFunction {
    /// Resolve the canonical body root for this specialization.
    ///
    /// Method specializations look up `method_root_for(type_name, name)` (the
    /// impl-method body namespace); free functions use `root_for(name)`.
    #[must_use]
    pub fn body_root(&self, canon: &CanonResult) -> CanId {
        match self.receiver_type_name {
            Some(type_name) => canon
                .method_root_for(type_name, self.original_name)
                .or_else(|| canon.root_for(self.original_name))
                .unwrap_or(canon.root),
            None => canon.root_for(self.original_name).unwrap_or(canon.root),
        }
    }
}

/// Nominal type name of a (possibly generic-instantiated) user type, for the
/// impl-method body-namespace lookup. Returns `None` for non-nominal types.
fn nominal_type_name(pool: &Pool, idx: Idx) -> Option<Name> {
    let resolved = pool.resolve_fully(idx);
    match pool.tag(resolved) {
        Tag::Applied => Some(pool.applied_name(resolved)),
        Tag::Struct => Some(pool.struct_name(resolved)),
        Tag::Enum => Some(pool.enum_name(resolved)),
        Tag::Named => Some(pool.named_name(resolved)),
        _ => None,
    }
}

// Collection

/// Collect monomorphized functions from type-checker `MonoInstance` records.
///
/// Builds one deduped `MonoFunction` (mangled name + concrete sig) per
/// unique instance. Lookup chain by instance shape: top-level instances
/// (`receiver_type = None`) consult `function_sigs`, then `import_sigs`;
/// method instances consult `impl_sigs` only (imported methods are out of
/// scope for this chain). Instances whose generic function is in no list
/// are silently skipped (it may live in an uncompiled module).
pub fn collect_mono_functions(
    mono_instances: &[MonoInstance],
    function_sigs: &[FunctionSig],
    impl_sigs: &[(Idx, Name, FunctionSig)],
    import_sigs: &[(Name, FunctionSig)],
    interner: &StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction> {
    // INVARIANT: receiver-type discrimination is enforced upstream by MonoInstance dedup.
    let fn_sig_by_name: FxHashMap<Name, &FunctionSig> =
        function_sigs.iter().map(|s| (s.name, s)).collect();
    // Inherent-method sigs are keyed by (method_name, receiver generic shell).
    // The shell (`Box<_>`) discriminates per-receiver impl blocks so
    // `Box<int>.unwrap` and `Box<str>.unwrap` resolve to distinct mono
    // functions instead of colliding on a name-only first-match.
    //
    // `shell_pool` is a dedicated interning context: shells are content-
    // addressed there, leaving the shared read-only `pool` untouched. The
    // owning impl receiver type (`Box<T>`, threaded as the impl-sig triple's
    // first element) carries the impl block's receiver pattern; its shell
    // matches every concrete receiver's shell at lookup. Keying on the receiver
    // — NOT `sig.param_types.first()` — is load-bearing for a no-`self`
    // associated function, whose first param is a VALUE param, not the receiver.
    let mut shell_pool = Pool::new();
    let mut impl_sig_by_name: FxHashMap<(Name, Option<Idx>), &FunctionSig> =
        FxHashMap::with_capacity_and_hasher(impl_sigs.len(), FxBuildHasher);
    for (receiver, name, sig) in impl_sigs {
        let shell = Some(shell_pool.generic_shell(pool, *receiver));
        impl_sig_by_name.entry((*name, shell)).or_insert(sig);
    }
    // Consulted after `function_sigs` misses on the top-level path; first registration wins.
    let mut import_sig_by_name: FxHashMap<Name, &FunctionSig> =
        FxHashMap::with_capacity_and_hasher(import_sigs.len(), FxBuildHasher);
    for (name, sig) in import_sigs {
        import_sig_by_name.entry(*name).or_insert(sig);
    }

    let mut result: Vec<MonoFunction> = Vec::with_capacity(mono_instances.len());
    let mut name_to_index: FxHashMap<Name, usize> = FxHashMap::default();

    tracing::debug!(
        target: "ori_llvm::mono",
        instance_count = mono_instances.len(),
        names = ?mono_instances
            .iter()
            .map(|i| (interner.lookup(i.fn_name), i.receiver_type.is_some()))
            .collect::<Vec<_>>(),
        "collect_mono_functions: instances received"
    );

    #[expect(
        clippy::cast_possible_truncation,
        reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
    )]
    for (idx, instance) in mono_instances.iter().enumerate() {
        let instance_id = MonoInstanceId::new(idx as u32);
        let (lookup, is_imported) = if instance.receiver_type.is_some() {
            let shell = instance
                .receiver_type
                .map(|r| shell_pool.generic_shell(pool, r));
            (impl_sig_by_name.get(&(instance.fn_name, shell)), false)
        } else if let Some(sig) = fn_sig_by_name.get(&instance.fn_name) {
            (Some(sig), false)
        } else {
            (import_sig_by_name.get(&instance.fn_name), true)
        };
        let Some(generic_sig) = lookup else {
            let name_str = interner.lookup(instance.fn_name);
            tracing::debug!(
                target: "ori_llvm::mono",
                fn_name = ?instance.fn_name,
                name = name_str,
                is_method = instance.receiver_type.is_some(),
                lookup_shell = ?instance.receiver_type.map(|r| shell_pool.generic_shell(pool, r)),
                impl_sig_keys = impl_sig_by_name.len(),
                impl_shells = ?impl_sig_by_name.keys().collect::<Vec<_>>(),
                "mono instance for unknown function — skipping"
            );
            continue;
        };

        let mangled_name = mangle_mono_name(
            instance.fn_name,
            &instance.generic_args,
            &instance.impl_args,
            &instance.method_args,
            instance.receiver_type,
            interner,
            pool,
        );

        // Why: colliding instances append their id so the abstract-index
        // dispatch table maps every contributing instance to the survivor.
        if let Some(&existing) = name_to_index.get(&mangled_name) {
            result[existing].instance_ids.push(instance_id);
            continue;
        }

        let concrete_sig = concrete_sig_for_instance(instance, generic_sig, pool, mangled_name);

        let receiver_type_name = instance
            .receiver_type
            .and_then(|r| nominal_type_name(pool, r));
        let mut body_type_map: FxHashMap<Idx, Idx> =
            instance.body_type_map.iter().copied().collect();
        // For a method WITH `self`, the body references the generic receiver
        // type (`Box<T>`) via `self`-projections; map it to the concrete
        // receiver (`Box<int>`) so those projections resolve to the
        // monomorphized layout. The generic sig keeps `self` as `param_types[0]`
        // exactly when it has one MORE param than the instance's non-`self`
        // concrete params (the same signal `concrete_sig_for_instance` uses for
        // `receiver_self`). A no-`self` associated function has NO self
        // projection — `param_types.first()` is a VALUE param, not the receiver —
        // so this mapping is skipped (its body types come from
        // `instance.body_type_map` alone).
        let has_self_receiver =
            instance.concrete_param_types.len() + 1 == generic_sig.param_types.len();
        if let (Some(recv), true, Some(&self_generic)) = (
            instance.receiver_type,
            has_self_receiver,
            generic_sig.param_types.first(),
        ) {
            body_type_map.entry(self_generic).or_insert(recv);
        }
        name_to_index.insert(mangled_name, result.len());
        result.push(MonoFunction {
            mangled_name,
            original_name: instance.fn_name,
            sig: concrete_sig,
            body_type_map,
            instance_ids: vec![instance_id],
            is_imported,
            receiver_type_name,
        });
    }

    result
}

/// Build the concrete (non-generic) [`FunctionSig`] for one mono instance.
///
/// Copies non-generic metadata (param names, capabilities, defaults,
/// `is_fbip`, `required_params`) from `generic_sig`, substitutes the
/// instance's concrete param / return types plus their pool Merkle hashes,
/// and empties every generic-only field so `is_generic()` returns false.
/// Shared by [`collect_mono_functions`] (local monos) and the driver's
/// imported-mono builder (cross-module monos on the merged pool).
pub fn concrete_sig_for_instance(
    instance: &MonoInstance,
    generic_sig: &FunctionSig,
    pool: &Pool,
    mangled_name: Name,
) -> FunctionSig {
    // Method instances carry the non-`self` params in `concrete_param_types`
    // (the type-checker's `ImplMethodSig.params` excludes the receiver), but
    // the generic impl sig keeps `self` as `param_types[0]`. The mono'd method
    // still needs `self` as param 0 for ARC lowering, so prepend the concrete
    // receiver when the generic sig has exactly one more param than the
    // instance's non-`self` concrete params.
    let receiver_self = instance
        .receiver_type
        .filter(|_| instance.concrete_param_types.len() + 1 == generic_sig.param_types.len());
    let param_types: Vec<Idx> = match receiver_self {
        Some(recv) => {
            let mut pt = Vec::with_capacity(instance.concrete_param_types.len() + 1);
            pt.push(recv);
            pt.extend_from_slice(&instance.concrete_param_types);
            pt
        }
        None => instance.concrete_param_types.clone(),
    };
    let param_hashes: Vec<u64> = param_types.iter().map(|&idx| pool.hash(idx)).collect();
    let return_hash = pool.hash(instance.concrete_return_type);

    FunctionSig {
        name: mangled_name,
        type_params: vec![],
        const_params: vec![],
        param_names: generic_sig.param_names.clone(),
        param_types,
        return_type: instance.concrete_return_type,
        capabilities: generic_sig.capabilities.clone(),
        is_public: false, // mono specializations are internal
        is_test: false,
        is_main: false,
        is_fbip: generic_sig.is_fbip,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: generic_sig.required_params,
        param_defaults: generic_sig.param_defaults.clone(),
        param_hashes,
        return_hash,
        return_projection: None,
    }
}

#[cfg(test)]
mod tests;
