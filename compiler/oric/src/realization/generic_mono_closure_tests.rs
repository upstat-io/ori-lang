//! Focused pins for generic target closure.

use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue, Ownership,
};
use ori_ir::{DerivedImplId, Name, StringInterner};
use ori_types::{
    ConcreteMethodMono, FunctionSig, GenericArg, Idx, MethodProducer, MonoInstance, Pool, Tag,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::*;

fn var(raw: u32) -> ArcVarId {
    ArcVarId::new(raw)
}

fn generic_signature(name: Name, parameter: Idx, var_id: u32) -> FunctionSig {
    let mut signature = FunctionSig::simple(name, vec![parameter], parameter);
    signature.type_params = vec![Name::from_raw(900)];
    signature.scheme_var_ids = vec![var_id];
    signature.generic_param_mapping = vec![Some(0)];
    signature
}

fn mono_instance(name: Name, argument: Idx) -> MonoInstance {
    MonoInstance::new_top_level(
        name,
        vec![GenericArg::Type(argument)],
        vec![argument],
        argument,
        Vec::new(),
    )
}

fn imported_mono_function(
    instance: &MonoInstance,
    instance_id: MonoInstanceId,
    signature: &FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
) -> MonoFunction {
    let mangled_name = mangle_mono_instance_name(instance, interner, pool);
    MonoFunction {
        mangled_name,
        origin: MonoFunctionOrigin::Source,
        identity: MonoFunctionIdentity::new(instance, instance_id),
        sig: concrete_sig_for_instance(instance, signature, pool, mangled_name),
        body_type_map: FxHashMap::default(),
        is_imported: true,
        receiver_type_name: None,
    }
}

fn imported_template(
    local_name: Name,
    source_name: Name,
    signature: FunctionSig,
) -> ImportedGenericTemplate {
    ImportedGenericTemplate {
        local_name,
        signature,
        module_index: 0,
        source_name,
        generic_type_params: FxHashMap::default(),
    }
}

#[test]
fn imported_generic_nominal_and_resolved_coordinates_merge_one_executable_identity() {
    let interner = StringInterner::new();
    let local_name = interner.intern("point_identity");
    let source_name = interner.intern("provider_point_identity");
    let point = interner.intern("Point");
    let field = interner.intern("value");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 91);
    let named = pool.named(point);
    let resolved = pool.struct_type(point, &[(field, Idx::INT)]);
    pool.set_resolution(named, resolved);
    let signature = generic_signature(source_name, bound, 91);
    let import = imported_template(local_name, source_name, signature);
    let signatures = generic_signature_census(&[], std::slice::from_ref(&import));
    let nominal_instance = mono_instance(local_name, named);
    let resolved_instance = mono_instance(local_name, resolved);
    let initial = imported_mono_function(
        &nominal_instance,
        MonoInstanceId::new(77),
        &import.signature,
        &interner,
        &pool,
    );
    assert_eq!(
        initial.mangled_name,
        mangle_mono_instance_name(&resolved_instance, &interner, &pool),
        "nominal and resolved coordinates must name one executable specialization"
    );

    let functions = collect_imported_mono_functions(
        &[nominal_instance, resolved_instance],
        &signatures,
        &[initial],
        &interner,
        &pool,
    )
    .unwrap_or_else(|e| panic!("structurally identical imported signatures must merge: {e:?}"));

    assert_eq!(functions.len(), 1);
    assert_eq!(
        functions[0].identity.instance_ids(),
        [
            MonoInstanceId::new(77),
            MonoInstanceId::new(0),
            MonoInstanceId::new(1)
        ]
    );
}

