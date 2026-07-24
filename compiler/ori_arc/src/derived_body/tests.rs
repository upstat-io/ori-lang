use ori_ir::{BinaryOp, Name, StringInterner};
use ori_types::{ConstParamInfo, FunctionSig, Idx, Pool};

use crate::{
    build_derived_clone_identity, build_derived_default, build_derived_eq, ArcInstr, ArcTerminator,
    ArcValue, ArcVarId, CtorKind, DerivedCloneBodyError, DerivedDefaultBodyError,
    DerivedEqBodyError, LitValue, MethodCallForm, Ownership, PrimOp, ValueRepr,
    VariableMetadataState,
};

const EXECUTABLE: Name = Name::from_raw(1);
const METHOD: Name = Name::from_raw(2);
const SELF_NAME: Name = Name::from_raw(3);
const OTHER_NAME: Name = Name::from_raw(4);
const EQ_SIGNATURE_NAME: Name = Name::from_raw(5);

#[test]
fn eq_index_overflow_preserves_conversion_source() {
    let Err(source) = u32::try_from(u64::MAX) else {
        panic!("u64::MAX must exceed the u32 index carrier");
    };
    let error = DerivedEqBodyError::IndexOverflow {
        receiver_type: Idx::INT,
        index_kind: "field",
        index: 0,
        source: Some(source),
    };

    assert!(std::error::Error::source(&error).is_some());
}

fn signature(self_type: Idx, return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![self_type], return_type)
}

fn eq_signature(self_type: Idx, other_type: Idx, return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(
        EQ_SIGNATURE_NAME,
        vec![SELF_NAME, OTHER_NAME],
        vec![self_type, other_type],
        return_type,
    )
}

fn default_signature(return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(METHOD, Vec::new(), Vec::new(), return_type)
}

fn clone_body_or_panic(
    signature: &FunctionSig,
    pool: &Pool,
    expectation: &str,
) -> crate::ArcFunction {
    match build_derived_clone_identity(EXECUTABLE, signature, pool) {
        Ok(body) => body,
        Err(error) => panic!("{expectation}: {error}"),
    }
}

fn eq_body_or_panic(
    owner: Name,
    signature: &FunctionSig,
    pool: &Pool,
    expectation: &str,
) -> crate::ArcFunction {
    match build_derived_eq(EXECUTABLE, owner, METHOD, signature, pool) {
        Ok(body) => body,
        Err(error) => panic!("{expectation}: {error}"),
    }
}

fn default_body_or_panic(
    owner: Name,
    method: Name,
    signature: &FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
    expectation: &str,
) -> crate::ArcFunction {
    match build_derived_default(EXECUTABLE, owner, method, signature, interner, pool) {
        Ok(body) => body,
        Err(error) => panic!("{expectation}: {error}"),
    }
}

#[test]
fn scalar_identity_is_representation_ready_and_instruction_free() {
    let pool = Pool::new();
    let body = clone_body_or_panic(
        &signature(Idx::INT, Idx::INT),
        &pool,
        "concrete identity signature must produce a derived Clone body",
    );

    assert_eq!(body.name, EXECUTABLE);
    assert_eq!(body.params.len(), 1);
    assert_eq!(body.params[0].var, ArcVarId::new(0));
    assert_eq!(body.params[0].ty, Idx::INT);
    assert_eq!(body.params[0].ownership, Ownership::Owned);
    assert_eq!(body.return_type, Idx::INT);
    assert_eq!(body.var_types, vec![Idx::INT]);
    assert_eq!(body.var_reprs, vec![ValueRepr::Scalar]);
    assert_eq!(
        body.var_metadata_state,
        VariableMetadataState::RepresentationsReady
    );
    assert_eq!(body.blocks.len(), 1);
    assert!(body.blocks[0].body.is_empty());
    assert_eq!(
        body.blocks[0].terminator,
        ArcTerminator::Return {
            value: ArcVarId::new(0)
        }
    );
}

#[test]
fn managed_identity_has_shape_metadata_but_no_ownership_instructions() {
    let pool = Pool::new();
    let body = clone_body_or_panic(
        &signature(Idx::STR, Idx::STR),
        &pool,
        "concrete managed identity signature must produce a derived Clone body",
    );

    assert_eq!(body.var_reprs, vec![ValueRepr::FatValue]);
    assert!(body.blocks[0].body.is_empty());
    assert_eq!(
        body.blocks[0].terminator,
        ArcTerminator::Return {
            value: ArcVarId::new(0)
        }
    );
}

