use ori_ir::{DerivedImplId, DerivedTrait, Name, Span, StringInterner};
use ori_repr::monomorphize::{MonoFunction, MonoFunctionIdentity, MonoFunctionOrigin};
use ori_types::{
    AcceptedDerivedImpl, ConcreteMethodMono, DerivedCallPlan, DerivedCallPosition,
    DerivedCallSelection, DerivedDirectCallSelection, EnumVariant, FunctionSig, GenericArg, Idx,
    MethodProducer, MonoInstance, Pool, RegistryMethodIdentity, RegistryPreludeIdentity,
};
use rustc_hash::FxHashMap;

use super::{
    build_supported_derived_body, extend_mono_method_targets,
    lower_non_generic_derived_methods_for_analysis, method_receiver_key,
};

fn mono_method(
    mangled_name: Name,
    method_name: Name,
    receiver_type: Idx,
    derived_id: u32,
) -> MonoFunction {
    mono_method_with_args(
        mangled_name,
        method_name,
        receiver_type,
        derived_id,
        Vec::new(),
    )
}

fn mono_method_with_args(
    mangled_name: Name,
    method_name: Name,
    receiver_type: Idx,
    derived_id: u32,
    method_args: Vec<GenericArg>,
) -> MonoFunction {
    let instance = MonoInstance::new_method(
        method_name,
        MethodProducer::Derived(DerivedImplId::new(derived_id)),
        Vec::new(),
        method_args,
        ConcreteMethodMono {
            receiver_type,
            param_types: Vec::new(),
            return_type: receiver_type,
            body_type_map: Vec::new(),
        },
    );
    MonoFunction {
        mangled_name,
        origin: MonoFunctionOrigin::Derived(DerivedImplId::new(derived_id)),
        identity: MonoFunctionIdentity::generated(&instance),
        sig: FunctionSig::synthetic(mangled_name, Vec::new(), Vec::new(), receiver_type),
        body_type_map: FxHashMap::default(),
        is_imported: false,
        receiver_type_name: None,
    }
}

fn accepted_hashable(
    id: u32,
    owner_name: Name,
    owner_type: Idx,
    method_name: Name,
    self_name: Name,
    trait_type: Idx,
) -> AcceptedDerivedImpl {
    AcceptedDerivedImpl {
        id: DerivedImplId::new(id),
        owner_name,
        owner_type,
        trait_type,
        trait_kind: DerivedTrait::Hashable,
        method_name,
        signature: FunctionSig::synthetic(method_name, vec![self_name], vec![owner_type], Idx::INT),
        span: Span::DUMMY,
    }
}

fn call_plan(
    accepted: &AcceptedDerivedImpl,
    interner: &StringInterner,
    pool: &Pool,
) -> DerivedCallPlan {
    let resolved = pool.resolve_fully(accepted.owner_type);
    let (position, receiver_type) = if pool.is_newtype_ctor(accepted.owner_name) {
        (DerivedCallPosition::Newtype, resolved)
    } else {
        let fields = pool.struct_fields(resolved);
        let Some((_, receiver_type)) = fields.first().copied() else {
            panic!("test derive fixture must have one generated field call")
        };
        (DerivedCallPosition::Field(0), receiver_type)
    };
    let calls = if accepted.trait_kind == DerivedTrait::Eq {
        assert!(pool
            .builtin_type_tag(pool.resolve_fully(receiver_type))
            .is_some());
        Vec::new()
    } else {
        let Some(receiver_tag) = pool.builtin_type_tag(pool.resolve_fully(receiver_type)) else {
            panic!("test derive fixture must select a builtin nested producer")
        };
        let method_text = interner.lookup(accepted.method_name);
        let Some(method_identity) = ori_registry::find_method_id(receiver_tag, method_text) else {
            panic!("test derive fixture must resolve {receiver_tag:?}.{method_text}")
        };
        vec![DerivedCallSelection {
            position,
            receiver_type,
            trait_type: accepted.trait_type,
            method_name: accepted.method_name,
            has_self: true,
            producer: MethodProducer::Registry(RegistryMethodIdentity::from_registered(
                method_identity,
            )),
        }]
    };
    let direct_calls = if accepted.trait_kind == DerivedTrait::Hashable
        && !pool.is_newtype_ctor(accepted.owner_name)
    {
        let function_name = interner.intern("hash_combine");
        let Some(identity) = ori_registry::find_prelude_function_id("hash_combine") else {
            panic!("hash_combine must remain in the prelude registry")
        };
        vec![DerivedDirectCallSelection {
            position: DerivedCallPosition::FieldCombine(0),
            function_name,
            producer: MethodProducer::Prelude(RegistryPreludeIdentity::from_registered(identity)),
        }]
    } else {
        Vec::new()
    };
    DerivedCallPlan {
        derived: accepted.id,
        binder_substitutions: Vec::new(),
        calls,
        direct_calls,
    }
}

