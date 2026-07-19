use ori_ir::{BinaryOp, DerivedTrait, Name, StringInterner};
use ori_types::{EnumVariant, FunctionSig, Idx, Pool};

use crate::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue, MethodCallFact, MethodCallForm,
    Ownership, PrimOp, ValueRepr, VariableMetadataState,
};

use super::{build_derived_format, DerivedFormatBodyError};

const EXECUTABLE: Name = Name::from_raw(1);
const SIGNATURE_NAME: Name = Name::from_raw(2);
const SELF_NAME: Name = Name::from_raw(3);

fn signature(receiver: Idx, return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(SIGNATURE_NAME, vec![SELF_NAME], vec![receiver], return_type)
}

fn body_or_panic(
    trait_kind: DerivedTrait,
    owner_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
) -> ArcFunction {
    match build_derived_format(
        trait_kind,
        EXECUTABLE,
        owner_name,
        method_name,
        signature,
        interner,
        pool,
    ) {
        Ok(body) => body,
        Err(error) => panic!("concrete derived format must produce a shared body: {error:?}"),
    }
}

fn calls(function: &ArcFunction) -> Vec<(crate::ArcVarId, Name, Vec<crate::ArcVarId>)> {
    let mut calls = Vec::new();
    for block in &function.blocks {
        calls.extend(
            block
                .body
                .iter()
                .filter_map(|instruction| match instruction {
                    ArcInstr::Apply {
                        dst, func, args, ..
                    } => Some((*dst, *func, args.clone())),
                    _ => None,
                }),
        );
        if let ArcTerminator::Invoke {
            dst, func, args, ..
        } = &block.terminator
        {
            calls.push((*dst, *func, args.clone()));
        }
    }
    calls
}

fn string_literals(function: &ArcFunction, interner: &StringInterner) -> Vec<String> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instruction| match instruction {
            ArcInstr::Let {
                ty: Idx::STR,
                value: ArcValue::Literal(LitValue::String(name)),
                ..
            } => Some(interner.lookup(*name).to_owned()),
            _ => None,
        })
        .collect()
}

fn concat_count(function: &ArcFunction) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter(|instruction| {
            matches!(
                instruction,
                ArcInstr::Let {
                    ty: Idx::STR,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Add),
                        args,
                    },
                    ..
                } if args.len() == 2
            )
        })
        .count()
}

fn assert_semantic_only(function: &ArcFunction) {
    assert!(function
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .all(|instruction| matches!(
            instruction,
            ArcInstr::Let { .. } | ArcInstr::Project { .. } | ArcInstr::Apply { .. }
        )));
}

#[test]
fn printable_struct_uses_semantic_owner_method_and_exact_field_facts() {
    let interner = StringInterner::new();
    let owner = interner.intern("Pair");
    let method = interner.intern("to_str");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        owner,
        &[
            (interner.intern("left"), Idx::INT),
            (interner.intern("right"), Idx::STR),
        ],
    );

    let body = body_or_panic(
        DerivedTrait::Printable,
        owner,
        method,
        &signature(receiver, Idx::STR),
        &interner,
        &pool,
    );

    assert_eq!(body.name, EXECUTABLE);
    assert_eq!(body.params.len(), 1);
    assert_eq!(body.params[0].ty, receiver);
    assert_eq!(body.params[0].ownership, Ownership::Owned);
    assert_eq!(body.return_type, Idx::STR);
    assert_eq!(
        body.var_metadata_state,
        VariableMetadataState::RepresentationsReady
    );
    assert_eq!(body.var_reprs.len(), body.var_types.len());
    assert_eq!(body.var_reprs[0], ValueRepr::Aggregate);

    let calls = calls(&body);
    assert_eq!(calls.len(), 2);
    assert!(calls
        .iter()
        .all(|(_, function, args)| { *function == method && args.len() == 1 }));
    assert_eq!(
        body.method_call_facts,
        vec![
            MethodCallFact {
                destination: calls[0].0,
                receiver_type: Idx::INT,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
            MethodCallFact {
                destination: calls[1].0,
                receiver_type: Idx::STR,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
        ]
    );
    assert_eq!(string_literals(&body, &interner), ["Pair(", ", ", ")"]);
    assert_eq!(concat_count(&body), 4);
    assert_semantic_only(&body);
}

#[test]
fn debug_struct_includes_field_labels_and_debug_method_calls() {
    let interner = StringInterner::new();
    let owner = interner.intern("Point");
    let method = interner.intern("debug");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        owner,
        &[
            (interner.intern("x"), Idx::INT),
            (interner.intern("label"), Idx::STR),
        ],
    );

    let body = body_or_panic(
        DerivedTrait::Debug,
        owner,
        method,
        &signature(receiver, Idx::STR),
        &interner,
        &pool,
    );

    let calls = calls(&body);
    assert_eq!(calls.len(), 2);
    assert!(calls
        .iter()
        .all(|(_, function, args)| *function == method && args.len() == 1));
    assert_eq!(body.method_call_facts[0].receiver_type, Idx::INT);
    assert_eq!(body.method_call_facts[1].receiver_type, Idx::STR);
    assert_eq!(
        string_literals(&body, &interner),
        ["Point { ", "x: ", ", ", "label: ", " }"]
    );
    assert_eq!(concat_count(&body), 6);
    assert_eq!(
        body.blocks
            .iter()
            .filter(|block| matches!(block.terminator, ArcTerminator::Resume))
            .count(),
        0
    );
    assert_semantic_only(&body);
}

