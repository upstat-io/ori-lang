use super::*;

#[test]
fn test_type_id_to_str() {
    assert_eq!(type_id_to_str(TypeId::INT), "int");
    assert_eq!(type_id_to_str(TypeId::FLOAT), "float");
    assert_eq!(type_id_to_str(TypeId::BOOL), "bool");
    assert_eq!(type_id_to_str(TypeId::STR), "str");
    assert_eq!(type_id_to_str(TypeId::CHAR), "char");
    assert_eq!(type_id_to_str(TypeId::BYTE), "byte");
    assert_eq!(type_id_to_str(TypeId::VOID), "void");
    assert_eq!(type_id_to_str(TypeId::NEVER), "Never");
}
