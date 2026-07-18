use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
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
