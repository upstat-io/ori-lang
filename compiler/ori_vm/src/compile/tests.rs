use ori_arc::ir::{compute_var_rc_strategies, PrimitiveFacts};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, realize_closed_program, ArcBlock, ArcBlockId, ArcClassifier,
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, BuiltinOwnershipSets,
    CtorKind, FrozenClosureAdapters, LitValue, MemoryContract, Ownership, PrimOp, RcAtomicity,
    RcStrategy, RetainPlanTable,
};
use ori_ir::{BinaryOp, Name, SharedInterner};
use ori_registry::RuntimeOperator;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, ExternalCallable, ExternalUnwind,
    FunctionFamilyTopology, EXECUTABLE_PROGRAM_VERSION,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};

use crate::bytecode::{
    BytecodeProgram, CallArgument, Op, Register, VmClosureAdapterAction, VmRetainPlanId,
};
use crate::{
    execute_report, verify, CompileError, ExecutionConfig, ExitValue, IndexKind, VerifyError,
};

use super::{compile, compile_with_options, CompileOptions};

const ADMITTED_STRATEGIES: [RcStrategy; 5] = [
    RcStrategy::HeapPointer,
    RcStrategy::FatPointer,
    RcStrategy::AggregateFields,
    RcStrategy::InlineEnum,
    RcStrategy::Iterator,
];

#[test]
fn bytecode_compilation_requires_a_distinguished_cli_entry() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("library_root");
    let executable = close_test_artifact(
        symbols,
        Pool::new(),
        vec![minimal_unit_function(main, Vec::new())],
        vec![FunctionFamilyTopology::new(main, Vec::new())],
        main,
        None,
        Vec::new(),
    );

    assert!(matches!(
        compile(&executable),
        Err(CompileError::MissingCliEntry)
    ));
}

#[test]
fn bytecode_compilation_rejects_external_callable_targets() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let dependency = symbols.intern("dependency");
    let pool = Pool::new();
    let mut external_contract = MemoryContract::conservative(0);
    external_contract.effects.may_throw = false;
    let external = ExternalCallable::freeze(
        dependency,
        "_ori_dependency",
        Vec::new(),
        Idx::UNIT,
        external_contract,
        ExternalUnwind::NoUnwind,
        &pool,
    );
    let call = ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: dependency,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    };
    let executable = close_test_artifact(
        symbols,
        pool,
        vec![minimal_unit_function(main, vec![call])],
        vec![FunctionFamilyTopology::new(main, Vec::new())],
        main,
        Some(main),
        vec![external],
    );

    assert!(matches!(
        compile(&executable),
        Err(CompileError::UnsupportedExternalCall { function, .. }) if function == main
    ));
}