#[test]
fn same_named_associated_monos_are_keyed_by_concrete_receiver() {
    let interner = StringInterner::new();
    let default = interner.intern("default");
    let mut pool = Pool::new();
    let left = pool.struct_type(interner.intern("Left"), &[]);
    let right = pool.struct_type(interner.intern("Right"), &[]);
    let left_target = interner.intern("default$m$Left");
    let right_target = interner.intern("default$m$Right");
    let monos = vec![
        mono_method(left_target, default, left, 1),
        mono_method(right_target, default, right, 2),
    ];
    let mut targets = FxHashMap::default();

    if let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) {
        panic!("distinct concrete receivers must retain distinct targets: {problems:?}");
    }

    assert_eq!(targets.get(&(left, default)), Some(&left_target));
    assert_eq!(targets.get(&(right, default)), Some(&right_target));
}

#[test]
fn generic_mono_target_registers_materialized_receiver_alias() {
    let interner = StringInterner::new();
    let method = interner.intern("eq");
    let target = interner.intern("eq_box_int");
    let owner = interner.intern("Box");
    let mut pool = Pool::new();
    let receiver = pool.applied(owner, &[Idx::INT]);
    let body = pool.struct_type(owner, &[(interner.intern("item"), Idx::INT)]);
    pool.set_resolution(receiver, body);
    let monos = vec![mono_method(target, method, receiver, 1)];
    let mut targets = FxHashMap::default();

    if let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) {
        panic!(
            "one generic receiver must register exact semantic and physical targets: {problems:?}"
        );
    }

    assert_eq!(targets.get(&(receiver, method)), Some(&target));
    assert_eq!(targets.get(&(body, method)), Some(&target));
}

#[test]
fn ambiguous_materialized_receiver_alias_fails_closed() {
    let interner = StringInterner::new();
    let method = interner.intern("eq");
    let first_target = interner.intern("eq_box_int");
    let second_target = interner.intern("eq_box_str");
    let owner = interner.intern("Box");
    let mut pool = Pool::new();
    let first_receiver = pool.applied(owner, &[Idx::INT]);
    let second_receiver = pool.applied(owner, &[Idx::STR]);
    let shared_body = pool.struct_type(owner, &[]);
    pool.set_resolution(first_receiver, shared_body);
    pool.set_resolution(second_receiver, shared_body);
    let monos = vec![
        mono_method(first_target, method, first_receiver, 1),
        mono_method(second_target, method, second_receiver, 2),
    ];
    let mut targets = FxHashMap::default();

    let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) else {
        panic!("one physical operator receiver must not select between nominal targets");
    };

    assert_eq!(problems.len(), 1);
    assert_eq!(targets.get(&(first_receiver, method)), Some(&first_target));
    assert_eq!(
        targets.get(&(second_receiver, method)),
        Some(&second_target)
    );
    assert_eq!(targets.get(&(shared_body, method)), Some(&first_target));
}

#[test]
fn receiver_alias_registration_preserves_newtypes_and_identity_controls() {
    let interner = StringInterner::new();
    let method = interner.intern("eq");
    let newtype_target = interner.intern("eq_key");
    let struct_target = interner.intern("eq_record");
    let newtype_name = interner.intern("Key");
    let mut pool = Pool::new();
    let newtype = pool.named(newtype_name);
    pool.register_newtype_ctor(newtype_name, Idx::INT);
    pool.set_resolution(newtype, Idx::INT);
    let structural = pool.struct_type(interner.intern("Record"), &[]);
    let monos = vec![
        mono_method(newtype_target, method, newtype, 1),
        mono_method(struct_target, method, structural, 2),
    ];
    let mut targets = FxHashMap::default();

    if let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) {
        panic!("newtype and identity receiver aliases must remain exact: {problems:?}");
    }

    assert_eq!(targets.len(), 2);
    assert_eq!(targets.get(&(newtype, method)), Some(&newtype_target));
    assert_eq!(targets.get(&(structural, method)), Some(&struct_target));
    assert!(!targets.contains_key(&(Idx::INT, method)));
}