#[test]
fn imported_capability_provider_nominal_and_resolved_coordinates_merge() {
    let interner = StringInterner::new();
    let local_name = interner.intern("with_counter");
    let source_name = interner.intern("provider_with_counter");
    let capability = interner.intern("Counter");
    let counter = interner.intern("LocalCounter");
    let field = interner.intern("count");
    let mut pool = Pool::new();
    let provider_schema = pool.intern(Tag::BoundVar, 92);
    let named = pool.named(counter);
    let resolved = pool.struct_type(counter, &[(field, Idx::INT)]);
    pool.set_resolution(named, resolved);
    let mut signature = FunctionSig::simple(source_name, vec![Idx::INT], Idx::INT);
    signature.capabilities = vec![capability];
    signature.capability_params = vec![ori_types::CapabilityParam::Value {
        capability,
        provider_type: provider_schema,
        provider_var_id: 92,
    }];
    let import = imported_template(local_name, source_name, signature);
    let signatures = generic_signature_census(&[], std::slice::from_ref(&import));
    let nominal_instance = MonoInstance::new_top_level_with_capabilities(
        local_name,
        Vec::new(),
        vec![named],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let resolved_instance = MonoInstance::new_top_level_with_capabilities(
        local_name,
        Vec::new(),
        vec![resolved],
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let initial = imported_mono_function(
        &nominal_instance,
        MonoInstanceId::new(88),
        &import.signature,
        &interner,
        &pool,
    );

    let functions = collect_imported_mono_functions(
        &[nominal_instance, resolved_instance],
        &signatures,
        &[initial],
        &interner,
        &pool,
    )
    .unwrap_or_else(|e| {
        panic!("structurally identical capability-provider signatures must merge: {e:?}")
    });

    assert_eq!(functions.len(), 1);
    assert_eq!(
        functions[0].identity.instance_ids(),
        [
            MonoInstanceId::new(88),
            MonoInstanceId::new(0),
            MonoInstanceId::new(1)
        ]
    );
}

#[test]
fn imported_same_name_guard_rejects_genuinely_different_concrete_signature() {
    let interner = StringInterner::new();
    let local_name = interner.intern("guarded_identity");
    let source_name = interner.intern("provider_guarded_identity");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 93);
    let signature = generic_signature(source_name, bound, 93);
    let import = imported_template(local_name, source_name, signature);
    let signatures = generic_signature_census(&[], std::slice::from_ref(&import));
    let int_instance = mono_instance(local_name, Idx::INT);
    let str_instance = mono_instance(local_name, Idx::STR);
    let mut conflicting = imported_mono_function(
        &str_instance,
        MonoInstanceId::new(99),
        &import.signature,
        &interner,
        &pool,
    );
    conflicting.mangled_name = mangle_mono_instance_name(&int_instance, &interner, &pool);
    conflicting.sig.name = conflicting.mangled_name;

    let result = collect_imported_mono_functions(
        &[int_instance],
        &signatures,
        &[conflicting],
        &interner,
        &pool,
    );
    let Err(error) = result else {
        panic!("different concrete parameter and return types must remain a conflict");
    };

    assert!(matches!(
        error,
        GenericMonoClosureError::MonoInventory { .. }
    ));
}

#[test]
fn specialization_probe_leaves_canonical_group_and_pool_unmodified() {
    let interner = StringInterner::new();
    let parent_name = interner.intern("probe_parent");
    let lambda_name = interner.intern("probe_parent.__lambda0");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 71);
    let schema_function = pool.function1(bound, Idx::INT);
    let concrete_function = pool.function1(Idx::STR, Idx::INT);
    let parent = ArcFunction {
        name: parent_name,
        params: vec![ArcParam {
            var: var(2),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        var_types: vec![schema_function, concrete_function, Idx::STR, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: var(0),
                    ty: schema_function,
                    func: lambda_name,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: concrete_function,
                    value: ArcValue::Var(var(0)),
                },
                ArcInstr::ApplyIndirect {
                    dst: var(3),
                    ty: Idx::INT,
                    closure: var(1),
                    args: vec![var(2)],
                    arg_ownership: Vec::new(),
                },
            ],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..ArcFunction::default()
    };
    let lambda = ArcFunction {
        name: lambda_name,
        params: vec![ArcParam {
            var: var(0),
            ty: bound,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        var_types: vec![bound, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: var(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(1)),
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..ArcFunction::default()
    };
    let groups = vec![ArcFunctionGroup::new(parent, vec![lambda])];
    let canonical_bodies: Vec<_> = groups[0].bodies().cloned().collect();
    let canonical_pool_len = pool.len();

    let (probe, probe_pool) = specialized_probe(&groups, &pool, &interner)
        .unwrap_or_else(|e| panic!("specialized_probe must succeed: {e:?}"));

    assert_eq!(
        groups[0].bodies().cloned().collect::<Vec<_>>(),
        canonical_bodies
    );
    assert_eq!(pool.len(), canonical_pool_len);
    let probe_lambda = probe[0]
        .bodies()
        .nth(1)
        .unwrap_or_else(|| panic!("lambda must survive"));
    assert_eq!(
        probe_pool.resolve_fully(probe_lambda.params[0].ty),
        Idx::STR
    );
    assert_eq!(
        pool.tag(
            groups[0]
                .bodies()
                .nth(1)
                .unwrap_or_else(|| panic!("lambda must survive"))
                .params[0]
                .ty
        ),
        Tag::BoundVar
    );
}

#[test]
fn appending_discoveries_preserves_existing_instance_indices() {
    let interner = StringInterner::new();
    let first = interner.intern("first_generic");
    let second = interner.intern("second_generic");
    let pool = Pool::new();
    let original = vec![
        mono_instance(first, Idx::INT),
        mono_instance(second, Idx::STR),
    ];
    let mut instances = original.clone();
    let mut seen: FxHashSet<_> = instances.iter().map(instance_key).collect();

    let added = append_new_instances(
        &mut instances,
        &mut seen,
        vec![mono_instance(first, Idx::STR)],
        &interner,
        &pool,
    );

    assert_eq!(added, 1);
    assert_eq!(&instances[..original.len()], &original);
    assert_eq!(instances[2].generic_args, vec![GenericArg::Type(Idx::STR)]);
}

#[test]
fn exact_name_and_type_dedup_reaches_a_zero_addition_fixed_point() {
    let interner = StringInterner::new();
    let first = interner.intern("first_generic");
    let second = interner.intern("second_generic");
    let pool = Pool::new();
    let mut instances = vec![mono_instance(first, Idx::INT)];
    let mut seen: FxHashSet<_> = instances.iter().map(instance_key).collect();
    let discoveries = vec![
        mono_instance(first, Idx::INT),
        mono_instance(first, Idx::STR),
        mono_instance(first, Idx::STR),
        mono_instance(second, Idx::STR),
    ];

    assert_eq!(
        append_new_instances(
            &mut instances,
            &mut seen,
            discoveries.clone(),
            &interner,
            &pool,
        ),
        2
    );
    assert_eq!(
        append_new_instances(&mut instances, &mut seen, discoveries, &interner, &pool,),
        0
    );
    assert_eq!(instances.len(), 3);
}

#[test]
fn discovery_covers_apply_invoke_and_both_function_value_forms() {
    let interner = StringInterner::new();
    let generic = interner.intern("identity");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 73);
    let signature = generic_signature(generic, bound, 73);
    let str_function = pool.function1(Idx::STR, Idx::STR);
    let list_int = pool.list(Idx::INT);
    let list_function = pool.function1(list_int, list_int);
    let signatures = FxHashMap::from_iter([(
        generic,
        GenericSignature {
            signature: &signature,
            imported: None,
        },
    )]);
    let function = ArcFunction {
        name: interner.intern("caller"),
        var_types: vec![
            Idx::INT,
            Idx::INT,
            str_function,
            list_function,
            Idx::BOOL,
            Idx::BOOL,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: Idx::INT,
                    func: generic,
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(10)),
                },
                ArcInstr::PartialApply {
                    dst: var(2),
                    ty: str_function,
                    func: generic,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: var(3),
                    ty: list_function,
                    ctor: CtorKind::Closure { func: generic },
                    args: Vec::new(),
                },
            ],
            terminator: ArcTerminator::Invoke {
                dst: var(5),
                ty: Idx::BOOL,
                func: generic,
                args: vec![var(4)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(11)),
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
        }],
        ..ArcFunction::default()
    };
    let groups = vec![ArcFunctionGroup::new(function, Vec::new())];

    let uses = collect_generic_uses(&groups, &signatures, &pool);

    assert_eq!(uses.len(), 4);
    let parameter_types: FxHashSet<_> = uses
        .iter()
        .map(|generic_use| generic_use.param_types[0])
        .collect();
    assert_eq!(
        parameter_types,
        FxHashSet::from_iter([Idx::INT, Idx::STR, list_int, Idx::BOOL])
    );
}

