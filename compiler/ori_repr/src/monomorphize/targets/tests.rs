use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, CtorKind,
    MethodCallFact, MethodCallForm,
};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{DerivedImplId, Name, StringInterner};
use ori_types::{
    ConcreteMethodMono, FunctionSig, GenericArg, Idx, MethodProducer, MonoInstance, Pool,
};
use rustc_hash::FxHashMap;

use crate::monomorphize::{MonoFunction, MonoFunctionIdentity, MonoFunctionOrigin};

use super::MonoTargetMaps;

fn method_identity(
    method: Name,
    producer: MethodProducer,
    method_args: Vec<GenericArg>,
    receiver: Idx,
    instance_id: Option<u32>,
) -> MonoFunctionIdentity {
    let instance = MonoInstance::new_method(
        method,
        producer,
        Vec::new(),
        method_args,
        ConcreteMethodMono {
            receiver_type: receiver,
            param_types: Vec::new(),
            return_type: receiver,
            body_type_map: Vec::new(),
        },
    );
    match instance_id {
        Some(id) => MonoFunctionIdentity::new(&instance, MonoInstanceId::new(id)),
        None => MonoFunctionIdentity::generated(&instance),
    }
}

fn method_apply(
    dst: ArcVarId,
    ty: Idx,
    method: Name,
    mono_instance_id: Option<MonoInstanceId>,
) -> ArcInstr {
    ArcInstr::Apply {
        dst,
        ty,
        func: method,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Owned],
        mono_instance_id,
    }
}

fn top_level_mono(name: Name, target: Name, argument: Idx, instance_id: u32) -> MonoFunction {
    let instance = MonoInstance::new_top_level(
        name,
        vec![GenericArg::Type(argument)],
        vec![argument],
        argument,
        Vec::new(),
    );
    MonoFunction {
        mangled_name: target,
        origin: MonoFunctionOrigin::Source,
        identity: MonoFunctionIdentity::new(&instance, MonoInstanceId::new(instance_id)),
        sig: FunctionSig::synthetic(name, vec![Name::from_raw(9)], vec![argument], argument),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: None,
    }
}

fn single_return_block(body: Vec<ArcInstr>, value: ArcVarId) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Return { value },
    }
}

#[test]
fn generated_method_fact_is_reserved_for_exact_receiver_rewrite() {
    let interner = StringInterner::new();
    let hash = interner.intern("hash");
    let concrete_hash = interner.intern("hash$m$3_int$im$");
    let mut function = ArcFunction {
        name: interner.intern("generated_outer_hash"),
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: hash,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        method_call_facts: vec![MethodCallFact {
            destination: ArcVarId::new(1),
            receiver_type: Idx::INT,
            form: MethodCallForm::Instance,
            producer: None,
            derived_position: None,
        }],
        ..ArcFunction::default()
    };
    let producer = MethodProducer::Derived(DerivedImplId::new(0));
    let mono = MonoFunction {
        mangled_name: concrete_hash,
        origin: MonoFunctionOrigin::Derived(DerivedImplId::new(0)),
        identity: method_identity(hash, producer, Vec::new(), Idx::INT, None),
        sig: FunctionSig::synthetic(hash, vec![Name::from_raw(9)], vec![Idx::INT], Idx::INT),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: None,
    };

    MonoTargetMaps::new(&[mono], &Pool::new()).rewrite_function(
        &mut function,
        &mut [],
        &Pool::new(),
        &interner,
    );

    let ArcInstr::Apply { func, .. } = function.blocks[0].body[0] else {
        panic!("test fixture must remain an apply")
    };
    assert_eq!(func, hash);
}