#[test]
fn structurally_equal_alias_and_concrete_return_are_accepted() {
    let mut pool = Pool::new();
    let type_name = Name::from_raw(10);
    let field_name = Name::from_raw(11);
    let named = pool.named(type_name);
    let concrete = pool.struct_type(type_name, &[(field_name, Idx::STR)]);
    pool.set_resolution(named, concrete);

    assert_ne!(named, concrete);
    let body = clone_body_or_panic(
        &signature(named, concrete),
        &pool,
        "structurally equal alias and concrete return must produce a derived Clone body",
    );

    assert_eq!(body.params[0].ty, named);
    assert_eq!(body.return_type, concrete);
    assert_eq!(body.var_reprs, vec![ValueRepr::Aggregate]);
    assert!(body.blocks[0].body.is_empty());
}

#[test]
fn structurally_different_return_is_rejected() {
    let pool = Pool::new();
    let Err(error) =
        build_derived_clone_identity(EXECUTABLE, &signature(Idx::INT, Idx::STR), &pool)
    else {
        panic!("derived Clone accepted a return type different from Self")
    };

    assert_eq!(
        error,
        DerivedCloneBodyError::ReturnTypeMismatch {
            self_type: Idx::INT,
            return_type: Idx::STR,
        }
    );
}

#[test]
fn missing_or_extra_receiver_is_rejected() {
    let pool = Pool::new();
    let missing = FunctionSig::synthetic(METHOD, Vec::new(), Vec::new(), Idx::INT);
    let extra = FunctionSig::synthetic(
        METHOD,
        vec![SELF_NAME, Name::from_raw(4)],
        vec![Idx::INT, Idx::INT],
        Idx::INT,
    );

    assert!(matches!(
        build_derived_clone_identity(EXECUTABLE, &missing, &pool),
        Err(DerivedCloneBodyError::InvalidSelfParameterShape {
            parameter_names: 0,
            parameter_types: 0,
        })
    ));
    assert!(matches!(
        build_derived_clone_identity(EXECUTABLE, &extra, &pool),
        Err(DerivedCloneBodyError::InvalidSelfParameterShape {
            parameter_names: 2,
            parameter_types: 2,
        })
    ));
}

#[test]
fn generic_and_unresolved_signatures_are_rejected() {
    let mut pool = Pool::new();
    let mut generic = signature(Idx::INT, Idx::INT);
    generic.type_params.push(Name::from_raw(5));
    assert!(matches!(
        build_derived_clone_identity(EXECUTABLE, &generic, &pool),
        Err(DerivedCloneBodyError::GenericSignature {
            type_parameters: 1,
            const_parameters: 0,
        })
    ));

    let unresolved = pool.fresh_var();
    assert!(matches!(
        build_derived_clone_identity(
            EXECUTABLE,
            &signature(unresolved, unresolved),
            &pool
        ),
        Err(DerivedCloneBodyError::NonConcreteType {
            position: "self parameter",
            ty,
        }) if ty == unresolved
    ));
}

#[test]
fn foreign_pool_index_is_rejected_without_indexing_the_pool() {
    let pool = Pool::new();
    let invalid = Idx::from_raw(u32::MAX);
    assert!(matches!(
        build_derived_clone_identity(EXECUTABLE, &signature(invalid, invalid), &pool),
        Err(DerivedCloneBodyError::InvalidTypeIndex {
            position: "self parameter",
            ty,
        }) if ty == invalid
    ));
}

#[test]
fn scalar_struct_eq_is_fieldwise_short_circuit_arc() {
    let mut pool = Pool::new();
    let struct_type = pool.struct_type(
        Name::from_raw(20),
        &[
            (Name::from_raw(21), Idx::INT),
            (Name::from_raw(22), Idx::BOOL),
        ],
    );
    let body = eq_body_or_panic(
        Name::from_raw(20),
        &eq_signature(struct_type, struct_type, Idx::BOOL),
        &pool,
        "concrete scalar-shaped struct Eq signature must produce a derived Eq body",
    );

    assert_eq!(body.name, EXECUTABLE);
    assert_eq!(body.params.len(), 2);
    assert_eq!(body.params[0].var, ArcVarId::new(0));
    assert_eq!(body.params[0].ty, struct_type);
    assert_eq!(body.params[0].ownership, Ownership::Owned);
    assert_eq!(body.params[1].var, ArcVarId::new(1));
    assert_eq!(body.params[1].ty, struct_type);
    assert_eq!(body.params[1].ownership, Ownership::Owned);
    assert_eq!(body.return_type, Idx::BOOL);
    assert_eq!(body.var_types.len(), 10);
    assert!(body.var_reprs.iter().all(|repr| *repr == ValueRepr::Scalar));
    assert_eq!(
        body.var_metadata_state,
        VariableMetadataState::RepresentationsReady
    );
    assert_eq!(body.blocks.len(), 4);
    let instructions: Vec<_> = body.blocks.iter().flat_map(|block| &block.body).collect();
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, ArcInstr::Project { .. }))
            .count(),
        4
    );
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                ArcInstr::Let {
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Eq),
                        ..
                    },
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(!instructions.iter().any(|instruction| matches!(
        instruction,
        ArcInstr::Let {
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Eq),
                args,
            },
            ..
        } if args == &[ArcVarId::new(0), ArcVarId::new(1)]
    )));
    assert_eq!(
        body.blocks
            .iter()
            .filter(|block| matches!(block.terminator, ArcTerminator::Branch { .. }))
            .count(),
        2
    );
    assert!(body.method_call_facts.is_empty());
}

