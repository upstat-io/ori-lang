use super::*;

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