#[test]
fn method_generic_targets_dispatch_only_by_exact_instance_id() {
    let interner = StringInterner::new();
    let method = interner.intern("convert");
    let int_target = interner.intern("convert$m$$im$3_int");
    let str_target = interner.intern("convert$m$$im$3_str");
    let producer = MethodProducer::Derived(DerivedImplId::new(4));
    let mut function = ArcFunction {
        name: interner.intern("caller"),
        var_types: vec![Idx::INT, Idx::INT, Idx::STR, Idx::INT],
        blocks: vec![single_return_block(
            vec![
                method_apply(
                    ArcVarId::new(1),
                    Idx::INT,
                    method,
                    Some(MonoInstanceId::new(10)),
                ),
                method_apply(
                    ArcVarId::new(2),
                    Idx::STR,
                    method,
                    Some(MonoInstanceId::new(11)),
                ),
                method_apply(ArcVarId::new(3), Idx::INT, method, None),
            ],
            ArcVarId::new(3),
        )],
        method_call_facts: vec![MethodCallFact {
            destination: ArcVarId::new(3),
            receiver_type: Idx::INT,
            form: MethodCallForm::Instance,
            producer: Some(producer.clone()),
            derived_position: None,
        }],
        ..ArcFunction::default()
    };
    let mono = |target, argument, instance_id| MonoFunction {
        mangled_name: target,
        origin: MonoFunctionOrigin::Derived(DerivedImplId::new(4)),
        identity: method_identity(
            method,
            producer.clone(),
            vec![GenericArg::Type(argument)],
            Idx::INT,
            Some(instance_id),
        ),
        sig: FunctionSig::synthetic(method, Vec::new(), Vec::new(), argument),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: None,
    };
    let maps = MonoTargetMaps::new(
        &[
            mono(int_target, Idx::INT, 10),
            mono(str_target, Idx::STR, 11),
        ],
        &Pool::new(),
    );

    maps.rewrite_function(&mut function, &mut [], &Pool::new(), &interner);

    let targets: Vec<_> = function.blocks[0]
        .body
        .iter()
        .map(|instruction| match instruction {
            ArcInstr::Apply { func, .. } => *func,
            _ => panic!("test fixture must contain only apply instructions"),
        })
        .collect();
    assert_eq!(targets, vec![int_target, str_target, method]);
}

#[test]
fn exact_derived_targets_preserve_swapped_generic_receiver_identity() {
    let interner = StringInterner::new();
    let method = interner.intern("debug");
    let int_string_target = interner.intern("debug_pair_int_string");
    let string_int_target = interner.intern("debug_pair_string_int");
    let owner = Name::from_raw(100);
    let first_field = Name::from_raw(101);
    let second_field = Name::from_raw(102);
    let mut pool = Pool::new();
    let int_string = pool.applied(owner, &[Idx::INT, Idx::STR]);
    let string_int = pool.applied(owner, &[Idx::STR, Idx::INT]);
    let int_string_body =
        pool.struct_type(owner, &[(first_field, Idx::INT), (second_field, Idx::STR)]);
    let string_int_body =
        pool.struct_type(owner, &[(first_field, Idx::STR), (second_field, Idx::INT)]);
    pool.set_resolution(int_string, int_string_body);
    pool.set_resolution(string_int, string_int_body);

    let producer = MethodProducer::Derived(DerivedImplId::new(12));
    let mono = |receiver, target| MonoFunction {
        mangled_name: target,
        origin: MonoFunctionOrigin::Derived(DerivedImplId::new(12)),
        identity: method_identity(method, producer.clone(), Vec::new(), receiver, None),
        sig: FunctionSig::synthetic(method, Vec::new(), vec![receiver], Idx::STR),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: Some(owner),
    };
    let maps = MonoTargetMaps::new(
        &[
            mono(int_string, int_string_target),
            mono(string_int, string_int_target),
        ],
        &pool,
    );

    assert_eq!(
        maps.exact_method_target(&producer, int_string, &pool),
        Some(int_string_target)
    );
    assert_eq!(
        maps.exact_method_target(&producer, string_int, &pool),
        Some(string_int_target)
    );
}