#[test]
fn closure_lowering_freezes_target_capture_and_parameter_ownership() {
    let (executable, main, lambda) = closure_executable();
    let bytecode = compile(&executable)
        .unwrap_or_else(|error| panic!("closed closure program should compile: {error}"));
    let main_function = bytecode
        .functions
        .iter()
        .find(|function| function.name == main)
        .unwrap_or_else(|| panic!("main bytecode should exist"));
    let lambda_function = bytecode
        .functions
        .iter()
        .find(|function| function.name == lambda)
        .unwrap_or_else(|| panic!("lambda bytecode should exist"));

    assert_eq!(lambda_function.capture_count, 1);
    assert_eq!(
        lambda_function.param_ownership.as_ref(),
        [Ownership::Owned, Ownership::Borrowed]
    );
    let (callee, captures) = main_function
        .ops
        .iter()
        .find_map(|operation| match operation {
            Op::MakeClosure {
                callee, captures, ..
            } => Some((*callee, *captures)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("partial application should lower to MakeClosure"));
    assert_eq!(bytecode.functions[callee.index()].name, lambda);
    assert_eq!(bytecode.operands[captures.index()].len(), 1);
    let arguments = main_function
        .ops
        .iter()
        .find_map(|operation| match operation {
            Op::CallClosure { args, .. } => Some(*args),
            _ => None,
        })
        .unwrap_or_else(|| panic!("indirect application should lower to CallClosure"));
    assert_eq!(bytecode.call_arguments[arguments.index()].len(), 1);
    assert_eq!(
        bytecode.call_arguments[arguments.index()][0].ownership(),
        ArgOwnership::Borrowed
    );
    assert!(main_function.ops.iter().any(|operation| matches!(
        operation,
        Op::RcDec { semantics, .. } if semantics.strategy == RcStrategy::Closure
    )));
    must_verify(bytecode);
}

#[test]
fn verifier_rejects_closure_capture_arity_drift() {
    let (executable, main, _) = closure_executable();
    let mut bytecode = compile(&executable)
        .unwrap_or_else(|error| panic!("closed closure program should compile: {error}"));
    let (pc, target, captures) = bytecode
        .functions
        .iter()
        .find(|function| function.name == main)
        .unwrap_or_else(|| panic!("main bytecode should exist"))
        .ops
        .iter()
        .enumerate()
        .find_map(|(pc, operation)| match operation {
            Op::MakeClosure {
                callee, captures, ..
            } => Some((pc, *callee, *captures)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("partial application should lower to MakeClosure"));
    let original = bytecode.operands[captures.index()][0];
    bytecode.operands[captures.index()] = vec![original, original].into_boxed_slice();

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::ClosureCaptureArity {
            function,
            pc: found_pc,
            target: found_target,
            expected: 1,
            actual: 2,
        } if function == main && found_pc == pc && found_target == target
    ));
}

#[test]
fn verifier_rejects_parameter_ownership_metadata_drift() {
    let (executable, _, lambda) = closure_executable();
    let mut bytecode = compile(&executable)
        .unwrap_or_else(|error| panic!("closed closure program should compile: {error}"));
    let lambda_function = bytecode
        .functions
        .iter_mut()
        .find(|function| function.name == lambda)
        .unwrap_or_else(|| panic!("lambda bytecode should exist"));
    lambda_function.param_ownership = vec![Ownership::Owned].into_boxed_slice();

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::ParameterOwnershipMetadata {
            function,
            parameters: 2,
            ownership_entries: 1,
        } if function == lambda
    ));
}

#[test]
fn verifier_rejects_stale_closure_adapter_retain_plan() {
    let (executable, _, lambda) = closure_executable();
    let mut bytecode = compile(&executable)
        .unwrap_or_else(|error| panic!("closed closure program should compile: {error}"));
    let lambda_function = bytecode
        .functions
        .iter_mut()
        .find(|function| function.name == lambda)
        .unwrap_or_else(|| panic!("lambda bytecode should exist"));
    let adapter = lambda_function
        .closure_adapter
        .as_mut()
        .unwrap_or_else(|| panic!("closure target should carry an adapter"));
    adapter.slots[0].action = VmClosureAdapterAction::Retain(VmRetainPlanId::from_shared(
        ori_arc::RetainPlanId::from_raw(0),
    ));

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::InvalidIndex {
            function,
            pc: None,
            kind: IndexKind::RetainPlan,
            index: 0,
            bound: 0,
        } if function == lambda
    ));
}

#[test]
fn borrowed_list_closure_argument_receives_exact_entry_credit() {
    let executable = borrowed_list_closure_executable();
    let bytecode = compile(&executable)
        .unwrap_or_else(|error| panic!("borrowed-list closure should compile: {error}"));
    let verified = verify(bytecode)
        .unwrap_or_else(|error| panic!("borrowed-list closure should verify: {error}"));

    let report = execute_report(&verified, ExecutionConfig::default());

    assert_eq!(
        report
            .result
            .unwrap_or_else(|error| panic!("borrowed-list closure should execute: {error}")),
        ExitValue::List(vec![ExitValue::Int(7)]),
    );
}