#[test]
fn local_non_generic_signature_shadows_same_named_imported_generic_template() {
    let interner = StringInterner::new();
    let local_name = interner.intern("is_empty");
    let provider_name = interner.intern("prelude_is_empty");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 81);
    let local = FunctionSig::simple(local_name, vec![Idx::INT], Idx::BOOL);
    let imported = ImportedGenericTemplate {
        local_name,
        signature: generic_signature(provider_name, bound, 81),
        module_index: 0,
        source_name: provider_name,
        generic_type_params: FxHashMap::default(),
    };

    let local_signatures = [local];
    let imported_templates = [imported];
    let census = generic_signature_census(&local_signatures, &imported_templates);

    assert!(!census.contains_key(&local_name));
}

#[test]
fn imported_alias_materializes_under_the_local_call_identity() {
    let interner = StringInterner::new();
    let provider_name = interner.intern("provider_identity");
    let local_alias = interner.intern("aliased_identity");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 82);
    let signature = generic_signature(provider_name, bound, 82);
    let probe_pool = pool.clone();
    let generic_use = GenericUse {
        callee: local_alias,
        param_types: vec![Idx::INT],
        return_type: Idx::INT,
    };

    let instance = materialize_instance(
        &signature,
        &generic_use,
        &probe_pool,
        &mut pool,
        &FxHashMap::default(),
    )
    .unwrap_or_else(|| panic!("concrete alias use must materialize"));

    assert_eq!(instance.fn_name, local_alias);
}