#[test]
fn exact_derived_targets_accept_producer_qualified_materialized_receivers() {
    let interner = StringInterner::new();
    let method = interner.intern("debug");
    let inner_target = interner.intern("debug_wrap_bool");
    let outer_target = interner.intern("debug_wrap_wrap_bool");
    let other_target = interner.intern("debug_other_wrap_bool");
    let owner = interner.intern("Wrap");
    let field = interner.intern("value");
    let mut pool = Pool::new();
    let inner = pool.applied(owner, &[Idx::BOOL]);
    let inner_body = pool.struct_type(owner, &[(field, Idx::BOOL)]);
    pool.set_resolution(inner, inner_body);
    let outer = pool.applied(owner, &[inner]);
    let outer_body = pool.struct_type(owner, &[(field, inner)]);
    pool.set_resolution(outer, outer_body);

    let producer = MethodProducer::Derived(DerivedImplId::new(20));
    let other_producer = MethodProducer::Derived(DerivedImplId::new(21));
    let mono = |derived_id: u32, receiver, target| {
        let derived_id = DerivedImplId::new(derived_id);
        let producer = MethodProducer::Derived(derived_id);
        MonoFunction {
            mangled_name: target,
            origin: MonoFunctionOrigin::Derived(derived_id),
            identity: method_identity(method, producer, Vec::new(), receiver, None),
            sig: FunctionSig::synthetic(method, Vec::new(), vec![receiver], Idx::STR),
            body_type_map: FxHashMap::default(),
            is_imported: false,
            receiver_type_name: Some(owner),
        }
    };
    let maps = MonoTargetMaps::new(
        &[
            mono(20, inner, inner_target),
            mono(20, outer, outer_target),
            mono(21, inner, other_target),
        ],
        &pool,
    );

    assert_eq!(
        maps.exact_method_target(&producer, inner_body, &pool),
        Some(inner_target)
    );
    assert_eq!(
        maps.exact_method_target(&producer, outer_body, &pool),
        Some(outer_target)
    );
    assert_eq!(
        maps.exact_method_target(&other_producer, inner_body, &pool),
        Some(other_target)
    );
}

#[test]
fn generic_targets_are_indexed_by_structural_signature_hash() {
    let interner = StringInterner::new();
    let generic = interner.intern("identity");
    let int_target = interner.intern("identity$3_int");
    let str_target = interner.intern("identity$3_str");
    let pool = Pool::new();
    let maps = MonoTargetMaps::new(
        &[
            top_level_mono(generic, int_target, Idx::INT, 10),
            top_level_mono(generic, str_target, Idx::STR, 11),
        ],
        &pool,
    );
    let mut function = ArcFunction {
        name: interner.intern("caller"),
        var_types: vec![Idx::INT, Idx::STR, Idx::INT, Idx::STR],
        blocks: vec![single_return_block(
            vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    func: generic,
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    func: generic,
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            ArcVarId::new(3),
        )],
        ..ArcFunction::default()
    };

    assert_eq!(maps.generics.len(), 2);
    maps.rewrite_function(&mut function, &mut [], &pool, &interner);

    let targets = function.blocks[0]
        .body
        .iter()
        .map(|instruction| match instruction {
            ArcInstr::Apply { func, .. } => *func,
            _ => panic!("test fixture must contain only apply instructions"),
        })
        .collect::<Vec<_>>();
    assert_eq!(targets, vec![int_target, str_target]);
}

#[test]
fn stale_generic_site_id_uses_the_concrete_caller_signature() {
    let interner = StringInterner::new();
    let generic = interner.intern("identity");
    let int_target = interner.intern("identity_int");
    let float_target = interner.intern("identity_float");
    let mut caller = ArcFunction {
        name: interner.intern("wrap_int"),
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![single_return_block(
            vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: generic,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                // One source expression is cloned into every specialization
                // of its generic caller. Its source-level id can therefore
                // name a different concrete callee than this body requires.
                mono_instance_id: Some(MonoInstanceId::new(11)),
            }],
            ArcVarId::new(1),
        )],
        ..ArcFunction::default()
    };
    let maps = MonoTargetMaps::new(
        &[
            top_level_mono(generic, int_target, Idx::INT, 10),
            top_level_mono(generic, float_target, Idx::FLOAT, 11),
        ],
        &Pool::new(),
    );

    maps.rewrite_function(&mut caller, &mut [], &Pool::new(), &interner);

    assert!(matches!(
        caller.blocks[0].body[0],
        ArcInstr::Apply { func, .. } if func == int_target
    ));
}

