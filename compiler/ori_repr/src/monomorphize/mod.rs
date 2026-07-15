//! Backend-neutral monomorphization metadata and call-target realization.
//!
//! Transforms [`MonoInstance`] records (from the type checker) into
//! [`MonoFunction`] values ready for any executable backend. Each `MonoFunction`
//! carries a mangled name and a fully-concrete [`FunctionSig`] — existing
//! `declare_function()` / `define_function_body()` work unchanged.

use rustc_hash::{FxBuildHasher, FxHashMap};

use ori_ir::canon::{CanId, CanonResult, MonoInstanceId};
use ori_ir::{DerivedImplId, Name, StringInterner};
use ori_types::{AcceptedDerivedImpl, FunctionSig, Idx, ImplSig, MonoInstance, Pool, Tag};

use crate::executable::{
    validate_external_callables, ExternalCallable, ExternalCallableMetadata, RealizationError,
};

mod mangle;
mod targets;
pub use mangle::mangle_mono_name;
pub use targets::{callee_shadows_builtin_method, rewrite_apply_targets_for_monos, MonoTargetMaps};

// ImportSig

/// A cross-module imported function's declared signature.
///
/// `name` is the call-site local/aliased [`Name`] (the `codegen_ctx.functions`
/// key `resolve_callee` probes); `symbol` is the exporting module's exact
/// mangled symbol (never re-mangled against the host module path).
#[derive(Clone, Debug)]
pub struct ImportSig {
    pub name: Name,
    pub symbol: String,
    pub sig: FunctionSig,
    /// Required final producer facts for ownership/effect realization.
    ///
    /// There is deliberately no absent or default state: an imported callable
    /// cannot participate in AIMS until its producer artifact has supplied
    /// this carrier.
    pub metadata: ExternalCallableMetadata,
}

impl ImportSig {
    fn external_callable(&self) -> ExternalCallable {
        ExternalCallable::from_imported_metadata(
            self.name,
            self.symbol.clone(),
            self.sig.param_types.clone(),
            self.sig.return_type,
            self.metadata.clone(),
        )
    }
}

/// Reconstruct and validate every imported callable before AIMS consumes its
/// contract.
///
/// The importer-pool signature is checked against the producer's stable
/// identity, including the exact link symbol. Duplicate aliases and aliases
/// that disagree about one producer symbol are rejected as one batch.
pub fn realize_imported_callables(
    imports: &[ImportSig],
    pool: &Pool,
) -> Result<Vec<ExternalCallable>, RealizationError> {
    let callables: Vec<_> = imports.iter().map(ImportSig::external_callable).collect();
    validate_external_callables(&callables, pool)?;
    Ok(callables)
}

// MonoFunction

/// Semantic source of a monomorphized function body.
///
/// Imported and local source declarations both remain [`Self::Source`];
/// [`MonoFunction::is_imported`] retains that transport distinction. Generated
/// derived methods carry the accepted type-checker identity so later phases can
/// join the specialization to its generated Canon root without rediscovering a
/// derive from source attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonoFunctionOrigin {
    /// An ordinary source declaration, whether local or imported.
    Source,
    /// A compiler-generated method accepted by type checking and coherence.
    Derived(DerivedImplId),
}

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
    /// Semantic body provenance used to resolve the canonical body owner.
    pub origin: MonoFunctionOrigin,
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

#[derive(Clone, Copy)]
struct ResolvedMonoSignature<'a> {
    signature: &'a FunctionSig,
    is_imported: bool,
    origin: MonoFunctionOrigin,
}

struct MonoSignatureLookup<'a> {
    shell_pool: Pool,
    functions: FxHashMap<Name, &'a FunctionSig>,
    methods: FxHashMap<(Name, Option<Idx>), &'a FunctionSig>,
    derived_methods: FxHashMap<(Name, Option<Idx>), &'a AcceptedDerivedImpl>,
    imports: FxHashMap<Name, &'a FunctionSig>,
}

impl<'a> MonoSignatureLookup<'a> {
    fn new(
        function_sigs: &'a [FunctionSig],
        impl_sigs: &'a [ImplSig],
        accepted_derives: &'a [AcceptedDerivedImpl],
        import_sigs: &'a [ImportSig],
        pool: &Pool,
    ) -> Self {
        let functions = function_sigs.iter().map(|sig| (sig.name, sig)).collect();
        let mut shell_pool = Pool::new();
        let mut methods = FxHashMap::with_capacity_and_hasher(impl_sigs.len(), FxBuildHasher);
        for ImplSig {
            receiver,
            name,
            sig,
            ..
        } in impl_sigs
        {
            let shell = Some(shell_pool.generic_shell(pool, *receiver));
            methods.entry((*name, shell)).or_insert(sig);
        }
        let mut derived_methods =
            FxHashMap::with_capacity_and_hasher(accepted_derives.len(), FxBuildHasher);
        for accepted in accepted_derives {
            let shell = Some(shell_pool.generic_shell(pool, accepted.owner_type));
            derived_methods
                .entry((accepted.method_name, shell))
                .or_insert(accepted);
        }
        let mut imports = FxHashMap::with_capacity_and_hasher(import_sigs.len(), FxBuildHasher);
        for ImportSig { name, sig, .. } in import_sigs {
            imports.entry(*name).or_insert(sig);
        }
        Self {
            shell_pool,
            functions,
            methods,
            derived_methods,
            imports,
        }
    }

