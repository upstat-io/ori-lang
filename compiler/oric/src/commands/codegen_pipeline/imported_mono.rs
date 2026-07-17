//! Build imported monomorphization `MonoFunction` structs for the AOT path.
//!
//! For each `MonoInstance` in the host module's type-check output that
//! references an imported generic, construct the concrete `MonoFunction` —
//! mangled name, concrete sig, fresh `body_type_map` — keyed to merged-pool
//! `Idx` values. The caller owns the merged pool; this module mutates it
//! via `build_mono_body_type_map` (which pre-interns scheme-var `BoundVar`
//! entries per the type pool's re-intern contract).

#[cfg(feature = "llvm")]
use rustc_hash::FxHashMap;

#[cfg(feature = "llvm")]
use ori_types::{
    FunctionSig, GenericArg, Idx, MethodProducer, MonoInstance, Pool, TypeCheckResult,
};

/// Exact source body namespace for one imported specialization.
#[cfg(feature = "llvm")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImportedMonoBody {
    /// Top-level source function, looked up by name.
    Function(ori_ir::Name),
    /// Impl method, looked up by its producer-arena source body id.
    ImplMethod(ori_ir::ExprId),
}

/// One resolved imported specialization and its exact producer body.
#[cfg(feature = "llvm")]
#[derive(Clone, Debug)]
pub(crate) struct ImportedMonoFn {
    pub(crate) function: ori_repr::monomorphize::MonoFunction,
    pub(crate) module_index: usize,
    pub(crate) body: ImportedMonoBody,
}

/// Re-interned provider template for one imported impl method.
#[cfg(feature = "llvm")]
#[derive(Clone, Debug)]
pub(crate) struct ImportedImplTemplate {
    producer: MethodProducer,
    signature: FunctionSig,
    receiver: Idx,
    receiver_body: Option<Idx>,
    module_index: usize,
    source_body: ori_ir::ExprId,
    impl_type_params: Vec<ori_ir::Name>,
    method_type_params: Vec<ori_ir::Name>,
}

/// Source-module inputs for imported impl template reconstruction.
#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
pub(crate) struct ImportedImplTemplateSource<'a> {
    pub(crate) parse: &'a crate::parser::ParseOutput,
    pub(crate) typed: &'a ori_types::TypedModule,
    pub(crate) source_pool: &'a Pool,
    pub(crate) module_index: usize,
    pub(crate) module_identity: &'a str,
}

/// Borrowed view over the imported-generic codegen surfaces produced by the
/// host's merged-pool re-interning (`ImportedMonoState`).
///
/// Threads as ONE parameter through `compile_to_llvm_with_imported_monos` /
/// `compile_to_llvm_with_imports` → `run_codegen_pipeline` →
/// the shared ARC batch lowerer. The merged pool stays a separate parameter — its
/// `'ctx` lifetime ties to the LLVM context.
#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
pub(crate) struct ImportedSurfaces<'a> {
    /// One exact source owner per unique imported mono instance.
    pub(crate) imported_mono_fns: &'a [ImportedMonoFn],
    /// Per-imported-module canons re-interned into merged-pool coordinates;
    /// indexed by `imported_mono_fns[i].module_index`.
    pub(crate) re_interned_canons: &'a [ori_ir::canon::CanonResult],
}

