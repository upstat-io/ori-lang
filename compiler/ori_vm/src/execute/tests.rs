use ori_arc::ir::{compute_var_rc_strategies, PrimitiveFacts};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, ArcBlock, ArcBlockId, ArcClassifier, ArcFunction, ArcInstr,
    ArcTerminator, ArcValue, ArcVarId, ArgOwnership, CtorKind, FrozenClosureAdapters, LitValue,
    MemoryContract, RcAtomicity, RcStrategy, RetainPlanTable,
};
use ori_ir::SharedInterner;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, FunctionFamilyTopology, EXECUTABLE_PROGRAM_VERSION,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};

use super::arena::{Aggregate, AggregateKind, IteratorSourceState, IteratorState};
use super::frame::{Frame, ReturnTo};
use super::heap::HeapObject;
use super::{
    execute_report, ArenaAllocationRoots, ExecutionConfig, ExitValue, Interpreter, VmValue,
};
use crate::bytecode::{
    Register, VmCalleeOwnerDemand, VmClosureAdapterAction, VmClosureAdapterPlan,
    VmClosureAdapterSlot, VmClosureAdapterSource, VmRetainEdge, VmRetainPlan, VmRetainPlanId,
    VmRetainPlanKind, VmTypeId,
};
use crate::{compile, verify, ExecutionError, VerifiedProgram};

#[test]
fn report_success_after_heap_release_records_zero_exit_and_final_live_bytes() {
    let program = verified_list_program(
        true,
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );

    let report = execute_report(&program, ExecutionConfig::default());

    match report.result {
        Ok(value) => assert_eq!(value, ExitValue::Int(7)),
        Err(error) => panic!("released-list program should succeed: {error}"),
    }
    assert_eq!(report.metrics.cumulative_heap_allocations, 1);
    assert_eq!(report.metrics.exit_live_heap_objects, 0);
    assert_eq!(report.metrics.exit_live_heap_payload_bytes, 0);
    assert_eq!(report.metrics.final_live_heap_objects, 0);
    assert_eq!(report.metrics.final_live_heap_payload_bytes, 0);
    assert_eq!(report.metrics.peak_heap_objects, 1);
    assert!(report.metrics.peak_heap_payload_bytes >= std::mem::size_of::<super::VmValue>());
}

#[test]
fn report_error_with_live_heap_object_cleans_final_session_state() {
    let program = verified_list_program(false, ArcTerminator::Unreachable);

    let report = execute_report(&program, ExecutionConfig::default());

    assert!(matches!(
        report.result,
        Err(ExecutionError::ReachedUnreachable)
    ));
    assert_eq!(report.metrics.exit_live_heap_objects, 1);
    assert!(report.metrics.exit_live_heap_payload_bytes > 0);
    assert_eq!(report.metrics.final_live_heap_objects, 0);
    assert_eq!(report.metrics.final_live_heap_payload_bytes, 0);
}

#[test]
fn report_quota_after_heap_allocation_cleans_final_session_state() {
    let program = verified_list_program(
        false,
        ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    );
    let config = ExecutionConfig {
        max_steps: 2,
        ..ExecutionConfig::default()
    };

    let report = execute_report(&program, config);

    assert!(matches!(
        report.result,
        Err(ExecutionError::StepLimit { limit: 2 })
    ));
    assert_eq!(report.metrics.steps, 2);
    assert_eq!(report.metrics.exit_live_heap_objects, 1);
    assert!(report.metrics.exit_live_heap_payload_bytes > 0);
    assert_eq!(report.metrics.final_live_heap_objects, 0);
    assert_eq!(report.metrics.final_live_heap_payload_bytes, 0);
}

#[test]
fn panic_unwind_drops_owned_list_iterator_and_its_source() {
    let program = verified_iterator_unwind_program();

    let report = execute_report(&program, ExecutionConfig::default());

    assert!(matches!(
        report.result,
        Err(ExecutionError::Panic { ref message }) if message == "iterator unwind"
    ));
    assert_eq!(report.metrics.cumulative_heap_allocations, 1);
    assert_eq!(report.metrics.exit_live_heap_objects, 0);
    assert_eq!(report.metrics.final_live_heap_objects, 0);
    assert_eq!(
        report.metrics.cumulative_value_arena_iterator_allocations,
        1
    );
    assert_eq!(report.metrics.exit_value_arena_entries, 1);
    assert_eq!(report.metrics.final_value_arena_entries, 0);
}

