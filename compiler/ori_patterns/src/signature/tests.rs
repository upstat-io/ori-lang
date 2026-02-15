use super::*;

#[test]
fn test_pattern_signature_eq() {
    let name1 = Name::new(0, 1);
    let name2 = Name::new(0, 1);
    let name3 = Name::new(0, 2);

    let sig1 = PatternSignature::new(name1, TypeId::INT);
    let sig2 = PatternSignature::new(name2, TypeId::INT);
    let sig3 = PatternSignature::new(name3, TypeId::INT);

    assert_eq!(sig1, sig2);
    assert_ne!(sig1, sig3);
}

#[test]
fn test_pattern_signature_builder() {
    let name = Name::new(0, 1);
    let sig = PatternSignature::new(name, TypeId::INT)
        .with_input(TypeId::BOOL)
        .with_inputs([TypeId::STR, TypeId::FLOAT])
        .with_transform(FunctionSignature::unary(TypeId::INT, TypeId::BOOL));

    assert_eq!(sig.input_types.len(), 3);
    assert!(sig.transform_sig.is_some());
}

#[test]
fn test_function_signature() {
    let unary = FunctionSignature::unary(TypeId::INT, TypeId::BOOL);
    assert_eq!(unary.params.len(), 1);
    assert_eq!(unary.params[0], TypeId::INT);
    assert_eq!(unary.ret, TypeId::BOOL);

    let binary = FunctionSignature::binary(TypeId::INT, TypeId::STR, TypeId::FLOAT);
    assert_eq!(binary.params.len(), 2);
}
