//! Imported impl-method templates re-interned into the consumer pool.

use ori_types::{FunctionSig, Idx, MethodProducer, Pool};
use rustc_hash::FxHashMap;

/// Re-interned provider template for one imported impl method.
#[derive(Clone, Debug)]
pub(crate) struct ImportedImplTemplate {
    pub(super) producer: MethodProducer,
    pub(super) signature: FunctionSig,
    pub(super) receiver: Idx,
    pub(super) receiver_body: Option<Idx>,
    pub(super) module_index: usize,
    pub(super) source_body: ori_ir::ExprId,
    pub(super) impl_type_params: Vec<ori_ir::Name>,
    pub(super) method_type_params: Vec<ori_ir::Name>,
}

/// Source-module inputs for imported impl template reconstruction.
#[derive(Clone, Copy)]
pub(crate) struct ImportedImplTemplateSource<'a> {
    pub(crate) parse: &'a crate::parser::ParseOutput,
    pub(crate) typed: &'a ori_types::TypedModule,
    pub(crate) source_pool: &'a Pool,
    pub(crate) module_index: usize,
    pub(crate) module_identity: &'a str,
}

/// Reconstruct exact imported impl templates in merged-pool coordinates.
pub(crate) fn collect_imported_impl_templates(
    source: ImportedImplTemplateSource<'_>,
    merged_pool: &mut Pool,
    cache: &mut FxHashMap<Idx, Idx>,
    var_remap: &mut FxHashMap<u32, u32>,
    interner: &crate::ir::StringInterner,
) -> Vec<ImportedImplTemplate> {
    let mut templates = Vec::new();
    for (impl_index, implementation) in source.parse.module.impls.iter().enumerate() {
        let impl_type_params = source
            .parse
            .arena
            .get_generic_params(implementation.generics)
            .iter()
            .filter(|generic| !generic.is_const)
            .map(|generic| generic.name)
            .collect::<Vec<_>>();
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
            let source_receiver = signature.receiver;
            let signature = ori_types::re_intern_sig_with_var_remap(
                &signature.sig,
                source.source_pool,
                merged_pool,
                cache,
                var_remap,
            );
            let receiver = ori_types::re_intern_type_with_var_remap(
                source.source_pool,
                source_receiver,
                merged_pool,
                cache,
                var_remap,
            );
            let receiver_body =
                re_intern_receiver_body(source, source_receiver, merged_pool, cache, var_remap);
            templates.push(ImportedImplTemplate {
                producer,
                signature,
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
    let receiver_name = super::nominal_type_name(source.source_pool, receiver)?;
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