#[test]
fn list_concat_lowering_uses_frozen_runtime_identity_in_both_typed_modes() {
    let executable = list_concat_executable();

    for typed_primitives in [true, false] {
        let bytecode = compile_with_options(&executable, CompileOptions { typed_primitives })
            .unwrap_or_else(|error| panic!("list concat should compile: {error}"));
        assert!(bytecode.functions[0].ops.iter().any(|operation| matches!(
            operation,
            Op::RuntimeBinary {
                operator: RuntimeOperator::ListConcat,
                ..
            }
        )));
        must_verify(bytecode);
    }
}

#[test]
fn verifier_rejects_runtime_identity_without_an_execution_projection() {
    let executable = list_concat_executable();
    let mut bytecode =
        compile(&executable).unwrap_or_else(|error| panic!("list concat should compile: {error}"));
    let (pc, operator) = bytecode.functions[0]
        .ops
        .iter_mut()
        .enumerate()
        .find_map(|(pc, operation)| match operation {
            Op::RuntimeBinary { operator, .. } => Some((pc, operator)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("list concat bytecode should carry a runtime identity"));
    *operator = RuntimeOperator::StringConcat;
    let function = bytecode.functions[0].name;

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::UnsupportedRuntimePrimitive {
            function: found,
            pc: found_pc,
            operator: RuntimeOperator::StringConcat,
        } if found == function && found_pc == pc
    ));
}

#[test]
fn lowering_preserves_each_admitted_rc_semantics_and_verifies() {
    for strategy in ADMITTED_STRATEGIES {
        let bytecode = must_compile(strategy, RcAtomicity::Atomic);
        let semantics = bytecode.functions[0]
            .ops
            .iter()
            .filter_map(|operation| match operation {
                Op::RcInc { semantics, .. } | Op::RcDec { semantics, .. } => Some(*semantics),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(semantics.len(), 2);
        assert!(semantics.iter().all(|metadata| {
            metadata.strategy == strategy && metadata.atomicity == RcAtomicity::Atomic
        }));
        assert!(bytecode.functions[0]
            .register_rc_strategies
            .contains(&Some(strategy)));
        must_verify(bytecode);
    }
}

#[test]
fn lowering_rejects_non_atomic_rc_semantics() {
    let (function, result) = compile_result(RcStrategy::HeapPointer, RcAtomicity::NonAtomic);

    assert!(matches!(
        must_fail(result),
        CompileError::UnsupportedRcAtomicity {
            function: found,
            atomicity: RcAtomicity::NonAtomic,
        } if found == function
    ));
}

#[test]
fn lowering_rejects_user_drop_strategy() {
    let (function, result) = compile_result(RcStrategy::UserDrop, RcAtomicity::Atomic);

    assert!(matches!(
        must_fail(result),
        CompileError::UnsupportedRcStrategy {
            function: found,
            strategy: RcStrategy::UserDrop,
        } if found == function
    ));
}

#[test]
fn lowering_rejects_admitted_strategy_on_incompatible_register() {
    let (function, result) = compile_result_for_value(
        RcStrategy::HeapPointer,
        RcStrategy::AggregateFields,
        RcAtomicity::Atomic,
    );

    assert!(matches!(
        must_fail(result),
        CompileError::RcStrategyMismatch {
            function: found,
            register: 1,
            expected: Some(RcStrategy::HeapPointer),
            found: RcStrategy::AggregateFields,
        } if found == function
    ));
}

#[test]
fn verifier_rejects_forged_non_atomic_rc_semantics() {
    let mut bytecode = must_compile(RcStrategy::HeapPointer, RcAtomicity::Atomic);
    let (pc, semantics) = first_rc_semantics_mut(&mut bytecode);
    semantics.atomicity = RcAtomicity::NonAtomic;
    let function = bytecode.functions[0].name;

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::UnsupportedRcAtomicity {
            function: found,
            pc: found_pc,
            atomicity: RcAtomicity::NonAtomic,
        } if found == function && found_pc == pc
    ));
}