#[test]
fn struct_eq_checks_scalar_fields_before_managed_fields() {
    let mut pool = Pool::new();
    let struct_type = pool.struct_type(
        Name::from_raw(30),
        &[
            (Name::from_raw(31), Idx::STR),
            (Name::from_raw(32), Idx::INT),
        ],
    );
    let body = eq_body_or_panic(
        Name::from_raw(30),
        &eq_signature(struct_type, struct_type, Idx::BOOL),
        &pool,
        "mixed-cost struct Eq must compare the scalar field first",
    );

    let projected_fields: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instruction| match instruction {
            ArcInstr::Project { field, .. } => Some(*field),
            _ => None,
        })
        .collect();
    assert_eq!(projected_fields, vec![1, 1, 0, 0]);
}

#[test]
fn nested_user_field_is_an_exact_rewriteable_invoke() {
    let mut pool = Pool::new();
    let inner = pool.struct_type(Name::from_raw(50), &[(Name::from_raw(51), Idx::INT)]);
    let outer = pool.struct_type(Name::from_raw(52), &[(Name::from_raw(53), inner)]);
    let body = eq_body_or_panic(
        Name::from_raw(52),
        &eq_signature(outer, outer, Idx::BOOL),
        &pool,
        "nested concrete struct Eq signature must produce a derived Eq body",
    );

    let invoke = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            ArcTerminator::Invoke {
                dst, func, args, ..
            } => Some((*dst, *func, args.as_slice())),
            _ => None,
        });
    let Some((destination, function, arguments)) = invoke else {
        panic!("user-defined field equality must preserve an unwind edge");
    };
    assert_eq!(function, METHOD);
    assert_eq!(arguments.len(), 2);
    assert_eq!(
        body.method_call_facts,
        vec![crate::MethodCallFact {
            destination,
            receiver_type: inner,
            form: MethodCallForm::Instance,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }]
    );
    assert!(body
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, ArcTerminator::Resume)));
}

#[test]
fn newtype_eq_with_primitive_underlying_uses_typed_equality() {
    let owner = Name::from_raw(60);
    let mut pool = Pool::new();
    let receiver_type = pool.named(owner);
    pool.register_newtype_ctor(owner, Idx::STR);
    pool.set_resolution(receiver_type, Idx::STR);

    let body = eq_body_or_panic(
        owner,
        &eq_signature(receiver_type, receiver_type, Idx::BOOL),
        &pool,
        "newtype Eq must delegate to the underlying equality target",
    );
    let Some((destination, arguments)) = body.blocks.iter().find_map(|block| {
        block.body.iter().find_map(|instruction| match instruction {
            ArcInstr::Let {
                dst,
                ty: Idx::BOOL,
                value:
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Eq),
                        args,
                    },
            } => Some((*dst, args.as_slice())),
            _ => None,
        })
    }) else {
        panic!("primitive newtype Eq must use typed primitive equality")
    };

    assert_eq!(arguments.len(), 2);
    assert!(body.method_call_facts.is_empty());
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| &block.body)
            .filter(|instruction| matches!(instruction, ArcInstr::Let { .. }))
            .count(),
        3
    );
    assert!(body
        .blocks
        .iter()
        .all(|block| !matches!(block.terminator, ArcTerminator::Invoke { .. })));
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Return { value } if value == destination
    )));
}

