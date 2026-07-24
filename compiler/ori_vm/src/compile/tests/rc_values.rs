use super::*;

/// Build the value shape corresponding to an RC strategy.
pub(super) fn rc_value_fixture(
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

/// Build the scalar prefix shared by RC value fixtures.
pub(super) fn base_body() -> Vec<ArcInstr> {
    vec![ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(7)),
    }]
}

/// Build a list-backed heap-pointer RC fixture.
pub(super) fn heap_pointer_fixture(pool: &mut Pool) -> RcValueFixture {
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
        method_call_facts: Vec::new(),
    }
}

/// Build a string-backed fat-pointer RC fixture.
pub(super) fn fat_pointer_fixture(message: Name) -> RcValueFixture {
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
        method_call_facts: Vec::new(),
    }
}

/// Build an aggregate fixture containing a managed string.
pub(super) fn aggregate_fixture(pool: &mut Pool, message: Name) -> RcValueFixture {
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

/// Build an inline-enum fixture containing a managed string.
pub(super) fn inline_enum_fixture(
    pool: &mut Pool,
    message: Name,
    option_name: Name,
) -> RcValueFixture {
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

/// Build an iterator RC fixture rooted in a range.
pub(super) fn iterator_fixture(pool: &mut Pool, iter: Name) -> RcValueFixture {
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
        method_call_facts: vec![ori_arc::MethodCallFact {
            destination: ArcVarId::new(3),
            receiver_type: range_type,
            form: ori_arc::MethodCallForm::Instance,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }],
    }
}

/// Verify bytecode or fail the test with the verifier error.
pub(super) fn must_verify(bytecode: BytecodeProgram) {
    if let Err(error) = verify(bytecode) {
        panic!("test bytecode should verify: {error}");
    }
}

/// Return the error from an expected failure or fail the test.
pub(super) fn must_fail<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("operation should fail"),
        Err(error) => error,
    }
}