#[test]
fn verifier_rejects_forged_unsupported_rc_strategy() {
    let mut bytecode = must_compile(RcStrategy::HeapPointer, RcAtomicity::Atomic);
    let (pc, semantics) = first_rc_semantics_mut(&mut bytecode);
    semantics.strategy = RcStrategy::UserDrop;
    let function = bytecode.functions[0].name;

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::UnsupportedRcStrategy {
            function: found,
            pc: found_pc,
            strategy: RcStrategy::UserDrop,
        } if found == function && found_pc == pc
    ));
}

#[test]
fn verifier_rejects_admitted_strategy_on_incompatible_register() {
    let mut bytecode = must_compile(RcStrategy::HeapPointer, RcAtomicity::Atomic);
    let (pc, semantics) = first_rc_semantics_mut(&mut bytecode);
    semantics.strategy = RcStrategy::AggregateFields;
    let function = bytecode.functions[0].name;

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::RcStrategyMismatch {
            function: found,
            pc: found_pc,
            register: 1,
            expected: Some(RcStrategy::HeapPointer),
            found: RcStrategy::AggregateFields,
        } if found == function && found_pc == pc
    ));
}

#[test]
fn verifier_direct_register_out_of_bounds_reports_canonical_operand_error() {
    let mut bytecode = must_compile(RcStrategy::HeapPointer, RcAtomicity::Atomic);
    let register_count = bytecode.functions[0].register_count;
    let invalid = invalid_register(register_count);
    let (pc, register) = bytecode.functions[0]
        .ops
        .iter_mut()
        .enumerate()
        .find_map(|(pc, operation)| match operation {
            Op::RcInc { var, .. } => Some((pc, var)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("test bytecode should contain an RC increment"));
    *register = invalid;

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::InvalidIndex {
            pc: Some(found_pc),
            kind: IndexKind::Register,
            index,
            bound,
            ..
        } if found_pc == pc && index == register_count && bound == register_count
    ));
}

#[test]
fn verifier_call_argument_register_out_of_bounds_reports_canonical_operand_error() {
    let mut bytecode = must_compile(RcStrategy::Iterator, RcAtomicity::Atomic);
    let register_count = bytecode.functions[0].register_count;
    let argument = bytecode.call_arguments[0][0];
    bytecode.call_arguments[0][0] =
        CallArgument::new(invalid_register(register_count), argument.ownership());

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::InvalidIndex {
            pc: Some(_),
            kind: IndexKind::Register,
            index,
            bound,
            ..
        } if index == register_count && bound == register_count
    ));
}

#[test]
fn verifier_constructor_operand_out_of_bounds_reports_canonical_operand_error() {
    let mut bytecode = must_compile(RcStrategy::HeapPointer, RcAtomicity::Atomic);
    let register_count = bytecode.functions[0].register_count;
    bytecode.operands[0][0] = invalid_register(register_count);

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::InvalidIndex {
            pc: Some(_),
            kind: IndexKind::Register,
            index,
            bound,
            ..
        } if index == register_count && bound == register_count
    ));
}

fn first_rc_semantics_mut(
    bytecode: &mut BytecodeProgram,
) -> (usize, &mut crate::bytecode::RcSemantics) {
    for (pc, operation) in bytecode.functions[0].ops.iter_mut().enumerate() {
        match operation {
            Op::RcInc { semantics, .. } | Op::RcDec { semantics, .. } => {
                return (pc, semantics);
            }
            _ => {}
        }
    }
    panic!("test bytecode should contain an RC operation")
}

fn invalid_register(register_count: usize) -> Register {
    let raw = u32::try_from(register_count)
        .unwrap_or_else(|_| panic!("test register count should fit in bytecode identity"));
    Register::from_arc(ArcVarId::new(raw))
}

fn must_compile(strategy: RcStrategy, atomicity: RcAtomicity) -> BytecodeProgram {
    let (_, result) = compile_result(strategy, atomicity);
    match result {
        Ok(bytecode) => bytecode,
        Err(error) => panic!("test executable should compile: {error}"),
    }
}