#[test]
fn conflicting_generic_site_id_cannot_cross_callable_identity() {
    let interner = StringInterner::new();
    let selected = interner.intern("selected");
    let other = interner.intern("other");
    let selected_target = interner.intern("selected_int");
    let other_target = interner.intern("other_int");
    let caller = || ArcFunction {
        name: interner.intern("caller"),
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![single_return_block(
            vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: selected,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: Some(MonoInstanceId::new(10)),
            }],
            ArcVarId::new(1),
        )],
        ..ArcFunction::default()
    };
    let selected_mono = top_level_mono(selected, selected_target, Idx::INT, 10);
    let other_mono = top_level_mono(other, other_target, Idx::INT, 10);

    for monos in [
        vec![selected_mono.clone(), other_mono.clone()],
        vec![other_mono, selected_mono],
    ] {
        let mut function = caller();
        MonoTargetMaps::new(&monos, &Pool::new()).rewrite_function(
            &mut function,
            &mut [],
            &Pool::new(),
            &interner,
        );

        assert!(matches!(
            function.blocks[0].body[0],
            ArcInstr::Apply { func, .. } if func == selected_target
        ));
    }
}

#[test]
fn generic_function_values_rewrite_partial_apply_and_closure_construct_targets() {
    let interner = StringInterner::new();
    let generic = interner.intern("identity");
    let target = interner.intern("identity$3_int");
    let mut pool = Pool::new();
    let function_type = pool.function1(Idx::INT, Idx::INT);
    let maps = MonoTargetMaps::new(&[top_level_mono(generic, target, Idx::INT, 10)], &pool);
    let mut function = ArcFunction {
        name: interner.intern("function_value_caller"),
        var_types: vec![function_type, function_type, Idx::INT],
        blocks: vec![single_return_block(
            vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(0),
                    ty: function_type,
                    func: generic,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: function_type,
                    ctor: CtorKind::Closure { func: generic },
                    args: Vec::new(),
                },
            ],
            ArcVarId::new(2),
        )],
        ..ArcFunction::default()
    };

    maps.rewrite_function(&mut function, &mut [], &pool, &interner);

    assert!(matches!(
        function.blocks[0].body[0],
        ArcInstr::PartialApply { func, .. } if func == target
    ));
    assert!(matches!(
        function.blocks[0].body[1],
        ArcInstr::Construct {
            ctor: CtorKind::Closure { func },
            ..
        } if func == target
    ));
}

#[test]
fn generic_function_value_fallback_excludes_same_named_method_targets() {
    let interner = StringInterner::new();
    let generic = interner.intern("convert");
    let method_target = interner.intern("convert_method_int");
    let function_target = interner.intern("convert_function_int");
    let producer = MethodProducer::Derived(DerivedImplId::new(20));
    let method = MonoFunction {
        mangled_name: method_target,
        origin: MonoFunctionOrigin::Derived(DerivedImplId::new(20)),
        identity: method_identity(generic, producer, Vec::new(), Idx::INT, None),
        sig: FunctionSig::synthetic(generic, vec![Name::from_raw(9)], vec![Idx::INT], Idx::INT),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: None,
    };
    let function = top_level_mono(generic, function_target, Idx::INT, 10);
    let mut pool = Pool::new();
    let function_type = pool.function1(Idx::INT, Idx::INT);
    let mut caller = ArcFunction {
        name: interner.intern("function_value_caller"),
        var_types: vec![function_type, Idx::INT],
        blocks: vec![single_return_block(
            vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: function_type,
                func: generic,
                args: Vec::new(),
            }],
            ArcVarId::new(1),
        )],
        ..ArcFunction::default()
    };

    MonoTargetMaps::new(&[method, function], &pool).rewrite_function(
        &mut caller,
        &mut [],
        &pool,
        &interner,
    );

    assert!(matches!(
        caller.blocks[0].body[0],
        ArcInstr::PartialApply { func, .. } if func == function_target
    ));
}