/// Build imported monomorphization functions for the AOT path.
///
/// Returns one entry per unique imported mono instance. Dedups by mangled
/// name; skips instances whose `fn_name` is not in `imported_generic_sigs`.
#[cfg(feature = "llvm")]
pub(crate) fn build_imported_mono_functions(
    type_result: &TypeCheckResult,
    imported_generic_sigs: &FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    imported_impl_templates: &[ImportedImplTemplate],
    // Why: caches retained for signature stability; var-id watermark sourced from `Pool::next_var_id`.
    _per_module_caches: &[FxHashMap<Idx, Idx>],
    merged_pool: &mut Pool,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedMonoFn> {
    let mut imported_mono_fns: Vec<ImportedMonoFn> = Vec::new();
    let mut name_to_index: FxHashMap<ori_ir::Name, usize> = FxHashMap::default();

    #[expect(
        clippy::cast_possible_truncation,
        reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
    )]
    for (idx, instance) in type_result.typed.mono_instances.iter().enumerate() {
        let instance_id = ori_ir::canon::MonoInstanceId::new(idx as u32);

        // Defense-in-depth: a poison MonoInstance (a generic/impl/method arg
        // carrying a type-error) that slipped the ori_types record gates must
        // never materialize — its body codegens method invokes on the poison
        // receiver (AOT missing-mono). Read the canonical TypeFlags::is_recordable
        // predicate, not a bare Idx::ERROR slot compare — is_recordable also
        // refuses compound poison (List<Idx::ERROR> via HAS_ERROR on PROPAGATE_MASK)
        // and residual var/infer args. is_recordable in ori_types is the primary cure.
        if instance
            .generic_args
            .iter()
            .chain(instance.impl_args.iter())
            .chain(instance.method_args.iter())
            .any(|a| matches!(a, GenericArg::Type(t) if !merged_pool.flags(*t).is_recordable()))
        {
            continue;
        }

        let Some(template) =
            resolve_imported_template(instance, imported_generic_sigs, imported_impl_templates)
        else {
            continue;
        };

        let base_mangled = ori_repr::monomorphize::mangle_mono_name(
            instance.fn_name,
            &instance.generic_args,
            &instance.impl_args,
            &instance.method_args,
            instance.receiver_type,
            interner,
            merged_pool,
        );
        let mangled =
            imported_mangled_name(base_mangled, instance.method_producer.as_ref(), interner);
        if let Some(&existing) = name_to_index.get(&mangled) {
            imported_mono_fns[existing]
                .function
                .identity
                .push_instance_id(instance_id);
            continue;
        }

        let concrete_sig = ori_repr::monomorphize::concrete_sig_for_instance(
            instance,
            template.signature,
            merged_pool,
            mangled,
        );
        let body_type_map = match template.impl_binders {
            Some(binders) => build_method_body_type_map(
                merged_pool,
                instance,
                binders.impl_type_params,
                binders.method_type_params,
                binders.receiver,
                binders.receiver_body,
            ),
            None => build_body_type_map(merged_pool, instance, template.signature),
        };

        name_to_index.insert(mangled, imported_mono_fns.len());
        imported_mono_fns.push(ImportedMonoFn {
            function: ori_repr::monomorphize::MonoFunction {
                mangled_name: mangled,
                origin: ori_repr::monomorphize::MonoFunctionOrigin::Source,
                // The instance name is the local call-site identity (for
                // example an imported `assert_eq` aliased to `ae`).
                identity: ori_repr::monomorphize::MonoFunctionIdentity::new(instance, instance_id),
                sig: concrete_sig,
                body_type_map,
                is_imported: true,
                receiver_type_name: instance
                    .receiver_type
                    .and_then(|receiver| nominal_type_name(merged_pool, receiver)),
            },
            module_index: template.module_index,
            body: template.body,
        });
    }

    imported_mono_fns
}

#[derive(Clone, Copy)]
struct ImportedImplBinders<'a> {
    impl_type_params: &'a [ori_ir::Name],
    method_type_params: &'a [ori_ir::Name],
    receiver: Idx,
    receiver_body: Option<Idx>,
}

struct ResolvedImportedTemplate<'a> {
    signature: &'a FunctionSig,
    module_index: usize,
    body: ImportedMonoBody,
    impl_binders: Option<ImportedImplBinders<'a>>,
}

fn resolve_imported_template<'a>(
    instance: &MonoInstance,
    imported_generic_sigs: &'a FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    imported_impl_templates: &'a [ImportedImplTemplate],
) -> Option<ResolvedImportedTemplate<'a>> {
    if let Some(producer @ MethodProducer::Imported { .. }) = &instance.method_producer {
        let mut matches = imported_impl_templates
            .iter()
            .filter(|template| &template.producer == producer);
        let template = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        return Some(ResolvedImportedTemplate {
            signature: &template.signature,
            module_index: template.module_index,
            body: ImportedMonoBody::ImplMethod(template.source_body),
            impl_binders: Some(ImportedImplBinders {
                impl_type_params: &template.impl_type_params,
                method_type_params: &template.method_type_params,
                receiver: template.receiver,
                receiver_body: template.receiver_body,
            }),
        });
    }
    if instance.method_producer.is_some() {
        return None;
    }
    let (signature, module_index, source_name) = imported_generic_sigs.get(&instance.fn_name)?;
    Some(ResolvedImportedTemplate {
        signature,
        module_index: *module_index,
        body: ImportedMonoBody::Function(*source_name),
        impl_binders: None,
    })
}

