use super::*;

#[test]
fn type_id_to_str_primitives() {
    assert_eq!(type_id_to_str(TypeId::INT), "int");
    assert_eq!(type_id_to_str(TypeId::FLOAT), "float");
    assert_eq!(type_id_to_str(TypeId::BOOL), "bool");
    assert_eq!(type_id_to_str(TypeId::STR), "str");
    assert_eq!(type_id_to_str(TypeId::CHAR), "char");
    assert_eq!(type_id_to_str(TypeId::BYTE), "byte");
    assert_eq!(type_id_to_str(TypeId::NEVER), "Never");
    assert_eq!(type_id_to_str(TypeId::DURATION), "Duration");
    assert_eq!(type_id_to_str(TypeId::SIZE), "Size");
    assert_eq!(type_id_to_str(TypeId::ORDERING), "Ordering");
}

/// VOID is an alias for UNIT — both must render as "void" in type annotations.
/// The Ori spec defines `void` as the keyword for the unit type in type positions
/// (e.g., `-> void`). The value `()` is an expression, not a type keyword.
#[test]
fn type_id_to_str_void_is_keyword() {
    assert_eq!(type_id_to_str(TypeId::VOID), "void");
    assert_eq!(type_id_to_str(TypeId::UNIT), "void");
    // Verify VOID and UNIT are the same TypeId
    assert_eq!(TypeId::VOID, TypeId::UNIT);
}

#[test]
fn type_id_to_str_unknown_for_compound() {
    // Compound types should not appear in ParsedType::Primitive,
    // but if they do, we get "unknown" rather than a panic.
    assert_eq!(
        type_id_to_str(TypeId::from_raw(TypeId::FIRST_COMPOUND)),
        "unknown"
    );
}