#[test]
fn generic_function_value_fallback_distinguishes_return_only_specializations() {
    let interner = StringInterner::new();
    let generic = interner.intern("empty");
    let int_target = interner.intern("empty_int");
    let str_target = interner.intern("empty_str");
    let mono = |target, return_type, instance_id| {
        let instance = MonoInstance::new_top_level(
            generic,
            vec![GenericArg::Type(return_type)],
            Vec::new(),
            return_type,
            Vec::new(),
        );
        MonoFunction {
            mangled_name: target,
            origin: MonoFunctionOrigin::Source,
            identity: MonoFunctionIdentity::new(&instance, MonoInstanceId::new(instance_id)),
            sig: FunctionSig::synthetic(generic, vec![Name::from_raw(9)], Vec::new(), return_type),
            body_type_map: FxHashMap::default(),
            is_imported: false,
            receiver_type_name: None,
        }
    };
    let mut pool = Pool::new();
    let int_function = pool.function0(Idx::INT);
    let str_function = pool.function0(Idx::STR);
    let mut caller = ArcFunction {
        name: interner.intern("return_only_callers"),
        var_types: vec![int_function, str_function, Idx::INT],
        blocks: vec![single_return_block(
            vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(0),
                    ty: int_function,
                    func: generic,
                    args: Vec::new(),
                },
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: str_function,
                    func: generic,
                    args: Vec::new(),
                },
            ],
            ArcVarId::new(2),
        )],
        ..ArcFunction::default()
    };

    MonoTargetMaps::new(
        &[
            mono(int_target, Idx::INT, 10),
            mono(str_target, Idx::STR, 11),
        ],
        &pool,
    )
    .rewrite_function(&mut caller, &mut [], &pool, &interner);

    let targets: Vec<_> = caller.blocks[0]
        .body
        .iter()
        .map(|instruction| match instruction {
            ArcInstr::PartialApply { func, .. } => *func,
            _ => panic!("test fixture must contain only partial applications"),
        })
        .collect();
    assert_eq!(targets, vec![int_target, str_target]);
}

#[test]
fn generic_direct_fallback_distinguishes_return_only_specializations() {
    let interner = StringInterner::new();
    let generic = interner.intern("empty");
    let int_target = interner.intern("empty_int");
    let str_target = interner.intern("empty_str");
    let mono = |target, return_type, instance_id| {
        let instance = MonoInstance::new_top_level(
            generic,
            vec![GenericArg::Type(return_type)],
            Vec::new(),
            return_type,
            Vec::new(),
        );
        MonoFunction {
            mangled_name: target,
            origin: MonoFunctionOrigin::Source,
            identity: MonoFunctionIdentity::new(&instance, MonoInstanceId::new(instance_id)),
            sig: FunctionSig::synthetic(generic, vec![Name::from_raw(9)], Vec::new(), return_type),
            body_type_map: FxHashMap::default(),
            is_imported: false,
            receiver_type_name: None,
        }
    };
    let mut caller = ArcFunction {
        name: interner.intern("return_only_direct_callers"),
        var_types: vec![Idx::INT, Idx::STR],
        blocks: vec![single_return_block(
            vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(0),
                    ty: Idx::INT,
                    func: generic,
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: generic,
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                },
            ],
            ArcVarId::new(0),
        )],
        ..ArcFunction::default()
    };
    let pool = Pool::new();

    MonoTargetMaps::new(
        &[
            mono(int_target, Idx::INT, 10),
            mono(str_target, Idx::STR, 11),
        ],
        &pool,
    )
    .rewrite_function(&mut caller, &mut [], &pool, &interner);

    let targets: Vec<_> = caller.blocks[0]
        .body
        .iter()
        .map(|instruction| match instruction {
            ArcInstr::Apply { func, .. } => *func,
            _ => panic!("test fixture must contain only direct applications"),
        })
        .collect();
    assert_eq!(targets, vec![int_target, str_target]);
}