fn imported_mangled_name(
    base: ori_ir::Name,
    producer: Option<&MethodProducer>,
    interner: &crate::ir::StringInterner,
) -> ori_ir::Name {
    let Some(MethodProducer::Imported {
        symbol,
        signature_hash,
    }) = producer
    else {
        return base;
    };
    let symbol_hash = stable_symbol_hash(symbol);
    interner.intern(&format!(
        "{}$ip${symbol_hash:016x}{signature_hash:016x}",
        interner.lookup(base)
    ))
}

fn stable_symbol_hash(symbol: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in symbol.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn nominal_type_name(pool: &Pool, receiver: Idx) -> Option<ori_ir::Name> {
    let resolved = pool.resolve_fully(receiver);
    match pool.tag(resolved) {
        ori_types::Tag::Applied => Some(pool.applied_name(resolved)),
        ori_types::Tag::Struct => Some(pool.struct_name(resolved)),
        ori_types::Tag::Enum => Some(pool.enum_name(resolved)),
        ori_types::Tag::Named => Some(pool.named_name(resolved)),
        _ => None,
    }
}

fn build_method_body_type_map(
    pool: &mut Pool,
    instance: &MonoInstance,
    impl_type_params: &[ori_ir::Name],
    method_type_params: &[ori_ir::Name],
    receiver: Idx,
    receiver_body: Option<Idx>,
) -> FxHashMap<Idx, Idx> {
    let mut named_bindings = Vec::new();
    for (&name, argument) in impl_type_params.iter().zip(&instance.impl_args) {
        if let GenericArg::Type(concrete) = argument {
            named_bindings.push((name, *concrete));
        }
    }
    for (&name, argument) in method_type_params.iter().zip(&instance.method_args) {
        if let GenericArg::Type(concrete) = argument {
            named_bindings.push((name, *concrete));
        }
    }
    ori_types::build_impl_mono_body_type_map(
        pool,
        &named_bindings,
        receiver,
        receiver_body,
        instance.receiver_type,
    )
}

/// Reconstruct exact imported impl templates in merged-pool coordinates.
#[cfg(feature = "llvm")]
pub(crate) fn collect_imported_impl_templates(
    source: ImportedImplTemplateSource<'_>,
    merged_pool: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedImplTemplate> {
    let mut templates = Vec::new();
    for (impl_index, implementation) in source.parse.module.impls.iter().enumerate() {
        let impl_type_params: Vec<_> = source
            .parse
            .arena
            .get_generic_params(implementation.generics)
            .iter()
            .filter(|generic| !generic.is_const)
            .map(|generic| generic.name)
            .collect();
        for (method_index, method) in implementation.methods.iter().enumerate() {
            let id = ori_types::ImplMethodId::new(impl_index, method.body);
            let Some(signature) = source.typed.impl_sigs.iter().find(|sig| sig.id == id) else {
                continue;
            };
            let method_type_params = source
                .parse
                .arena
                .get_generic_params(method.generics)
                .iter()
                .filter(|generic| !generic.is_const)
                .map(|generic| generic.name)
                .collect();
            let producer = ori_types::imported_method_producer(
                source.module_identity,
                impl_index,
                method_index,
                method,
                &source.parse.arena,
                interner,
            );
            let re_interned_sig = ori_types::re_intern_sig_with_var_remap(
                &signature.sig,
                source.source_pool,
                merged_pool,
                cache,
                var_remap,
            );
            let receiver = ori_types::re_intern_type_with_var_remap(
                source.source_pool,
                signature.receiver,
                merged_pool,
                cache,
                var_remap,
            );
            let receiver_body =
                re_intern_receiver_body(source, signature.receiver, merged_pool, cache, var_remap);
            templates.push(ImportedImplTemplate {
                producer,
                signature: re_interned_sig,
                receiver,
                receiver_body,
                module_index: source.module_index,
                source_body: method.body,
                impl_type_params: impl_type_params.clone(),
                method_type_params,
            });
        }
    }
    templates
}

fn re_intern_receiver_body(
    source: ImportedImplTemplateSource<'_>,
    receiver: Idx,
    merged_pool: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
) -> Option<Idx> {
    let receiver_name = nominal_type_name(source.source_pool, receiver)?;
    let entry = source
        .typed
        .types
        .iter()
        .find(|entry| entry.name == receiver_name)?;
    let source_body = source.source_pool.resolve(entry.idx)?;
    Some(ori_types::re_intern_type_with_var_remap(
        source.source_pool,
        source_body,
        merged_pool,
        cache,
        var_remap,
    ))
}

/// Register every `pub` generic free function of the prelude module into
/// `imported_generic_sigs`, keyed by its source name.
///
/// Prelude generic free functions (`min`, `max`, …) are implicitly available
/// in every module but are NOT explicit `ImportedFunctionRef` entries, so the
/// explicit-import registration loop never sees them. Without this, their
/// recorded `MonoInstance`s find no generic sig in `build_imported_mono_functions`
/// and the call site emits `unresolved function ... missing mono instance`.
///
/// `prelude_module_index` is the prelude's slot in the per-module re-interning
/// arrays (`per_module_caches[prelude_module_index]` / `*_var_remaps`). The
/// caller owns the merged pool; the sig is re-interned into it via the shared
/// per-module var-remap so scheme-var ids stay coherent with the prelude's
/// re-interned canon.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "prelude sig registration threads the same re-interning state the explicit-import loop uses"
)]
pub(crate) fn register_prelude_generic_sigs(
    imported_generic_sigs: &mut FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)>,
    prelude_parse: &crate::parser::ParseOutput,
    prelude_typed: &ori_types::TypedModule,
    prelude_source_pool: &Pool,
    prelude_module_index: usize,
    merged_pool: &mut Pool,
    per_module_cache: &mut FxHashMap<Idx, Idx>,
    per_module_var_remap: &mut FxHashMap<u32, u32>,
) {
    for func in &prelude_parse.module.functions {
        let Some(sig) = prelude_typed.functions.iter().find(|s| s.name == func.name) else {
            continue;
        };
        if !sig.is_generic() {
            continue;
        }
        // First registration wins — an explicit import shadowing a prelude name
        // (e.g. `use mymod { min }`) already registered the explicit sig.
        if imported_generic_sigs.contains_key(&func.name) {
            continue;
        }
        let re_interned = ori_types::re_intern_sig_with_var_remap(
            sig,
            prelude_source_pool,
            merged_pool,
            per_module_cache,
            per_module_var_remap,
        );
        imported_generic_sigs.insert(func.name, (re_interned, prelude_module_index, func.name));
    }
}