fn verified_iterator_unwind_program() -> VerifiedProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let iter = symbols.intern("iter");
    let iter_drop = symbols.intern("ori_iter_drop");
    let panic = symbols.intern("ori_panic");
    let message = symbols.intern("iterator unwind");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::BOOL);
    let iterator_type = pool.iterator(Idx::BOOL);
    let mut function = test_function_with_blocks(
        main,
        vec![
            iterator_unwind_entry_block(list_type, iterator_type, iter, panic, message),
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(5),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(99)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![ArcInstr::Apply {
                    dst: ArcVarId::new(6),
                    ty: Idx::UNIT,
                    func: iter_drop,
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                }],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![
            Idx::BOOL,
            list_type,
            iterator_type,
            Idx::STR,
            Idx::UNIT,
            Idx::INT,
            Idx::UNIT,
        ],
        Idx::INT,
    );
    function.method_call_facts = vec![ori_arc::MethodCallFact {
        destination: ArcVarId::new(2),
        receiver_type: list_type,
        form: ori_arc::MethodCallForm::Instance,
    }];
    verified_program(symbols, pool, vec![function], main)
}

fn iterator_unwind_entry_block(
    list_type: Idx,
    iterator_type: Idx,
    iter: ori_ir::Name,
    panic: ori_ir::Name,
    message: ori_ir::Name,
) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(true)),
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: list_type,
                ctor: CtorKind::ListLiteral,
                args: vec![ArcVarId::new(0)],
            },
            ArcInstr::Apply {
                dst: ArcVarId::new(2),
                ty: iterator_type,
                func: iter,
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
            ArcInstr::Let {
                dst: ArcVarId::new(3),
                ty: Idx::STR,
                value: ArcValue::Literal(LitValue::String(message)),
            },
        ],
        terminator: ArcTerminator::Invoke {
            dst: ArcVarId::new(4),
            ty: Idx::UNIT,
            func: panic,
            args: vec![ArcVarId::new(3)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
        },
    }
}

#[test]
fn zero_frame_limit_rejects_before_root_activation_is_created() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_frames: 0,
        ..ExecutionConfig::default()
    };

    let report = execute_report(&program, config);

    assert!(matches!(
        report.result,
        Err(ExecutionError::ResourceLimit {
            resource: "call frames",
            limit: 0,
        })
    ));
    assert_eq!(report.metrics.steps, 0);
    assert_eq!(report.metrics.peak_frames, 0);
    assert_eq!(report.metrics.cumulative_heap_allocations, 0);
    assert_eq!(report.metrics.cumulative_value_arena_allocations, 0);
}

#[test]
fn report_aggregate_arena_metrics_include_explicit_session_cleanup() {
    let program = verified_aggregate_program();

    let report = execute_report(&program, ExecutionConfig::default());

    match report.result {
        Ok(value) => assert_eq!(
            value,
            ExitValue::Aggregate {
                variant: 0,
                fields: vec![ExitValue::Int(7)],
            }
        ),
        Err(error) => panic!("aggregate program should succeed: {error}"),
    }
    assert_eq!(report.metrics.cumulative_value_arena_allocations, 1);
    assert_eq!(
        report.metrics.cumulative_value_arena_aggregate_allocations,
        1
    );
    assert_eq!(
        report.metrics.cumulative_value_arena_iterator_allocations,
        0
    );
    assert_eq!(report.metrics.exit_value_arena_entries, 1);
    assert!(report.metrics.exit_value_arena_owned_bytes > 0);
    assert_eq!(report.metrics.final_value_arena_entries, 0);
    assert_eq!(report.metrics.final_value_arena_slots, 0);
    assert_eq!(report.metrics.final_value_arena_owned_bytes, 0);
    assert_eq!(report.metrics.peak_value_arena_entries, 1);
    assert_eq!(report.metrics.peak_value_arena_slots, 1);
    assert_eq!(report.metrics.cumulative_value_arena_collections, 0);
    assert_eq!(report.metrics.cumulative_value_arena_reclaimed_entries, 0);
    assert_eq!(report.metrics.cumulative_value_arena_reused_entries, 0);
    assert_eq!(
        report.metrics.peak_value_arena_owned_bytes,
        report.metrics.exit_value_arena_owned_bytes
    );
}

