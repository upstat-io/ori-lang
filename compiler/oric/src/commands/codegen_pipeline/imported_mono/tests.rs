//! Tests for the production AOT `ImportedMonoFn` carrier + builder.
//!
//! These tests verify the carrier shape + builder behavior post-promotion
//! from the JIT test-runner equivalent at `oric/src/test/runner/imported_mono.rs`.
//! The shared production type is reachable via `crate::commands::ImportedMonoFn`;
//! the builder produces identical output to the JIT version on identical input.

use rustc_hash::FxHashMap;

use ori_types::{
    ConcreteMethodMono, FunctionSig, GenericArg, Idx, MethodProducer, MonoInstance, Pool,
    TypeCheckResult, TypedModule,
};

use super::{
    build_imported_mono_functions, ImportedImplTemplate, ImportedMonoBody, ImportedMonoFn,
};
use crate::ir::StringInterner;

fn make_generic_sig(interner: &StringInterner) -> FunctionSig {
    let name = interner.intern("identity");
    let t_name = interner.intern("T");
    let x_name = interner.intern("x");

    FunctionSig {
        name,
        type_params: vec![t_name],
        const_params: vec![],
        param_names: vec![x_name],
        param_types: vec![Idx::from_raw(100)], // generic T placeholder
        return_type: Idx::from_raw(100),
        capabilities: vec![],
        is_public: true,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![vec![]],
        where_clauses: vec![],
        generic_param_mapping: vec![Some(0)],
        scheme_var_ids: vec![0],
        required_params: 1,
        param_defaults: vec![],
        param_hashes: vec![0],
        return_hash: 0,
        return_projection: None,
    }
}

fn imported_producer(symbol: &str, signature_hash: u64) -> MethodProducer {
    MethodProducer::Imported {
        symbol: symbol.into(),
        signature_hash,
    }
}

fn make_imported_method_fixture(
    interner: &StringInterner,
    pool: &mut Pool,
    producer: MethodProducer,
    module_index: usize,
    source_body: u32,
) -> (MonoInstance, ImportedImplTemplate, Idx, Idx) {
    let method_name = interner.intern("hash");
    let type_name = interner.intern("ImportedManual");
    let type_param = interner.intern("T");
    let self_name = interner.intern("self");
    let value_name = interner.intern("value");
    let named_param = pool.named(type_param);
    let receiver = pool.applied(type_name, &[named_param]);
    let receiver_body = pool.struct_type(type_name, &[(value_name, named_param)]);
    let concrete_receiver = pool.applied(type_name, &[Idx::INT]);

    let mut signature = make_generic_sig(interner);
    signature.name = method_name;
    signature.param_names = vec![self_name];
    signature.param_types = vec![receiver];
    signature.return_type = Idx::INT;
    signature.param_hashes = vec![pool.hash(receiver)];
    signature.return_hash = pool.hash(Idx::INT);

    let instance = MonoInstance::new_method(
        method_name,
        producer.clone(),
        vec![GenericArg::Type(Idx::INT)],
        Vec::new(),
        ConcreteMethodMono {
            receiver_type: concrete_receiver,
            param_types: Vec::new(),
            return_type: Idx::INT,
            body_type_map: Vec::new(),
        },
    );
    let template = ImportedImplTemplate {
        producer,
        signature,
        receiver,
        receiver_body: Some(receiver_body),
        module_index,
        source_body: ori_ir::ExprId::new(source_body),
        impl_type_params: vec![type_param],
        method_type_params: Vec::new(),
    };
    (instance, template, concrete_receiver, receiver_body)
}

#[test]
fn carrier_is_reachable_via_crate_commands_export() {
    // ImportedMonoFn is `pub(crate)` in the production module; it must be
    // reachable from the test runner side via `crate::commands::ImportedMonoFn`.
    // This test pins the re-export.
    let interner = StringInterner::new();
    let imp_name = interner.intern("imported");
    let src_name = interner.intern("imported");
    let instance =
        MonoInstance::new_top_level(imp_name, Vec::new(), Vec::new(), Idx::INT, Vec::new());
    let mono_fn = ori_repr::monomorphize::MonoFunction {
        mangled_name: imp_name,
        origin: ori_repr::monomorphize::MonoFunctionOrigin::Source,
        identity: ori_repr::monomorphize::MonoFunctionIdentity::generated(&instance),
        sig: make_generic_sig(&interner),
        body_type_map: FxHashMap::default(),
        is_imported: true,
        receiver_type_name: None,
    };
    let entry: crate::commands::ImportedMonoFn = ImportedMonoFn {
        function: mono_fn,
        module_index: 0,
        body: ImportedMonoBody::Function(src_name),
    };
    assert_eq!(entry.module_index, 0);
    assert_eq!(entry.body, ImportedMonoBody::Function(src_name));
}

#[test]
fn build_imported_mono_functions_empty_input() {
    // Empty mono_instances + empty imported_generic_sigs → empty output.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let typed_module = TypedModule::default();
    let type_result = TypeCheckResult::ok(typed_module);
    let imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();

    let result: Vec<ImportedMonoFn> = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &[],
        &mut pool,
        &interner,
    );

    assert!(result.is_empty());
}

#[test]
fn build_imported_mono_functions_skips_local_instances() {
    // Mono instance whose fn_name is NOT in imported_generic_sigs → skipped.
    // (Local mono instances are handled by the host's regular collect_mono_functions path.)
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let local_name = interner.intern("local_fn");

    let instance = MonoInstance::new_top_level(
        local_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);
    let type_result = TypeCheckResult::ok(typed_module);

    // Empty imported_generic_sigs — local_name is not imported.
    let imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &[],
        &mut pool,
        &interner,
    );

    assert!(
        result.is_empty(),
        "local mono instances must not appear in imported_mono_fns"
    );
}