/// Build the `body_type_map` (`generic_idx` → `concrete_idx`) for one mono
/// instance on the merged pool.
///
/// Seeds `var_subst` from `scheme_var_ids` × `generic_args`, extends it with
/// union-find root var ids via the shared SSOT helper, and delegates to
/// `build_mono_body_type_map` (which handles the `HAS_VAR|HAS_BOUND_VAR`
/// mask + scheme-var `BoundVar` pre-intern contract). Var capacity is
/// ensured up front because `substitute_in_pool` panics on out-of-bounds ids.
#[cfg(feature = "llvm")]
fn build_body_type_map(
    merged_pool: &mut Pool,
    instance: &MonoInstance,
    generic_sig: &FunctionSig,
) -> FxHashMap<Idx, Idx> {
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    for (i, &var_id) in generic_sig.scheme_var_ids.iter().enumerate() {
        if let Some(GenericArg::Type(concrete)) = instance.generic_args.get(i) {
            var_subst.insert(var_id, *concrete);
        }
    }

    // Why: `Pool::next_var_id` is the SSOT for the var_id watermark; re-deriving
    // via cache scan would scatter the lookup. `ensure_var_capacity` is idempotent
    // and guards against future re-intern paths that omit capacity extension.
    let watermark = merged_pool.next_var_id();
    merged_pool.ensure_var_capacity(watermark);

    ori_types::extend_var_subst_with_roots(
        merged_pool,
        &generic_sig.scheme_var_ids,
        &mut var_subst,
    );

    let mut body_type_map: FxHashMap<Idx, Idx> = FxHashMap::default();
    ori_types::build_mono_body_type_map(merged_pool, &var_subst, &mut body_type_map);
    body_type_map
}

#[cfg(test)]
#[cfg(feature = "llvm")]
mod tests;