#[test]
fn variant_projection_exposes_logical_tag_before_payload_fields() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let mut aggregate = Aggregate {
        length: 2,
        variant: 3,
        kind: AggregateKind::Variant,
        ..Aggregate::default()
    };
    aggregate.fields[0] = super::VmValue::int(17);
    aggregate.fields[1] = super::VmValue::int(29);
    let value = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(aggregate, interpreter.config.max_value_arena_entries),
        "variant should allocate",
    );

    assert_eq!(
        must_succeed(interpreter.project(value, 0), "tag should project"),
        super::VmValue::int(3)
    );
    assert_eq!(
        must_succeed(
            interpreter.project(value, 1),
            "first payload should project"
        ),
        super::VmValue::int(17)
    );
    assert_eq!(
        must_succeed(
            interpreter.project(value, 2),
            "second payload should project"
        ),
        super::VmValue::int(29)
    );
    assert!(matches!(
        interpreter.project(value, 3),
        Err(ExecutionError::AggregateFieldOutOfBounds {
            field: 3,
            length: 3,
        })
    ));
}

#[test]
fn collection_traces_active_frame_registers_and_nested_aggregate_fields() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 3,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let mut child = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    child.fields[0] = super::VmValue::int(37);
    let child = must_succeed(
        interpreter.value_arena.allocate_aggregate(child, 3),
        "child should allocate",
    );
    let mut parent = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    parent.fields[0] = child;
    let parent = must_succeed(
        interpreter.value_arena.allocate_aggregate(parent, 3),
        "parent should allocate",
    );
    let garbage = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 3),
        "garbage should allocate",
    );
    interpreter.frames[0].registers[0] = parent;

    must_succeed(
        interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::all_live()),
        "collection should preserve register graph",
    );
    let replacement = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 3),
        "reclaimed slot should be reusable",
    );

    let child = must_succeed(
        interpreter.value_arena.aggregate(child),
        "nested child should remain live",
    );
    assert_eq!(
        must_succeed(child.fields[0].as_int(), "child field should be an int"),
        37
    );
    assert!(matches!(
        interpreter.value_arena.aggregate(garbage),
        Err(ExecutionError::StaleValueArenaHandle { .. })
    ));
    assert_ne!(replacement, garbage);
}

#[test]
fn collection_traces_aggregate_handles_owned_by_heap_collections() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 2,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let child = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 2),
        "heap child should allocate",
    );
    let garbage = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 2),
        "garbage should allocate",
    );
    must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::List(vec![child]), config.max_heap_objects),
        "heap list should allocate",
    );

    must_succeed(
        interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::all_live()),
        "collection should trace heap-owned values",
    );

    must_succeed(
        interpreter.value_arena.aggregate(child),
        "heap child should remain live",
    );
    assert!(matches!(
        interpreter.value_arena.aggregate(garbage),
        Err(ExecutionError::StaleValueArenaHandle { .. })
    ));
}

#[test]
fn closure_environment_accounts_payload_and_roots_captures() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 2,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let captured = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 2),
        "captured aggregate should allocate",
    );
    let garbage = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 2),
        "unreachable aggregate should allocate",
    );
    let mut captures = Vec::with_capacity(3);
    captures.push(captured);
    let capture_bytes = captures.capacity() * std::mem::size_of::<super::VmValue>();
    let closure = must_succeed(
        interpreter.heap.allocate(
            HeapObject::Closure {
                callee: program.program.main,
                captures,
            },
            config.max_heap_objects,
        ),
        "closure environment should allocate",
    );

    assert_eq!(interpreter.heap.metrics().live_payload_bytes, capture_bytes);
    must_succeed(
        interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::all_live()),
        "collection should trace closure captures",
    );
    must_succeed(
        interpreter.value_arena.aggregate(captured),
        "captured aggregate should remain live",
    );
    assert!(matches!(
        interpreter.value_arena.aggregate(garbage),
        Err(ExecutionError::StaleValueArenaHandle { .. })
    ));

    must_succeed(
        interpreter.release_owned_value(closure),
        "closure release should reclaim its environment",
    );
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
    assert_eq!(interpreter.heap.metrics().live_payload_bytes, 0);
}