#[test]
fn newtype_eq_with_user_underlying_keeps_exact_method_dispatch() {
    let owner = Name::from_raw(61);
    let mut pool = Pool::new();
    let receiver_type = pool.named(owner);
    let underlying = pool.struct_type(Name::from_raw(62), &[(Name::from_raw(63), Idx::INT)]);
    pool.register_newtype_ctor(owner, underlying);
    pool.set_resolution(receiver_type, underlying);

    let body = eq_body_or_panic(
        owner,
        &eq_signature(receiver_type, receiver_type, Idx::BOOL),
        &pool,
        "newtype Eq must delegate to the user-defined underlying target",
    );
    let Some((destination, function, arguments)) = body.blocks.iter().find_map(|block| {
        let ArcTerminator::Invoke {
            dst, func, args, ..
        } = &block.terminator
        else {
            return None;
        };
        Some((*dst, *func, args.as_slice()))
    }) else {
        panic!("user-defined underlying Eq must preserve its unwind edge")
    };

    assert_eq!(function, METHOD);
    assert_eq!(arguments.len(), 2);
    assert_eq!(
        body.method_call_facts,
        vec![crate::MethodCallFact {
            destination,
            receiver_type: underlying,
            form: MethodCallForm::Instance,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }]
    );
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| &block.body)
            .filter(|instruction| matches!(
                instruction,
                ArcInstr::Let {
                    ty,
                    value: ArcValue::Var(_),
                    ..
                } if *ty == underlying
            ))
            .count(),
        2
    );
    assert!(!body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .any(|instruction| matches!(instruction, ArcInstr::Project { .. })));
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Return { value } if value == destination
    )));
}

#[test]
fn generic_eq_signature_is_rejected() {
    let pool = Pool::new();
    let mut generic = eq_signature(Idx::INT, Idx::INT, Idx::BOOL);
    generic.type_params.push(Name::from_raw(30));
    generic.const_params.push(ConstParamInfo {
        name: Name::from_raw(31),
        const_type: Idx::INT,
        default_value: None,
    });

    assert_eq!(
        build_derived_eq(EXECUTABLE, METHOD, METHOD, &generic, &pool),
        Err(DerivedEqBodyError::GenericSignature {
            type_parameters: 1,
            const_parameters: 1,
        })
    );
}

#[test]
fn wrong_eq_arity_is_rejected() {
    let pool = Pool::new();
    let missing = FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![Idx::INT], Idx::BOOL);
    let extra = FunctionSig::synthetic(
        METHOD,
        vec![SELF_NAME, OTHER_NAME, Name::from_raw(40)],
        vec![Idx::INT, Idx::INT, Idx::INT],
        Idx::BOOL,
    );

    assert_eq!(
        build_derived_eq(EXECUTABLE, METHOD, METHOD, &missing, &pool),
        Err(DerivedEqBodyError::InvalidParameterShape {
            parameter_names: 1,
            parameter_types: 1,
        })
    );
    assert_eq!(
        build_derived_eq(EXECUTABLE, METHOD, METHOD, &extra, &pool),
        Err(DerivedEqBodyError::InvalidParameterShape {
            parameter_names: 3,
            parameter_types: 3,
        })
    );
}

#[test]
fn mismatched_eq_receiver_types_are_rejected() {
    let pool = Pool::new();

    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            METHOD,
            METHOD,
            &eq_signature(Idx::INT, Idx::STR, Idx::BOOL),
            &pool,
        ),
        Err(DerivedEqBodyError::ReceiverTypeMismatch {
            self_type: Idx::INT,
            other_type: Idx::STR,
        })
    );
}

#[test]
fn distinct_newtypes_with_one_representation_are_rejected_as_eq_receivers() {
    let left_name = Name::from_raw(70);
    let right_name = Name::from_raw(71);
    let mut pool = Pool::new();
    let left = pool.named(left_name);
    let right = pool.named(right_name);
    pool.register_newtype_ctor(left_name, Idx::STR);
    pool.register_newtype_ctor(right_name, Idx::STR);
    pool.set_resolution(left, Idx::STR);
    pool.set_resolution(right, Idx::STR);

    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            left_name,
            METHOD,
            &eq_signature(left, right, Idx::BOOL),
            &pool,
        ),
        Err(DerivedEqBodyError::ReceiverTypeMismatch {
            self_type: left,
            other_type: right,
        })
    );
}

#[test]
fn non_bool_eq_return_is_rejected() {
    let pool = Pool::new();

    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            METHOD,
            METHOD,
            &eq_signature(Idx::INT, Idx::INT, Idx::INT),
            &pool,
        ),
        Err(DerivedEqBodyError::ReturnTypeMismatch {
            return_type: Idx::INT,
        })
    );
}

