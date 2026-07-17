//! Monomorphized-function collection and signature lookup.

use rustc_hash::{FxBuildHasher, FxHashMap};

use ori_ir::canon::MonoInstanceId;
use ori_ir::{DerivedImplId, Name, StringInterner};
use ori_types::{
    AcceptedDerivedImpl, FunctionSig, Idx, ImplMethodId, ImplSig, MethodProducer, MonoInstance,
    Pool,
};

use super::{
    concrete_sig_for_instance, mangle_mono_name, nominal_type_name, ImportSig, MonoFunction,
    MonoFunctionIdentity, MonoFunctionOrigin,
};

#[derive(Clone, Copy)]
struct ResolvedMonoSignature<'a> {
    signature: &'a FunctionSig,
    is_imported: bool,
    origin: MonoFunctionOrigin,
}

struct MonoSignatureLookup<'a> {
    functions: FxHashMap<Name, &'a FunctionSig>,
    methods: FxHashMap<ImplMethodId, &'a ImplSig>,
    derived_methods: FxHashMap<DerivedImplId, &'a AcceptedDerivedImpl>,
    imports: FxHashMap<Name, &'a FunctionSig>,
}

impl<'a> MonoSignatureLookup<'a> {
    fn new(
        function_sigs: &'a [FunctionSig],
        impl_sigs: &'a [ImplSig],
        accepted_derives: &'a [AcceptedDerivedImpl],
        import_sigs: &'a [ImportSig],
        _pool: &Pool,
    ) -> Self {
        let functions = function_sigs.iter().map(|sig| (sig.name, sig)).collect();
        let mut methods = FxHashMap::with_capacity_and_hasher(impl_sigs.len(), FxBuildHasher);
        for signature in impl_sigs {
            methods.entry(signature.id).or_insert(signature);
        }
        let mut derived_methods =
            FxHashMap::with_capacity_and_hasher(accepted_derives.len(), FxBuildHasher);
        for accepted in accepted_derives {
            derived_methods.entry(accepted.id).or_insert(accepted);
        }
        let mut imports = FxHashMap::with_capacity_and_hasher(import_sigs.len(), FxBuildHasher);
        for ImportSig { name, sig, .. } in import_sigs {
            imports.entry(*name).or_insert(sig);
        }
        Self {
            functions,
            methods,
            derived_methods,
            imports,
        }
    }

    fn resolve(&self, instance: &MonoInstance, _pool: &Pool) -> Option<ResolvedMonoSignature<'a>> {
        if let Some(producer) = &instance.method_producer {
            match producer {
                MethodProducer::Impl(id) => {
                    self.methods
                        .get(id)
                        .map(|implementation| ResolvedMonoSignature {
                            signature: &implementation.sig,
                            is_imported: false,
                            origin: MonoFunctionOrigin::Impl(*id),
                        })
                }
                MethodProducer::Derived(id) => {
                    self.derived_methods
                        .get(id)
                        .map(|accepted| ResolvedMonoSignature {
                            signature: &accepted.signature,
                            is_imported: false,
                            origin: MonoFunctionOrigin::Derived(*id),
                        })
                }
                MethodProducer::Registry(_)
                | MethodProducer::Prelude(_)
                | MethodProducer::Imported { .. } => None,
            }
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
    signatures: &MonoSignatureLookup<'_>,
    instance: &MonoInstance,
    interner: &StringInterner,
) {
    let name_str = interner.lookup(instance.fn_name);
    tracing::debug!(
        target: "ori_llvm::mono",
        fn_name = ?instance.fn_name,
        name = name_str,
        is_method = instance.receiver_type.is_some(),
        impl_sig_keys = signatures.methods.len(),
        method_producer = ?instance.method_producer,
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
        origin: resolved.origin,
        identity: MonoFunctionIdentity::new(instance, instance_id),
        sig: concrete_sig,
        body_type_map,
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
    let signatures = MonoSignatureLookup::new(
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
            log_unknown_mono_instance(&signatures, instance, interner);
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
            result[existing].identity.push_instance_id(instance_id);
            continue;
        }

        let mono_function =
            build_mono_function(instance, instance_id, resolved, mangled_name, pool);
        name_to_index.insert(mangled_name, result.len());
        result.push(mono_function);
    }

    result
}
