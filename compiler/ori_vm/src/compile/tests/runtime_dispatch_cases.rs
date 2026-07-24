use super::*;

#[test]
fn bytecode_compilation_admits_only_catch_recover_compiler_operation() {
    let bytecode = compile(&catch_recover_executable())
        .unwrap_or_else(|error| panic!("catch recovery should compile for the VM: {error}"));
    assert!(bytecode.functions[0].ops.iter().any(|operation| matches!(
        operation,
        Op::Call {
            callee: CallableTarget::Runtime(RuntimeCall::Compiler(CompilerOperation::CatchRecover)),
            ..
        }
    )));

    let executable = unsupported_compiler_operation_executable();
    let function = executable.functions()[0].name;
    assert!(matches!(
        compile(&executable),
        Err(CompileError::UnsupportedRuntimeCall {
            function: found,
            call: RuntimeCall::Compiler(CompilerOperation::InjectTrace),
        }) if found == function
    ));
}

#[test]
fn verifier_rejects_catch_recover_with_an_argument() {
    let mut bytecode = compile(&catch_recover_executable())
        .unwrap_or_else(|error| panic!("catch recovery should compile for the VM: {error}"));
    let (function, pc, arguments, target) = bytecode.functions[0]
        .ops
        .iter()
        .enumerate()
        .find_map(|(pc, operation)| match operation {
            Op::Call {
                callee:
                    target @ CallableTarget::Runtime(RuntimeCall::Compiler(
                        CompilerOperation::CatchRecover,
                    )),
                args,
                ..
            } => Some((bytecode.functions[0].name, pc, *args, *target)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("catch recovery bytecode should contain its runtime call"));
    bytecode.call_arguments[arguments.index()] = vec![CallArgument::new(
        Register::from_arc(ArcVarId::new(0)),
        ArgOwnership::Borrowed,
    )]
    .into_boxed_slice();

    assert!(matches!(
        must_fail(verify(bytecode)),
        VerifyError::CallArity {
            function: found,
            pc: found_pc,
            target: found_target,
            expected: 0,
            actual: 1,
        } if found == function && found_pc == pc && found_target == target
    ));
}

#[test]
fn catch_recover_without_pending_panic_fails_through_verified_dispatch() {
    let bytecode = compile(&catch_recover_executable())
        .unwrap_or_else(|error| panic!("catch recovery should compile for the VM: {error}"));
    let verified = verify(bytecode)
        .unwrap_or_else(|error| panic!("catch recovery bytecode should verify: {error}"));

    let report = execute_report(&verified, ExecutionConfig::default());

    assert!(matches!(
        report.result,
        Err(crate::ExecutionError::CatchRecoverWithoutPanic)
    ));
    assert_eq!(report.metrics.exit_live_heap_objects, 0);
    assert_eq!(report.metrics.exit_value_arena_entries, 0);
}
