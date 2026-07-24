//! Focused pins for imported monomorphization state.

use std::path::PathBuf;
use std::sync::Arc;

use ori_ir::{Name, Span, StringInterner};
use ori_types::{
    CapabilityParam, FunctionSig, Idx, MonoInstance, Pool, TypeCheckResult, TypedModule,
};
use rustc_hash::FxHashMap;

use super::{re_intern_generic_function_sigs, retain_generic_templates};
use crate::imports::{ImportedFunctionRef, ResolvedImportedModule, ResolvedImports};

fn resolved_import(
    local_name: Name,
    source_name: Name,
    interner: &StringInterner,
) -> ResolvedImports {
    let tokens = crate::lex("", interner);
    let parse_output = crate::parser::parse(&tokens, interner);

    ResolvedImports {
        prelude: None,
        modules: vec![ResolvedImportedModule {
            parse_output,
            module_path: PathBuf::from("provider.ori"),
            source_file: None,
            import_index: 0,
        }],
        imported_functions: vec![ImportedFunctionRef {
            local_name,
            original_name: source_name,
            module_index: 0,
            is_module_alias: false,
            span: Span::DUMMY,
        }],
        imported_constants: Vec::new(),
        imported_types: Vec::new(),
        errors: Vec::new(),
    }
}

#[test]
fn capability_only_imported_template_retains_provider_specialization() {
    let interner = StringInterner::new();
    let local_name = interner.intern("local_counter_user");
    let source_name = interner.intern("counter_user");
    let capability = interner.intern("Counter");
    let mut source_pool = Pool::new();
    let provider_type = source_pool.fresh_named_var(capability);
    let provider_var_id = source_pool.data(provider_type);
    let mut signature = FunctionSig::simple(source_name, Vec::new(), Idx::INT);
    signature.capabilities.push(capability);
    signature.capability_params.push(CapabilityParam::Value {
        capability,
        provider_type,
        provider_var_id,
    });
    signature.is_public = true;

    let mut provider_module = TypedModule::default();
    provider_module.functions.push(signature);
    let imported_type_results = [TypeCheckResult::ok(provider_module)];
    let imported_pools = [Arc::new(source_pool)];
    let resolved_imports = resolved_import(local_name, source_name, &interner);
    let mut merged_pool = Pool::new();
    let mut per_module_caches = [FxHashMap::default()];
    let mut per_module_var_remaps = [FxHashMap::default()];

    let signatures = re_intern_generic_function_sigs(
        &resolved_imports,
        &imported_type_results,
        &imported_pools,
        &mut merged_pool,
        &mut per_module_caches,
        &mut per_module_var_remaps,
    );
    let templates = retain_generic_templates(&signatures, &imported_type_results);
    let [template] = templates.as_slice() else {
        panic!("the capability-only import must remain available as one deferred template")
    };
    let Some(CapabilityParam::Value {
        provider_type: retained_provider_type,
        ..
    }) = template.signature.capability_params.first().copied()
    else {
        panic!("the deferred template must retain its value-capability provider binder")
    };
    assert!(template.signature.type_params.is_empty());
    assert!(template.signature.requires_specialization());

    let concrete_provider = merged_pool.struct_type(interner.intern("ConcreteCounter"), &[]);
    let mut consumer_module = TypedModule::default();
    consumer_module
        .mono_instances
        .push(MonoInstance::new_top_level_with_capabilities(
            local_name,
            Vec::new(),
            vec![concrete_provider],
            Vec::new(),
            Idx::INT,
            Vec::new(),
        ));
    let materialized = crate::commands::build_imported_mono_functions_for_test_runner(
        &TypeCheckResult::ok(consumer_module),
        &signatures,
        &[],
        &per_module_caches,
        &mut merged_pool,
        &interner,
    );
    let [function] = materialized.as_slice() else {
        panic!("later capability demand must materialize exactly one imported callable")
    };
    assert_eq!(
        function.function.identity.capability_args(),
        &[concrete_provider]
    );
    assert_eq!(
        function.function.body_type_map.get(&retained_provider_type),
        Some(&concrete_provider)
    );
}
