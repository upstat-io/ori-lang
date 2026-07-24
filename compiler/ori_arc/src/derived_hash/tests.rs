use ori_ir::{Name, StringInterner};
use ori_types::{ConstParamInfo, EnumVariant, FunctionSig, Idx, Pool, Tag};

use crate::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue, MethodCallFact, MethodCallForm,
    Ownership, VariableMetadataState,
};

use super::{build_derived_hash, DerivedHashBodyError};

const EXECUTABLE: Name = Name::from_raw(1);
const METHOD: Name = Name::from_raw(2);
const COMBINE: Name = Name::from_raw(3);
const SELF_NAME: Name = Name::from_raw(4);
const SIGNATURE_NAME: Name = Name::from_raw(5);

#[test]
fn index_overflow_preserves_conversion_source() {
    let Err(source) = u32::try_from(u64::MAX) else {
        panic!("u64::MAX must exceed the u32 index carrier");
    };
    let error = DerivedHashBodyError::IndexOverflow {
        receiver_type: Idx::INT,
        index_kind: "field",
        index: 0,
        source: Some(source),
    };

    assert!(std::error::Error::source(&error).is_some());
}

fn signature(receiver: Idx, return_type: Idx) -> FunctionSig {
    FunctionSig::synthetic(SIGNATURE_NAME, vec![SELF_NAME], vec![receiver], return_type)
}

fn body_or_panic(owner: Name, signature: &FunctionSig, pool: &Pool) -> ArcFunction {
    match build_derived_hash(EXECUTABLE, owner, METHOD, COMBINE, signature, pool) {
        Ok(body) => body,
        Err(error) => panic!("concrete Hashable derive must produce a shared body: {error}"),
    }
}

fn invokes(body: &ArcFunction) -> Vec<(crate::ArcVarId, Name, Vec<crate::ArcVarId>)> {
    body.blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            ArcTerminator::Invoke {
                dst, func, args, ..
            } => Some((*dst, *func, args.clone())),
            _ => None,
        })
        .collect()
}

fn applies(body: &ArcFunction) -> Vec<(crate::ArcVarId, Name, Vec<crate::ArcVarId>)> {
    body.blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instruction| match instruction {
            ArcInstr::Apply {
                dst, func, args, ..
            } => Some((*dst, *func, args.clone())),
            _ => None,
        })
        .collect()
}

fn zero_seed(body: &ArcFunction) -> Option<crate::ArcVarId> {
    body.blocks
        .iter()
        .flat_map(|block| &block.body)
        .find_map(|instruction| match instruction {
            ArcInstr::Let {
                dst,
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
            } => Some(*dst),
            _ => None,
        })
}

fn assert_semantic_only(body: &ArcFunction) {
    assert!(body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .all(|instruction| matches!(
            instruction,
            ArcInstr::Let { .. } | ArcInstr::Project { .. } | ArcInstr::Apply { .. }
        )));
}

#[test]
fn product_folds_non_unit_fields_from_zero_with_exact_method_facts() {
    let interner = StringInterner::new();
    let owner = interner.intern("Record");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        owner,
        &[
            (interner.intern("left"), Idx::INT),
            (interner.intern("empty"), Idx::UNIT),
            (interner.intern("right"), Idx::STR),
        ],
    );
    let body = body_or_panic(owner, &signature(receiver, Idx::INT), &pool);

    assert_eq!(body.params.len(), 1);
    assert_eq!(body.params[0].ty, receiver);
    assert_eq!(body.params[0].ownership, Ownership::Owned);
    assert_eq!(body.return_type, Idx::INT);
    assert_eq!(
        body.var_metadata_state,
        VariableMetadataState::RepresentationsReady
    );

    let calls = invokes(&body);
    assert_eq!(calls.len(), 2);
    assert!(calls
        .iter()
        .all(|(_, function, args)| *function == METHOD && args.len() == 1));
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

    let seed = zero_seed(&body).unwrap_or_else(|| panic!("product hash must start at zero"));
    let combines = applies(&body);
    assert_eq!(combines.len(), 2);
    assert!(combines
        .iter()
        .all(|(_, function, args)| *function == COMBINE && args.len() == 2));
    assert_eq!(combines[0].2, vec![seed, calls[0].0]);
    assert_eq!(combines[1].2, vec![combines[0].0, calls[1].0]);
    assert!(!body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .any(|instruction| matches!(instruction, ArcInstr::Project { field: 1, .. })));
    assert_semantic_only(&body);
}