#[test]
fn receiver_keys_resolve_nominals_and_preserve_generic_carriers() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let record_name = interner.intern("Record");
    let record_nominal = pool.named(record_name);
    let record = pool.struct_type(record_name, &[(interner.intern("value"), Idx::INT)]);
    pool.set_resolution(record_nominal, record);

    let choice_name = interner.intern("Choice");
    let choice_nominal = pool.named(choice_name);
    let choice = pool.enum_type(
        choice_name,
        &[EnumVariant {
            name: interner.intern("Only"),
            field_types: vec![Idx::STR],
        }],
    );
    pool.set_resolution(choice_nominal, choice);

    let box_name = interner.intern("Box");
    let concrete_box = pool.applied(box_name, &[Idx::INT]);
    let box_body = pool.struct_type(box_name, &[(interner.intern("item"), Idx::INT)]);
    pool.set_resolution(concrete_box, box_body);

    assert_eq!(method_receiver_key(&pool, record_nominal), record);
    assert_eq!(method_receiver_key(&pool, record), record);
    assert_eq!(method_receiver_key(&pool, choice_nominal), choice);
    assert_eq!(method_receiver_key(&pool, choice), choice);
    assert_eq!(method_receiver_key(&pool, concrete_box), concrete_box);
    assert_eq!(method_receiver_key(&pool, box_body), box_body);
    assert_ne!(
        method_receiver_key(&pool, concrete_box),
        method_receiver_key(&pool, box_body),
        "generic dispatch identity must not collapse through its representation body"
    );
}

#[test]
fn conflicting_target_for_one_concrete_receiver_fails_closed() {
    let interner = StringInterner::new();
    let default = interner.intern("default");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(interner.intern("Box"), &[]);
    let first = interner.intern("default$m$Box$first");
    let second = interner.intern("default$m$Box$second");
    let monos = vec![
        mono_method(first, default, receiver, 1),
        mono_method(second, default, receiver, 2),
    ];
    let mut targets = FxHashMap::default();

    let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) else {
        panic!("one concrete receiver/method identity accepted conflicting bodies");
    };

    assert_eq!(targets.get(&(receiver, default)), Some(&first));
    assert_eq!(problems.len(), 1);
    let message = format!("{:?}", problems[0]);
    assert!(message.contains("conflicting realized targets"));
    assert!(message.contains("default$m$Box$first"));
    assert!(message.contains("default$m$Box$second"));
}

#[test]
fn method_generic_monos_do_not_enter_receiver_only_fallback() {
    let interner = StringInterner::new();
    let method = interner.intern("convert");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(interner.intern("Box"), &[]);
    let int_target = interner.intern("convert$m$Box$im$int");
    let str_target = interner.intern("convert$m$Box$im$str");
    let monos = vec![
        mono_method_with_args(
            int_target,
            method,
            receiver,
            1,
            vec![GenericArg::Type(Idx::INT)],
        ),
        mono_method_with_args(
            str_target,
            method,
            receiver,
            1,
            vec![GenericArg::Type(Idx::STR)],
        ),
    ];
    let mut targets = FxHashMap::default();

    if let Err(problems) = extend_mono_method_targets(&mut targets, &monos, &interner, &pool) {
        panic!("exact method-generic instances must not collide in a coarse map: {problems:?}");
    }

    assert!(!targets.contains_key(&(receiver, method)));
}