fn compile_result(
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) -> (Name, Result<BytecodeProgram, CompileError>) {
    compile_result_for_value(strategy, strategy, atomicity)
}

fn compile_result_for_value(
    value_strategy: RcStrategy,
    strategy: RcStrategy,
    atomicity: RcAtomicity,
) -> (Name, Result<BytecodeProgram, CompileError>) {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let message = symbols.intern("rc semantics");
    let iter = symbols.intern("iter");
    let option_name = symbols.intern("Option");
    let mut pool = Pool::new();
    let RcValueFixture {
        mut body,
        rc_var,
        var_types,
    } = rc_value_fixture(value_strategy, &mut pool, message, iter, option_name);
    body.extend([
        ArcInstr::RcInc {
            var: rc_var,
            count: 2,
            strategy,
            atomicity,
        },
        ArcInstr::RcDec {
            var: rc_var,
            strategy,
            atomicity,
        },
    ]);
    let mut function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
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
    };
    let classifier = ArcClassifier::new(&pool);
    function.var_reprs = ori_arc::compute_var_reprs(&function, &classifier, &pool);
    function.var_metadata_state = ori_arc::VariableMetadataState::RepresentationsReady;
    function.var_rc_strategies = compute_var_rc_strategies(&function, &pool);
    function.var_metadata_state = ori_arc::VariableMetadataState::Realized;
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
    let executable = match ExecutableProgram::validate(ExecutableProgramParts {
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
    }) {
        Ok(program) => program,
        Err(error) => panic!("test executable should validate: {error}"),
    };

    (main, compile(&executable))
}

fn closure_executable() -> (ExecutableProgram, Name, Name) {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let lambda = symbols.intern("__lambda_main_0");
    let mut pool = Pool::new();
    let closure_type = pool.function(&[Idx::INT], Idx::INT);

    let main_function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(7)),
                },
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: closure_type,
                    func: lambda,
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(11)),
                },
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    closure: ArcVarId::new(1),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                    strategy: RcStrategy::Closure,
                    atomicity: RcAtomicity::Atomic,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, closure_type, Idx::INT, Idx::INT],
        num_captures: 0,
        ..ArcFunction::default()
    };
    let lambda_function = ArcFunction {
        name: lambda,
        params: vec![
            ori_arc::ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ori_arc::ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::INT,
                ownership: Ownership::Borrowed,
            },
        ],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::INT],
        num_captures: 1,
        ..ArcFunction::default()
    };
    let executable = close_test_executable(
        symbols,
        pool,
        vec![main_function, lambda_function],
        vec![FunctionFamilyTopology::new(main, vec![lambda])],
        main,
    );

    (executable, main, lambda)
}

fn borrowed_list_closure_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let lambda = symbols.intern("__lambda_main_borrowed_list");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::INT);
    let closure_type = pool.function(&[list_type], list_type);

    let main_function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
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
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(2),
                    ty: closure_type,
                    func: lambda,
                    args: Vec::new(),
                },
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(3),
                    ty: list_type,
                    closure: ArcVarId::new(2),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(2),
                    strategy: RcStrategy::Closure,
                    atomicity: RcAtomicity::Atomic,
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(1),
                    strategy: RcStrategy::HeapPointer,
                    atomicity: RcAtomicity::Atomic,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, list_type, closure_type, list_type],
        num_captures: 0,
        ..ArcFunction::default()
    };
    let lambda_function = ArcFunction {
        name: lambda,
        params: vec![ori_arc::ArcParam {
            var: ArcVarId::new(0),
            ty: list_type,
            ownership: Ownership::Borrowed,
        }],
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![list_type],
        num_captures: 0,
        ..ArcFunction::default()
    };

    close_test_executable(
        symbols,
        pool,
        vec![main_function, lambda_function],
        vec![FunctionFamilyTopology::new(main, vec![lambda])],
        main,
    )
}

