use super::*;

#[test]
fn value_range_is_interval_lattice() {
    assert_eq!(ValueRange::default(), ValueRange::Top);
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 10 }.join(ValueRange::Bounded { lo: 5, hi: 20 }),
        ValueRange::Bounded { lo: 0, hi: 20 }
    );
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 10 }.meet(ValueRange::Bounded { lo: 5, hi: 20 }),
        ValueRange::Bounded { lo: 5, hi: 10 }
    );
}

#[test]
fn compiled_allocation_mechanism_properties_cover_every_mode() {
    let runtime = crate::CompiledAllocationMechanism::RuntimeHeap {
        extent: YieldExtent::Unknown,
    };
    let managed = crate::CompiledAllocationMechanism::ManagedStack { capacity: 4 };
    let compact = crate::CompiledAllocationMechanism::CompactStack { capacity: 4 };

    assert!(!runtime.is_stack());
    assert!(runtime.requires_runtime_header());
    assert!(managed.is_stack());
    assert!(managed.requires_runtime_header());
    assert!(compact.is_stack());
    assert!(!compact.requires_runtime_header());
}

fn yield_fact(
    site: u32,
    builder: u32,
    result: u32,
    elem_size: u64,
    extent: YieldExtent,
    locality: YieldAllocationLocality,
) -> YieldAllocationFact {
    YieldAllocationFact {
        site: AllocationSiteId::new(site),
        builder: ArcVarId::new(builder),
        result: ArcVarId::new(result),
        elem_ty: ori_types::Idx::BOOL,
        elem_size_var: ArcVarId::new(result),
        elem_size,
        extent,
        locality,
    }
}

fn yield_allocation_function(
    name: Name,
    facts: Vec<YieldAllocationFact>,
    repeated_builder: ArcVarId,
) -> ArcFunction {
    let body = facts
        .iter()
        .filter(|fact| fact.builder != repeated_builder)
        .map(|fact| ArcInstr::Let {
            dst: fact.builder,
            ty: Idx::UNIT,
            value: ArcValue::Literal(LitValue::Unit),
        })
        .collect::<Vec<_>>();
    ArcFunction {
        name,
        return_type: Idx::UNIT,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body,
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: repeated_builder,
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT; 18],
        yield_allocations: facts,
        ..ArcFunction::default()
    }
}

#[test]
fn escape_info_only_admits_aims_proven_local_identities() {
    let local = yield_fact(
        0,
        1,
        2,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Local,
    );
    let escaping = yield_fact(
        1,
        3,
        4,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Escaping,
    );
    let unknown = yield_fact(
        2,
        5,
        6,
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Unknown,
    );
    let info = EscapeInfo::from_yield_allocations(&[local, escaping, unknown]);

    assert!(!info.escapes(local.builder));
    assert!(!info.escapes(local.result));
    assert!(info.escapes(escaping.builder));
    assert!(info.escapes(escaping.result));
    assert!(info.escapes(unknown.builder));
    assert!(info.escapes(unknown.result));
    assert!(info.escapes(ArcVarId::new(99)));
}

#[test]
fn yield_allocation_selection_is_exact_bounded_and_fail_closed() {
    let function = Name::new(0, 91);
    let local = yield_fact(
        0,
        1,
        2,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Local,
    );

    let oversized = yield_fact(
        1,
        3,
        4,
        8,
        YieldExtent::StaticExact(513),
        YieldAllocationLocality::Local,
    );

    let dynamic = yield_fact(
        2,
        5,
        6,
        8,
        YieldExtent::RuntimeExact(ArcVarId::new(7)),
        YieldAllocationLocality::Local,
    );

    let escaping = yield_fact(
        3,
        8,
        9,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Escaping,
    );

    let unknown = yield_fact(
        5,
        12,
        13,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Unknown,
    );

    let at_limit = yield_fact(
        6,
        14,
        15,
        8,
        YieldExtent::StaticExact(512),
        YieldAllocationLocality::Local,
    );

    let overflow = yield_fact(
        7,
        16,
        17,
        u64::MAX,
        YieldExtent::StaticExact(2),
        YieldAllocationLocality::Local,
    );

    let repeated = yield_fact(
        4,
        10,
        11,
        8,
        YieldExtent::StaticExact(32),
        YieldAllocationLocality::Local,
    );
    let function_ir = yield_allocation_function(
        function,
        vec![
            local, oversized, dynamic, escaping, unknown, at_limit, overflow, repeated,
        ],
        repeated.builder,
    );
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.freeze_yield_allocations(&[function_ir]);

    let Some(local_decision) = plan.yield_allocation_for_builder(function, local.builder) else {
        panic!("local allocation decision");
    };

    assert!(matches!(
        local_decision.mechanism,
        crate::CompiledAllocationMechanism::ManagedStack { capacity: 32 }
    ));

    let Some(at_limit_decision) = plan.yield_allocation_for_builder(function, at_limit.builder)
    else {
        panic!("at-limit allocation decision");
    };

    assert!(matches!(
        at_limit_decision.mechanism,
        crate::CompiledAllocationMechanism::ManagedStack { .. }
    ));

    for fact in [oversized, dynamic, escaping, unknown, overflow, repeated] {
        let Some(decision) = plan.yield_allocation_for_result(function, fact.result) else {
            panic!("managed allocation decision");
        };

        assert!(matches!(
            decision.mechanism,
            crate::CompiledAllocationMechanism::RuntimeHeap { .. }
        ));
    }
}

#[test]
fn yield_header_elision_requires_exact_runtime_call_targets() {
    let function_name = Name::new(0, 95);
    let observer_name = Name::new(0, 96);
    let result = ArcVarId::new(0);
    let observed = ArcVarId::new(1);
    let fact = yield_fact(
        0,
        2,
        result.raw(),
        1,
        YieldExtent::StaticExact(4),
        YieldAllocationLocality::Local,
    );
    let function = ArcFunction {
        name: function_name,
        return_type: ori_types::Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: fact.builder,
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
                ArcInstr::Apply {
                    dst: observed,
                    ty: ori_types::Idx::INT,
                    func: observer_name,
                    args: vec![result],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: observed },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::BOOL, ori_types::Idx::INT, Idx::UNIT],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar],
        spans: vec![vec![None; 2]],
        yield_allocations: vec![fact],
        ..ArcFunction::default()
    };
    let pool = ori_types::Pool::new();

    let mut runtime_plan = ReprPlan::new(NarrowingPolicy::Disabled);
    runtime_plan.freeze_yield_allocations(std::slice::from_ref(&function));
    runtime_plan.close_yield_runtime_header_requirements(
        std::slice::from_ref(&function),
        &pool,
        |_, dst| (dst == observed).then_some(crate::plan::YieldLineageRuntimeCall::BorrowedRead),
    );
    let Some(runtime_decision) = runtime_plan.yield_allocation_for_result(function_name, result)
    else {
        panic!("runtime-target yield decision");
    };
    assert!(matches!(
        runtime_decision.mechanism,
        crate::CompiledAllocationMechanism::CompactStack { .. }
    ));

    let mut exact_plan = ReprPlan::new(NarrowingPolicy::Disabled);
    exact_plan.freeze_yield_allocations(std::slice::from_ref(&function));
    exact_plan.close_yield_runtime_header_requirements(&[function], &pool, |_, _| None);
    let Some(exact_decision) = exact_plan.yield_allocation_for_result(function_name, result) else {
        panic!("exact-target yield decision");
    };
    assert!(
        exact_decision.mechanism.requires_runtime_header(),
        "same-spelled local/imported callables must fail closed to headerful storage"
    );
}