#[test]
fn closure_final_release_transitively_releases_captured_heap_owner() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    let captured = must_succeed(
        interpreter.heap.allocate(
            HeapObject::List(vec![VmValue::int(7)]),
            interpreter.config.max_heap_objects,
        ),
        "captured list should allocate",
    );
    let closure = must_succeed(
        interpreter.heap.allocate(
            HeapObject::Closure {
                callee: program.program.main,
                captures: vec![captured],
            },
            interpreter.config.max_heap_objects,
        ),
        "closure environment should allocate",
    );

    must_succeed(
        interpreter.release_owned_value(closure),
        "closure release should reach its captured owner",
    );

    assert_eq!(interpreter.heap.metrics().live_objects, 0);
    assert!(matches!(
        interpreter.heap.get(captured),
        Err(ExecutionError::ReleasedHeap { .. })
    ));
}

#[test]
fn projected_nested_closure_adapter_copies_only_the_owned_path() {
    let mut program = verified_aggregate_program();
    install_projected_nested_closure_adapter(&mut program);
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    let fixture = allocate_projected_nested_values(&mut interpreter);
    let mut values = [fixture.outer];

    must_succeed(
        interpreter.prepare_closure_adapter_values(program.program.main, &mut values),
        "projected adapter should prepare an exact independent owner",
    );
    commit_projected_retain(&mut interpreter, &fixture);
    assert_projected_copy_is_independent(&mut interpreter, &fixture, values[0]);

    must_succeed(
        interpreter.release_owned_value(fixture.outer),
        "caller's complete value should release normally",
    );
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

fn install_projected_nested_closure_adapter(program: &mut VerifiedProgram) {
    let function = program.program.main;
    let shared_plan = VmRetainPlanId::from_shared(ori_arc::RetainPlanId::from_raw(0));
    let inner_plan = VmRetainPlanId::from_shared(ori_arc::RetainPlanId::from_raw(1));
    let outer_plan = VmRetainPlanId::from_shared(ori_arc::RetainPlanId::from_raw(2));
    let shared_type = VmTypeId::from_raw(41);
    let inner_type = VmTypeId::from_raw(42);
    let outer_type = VmTypeId::from_raw(43);
    program.program.retain_plans = vec![
        VmRetainPlan {
            ty: shared_type,
            kind: VmRetainPlanKind::SelfOwnedIdentity,
        },
        VmRetainPlan {
            ty: inner_type,
            kind: VmRetainPlanKind::OwnedFields(
                vec![VmRetainEdge {
                    field: 0,
                    child: shared_plan,
                }]
                .into_boxed_slice(),
            ),
        },
        VmRetainPlan {
            ty: outer_type,
            kind: VmRetainPlanKind::OwnedFields(
                vec![VmRetainEdge {
                    field: 0,
                    child: inner_plan,
                }]
                .into_boxed_slice(),
            ),
        },
    ]
    .into_boxed_slice();
    program.program.functions[function.index()].closure_adapter = Some(VmClosureAdapterPlan {
        capture_count: 0,
        slots: vec![VmClosureAdapterSlot {
            source: VmClosureAdapterSource::BorrowedCallArgument,
            ty: outer_type,
            demand: VmCalleeOwnerDemand::ProjectedField(0),
            action: VmClosureAdapterAction::Retain(outer_plan),
        }]
        .into_boxed_slice(),
    });
}

struct ProjectedNestedValues {
    selected: VmValue,
    unselected: VmValue,
    inner: VmValue,
    outer: VmValue,
}

fn allocate_projected_nested_values(interpreter: &mut Interpreter<'_>) -> ProjectedNestedValues {
    let selected = must_succeed(
        interpreter.heap.allocate(
            HeapObject::List(vec![VmValue::int(7)]),
            interpreter.config.max_heap_objects,
        ),
        "selected nested owner should allocate",
    );
    let unselected = must_succeed(
        interpreter.heap.allocate(
            HeapObject::List(vec![VmValue::int(11)]),
            interpreter.config.max_heap_objects,
        ),
        "unselected sibling should allocate",
    );
    let mut inner = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    inner.fields[0] = selected;
    let inner = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(inner, interpreter.config.max_value_arena_entries),
        "inner product should allocate",
    );
    let mut outer = Aggregate {
        length: 2,
        ..Aggregate::default()
    };
    outer.fields[0] = inner;
    outer.fields[1] = unselected;
    let outer = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(outer, interpreter.config.max_value_arena_entries),
        "outer product should allocate",
    );

    ProjectedNestedValues {
        selected,
        unselected,
        inner,
        outer,
    }
}

