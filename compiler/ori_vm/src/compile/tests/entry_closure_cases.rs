use super::*;

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
    assert_eq!(report.metrics.exit_live_heap_objects, 0);
    assert_eq!(report.metrics.exit_value_arena_entries, 0);
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