#[test]
fn sum_folds_zero_based_tag_before_active_payload_fields() {
    let interner = StringInterner::new();
    let owner = interner.intern("Choice");
    let mut pool = Pool::new();
    let receiver = pool.enum_type(
        owner,
        &[
            EnumVariant {
                name: interner.intern("Empty"),
                field_types: Vec::new(),
            },
            EnumVariant {
                name: interner.intern("Pair"),
                field_types: vec![Idx::UNIT, Idx::STR],
            },
        ],
    );
    let body = body_or_panic(owner, &signature(receiver, Idx::INT), &pool);

    let seed = zero_seed(&body).unwrap_or_else(|| panic!("sum hash must start at zero"));
    let tag = body.blocks[0]
        .body
        .iter()
        .find_map(|instruction| match instruction {
            ArcInstr::Project {
                dst,
                ty: Idx::INT,
                field: 0,
                ..
            } => Some(*dst),
            _ => None,
        })
        .unwrap_or_else(|| panic!("sum hash must project its declaration ordinal"));
    let combines = applies(&body);
    assert_eq!(combines.len(), 2);
    assert_eq!(combines[0].2, vec![seed, tag]);

    let calls = invokes(&body);
    assert_eq!(calls.len(), 1);
    assert_eq!(body.method_call_facts[0].receiver_type, Idx::STR);
    assert_eq!(combines[1].2, vec![combines[0].0, calls[0].0]);

    let variant_switch = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            ArcTerminator::Switch { cases, default, .. }
                if cases.iter().map(|(value, _)| *value).eq([0, 1]) =>
            {
                Some((cases, *default))
            }
            _ => None,
        });
    let Some((cases, default)) = variant_switch else {
        panic!("sum hash must dispatch by zero-based declaration ordinal")
    };
    assert!(matches!(
        body.blocks[default.index()].terminator,
        ArcTerminator::Unreachable
    ));
    assert!(matches!(
        body.blocks[cases[0].1.index()].terminator,
        ArcTerminator::Return { value: combined } if combined == combines[0].0
    ));
    assert!(!body.blocks[cases[1].1.index()]
        .body
        .iter()
        .any(|instruction| matches!(instruction, ArcInstr::Project { field: 1, .. })));
    assert!(body
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .any(|instruction| matches!(instruction, ArcInstr::Project { field: 2, .. })));
    assert_semantic_only(&body);
}

#[test]
fn newtype_delegates_to_underlying_hash_without_combining() {
    let interner = StringInterner::new();
    let owner = interner.intern("UserId");
    let mut pool = Pool::new();
    let receiver = pool.named(owner);
    pool.register_newtype_ctor(owner, Idx::STR);
    pool.set_resolution(receiver, Idx::STR);
    let body = body_or_panic(owner, &signature(receiver, Idx::INT), &pool);

    assert!(zero_seed(&body).is_none());
    assert!(applies(&body).is_empty());
    let calls = invokes(&body);
    assert_eq!(calls.len(), 1);
    assert_eq!(
        body.method_call_facts,
        vec![MethodCallFact {
            destination: calls[0].0,
            receiver_type: Idx::STR,
            form: MethodCallForm::Instance,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }]
    );
    assert!(body.blocks[0].body.iter().any(|instruction| matches!(
        instruction,
        ArcInstr::Let {
            ty: Idx::STR,
            value: ArcValue::Var(source),
            ..
        } if *source == body.params[0].var
    )));
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        ArcTerminator::Return { value: result } if result == calls[0].0
    )));
    assert_semantic_only(&body);
}

#[test]
fn empty_or_all_unit_product_hashes_to_zero_without_calls() {
    let interner = StringInterner::new();
    let owner = interner.intern("UnitProduct");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(
        owner,
        &[
            (interner.intern("first"), Idx::UNIT),
            (interner.intern("second"), Idx::UNIT),
        ],
    );
    let body = body_or_panic(owner, &signature(receiver, Idx::INT), &pool);

    let seed = zero_seed(&body).unwrap_or_else(|| panic!("Unit product must hash to zero"));
    assert!(invokes(&body).is_empty());
    assert!(applies(&body).is_empty());
    assert!(matches!(
        body.blocks[0].terminator,
        ArcTerminator::Return { value: result } if result == seed
    ));
    assert_semantic_only(&body);
}

#[test]
fn invalid_signatures_fail_closed() {
    let interner = StringInterner::new();
    let owner = interner.intern("Item");
    let mut pool = Pool::new();
    let receiver = pool.struct_type(owner, &[]);

    let mut generic = signature(receiver, Idx::INT);
    generic.type_params.push(Name::from_raw(20));
    generic.const_params.push(ConstParamInfo {
        name: Name::from_raw(21),
        const_type: Idx::INT,
        default_value: None,
    });
    assert_eq!(
        build_derived_hash(EXECUTABLE, owner, METHOD, COMBINE, &generic, &pool),
        Err(DerivedHashBodyError::GenericSignature {
            type_parameters: 1,
            const_parameters: 1,
        })
    );
    assert_eq!(
        build_derived_hash(
            EXECUTABLE,
            owner,
            METHOD,
            COMBINE,
            &FunctionSig::synthetic(SIGNATURE_NAME, vec![], vec![], Idx::INT),
            &pool,
        ),
        Err(DerivedHashBodyError::InvalidParameterShape {
            parameter_names: 0,
            parameter_types: 0,
        })
    );
    assert_eq!(
        build_derived_hash(
            EXECUTABLE,
            owner,
            METHOD,
            COMBINE,
            &signature(receiver, Idx::BOOL),
            &pool,
        ),
        Err(DerivedHashBodyError::ReturnTypeMismatch {
            return_type: Idx::BOOL,
        })
    );
    assert!(matches!(
        build_derived_hash(
            EXECUTABLE,
            owner,
            METHOD,
            COMBINE,
            &signature(Idx::INT, Idx::INT),
            &pool,
        ),
        Err(DerivedHashBodyError::UnsupportedReceiverType {
            receiver_type: Idx::INT,
            tag: Tag::Int,
        })
    ));
}