fn commit_projected_retain(interpreter: &mut Interpreter<'_>, fixture: &ProjectedNestedValues) {
    assert_eq!(
        must_succeed(
            interpreter.heap.references(fixture.selected),
            "selected owner should remain live",
        ),
        1,
        "retains stay staged until the callee frame is ready",
    );
    interpreter.commit_prepared_retains(1);
    assert_eq!(
        must_succeed(
            interpreter.heap.references(fixture.selected),
            "selected owner should be credited",
        ),
        2,
    );
    assert_eq!(
        must_succeed(
            interpreter.heap.references(fixture.unselected),
            "unselected sibling should remain live",
        ),
        1,
        "projected demand must not credit an unrelated field",
    );
}

fn assert_projected_copy_is_independent(
    interpreter: &mut Interpreter<'_>,
    fixture: &ProjectedNestedValues,
    copied_outer: VmValue,
) {
    assert_ne!(copied_outer, fixture.outer);
    let copied_inner = must_succeed(
        interpreter.value_arena.aggregate(copied_outer),
        "copied outer product should remain live",
    )
    .fields[0];
    assert_ne!(copied_inner, fixture.inner);
    assert_eq!(
        must_succeed(
            interpreter.value_arena.aggregate(fixture.outer),
            "caller's outer product should remain live",
        )
        .fields[0],
        fixture.inner,
    );

    must_succeed(
        interpreter.release_owned_value(copied_inner),
        "callee's projected owner should release independently",
    );
    must_succeed(
        interpreter.set_field(copied_inner, 0, VmValue::int(99)),
        "callee copy should remain independently mutable",
    );
    assert_eq!(
        must_succeed(
            interpreter.value_arena.aggregate(fixture.inner),
            "caller's inner product should remain live",
        )
        .fields[0],
        fixture.selected,
    );
    assert_eq!(
        must_succeed(
            interpreter.heap.references(fixture.selected),
            "caller should retain its selected owner",
        ),
        1,
    );
    assert_eq!(
        must_succeed(
            interpreter.heap.references(fixture.unselected),
            "caller should retain its unselected sibling",
        ),
        1,
    );
}

#[test]
fn closure_rc_strategy_requires_exact_object_and_materialization_rejects_escape() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    let list = must_succeed(
        interpreter.heap.allocate(
            HeapObject::List(Vec::new()),
            interpreter.config.max_heap_objects,
        ),
        "list should allocate",
    );
    assert!(matches!(
        interpreter.retain_with_strategy(list, 1, RcStrategy::Closure),
        Err(ExecutionError::InvalidClosureObject)
    ));
    assert_eq!(
        must_succeed(interpreter.heap.references(list), "list should remain live"),
        1
    );

    let closure = must_succeed(
        interpreter.heap.allocate(
            HeapObject::Closure {
                callee: program.program.main,
                captures: Vec::new(),
            },
            interpreter.config.max_heap_objects,
        ),
        "closure environment should allocate",
    );
    must_succeed(
        interpreter.retain_with_strategy(closure, 1, RcStrategy::Closure),
        "closure retain should increment only its environment",
    );
    assert_eq!(
        must_succeed(
            interpreter.heap.references(closure),
            "closure should remain live",
        ),
        2
    );
    must_succeed(
        interpreter.release_with_strategy(closure, RcStrategy::Closure),
        "closure release should decrement its environment",
    );
    assert!(matches!(
        interpreter.materialize(closure),
        Err(ExecutionError::EscapingClosure)
    ));
    must_succeed(
        interpreter.release_with_strategy(closure, RcStrategy::Closure),
        "final closure release should reclaim its environment",
    );
    must_succeed(
        interpreter.release_owned_value(list),
        "unrelated list should release normally",
    );
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn allocation_collection_excludes_only_the_overwritten_register() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let destination = Register::from_arc(ArcVarId::new(0));

    for _ in 0..3 {
        let old = must_succeed(
            interpreter
                .value_arena
                .allocate_aggregate(Aggregate::default(), 1),
            "old destination should allocate",
        );
        interpreter.set_register(0, destination, old);
        must_succeed(
            interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::overwriting(
                0,
                destination,
                &[],
            )),
            "killed destination should not root its old value",
        );
        assert!(matches!(
            interpreter.value_arena.aggregate(old),
            Err(ExecutionError::StaleValueArenaHandle { .. })
        ));
    }
}