#[test]
fn realization_discovery_registers_concrete_generic_composite_bodies() {
    let interner = StringInterner::new();
    let generic = interner.intern("round_trip");
    let wrapper = interner.intern("Wrapper");
    let parameter = interner.intern("T");
    let field = interner.intern("value");
    let mut pool = Pool::new();

    let named_parameter = pool.named(parameter);
    let generic_body = pool.struct_type(wrapper, &[(field, named_parameter)]);
    let named_wrapper = pool.named(wrapper);
    pool.set_resolution(named_wrapper, generic_body);

    let bound = pool.intern(Tag::BoundVar, 85);
    let schema = pool.applied(wrapper, &[bound]);
    let concrete = pool.applied(wrapper, &[Idx::STR]);
    assert!(pool.resolve(concrete).is_none());

    let signature = generic_signature(generic, schema, 85);
    let probe_pool = pool.clone();
    let generic_use = GenericUse {
        callee: generic,
        param_types: vec![concrete],
        return_type: concrete,
    };
    let generic_type_params = FxHashMap::from_iter([(wrapper, vec![parameter])]);

    let instance = materialize_instance(
        &signature,
        &generic_use,
        &probe_pool,
        &mut pool,
        &generic_type_params,
    )
    .unwrap_or_else(|| panic!("concrete generic-composite use must materialize"));

    assert!(instance
        .body_type_map
        .iter()
        .any(|&(source, target)| source == schema && target == concrete));
    let concrete_body = pool
        .resolve(concrete)
        .unwrap_or_else(|| panic!("realization-discovered Applied type needs a concrete body"));
    assert_eq!(pool.struct_fields(concrete_body), vec![(field, Idx::STR)]);
}

#[test]
fn imported_free_template_does_not_claim_same_named_method_instance() {
    let interner = StringInterner::new();
    let local_name = interner.intern("convert");
    let provider_name = interner.intern("provider_convert");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 83);
    let imported = ImportedGenericTemplate {
        local_name,
        signature: generic_signature(provider_name, bound, 83),
        module_index: 0,
        source_name: provider_name,
        generic_type_params: FxHashMap::default(),
    };
    let signatures = generic_signature_census(&[], std::slice::from_ref(&imported));
    let method = MonoInstance::new_method(
        local_name,
        MethodProducer::Derived(DerivedImplId::new(5)),
        Vec::new(),
        Vec::new(),
        ConcreteMethodMono {
            receiver_type: Idx::INT,
            param_types: vec![Idx::INT],
            return_type: Idx::INT,
            body_type_map: Vec::new(),
        },
    );

    let functions = collect_imported_mono_functions(&[method], &signatures, &[], &interner, &pool)
        .unwrap_or_else(|e| {
            panic!(
                "method instances must be ignored by the imported free-function collector: {e:?}"
            )
        });

    assert!(functions.is_empty());
}

#[test]
fn projected_return_materialization_uses_the_checked_function_value_return() {
    let interner = StringInterner::new();
    let generic = interner.intern("container_item");
    let parameter_name = interner.intern("C");
    let associated_name = interner.intern("Item");
    let mut pool = Pool::new();
    let bound = pool.intern(Tag::BoundVar, 84);
    let mut signature = generic_signature(generic, bound, 84);
    signature.return_type = Idx::ERROR;
    signature.return_projection = Some((parameter_name, associated_name));
    let probe_pool = pool.clone();
    let generic_use = GenericUse {
        callee: generic,
        param_types: vec![Idx::INT],
        return_type: Idx::STR,
    };

    let instance = materialize_instance(
        &signature,
        &generic_use,
        &probe_pool,
        &mut pool,
        &FxHashMap::default(),
    )
    .unwrap_or_else(|| panic!("the checked projected return is concrete realization evidence"));

    assert_eq!(instance.concrete_param_types, vec![Idx::INT]);
    assert_eq!(instance.concrete_return_type, Idx::STR);
}