#[test]
fn debug_enum_switches_variants_and_formats_payloads_without_struct_labels() {
    let interner = StringInterner::new();
    let owner = interner.intern("MaybePair");
    let method = interner.intern("debug");
    let mut pool = Pool::new();
    let receiver = pool.enum_type(
        owner,
        &[
            EnumVariant {
                name: interner.intern("None"),
                field_types: Vec::new(),
            },
            EnumVariant {
                name: interner.intern("Pair"),
                field_types: vec![Idx::STR, Idx::INT],
            },
        ],
    );

    let body = body_or_panic(
        DerivedTrait::Debug,
        owner,
        method,
        &signature(receiver, Idx::STR),
        &interner,
        &pool,
    );

    let switch = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => Some((*scrutinee, cases.clone(), *default)),
            _ => None,
        });
    let Some((_tag, cases, default)) = switch else {
        panic!("derived enum formatting must switch on the active variant");
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].0, 0);
    assert_eq!(cases[1].0, 1);
    assert!(matches!(
        body.blocks[default.index()].terminator,
        ArcTerminator::Unreachable
    ));

    let projections: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instruction| match instruction {
            ArcInstr::Project { ty, field, .. } => Some((*ty, *field)),
            _ => None,
        })
        .collect();
    assert_eq!(projections, [(Idx::INT, 0), (Idx::STR, 1), (Idx::INT, 2)]);
    assert_eq!(
        string_literals(&body, &interner),
        ["None", "Pair(", ", ", ")"]
    );
    let calls = calls(&body);
    assert_eq!(calls.len(), 2);
    assert_eq!(
        body.method_call_facts,
        vec![
            MethodCallFact {
                destination: calls[0].0,
                receiver_type: Idx::STR,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
            MethodCallFact {
                destination: calls[1].0,
                receiver_type: Idx::INT,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
        ]
    );
    assert_eq!(concat_count(&body), 4);
    assert_semantic_only(&body);
}

#[test]
fn unsupported_trait_is_rejected_before_body_generation() {
    let interner = StringInterner::new();
    let owner = interner.intern("Thing");
    let method = interner.intern("eq");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(owner, &[]);

    assert!(matches!(
        build_derived_format(
            DerivedTrait::Eq,
            EXECUTABLE,
            owner,
            method,
            &signature(receiver, Idx::STR),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::UnsupportedTrait {
            trait_kind: DerivedTrait::Eq,
        })
    ));
}

#[test]
fn semantic_method_name_must_match_requested_trait() {
    let interner = StringInterner::new();
    let owner = interner.intern("Thing");
    let debug = interner.intern("debug");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(owner, &[]);

    assert_eq!(
        build_derived_format(
            DerivedTrait::Printable,
            EXECUTABLE,
            owner,
            debug,
            &signature(receiver, Idx::STR),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::MethodNameMismatch {
            method_name: debug,
            expected: "to_str",
        })
    );
}

#[test]
fn unknown_owner_name_fails_closed_without_interner_lookup_panic() {
    let interner = StringInterner::new();
    let declared_owner = interner.intern("Thing");
    let unknown_owner = Name::from_raw(u32::MAX);
    let method = interner.intern("debug");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(declared_owner, &[]);

    assert_eq!(
        build_derived_format(
            DerivedTrait::Debug,
            EXECUTABLE,
            unknown_owner,
            method,
            &signature(receiver, Idx::STR),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::UnknownName {
            role: "owner",
            name: unknown_owner,
        })
    );
}

#[test]
fn invalid_signature_shapes_fail_closed() {
    let interner = StringInterner::new();
    let owner = interner.intern("Thing");
    let method = interner.intern("to_str");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(owner, &[]);
    let mut generic = signature(receiver, Idx::STR);
    generic.type_params.push(interner.intern("T"));
    let missing_receiver = FunctionSig::synthetic(method, Vec::new(), Vec::new(), Idx::STR);

    assert!(matches!(
        build_derived_format(
            DerivedTrait::Printable,
            EXECUTABLE,
            owner,
            method,
            &generic,
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::GenericSignature {
            type_parameters: 1,
            const_parameters: 0,
        })
    ));
    assert!(matches!(
        build_derived_format(
            DerivedTrait::Printable,
            EXECUTABLE,
            owner,
            method,
            &missing_receiver,
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::InvalidParameterShape {
            parameter_names: 0,
            parameter_types: 0,
        })
    ));
    assert_eq!(
        build_derived_format(
            DerivedTrait::Printable,
            EXECUTABLE,
            owner,
            method,
            &signature(receiver, Idx::BOOL),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::ReturnTypeMismatch {
            return_type: Idx::BOOL,
        })
    );
}

#[test]
fn non_nominal_and_unresolved_receivers_fail_closed() {
    let interner = StringInterner::new();
    let owner = interner.intern("Thing");
    let method = interner.intern("debug");
    let mut pool = Pool::new();
    let unresolved = pool.fresh_var();

    assert!(matches!(
        build_derived_format(
            DerivedTrait::Debug,
            EXECUTABLE,
            owner,
            method,
            &signature(Idx::INT, Idx::STR),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::UnsupportedReceiverType {
            receiver_type: Idx::INT,
            ..
        })
    ));
    assert!(matches!(
        build_derived_format(
            DerivedTrait::Debug,
            EXECUTABLE,
            owner,
            method,
            &signature(unresolved, Idx::STR),
            &interner,
            &pool,
        ),
        Err(DerivedFormatBodyError::NonConcreteType {
            position: "self parameter",
            ty,
        }) if ty == unresolved
    ));
}