#[test]
fn allocation_local_alias_roots_destination_operand_before_overwrite() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let destination = Register::from_arc(ArcVarId::new(0));
    let old = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 1),
        "operand should allocate",
    );
    interpreter.set_register(0, destination, old);

    must_succeed(
        interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::overwriting(
            0,
            destination,
            &[old],
        )),
        "allocation-local operand should remain reachable",
    );
    let Err(error) = interpreter
        .value_arena
        .allocate_aggregate(Aggregate::default(), 1)
    else {
        panic!("a live operand and result cannot share one live-entry slot")
    };

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "live value arena entries",
            limit: 1,
        }
    ));
    must_succeed(
        interpreter.value_arena.aggregate(old),
        "allocation-local alias must not become stale",
    );
}

#[test]
fn non_destination_alias_keeps_old_value_live_during_allocation_collection() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_value_arena_entries: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let destination = Register::from_arc(ArcVarId::new(0));
    let alias = Register::from_arc(ArcVarId::new(1));
    let old = must_succeed(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 1),
        "aliased destination should allocate",
    );
    interpreter.set_register(0, destination, old);
    interpreter.set_register(0, alias, old);

    must_succeed(
        interpreter.prepare_value_arena_allocation(ArenaAllocationRoots::overwriting(
            0,
            destination,
            &[],
        )),
        "non-destination aliases should remain roots",
    );

    assert!(matches!(
        interpreter
            .value_arena
            .allocate_aggregate(Aggregate::default(), 1),
        Err(ExecutionError::ResourceLimit {
            resource: "live value arena entries",
            limit: 1,
        })
    ));
    must_succeed(
        interpreter.value_arena.aggregate(old),
        "forged alias should keep its target live",
    );
}

#[test]
fn direct_iterator_return_transfers_arena_identity_to_caller() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let iterator = must_succeed(
        interpreter.value_arena.allocate_iterator(
            IteratorState {
                source: IteratorSourceState::Range {
                    current: 3,
                    end: 8,
                    step: 2,
                    inclusive: false,
                },
                live: true,
            },
            interpreter.config.max_value_arena_entries,
        ),
        "iterator should allocate",
    );
    let destination = Register::from_arc(ArcVarId::new(0));
    push_synthetic_return_frame(&mut interpreter, destination);

    assert!(must_succeed(
        interpreter.return_from(1, iterator),
        "iterator return should transfer"
    )
    .is_none());
    let returned = interpreter.register(0, destination);
    let state = must_succeed(
        interpreter.iterator_mut(returned),
        "caller should consume returned iterator",
    );
    let IteratorSourceState::Range { current, step, .. } = &mut state.source else {
        panic!("returned iterator should preserve its range source")
    };
    assert_eq!(*current, 3);
    *current += *step;
    assert_eq!(*current, 5);
}

#[test]
fn aggregate_nested_iterator_return_and_drop_are_transitive() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let iterator = must_succeed(
        interpreter
            .value_arena
            .allocate_iterator(live_iterator_state(), 2),
        "iterator should allocate",
    );
    let mut aggregate = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    aggregate.fields[0] = iterator;
    let aggregate = must_succeed(
        interpreter.value_arena.allocate_aggregate(aggregate, 2),
        "aggregate should allocate",
    );
    let destination = Register::from_arc(ArcVarId::new(0));
    push_synthetic_return_frame(&mut interpreter, destination);

    must_succeed(
        interpreter.return_from(1, aggregate),
        "aggregate return should transfer",
    );
    let returned = interpreter.register(0, destination);
    let nested = must_succeed(
        interpreter.project(returned, 0),
        "caller should project nested iterator",
    );
    assert!(must_succeed(interpreter.iterator_mut(nested), "iterator should be live").live);
    must_succeed(
        interpreter.release_owned_value(returned),
        "aggregate drop should reach iterator",
    );
    assert!(
        !must_succeed(
            interpreter.iterator_mut(nested),
            "dropped iterator identity should remain inspectable until arena collection",
        )
        .live
    );
}

