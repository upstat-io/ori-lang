use ori_ir::{builtin_constants::ordering, Name, StringInterner};
use ori_types::{ConstParamInfo, EnumVariant, FunctionSig, Idx, Pool, Tag};

use crate::ir::{ArcInstr, ArcTerminator, CtorKind, MethodCallFact, MethodCallForm};
use crate::{Ownership, VariableMetadataState};

use super::{build_derived_compare, DerivedCompareBodyError};

const EXECUTABLE: Name = Name::from_raw(1);
const METHOD: Name = Name::from_raw(2);
const SELF_NAME: Name = Name::from_raw(3);
const OTHER_NAME: Name = Name::from_raw(4);
const SIGNATURE_NAME: Name = Name::from_raw(5);

fn signature(receiver: Idx, other: Idx, return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(
        SIGNATURE_NAME,
        vec![SELF_NAME, OTHER_NAME],
        vec![receiver, other],
        return_type,
    )
}

fn body_or_panic(ordering_name: Name, signature: &FunctionSig, pool: &Pool) -> crate::ArcFunction {
    match build_derived_compare(EXECUTABLE, ordering_name, METHOD, signature, pool) {
        Ok(body) => body,
        Err(error) => panic!("concrete Comparable derive must produce a shared body: {error}"),
    }
}

fn invokes(body: &crate::ArcFunction) -> Vec<(crate::ArcVarId, Name)> {
    body.blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            ArcTerminator::Invoke { dst, func, .. } => Some((*dst, *func)),
            _ => None,
        })
        .collect()
}

#[test]
fn struct_fields_compare_lexicographically_with_exact_provenance() {
    let interner = StringInterner::new();
    let ordering_name = interner.intern("Ordering");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        interner.intern("Pair"),
        &[
            (interner.intern("first"), Idx::INT),
            (interner.intern("second"), Idx::STR),
        ],
    );
    let body = body_or_panic(
        ordering_name,
        &signature(receiver, receiver, Idx::ORDERING),
        &pool,
    );

    assert_eq!(body.params.len(), 2);
    assert!(body
        .params
        .iter()
        .all(|parameter| { parameter.ty == receiver && parameter.ownership == Ownership::Owned }));
    assert_eq!(body.return_type, Idx::ORDERING);
    assert_eq!(
        body.var_metadata_state,
        VariableMetadataState::RepresentationsReady
    );

    let calls = invokes(&body);
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|(_, function)| *function == METHOD));
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
    assert_eq!(
        body.blocks
            .iter()
            .filter(|block| matches!(block.terminator, ArcTerminator::Switch { .. }))
            .count(),
        2
    );
    assert!(body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .all(|instruction| matches!(
            instruction,
            ArcInstr::Project { .. } | ArcInstr::Construct { .. }
        )));
}

#[test]
fn enum_compares_declaration_ordinal_before_active_payload() {
    let interner = StringInterner::new();
    let ordering_name = interner.intern("Ordering");
    let mut pool = Pool::new();
    let receiver = pool.enum_type(
        interner.intern("Choice"),
        &[
            EnumVariant {
                name: interner.intern("Empty"),
                field_types: Vec::new(),
            },
            EnumVariant {
                name: interner.intern("Pair"),
                field_types: vec![Idx::INT, Idx::BOOL],
            },
        ],
    );
    let body = body_or_panic(
        ordering_name,
        &signature(receiver, receiver, Idx::ORDERING),
        &pool,
    );

    let calls = invokes(&body);
    assert_eq!(calls.len(), 3);
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
                receiver_type: Idx::INT,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
            MethodCallFact {
                destination: calls[2].0,
                receiver_type: Idx::BOOL,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            },
        ]
    );

    let variant_switch = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            ArcTerminator::Switch { cases, default, .. }
                if cases.len() == 2 && cases[0].0 == 0 && cases[1].0 == 1 =>
            {
                Some((cases, *default))
            }
            _ => None,
        });
    let Some((cases, default)) = variant_switch else {
        panic!("enum Comparable body must dispatch in declaration order")
    };
    assert!(matches!(
        body.blocks[default.index()].terminator,
        ArcTerminator::Unreachable
    ));
    assert!(matches!(
        body.blocks[cases[0].1.index()].terminator,
        ArcTerminator::Jump { .. }
    ));
    assert!(body.blocks[cases[1].1.index()]
        .body
        .iter()
        .any(|instruction| matches!(instruction, ArcInstr::Project { field: 1, .. })));
}