    fn receiver_shell(&mut self, instance: &MonoInstance, pool: &Pool) -> Option<Idx> {
        instance
            .receiver_type
            .map(|receiver| self.shell_pool.generic_shell(pool, receiver))
    }

    fn resolve(
        &mut self,
        instance: &MonoInstance,
        pool: &Pool,
    ) -> Option<ResolvedMonoSignature<'a>> {
        if instance.receiver_type.is_some() {
            let key = (instance.fn_name, self.receiver_shell(instance, pool));
            if let Some(&signature) = self.methods.get(&key) {
                return Some(ResolvedMonoSignature {
                    signature,
                    is_imported: false,
                    origin: MonoFunctionOrigin::Source,
                });
            }
            self.derived_methods
                .get(&key)
                .map(|accepted| ResolvedMonoSignature {
                    signature: &accepted.signature,
                    is_imported: false,
                    origin: MonoFunctionOrigin::Derived(accepted.id),
                })
        } else if let Some(&signature) = self.functions.get(&instance.fn_name) {
            Some(ResolvedMonoSignature {
                signature,
                is_imported: false,
                origin: MonoFunctionOrigin::Source,
            })
        } else {
            self.imports
                .get(&instance.fn_name)
                .copied()
                .map(|signature| ResolvedMonoSignature {
                    signature,
                    is_imported: true,
                    origin: MonoFunctionOrigin::Source,
                })
        }
    }
}

fn log_unknown_mono_instance(
    signatures: &mut MonoSignatureLookup<'_>,
    instance: &MonoInstance,
    interner: &StringInterner,
    pool: &Pool,
) {
    let name_str = interner.lookup(instance.fn_name);
    let lookup_shell = signatures.receiver_shell(instance, pool);
    tracing::debug!(
        target: "ori_llvm::mono",
        fn_name = ?instance.fn_name,
        name = name_str,
        is_method = instance.receiver_type.is_some(),
        ?lookup_shell,
        impl_sig_keys = signatures.methods.len(),
        impl_shells = ?signatures.methods.keys().collect::<Vec<_>>(),
        "mono instance for unknown function — skipping"
    );
}

fn build_mono_function(
    instance: &MonoInstance,
    instance_id: MonoInstanceId,
    resolved: ResolvedMonoSignature<'_>,
    mangled_name: Name,
    pool: &Pool,
) -> MonoFunction {
    let concrete_sig = concrete_sig_for_instance(instance, resolved.signature, pool, mangled_name);

    let receiver_type_name = instance
        .receiver_type
        .and_then(|receiver| nominal_type_name(pool, receiver));
    let mut body_type_map: FxHashMap<Idx, Idx> = instance.body_type_map.iter().copied().collect();
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
        instance.concrete_param_types.len() + 1 == resolved.signature.param_types.len();
    if let (Some(receiver), true, Some(&self_generic)) = (
        instance.receiver_type,
        has_self_receiver,
        resolved.signature.param_types.first(),
    ) {
        body_type_map.entry(self_generic).or_insert(receiver);
    }

    MonoFunction {
        mangled_name,
        original_name: instance.fn_name,
        origin: resolved.origin,
        sig: concrete_sig,
        body_type_map,
        instance_ids: vec![instance_id],
        is_imported: resolved.is_imported,
        receiver_type_name,
    }
}

/// Collect monomorphized functions from type-checker `MonoInstance` records.
///
/// Builds one deduped `MonoFunction` (mangled name + concrete sig) per
/// unique instance. Lookup chain by instance shape: top-level instances
/// (`receiver_type = None`) consult `function_sigs`, then `import_sigs`;
/// method instances consult ordinary `impl_sigs`, then type-checker accepted
/// derived signatures (imported methods are out of scope for this chain).
/// Instances whose generic function is in no list are silently skipped (it may
/// live in an uncompiled module).
pub fn collect_mono_functions(
    mono_instances: &[MonoInstance],
    function_sigs: &[FunctionSig],
    impl_sigs: &[ImplSig],
    accepted_derives: &[AcceptedDerivedImpl],
    import_sigs: &[ImportSig],
    interner: &StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction> {
    // INVARIANT: receiver-type discrimination is enforced upstream by MonoInstance dedup.
    // Inherent-method sigs are keyed by (method_name, receiver generic shell).
    // The shell (`Box<_>`) discriminates per-receiver impl blocks so
    // `Box<int>.unwrap` and `Box<str>.unwrap` resolve to distinct mono
    // functions instead of colliding on a name-only first-match.
    //
    // `shell_pool` is a dedicated interning context: shells are content-
    // addressed there, leaving the shared read-only `pool` untouched. The
    // owning impl receiver type (`Box<T>`, `ImplSig::receiver`) carries the
    // impl block's receiver pattern; its shell matches every concrete
    // receiver's shell at lookup. Keying on the receiver — NOT
    // `sig.param_types.first()` — is load-bearing for a no-`self` associated
    // function, whose first param is a VALUE param, not the receiver.
    let mut signatures = MonoSignatureLookup::new(
        function_sigs,
        impl_sigs,
        accepted_derives,
        import_sigs,
        pool,
    );

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
        let Some(resolved) = signatures.resolve(instance, pool) else {
            log_unknown_mono_instance(&mut signatures, instance, interner, pool);
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

        let mono_function =
            build_mono_function(instance, instance_id, resolved, mangled_name, pool);
        name_to_index.insert(mangled_name, result.len());
        result.push(mono_function);
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