#[test]
fn list_nested_iterator_return_and_drop_are_transitive() {
    let program = verified_aggregate_program();
    let mut interpreter = Interpreter::new(&program, ExecutionConfig::default());
    must_succeed(interpreter.push_root(), "root frame should initialize");
    let iterator = must_succeed(
        interpreter
            .value_arena
            .allocate_iterator(live_iterator_state(), 1),
        "iterator should allocate",
    );
    let list = must_succeed(
        interpreter.heap.allocate(
            HeapObject::List(vec![iterator]),
            interpreter.config.max_heap_objects,
        ),
        "list should allocate",
    );
    let destination = Register::from_arc(ArcVarId::new(0));
    push_synthetic_return_frame(&mut interpreter, destination);

    must_succeed(
        interpreter.return_from(1, list),
        "list return should transfer",
    );
    let returned = interpreter.register(0, destination);
    let nested = match &must_succeed(
        interpreter.heap.get(returned),
        "caller should receive live list",
    )
    .object
    {
        HeapObject::List(values) => values[0],
        HeapObject::Builder(_)
        | HeapObject::String(_)
        | HeapObject::Closure { .. }
        | HeapObject::Vacant => {
            panic!("returned object should remain a list")
        }
    };
    assert!(must_succeed(interpreter.iterator_mut(nested), "iterator should be live").live);
    must_succeed(
        interpreter.release_owned_value(returned),
        "list drop should reach iterator",
    );
    assert!(
        !must_succeed(
            interpreter.iterator_mut(nested),
            "dropped iterator identity should remain inspectable until arena collection",
        )
        .live
    );
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

fn push_synthetic_return_frame(interpreter: &mut Interpreter<'_>, destination: Register) {
    let root = &interpreter.frames[0];
    let frame = Frame::new_layout(
        root.function,
        root.pc,
        root.registers.len(),
        Some(ReturnTo {
            destination: destination.into(),
            normal: root.pc,
            unwind: None,
        }),
    );
    interpreter.frames.push(frame);
    interpreter.depth = 2;
    interpreter.peak_frames = 2;
}

const fn live_iterator_state() -> IteratorState {
    IteratorState {
        source: IteratorSourceState::Range {
            current: 1,
            end: 4,
            step: 1,
            inclusive: false,
        },
        live: true,
    }
}

pub(super) fn verified_list_program(release: bool, terminator: ArcTerminator) -> VerifiedProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::INT);
    let mut body = vec![
        ArcInstr::Let {
            dst: ArcVarId::new(0),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(7)),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(1),
            ty: list_type,
            ctor: CtorKind::ListLiteral,
            args: vec![ArcVarId::new(0)],
        },
    ];
    if release {
        body.push(ArcInstr::RcDec {
            var: ArcVarId::new(1),
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::default_atomic(),
        });
    }
    let function = test_function(main, body, terminator, vec![Idx::INT, list_type], Idx::INT);
    verified_program(symbols, pool, vec![function], main)
}

pub(super) fn verified_aggregate_program() -> VerifiedProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut pool = Pool::new();
    let tuple_type = pool.tuple(&[Idx::INT]);
    let mut function = test_function(
        main,
        vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(7)),
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: tuple_type,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0)],
            },
        ],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
        vec![Idx::INT, tuple_type],
        tuple_type,
    );
    populate_fixture_metadata(std::slice::from_mut(&mut function), &pool);
    let contract = MemoryContract::conservative(function.params.len());
    let function_effects = [(main, contract.function_effect_facts(&function))]
        .into_iter()
        .collect();
    let fresh_return_facts = [(main, contract.fresh_self_allocation_facts())]
        .into_iter()
        .collect();
    let param_disjointness = [(main, prove_param_disjointness(&[], &pool))]
        .into_iter()
        .collect();
    let contracts = [(main, contract)].into_iter().collect();
    let functions = vec![function];
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    let program = must_succeed(
        ExecutableProgram::validate(ExecutableProgramParts {
            version: EXECUTABLE_PROGRAM_VERSION,
            symbols,
            pool,
            functions,
            function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
            contracts,
            function_effects,
            fresh_return_facts,
            param_disjointness,
            callable_facts,
            closure_adapters: FrozenClosureAdapters::default().adapters,
            retain_plans: RetainPlanTable::default(),
            roots: vec![main],
            cli_entry: Some(main),
            externals: Vec::new(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry: TypeRegistry::new(),
        }),
        "test executable should validate",
    );
    let bytecode = must_succeed(compile(&program), "test executable should compile");
    must_succeed(verify(bytecode), "test bytecode should verify")
}