#[test]
fn unresolved_and_invalid_eq_types_are_rejected_without_pool_indexing() {
    let mut pool = Pool::new();
    let unresolved = pool.fresh_var();
    let invalid = Idx::from_raw(u32::MAX);

    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            METHOD,
            METHOD,
            &eq_signature(unresolved, unresolved, Idx::BOOL),
            &pool,
        ),
        Err(DerivedEqBodyError::NonConcreteType {
            position: "self parameter",
            ty: unresolved,
        })
    );
    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            METHOD,
            METHOD,
            &eq_signature(Idx::INT, invalid, Idx::BOOL),
            &pool,
        ),
        Err(DerivedEqBodyError::InvalidTypeIndex {
            position: "other parameter",
            ty: invalid,
        })
    );
    assert_eq!(
        build_derived_eq(
            EXECUTABLE,
            METHOD,
            METHOD,
            &eq_signature(Idx::INT, Idx::INT, invalid),
            &pool,
        ),
        Err(DerivedEqBodyError::InvalidTypeIndex {
            position: "return type",
            ty: invalid,
        })
    );
}

#[test]
fn default_struct_materializes_primitive_zeroes_and_constructs_owner() {
    let interner = StringInterner::new();
    let owner = interner.intern("Config");
    let mut pool = Pool::new();
    let config = pool.struct_type(
        owner,
        &[
            (interner.intern("name"), Idx::STR),
            (interner.intern("count"), Idx::INT),
            (interner.intern("enabled"), Idx::BOOL),
        ],
    );
    let body = default_body_or_panic(
        owner,
        METHOD,
        &default_signature(config),
        &interner,
        &pool,
        "concrete product Default must produce a shared body",
    );

    assert!(body.params.is_empty());
    assert_eq!(body.return_type, config);
    assert!(body.method_call_facts.is_empty());
    assert_eq!(body.blocks.len(), 1);
    assert!(matches!(
        &body.blocks[0].body[0],
        ArcInstr::Let {
            ty,
            value: ArcValue::Literal(LitValue::String(name)),
            ..
        } if *ty == Idx::STR && interner.lookup(*name).is_empty()
    ));
    assert!(matches!(
        &body.blocks[0].body[1],
        ArcInstr::Let {
            ty: Idx::INT,
            value: ArcValue::Literal(LitValue::Int(0)),
            ..
        }
    ));
    assert!(matches!(
        &body.blocks[0].body[2],
        ArcInstr::Let {
            ty: Idx::BOOL,
            value: ArcValue::Literal(LitValue::Bool(false)),
            ..
        }
    ));
    assert!(matches!(
        &body.blocks[0].body[3],
        ArcInstr::Construct {
            ty,
            ctor: CtorKind::Struct(name),
            args,
            ..
        } if *ty == config && *name == owner && args.len() == 3
    ));
}

#[test]
fn nested_default_is_an_exact_associated_call() {
    let interner = StringInterner::new();
    let inner_name = interner.intern("Inner");
    let outer_name = interner.intern("Outer");
    let mut pool = Pool::new();
    let inner = pool.struct_type(inner_name, &[(interner.intern("value"), Idx::INT)]);
    let outer = pool.struct_type(outer_name, &[(interner.intern("inner"), inner)]);
    let mut concrete_signature = default_signature(outer);
    concrete_signature.name = Name::from_raw(99);
    let body = default_body_or_panic(
        outer_name,
        METHOD,
        &concrete_signature,
        &interner,
        &pool,
        "nested product Default must preserve associated owner provenance",
    );

    let Some((dst, func, args)) = body.blocks.iter().find_map(|block| {
        let ArcTerminator::Invoke {
            dst, func, args, ..
        } = &block.terminator
        else {
            return None;
        };
        Some((*dst, *func, args))
    }) else {
        panic!("nested field must be produced by an associated Default call")
    };
    assert_eq!(func, METHOD);
    assert!(args.is_empty());
    assert_eq!(
        body.method_call_facts,
        vec![crate::MethodCallFact {
            destination: dst,
            receiver_type: inner,
            form: MethodCallForm::Associated,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }]
    );
}

#[test]
fn default_rejects_parameters_and_non_product_results() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let parameterized = FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![Idx::INT], Idx::INT);

    assert_eq!(
        build_derived_default(
            EXECUTABLE,
            Name::from_raw(20),
            METHOD,
            &parameterized,
            &interner,
            &pool
        ),
        Err(DerivedDefaultBodyError::InvalidParameterShape {
            parameter_names: 1,
            parameter_types: 1,
        })
    );
    assert_eq!(
        build_derived_default(
            EXECUTABLE,
            Name::from_raw(20),
            METHOD,
            &default_signature(Idx::INT),
            &interner,
            &pool,
        ),
        Err(DerivedDefaultBodyError::UnsupportedReturnType {
            return_type: Idx::INT,
            tag: ori_types::Tag::Int,
        })
    );
}