fn close_test_executable(
    symbols: SharedInterner,
    pool: Pool,
    functions: Vec<ArcFunction>,
    function_families: Vec<FunctionFamilyTopology>,
    main: Name,
) -> ExecutableProgram {
    close_test_artifact(
        symbols,
        pool,
        functions,
        function_families,
        main,
        Some(main),
        Vec::new(),
    )
}

fn close_test_artifact(
    symbols: SharedInterner,
    pool: Pool,
    mut functions: Vec<ArcFunction>,
    function_families: Vec<FunctionFamilyTopology>,
    root: Name,
    cli_entry: Option<Name>,
    externals: Vec<ExternalCallable>,
) -> ExecutableProgram {
    let classifier = ArcClassifier::new(&pool);
    for function in &mut functions {
        function.var_reprs = ori_arc::compute_var_reprs(function, &classifier, &pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::RepresentationsReady;
        function.var_rc_strategies = compute_var_rc_strategies(function, &pool);
        function.var_metadata_state = ori_arc::VariableMetadataState::Realized;
    }

    let contracts: std::collections::HashMap<Name, MemoryContract> = functions
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
            let contract = contracts
                .get(&function.name)
                .unwrap_or_else(|| panic!("test contract should exist"));
            (function.name, contract.function_effect_facts(function))
        })
        .collect();
    let fresh_return_facts = functions
        .iter()
        .map(|function| {
            let contract = contracts
                .get(&function.name)
                .unwrap_or_else(|| panic!("test contract should exist"));
            (function.name, contract.fresh_self_allocation_facts())
        })
        .collect();
    let param_disjointness = functions
        .iter()
        .map(|function| {
            let param_types = function
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect::<Vec<_>>();
            (function.name, prove_param_disjointness(&param_types, &pool))
        })
        .collect();
    let type_registry = TypeRegistry::new();
    let frozen_closure_adapters =
        ori_arc::freeze_closure_adapter_plans(&functions, &contracts, &pool, &type_registry)
            .unwrap_or_else(|errors| panic!("closure adapter facts should freeze: {errors:?}"));

    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families,
        contracts: contracts.into_iter().collect(),
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts: frozen_closure_adapters.callable_facts,
        closure_adapters: frozen_closure_adapters.adapters,
        retain_plans: frozen_closure_adapters.retain_plans,
        roots: vec![root],
        cli_entry,
        externals,
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry,
    })
    .unwrap_or_else(|error| panic!("test executable should validate: {error}"))
}

fn minimal_unit_function(name: Name, body: Vec<ArcInstr>) -> ArcFunction {
    ArcFunction {
        name,
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT],
        ..ArcFunction::default()
    }
}

fn list_concat_executable() -> ExecutableProgram {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut pool = Pool::new();
    let list_type = pool.list(Idx::INT);
    let function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: list_type,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: list_type,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: list_type,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: list_type,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![list_type; 3],
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
    };
    let mut functions = vec![function];
    let classifier = ArcClassifier::new(&pool);
    let builtins = BuiltinOwnershipSets::new(&symbols);
    let type_registry = TypeRegistry::new();
    let external_contracts: std::collections::HashMap<ori_ir::Name, ori_arc::MemoryContract> =
        std::collections::HashMap::default();
    let callable_boundaries = ori_arc::CallableBoundaryFacts::default();
    let realization = realize_closed_program(
        &mut functions,
        &ori_arc::ArcPipelineContext {
            classifier: &classifier,
            interner: &symbols,
            pool: &pool,
            builtins: &builtins,
            type_registry: &type_registry,
            callable_boundaries: &callable_boundaries,
            verify_arc: true,
            external_contracts: &external_contracts,
        },
    )
    .unwrap_or_else(|errors| panic!("list concat realization should succeed: {errors:?}"));

    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
        contracts: realization.contracts,
        function_effects: realization.function_effects,
        fresh_return_facts: realization.fresh_return_facts,
        param_disjointness: realization.param_disjointness,
        callable_facts: realization.callable_facts,
        closure_adapters: realization.closure_adapters,
        retain_plans: realization.retain_plans,
        roots: vec![main],
        cli_entry: Some(main),
        externals: Vec::new(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry,
    })
    .unwrap_or_else(|error| panic!("list concat executable should validate: {error}"))
}