pub(super) fn verified_constant_string_program(text: &str) -> VerifiedProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let string = symbols.intern(text);
    let pool = Pool::new();
    let function = test_function(
        main,
        vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::STR,
                value: ArcValue::Literal(LitValue::String(string)),
            },
            ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            },
        ],
        ArcTerminator::Return {
            value: ArcVarId::new(1),
        },
        vec![Idx::STR, Idx::INT],
        Idx::INT,
    );
    verified_program(symbols, pool, vec![function], main)
}

pub(super) fn verified_program(
    symbols: SharedInterner,
    pool: Pool,
    mut functions: Vec<ArcFunction>,
    main: ori_ir::Name,
) -> VerifiedProgram {
    populate_fixture_metadata(&mut functions, &pool);
    let type_registry = TypeRegistry::new();
    let contracts = functions
        .iter()
        .map(|function| {
            (
                function.name,
                MemoryContract::conservative(function.params.len()),
            )
        })
        .collect();
    let function_effects = functions
        .iter()
        .map(|function| {
            let contract = MemoryContract::conservative(function.params.len());
            (function.name, contract.function_effect_facts(function))
        })
        .collect();
    let fresh_return_facts = functions
        .iter()
        .map(|function| {
            let contract = MemoryContract::conservative(function.params.len());
            (function.name, contract.fresh_self_allocation_facts())
        })
        .collect();
    let param_disjointness = functions
        .iter()
        .map(|function| {
            let params: Vec<_> = function.params.iter().map(|param| param.ty).collect();
            (function.name, prove_param_disjointness(&params, &pool))
        })
        .collect();
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    let function_families = functions
        .iter()
        .map(|function| FunctionFamilyTopology::new(function.name, Vec::new()))
        .collect();
    let program = must_succeed(
        ExecutableProgram::validate(ExecutableProgramParts {
            version: EXECUTABLE_PROGRAM_VERSION,
            symbols,
            pool,
            functions,
            function_families,
            contracts,
            function_effects,
            fresh_return_facts,
            param_disjointness,
            callable_facts,
            closure_adapters: FrozenClosureAdapters::default().adapters,
            retain_plans: RetainPlanTable::default(),
            roots: vec![main],
            cli_entry: Some(main),
            externals: Vec::new(),
            user_drop_bindings: Vec::new(),
            repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
            type_registry,
        }),
        "test executable should validate",
    );
    let bytecode = must_succeed(compile(&program), "test executable should compile");
    must_succeed(verify(bytecode), "test bytecode should verify")
}

fn populate_fixture_metadata(functions: &mut [ArcFunction], pool: &Pool) {
    let classifier = ArcClassifier::new(pool);
    ori_arc::freeze_primitive_facts(functions, &classifier)
        .unwrap_or_else(|errors| panic!("test fixture primitive facts should freeze: {errors:?}"));
    for function in functions {
        function.var_reprs = ori_arc::compute_var_reprs(function, &classifier, pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::RepresentationsReady;
        function.var_rc_strategies = compute_var_rc_strategies(function, pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::Realized;
    }
}

fn test_function(
    name: ori_ir::Name,
    body: Vec<ArcInstr>,
    terminator: ArcTerminator,
    var_types: Vec<Idx>,
    return_type: Idx,
) -> ArcFunction {
    test_function_with_blocks(
        name,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator,
        }],
        var_types,
        return_type,
    )
}

fn test_function_with_blocks(
    name: ori_ir::Name,
    blocks: Vec<ArcBlock>,
    var_types: Vec<Idx>,
    return_type: Idx,
) -> ArcFunction {
    ArcFunction {
        name,
        params: Vec::new(),
        return_type,
        blocks,
        entry: ArcBlockId::new(0),
        var_types,
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        var_metadata_state: ori_arc::VariableMetadataState::Unrealized,
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        primitive_facts: PrimitiveFacts::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
        reassign_deaths: Vec::new(),
        catch_scoped_checked_ops: Vec::new(),
        method_call_facts: Vec::new(),
        class_ledger_emission: false,
    }
}

fn must_succeed<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error}"),
    }
}
