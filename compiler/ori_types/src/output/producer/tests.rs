use ori_ir::{
    ExprArena, ExprId, GenericParamRange, ImplMethod, ParamRange, ParsedType, Span, StringInterner,
    TypeId,
};

use super::{
    imported_method_producer, MethodProducer, RegistryMethodIdentity, RegistryPreludeIdentity,
    REGISTRY_PRODUCER_SCHEMA,
};

fn imported_method(name: ori_ir::Name, return_type: TypeId, body: u32) -> ImplMethod {
    ImplMethod {
        name,
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: ParsedType::Primitive(return_type),
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        body: ExprId::new(body),
        span: Span::DUMMY,
    }
}

#[test]
fn every_registry_method_identity_round_trips_without_saturation() {
    for receiver in ori_registry::TypeTag::all().iter().copied() {
        for method_name in ori_registry::method_names_for(receiver) {
            let registered =
                ori_registry::find_method_id(receiver, method_name).unwrap_or_else(|| {
                    panic!("missing registered identity for {receiver:?}.{method_name}")
                });
            let projected = RegistryMethodIdentity::from_registered(registered);
            assert_eq!(projected.schema(), REGISTRY_PRODUCER_SCHEMA);
            assert_eq!(projected.resolve(), Some(registered));
        }
    }
}

#[test]
fn every_prelude_identity_round_trips_without_saturation() {
    for function in ori_registry::PRELUDE_FUNCTIONS {
        let registered = ori_registry::find_prelude_function_id(function.name)
            .unwrap_or_else(|| panic!("missing registered prelude identity for {}", function.name));
        let projected = RegistryPreludeIdentity::from_registered(registered);
        assert_eq!(projected.schema(), REGISTRY_PRODUCER_SCHEMA);
        assert_eq!(projected.resolve(), Some(registered));
    }
}

#[test]
fn imported_method_symbol_distinguishes_provider_modules() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let method = imported_method(interner.intern("hash"), TypeId::INT, 1);
    let first = imported_method_producer("provider-a.ori", 2, 3, &method, &arena, &interner);
    let second = imported_method_producer("provider-b.ori", 2, 3, &method, &arena, &interner);

    let (
        MethodProducer::Imported {
            symbol: first_symbol,
            signature_hash: first_hash,
        },
        MethodProducer::Imported {
            symbol: second_symbol,
            signature_hash: second_hash,
        },
    ) = (first, second)
    else {
        panic!("imported producer constructor must return Imported")
    };
    assert_ne!(first_symbol, second_symbol);
    assert_eq!(first_hash, second_hash);
}

#[test]
fn imported_method_signature_change_keeps_symbol_and_changes_hash() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let method_name = interner.intern("hash");
    let original = imported_method(method_name, TypeId::INT, 1);
    let changed = imported_method(method_name, TypeId::BOOL, 99);
    let first = imported_method_producer("provider.ori", 2, 3, &original, &arena, &interner);
    let second = imported_method_producer("provider.ori", 2, 3, &changed, &arena, &interner);

    let (
        MethodProducer::Imported {
            symbol: first_symbol,
            signature_hash: first_hash,
        },
        MethodProducer::Imported {
            symbol: second_symbol,
            signature_hash: second_hash,
        },
    ) = (first, second)
    else {
        panic!("imported producer constructor must return Imported")
    };
    assert_eq!(first_symbol, second_symbol);
    assert_ne!(first_hash, second_hash);
}