#[test]
fn distinct_newtype_derives_keep_distinct_receiver_targets() {
    let interner = StringInterner::new();
    let left_name = interner.intern("LeftKey");
    let right_name = interner.intern("RightKey");
    let hash = interner.intern("hash");
    let self_name = interner.intern("self");
    let mut pool = Pool::new();
    let hashable = pool.named(interner.intern("Hashable"));
    let left = pool.named(left_name);
    let right = pool.named(right_name);
    pool.register_newtype_ctor(left_name, Idx::INT);
    pool.register_newtype_ctor(right_name, Idx::INT);
    pool.set_resolution(left, Idx::INT);
    pool.set_resolution(right, Idx::INT);
    let accepted = vec![
        accepted_hashable(1, left_name, left, hash, self_name, hashable),
        accepted_hashable(2, right_name, right, hash, self_name, hashable),
    ];

    let plans: Vec<_> = accepted
        .iter()
        .map(|item| call_plan(item, &interner, &pool))
        .collect();
    let analysis =
        lower_non_generic_derived_methods_for_analysis(&accepted, &plans, &interner, &pool)
            .unwrap_or_else(|problems| {
                panic!("distinct newtype derives must not collide by representation: {problems:?}")
            });

    assert_eq!(analysis.groups.len(), 2);
    assert_eq!(analysis.targets.len(), 2);
    let left_target = analysis.targets.get(&(left, hash));
    let right_target = analysis.targets.get(&(right, hash));
    assert!(left_target.is_some());
    assert!(right_target.is_some());
    assert_ne!(left_target, right_target);
}

#[test]
fn accepted_hashable_builds_a_shared_arc_body() {
    let interner = StringInterner::new();
    let owner_name = interner.intern("Key");
    let method_name = interner.intern("hash");
    let self_name = interner.intern("self");
    let executable_name = interner.intern("hash$derived$0");
    let mut pool = Pool::new();
    let owner_type = pool.struct_type(owner_name, &[(interner.intern("value"), Idx::INT)]);
    let signature =
        FunctionSig::synthetic(method_name, vec![self_name], vec![owner_type], Idx::INT);
    let accepted = AcceptedDerivedImpl {
        id: DerivedImplId::new(0),
        owner_name,
        owner_type,
        trait_type: pool.named(interner.intern("Hashable")),
        trait_kind: DerivedTrait::Hashable,
        method_name,
        signature: signature.clone(),
        span: Span::DUMMY,
    };

    let plan = call_plan(&accepted, &interner, &pool);
    let Some(result) = build_supported_derived_body(
        &accepted,
        &plan,
        executable_name,
        &signature,
        &interner,
        &pool,
    ) else {
        panic!("accepted Hashable must not remain outside the shared body inventory")
    };
    let body = result.unwrap_or_else(|error| {
        panic!("accepted concrete Hashable must build a shared ARC body: {error}")
    });

    assert_eq!(body.name, executable_name);
    assert_eq!(body.params[0].ty, owner_type);
    assert_eq!(body.return_type, Idx::INT);
    assert_eq!(body.method_call_facts.len(), 1);
    assert_eq!(body.method_call_facts[0].receiver_type, Idx::INT);
}

#[test]
fn accepted_newtype_eq_delegates_with_nominal_target_identity() {
    let interner = StringInterner::new();
    let owner_name = interner.intern("UserId");
    let method_name = interner.intern("equals");
    let self_name = interner.intern("self");
    let other_name = interner.intern("other");
    let executable_name = interner.intern("equals$derived$0");
    let mut pool = Pool::new();
    let owner_type = pool.named(owner_name);
    pool.register_newtype_ctor(owner_name, Idx::STR);
    pool.set_resolution(owner_type, Idx::STR);
    let signature = FunctionSig::synthetic(
        method_name,
        vec![self_name, other_name],
        vec![owner_type, owner_type],
        Idx::BOOL,
    );
    let accepted = AcceptedDerivedImpl {
        id: DerivedImplId::new(0),
        owner_name,
        owner_type,
        trait_type: pool.named(interner.intern("Eq")),
        trait_kind: DerivedTrait::Eq,
        method_name,
        signature: signature.clone(),
        span: Span::DUMMY,
    };

    let plan = call_plan(&accepted, &interner, &pool);
    let Some(result) = build_supported_derived_body(
        &accepted,
        &plan,
        executable_name,
        &signature,
        &interner,
        &pool,
    ) else {
        panic!("accepted newtype Eq must remain in the shared body inventory")
    };
    let body = result.unwrap_or_else(|error| {
        panic!("accepted newtype Eq must delegate to its underlying target: {error}")
    });

    assert_eq!(body.name, executable_name);
    assert!(body
        .params
        .iter()
        .all(|parameter| parameter.ty == owner_type));
    assert_eq!(body.return_type, Idx::BOOL);
    assert!(body.method_call_facts.is_empty());
}