#[test]
fn build_imported_mono_functions_constructs_imported_entry() {
    // A mono instance whose fn_name IS in imported_generic_sigs → entry built.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let imported_local_name = generic_sig.name; // local == source for this test
    let source_name = generic_sig.name;

    let instance = MonoInstance::new_top_level(
        imported_local_name,
        vec![GenericArg::Type(Idx::INT)],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);
    let type_result = TypeCheckResult::ok(typed_module);

    let mut imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();
    imported_generic_sigs.insert(imported_local_name, (generic_sig, 5usize, source_name));

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(result.len(), 1);
    let entry = &result[0];
    let mono_fn = &entry.function;
    assert_eq!(
        interner.lookup(mono_fn.mangled_name),
        "identity$m$3_int",
        "mangled name follows the same scheme as collect_mono_functions"
    );
    assert_eq!(mono_fn.identity.original_name(), imported_local_name);
    assert!(mono_fn.is_imported);
    assert_eq!(entry.module_index, 5usize);
    assert_eq!(entry.body, ImportedMonoBody::Function(source_name));
}

#[test]
fn build_imported_mono_functions_dedups_by_mangled_name() {
    // Two instances with the same generic args → one MonoFunction with
    // both instance_ids appended.
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let generic_sig = make_generic_sig(&interner);
    let name = generic_sig.name;

    let mut typed_module = TypedModule::default();
    for _ in 0..2 {
        typed_module
            .mono_instances
            .push(MonoInstance::new_top_level(
                name,
                vec![GenericArg::Type(Idx::INT)],
                vec![Idx::INT],
                Idx::INT,
                Vec::new(),
            ));
    }
    let type_result = TypeCheckResult::ok(typed_module);

    let mut imported_generic_sigs: FxHashMap<ori_ir::Name, (FunctionSig, usize, ori_ir::Name)> =
        FxHashMap::default();
    imported_generic_sigs.insert(name, (generic_sig, 0usize, name));

    let result = build_imported_mono_functions(
        &type_result,
        &imported_generic_sigs,
        &[],
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(
        result.len(),
        1,
        "duplicate (fn, args) deduped by mangled name"
    );
    assert_eq!(
        result[0].function.identity.instance_ids().len(),
        2,
        "both instance_ids accumulated on the single MonoFunction"
    );
}

#[test]
fn imported_method_template_materializes_concrete_receiver_body() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let producer = imported_producer("provider-a::hash", 17);
    let (instance, template, concrete_receiver, generic_body) =
        make_imported_method_fixture(&interner, &mut pool, producer.clone(), 2, 31);
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);
    let type_result = TypeCheckResult::ok(typed_module);

    let result = build_imported_mono_functions(
        &type_result,
        &FxHashMap::default(),
        &[template],
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].function.identity.method_producer(),
        Some(&producer)
    );
    assert_eq!(result[0].module_index, 2);
    assert_eq!(
        result[0].body,
        ImportedMonoBody::ImplMethod(ori_ir::ExprId::new(31))
    );
    let Some(concrete_body) = pool.resolve(concrete_receiver) else {
        panic!("concrete imported receiver must carry its materialized body");
    };
    assert_eq!(pool.struct_fields(concrete_body)[0].1, Idx::INT);
    assert_eq!(
        result[0].function.body_type_map.get(&generic_body),
        Some(&concrete_body)
    );
}

#[test]
fn imported_method_template_fails_closed_on_stale_producer() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let selected = imported_producer("provider-a::hash", 17);
    let stale = imported_producer("provider-a::hash", 18);
    let (instance, mut template, _, _) =
        make_imported_method_fixture(&interner, &mut pool, selected, 2, 31);
    template.producer = stale;
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);

    let result = build_imported_mono_functions(
        &TypeCheckResult::ok(typed_module),
        &FxHashMap::default(),
        &[template],
        &[],
        &mut pool,
        &interner,
    );

    assert!(result.is_empty());
}

#[test]
fn imported_method_template_fails_closed_on_duplicate_producer() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let producer = imported_producer("provider-a::hash", 17);
    let (instance, template, _, _) =
        make_imported_method_fixture(&interner, &mut pool, producer, 2, 31);
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(instance);

    let result = build_imported_mono_functions(
        &TypeCheckResult::ok(typed_module),
        &FxHashMap::default(),
        &[template.clone(), template],
        &[],
        &mut pool,
        &interner,
    );

    assert!(result.is_empty());
}

#[test]
fn same_spelling_imported_methods_keep_distinct_mangled_names() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let (first_instance, first_template, _, _) = make_imported_method_fixture(
        &interner,
        &mut pool,
        imported_producer("provider-a::hash", 17),
        2,
        31,
    );
    let (second_instance, second_template, _, _) = make_imported_method_fixture(
        &interner,
        &mut pool,
        imported_producer("provider-b::hash", 17),
        3,
        47,
    );
    let mut typed_module = TypedModule::default();
    typed_module.mono_instances.push(first_instance);
    typed_module.mono_instances.push(second_instance);

    let result = build_imported_mono_functions(
        &TypeCheckResult::ok(typed_module),
        &FxHashMap::default(),
        &[first_template, second_template],
        &[],
        &mut pool,
        &interner,
    );

    assert_eq!(result.len(), 2);
    assert_ne!(
        result[0].function.mangled_name,
        result[1].function.mangled_name
    );
    assert!(result.iter().all(|entry| interner
        .lookup(entry.function.mangled_name)
        .contains("$ip$")));
}