#[test]
fn equal_result_is_an_ordinary_ordering_variant_construct() {
    let interner = StringInterner::new();
    let ordering_name = interner.intern("Ordering");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(interner.intern("UnitLike"), &[]);
    let body = body_or_panic(
        ordering_name,
        &signature(receiver, receiver, Idx::ORDERING),
        &pool,
    );

    assert!(body.method_call_facts.is_empty());
    let construct =
        body.blocks.iter().flat_map(|block| &block.body).find_map(
            |instruction| match instruction {
                ArcInstr::Construct { ty, ctor, args, .. } => Some((*ty, *ctor, args)),
                _ => None,
            },
        );
    assert!(matches!(
        construct,
        Some((
            Idx::ORDERING,
            CtorKind::EnumVariant {
                enum_name,
                variant,
            },
            args,
        )) if enum_name == ordering_name
            && u64::from(variant) == ordering::unsigned::EQUAL
            && args.is_empty()
    ));
    assert_eq!(ordering::unsigned::LESS, 0);
    assert_eq!(ordering::unsigned::EQUAL, 1);
    assert_eq!(ordering::unsigned::GREATER, 2);
}

#[test]
fn unit_fields_are_equal_without_an_unregistered_method_call() {
    let interner = StringInterner::new();
    let ordering_name = interner.intern("Ordering");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        interner.intern("UnitField"),
        &[(interner.intern("value"), Idx::UNIT)],
    );
    let body = body_or_panic(
        ordering_name,
        &signature(receiver, receiver, Idx::ORDERING),
        &pool,
    );

    assert!(body.method_call_facts.is_empty());
    assert!(invokes(&body).is_empty());
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Jump { target, .. }
            if body.blocks[target.index()].body.iter().any(|instruction| matches!(
                instruction,
                ArcInstr::Construct {
                    ty: Idx::ORDERING,
                    ..
                }
            ))
    )));
}

#[test]
fn invalid_signatures_fail_closed() {
    let interner = StringInterner::new();
    let ordering_name = interner.intern("Ordering");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(interner.intern("Item"), &[]);
    let other = pool.struct_type(interner.intern("Other"), &[]);

    let mut generic = signature(receiver, receiver, Idx::ORDERING);
    generic.type_params.push(Name::from_raw(20));
    generic.const_params.push(ConstParamInfo {
        name: Name::from_raw(21),
        const_type: Idx::INT,
        default_value: None,
    });
    assert_eq!(
        build_derived_compare(EXECUTABLE, ordering_name, METHOD, &generic, &pool),
        Err(DerivedCompareBodyError::GenericSignature {
            type_parameters: 1,
            const_parameters: 1,
        })
    );
    assert!(matches!(
        build_derived_compare(
            EXECUTABLE,
            ordering_name,
            METHOD,
            &signature(receiver, other, Idx::ORDERING),
            &pool,
        ),
        Err(DerivedCompareBodyError::ReceiverTypeMismatch { .. })
    ));
    assert_eq!(
        build_derived_compare(
            EXECUTABLE,
            ordering_name,
            METHOD,
            &signature(receiver, receiver, Idx::BOOL),
            &pool,
        ),
        Err(DerivedCompareBodyError::ReturnTypeMismatch {
            return_type: Idx::BOOL,
        })
    );
    assert!(matches!(
        build_derived_compare(
            EXECUTABLE,
            ordering_name,
            METHOD,
            &signature(Idx::INT, Idx::INT, Idx::ORDERING),
            &pool,
        ),
        Err(DerivedCompareBodyError::UnsupportedReceiverType {
            receiver_type: Idx::INT,
            tag: Tag::Int,
        })
    ));
}