struct RcValueFixture {
    body: Vec<ArcInstr>,
    rc_var: ArcVarId,
    var_types: Vec<Idx>,
}

fn rc_value_fixture(
    strategy: RcStrategy,
    pool: &mut Pool,
    message: Name,
    iter: Name,
    option_name: Name,
) -> RcValueFixture {
    match strategy {
        RcStrategy::FatPointer => fat_pointer_fixture(message),
        RcStrategy::AggregateFields => aggregate_fixture(pool, message),
        RcStrategy::InlineEnum => inline_enum_fixture(pool, message, option_name),
        RcStrategy::Iterator => iterator_fixture(pool, iter),
        RcStrategy::HeapPointer | RcStrategy::Closure | RcStrategy::UserDrop => {
            heap_pointer_fixture(pool)
        }
    }
}

fn base_body() -> Vec<ArcInstr> {
    vec![ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(7)),
    }]
}

fn heap_pointer_fixture(pool: &mut Pool) -> RcValueFixture {
    let list_type = pool.list(Idx::INT);
    let mut body = base_body();
    body.push(ArcInstr::Construct {
        dst: ArcVarId::new(1),
        ty: list_type,
        ctor: CtorKind::ListLiteral,
        args: vec![ArcVarId::new(0)],
    });
    RcValueFixture {
        body,
        rc_var: ArcVarId::new(1),
        var_types: vec![Idx::INT, list_type],
    }
}

fn fat_pointer_fixture(message: Name) -> RcValueFixture {
    let mut body = base_body();
    body.push(ArcInstr::Let {
        dst: ArcVarId::new(1),
        ty: Idx::STR,
        value: ArcValue::Literal(LitValue::String(message)),
    });
    RcValueFixture {
        body,
        rc_var: ArcVarId::new(1),
        var_types: vec![Idx::INT, Idx::STR],
    }
}

fn aggregate_fixture(pool: &mut Pool, message: Name) -> RcValueFixture {
    let tuple_type = pool.tuple(&[Idx::STR]);
    let mut fixture = fat_pointer_fixture(message);
    fixture.body.push(ArcInstr::Construct {
        dst: ArcVarId::new(2),
        ty: tuple_type,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(1)],
    });
    fixture.rc_var = ArcVarId::new(2);
    fixture.var_types.push(tuple_type);
    fixture
}

fn inline_enum_fixture(pool: &mut Pool, message: Name, option_name: Name) -> RcValueFixture {
    let option_type = pool.option(Idx::STR);
    let mut fixture = fat_pointer_fixture(message);
    fixture.body.push(ArcInstr::Construct {
        dst: ArcVarId::new(2),
        ty: option_type,
        ctor: CtorKind::EnumVariant {
            enum_name: option_name,
            variant: 1,
        },
        args: vec![ArcVarId::new(1)],
    });
    fixture.rc_var = ArcVarId::new(2);
    fixture.var_types.push(option_type);
    fixture
}

fn iterator_fixture(pool: &mut Pool, iter: Name) -> RcValueFixture {
    let range_type = pool.range(Idx::INT);
    let iterator_type = pool.iterator(Idx::INT);
    let mut body = base_body();
    body.extend([
        ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(9)),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: range_type,
            ctor: CtorKind::Tuple,
            args: vec![ArcVarId::new(0), ArcVarId::new(1)],
        },
        ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: iterator_type,
            func: iter,
            args: vec![ArcVarId::new(2)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
    ]);
    RcValueFixture {
        body,
        rc_var: ArcVarId::new(3),
        var_types: vec![Idx::INT, Idx::INT, range_type, iterator_type],
    }
}

fn must_verify(bytecode: BytecodeProgram) {
    if let Err(error) = verify(bytecode) {
        panic!("test bytecode should verify: {error}");
    }
}

fn must_fail<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("operation should fail"),
        Err(error) => error,
    }
}
