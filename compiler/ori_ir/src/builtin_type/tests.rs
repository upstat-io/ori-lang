use super::*;

#[test]
fn test_from_type_id() {
    assert_eq!(
        BuiltinType::from_type_id(TypeId::INT),
        Some(BuiltinType::Int)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::FLOAT),
        Some(BuiltinType::Float)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::BOOL),
        Some(BuiltinType::Bool)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::STR),
        Some(BuiltinType::Str)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::CHAR),
        Some(BuiltinType::Char)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::BYTE),
        Some(BuiltinType::Byte)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::UNIT),
        Some(BuiltinType::Unit)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::NEVER),
        Some(BuiltinType::Never)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::DURATION),
        Some(BuiltinType::Duration)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::SIZE),
        Some(BuiltinType::Size)
    );
    assert_eq!(
        BuiltinType::from_type_id(TypeId::ORDERING),
        Some(BuiltinType::Ordering)
    );

    // ERROR, INFER, SELF_TYPE, and compound types return None
    assert_eq!(BuiltinType::from_type_id(TypeId::ERROR), None);
    assert_eq!(BuiltinType::from_type_id(TypeId::INFER), None);
    assert_eq!(BuiltinType::from_type_id(TypeId::SELF_TYPE), None);
    assert_eq!(BuiltinType::from_type_id(TypeId::from_raw(100)), None);
}

#[test]
fn test_type_id_roundtrip() {
    for builtin in [
        BuiltinType::Int,
        BuiltinType::Float,
        BuiltinType::Bool,
        BuiltinType::Str,
        BuiltinType::Char,
        BuiltinType::Byte,
        BuiltinType::Unit,
        BuiltinType::Never,
        BuiltinType::Duration,
        BuiltinType::Size,
        BuiltinType::Ordering,
    ] {
        let Some(type_id) = builtin.type_id() else {
            panic!("primitive {builtin:?} should have TypeId");
        };
        let Some(recovered) = BuiltinType::from_type_id(type_id) else {
            panic!("should recover builtin from TypeId");
        };
        assert_eq!(builtin, recovered);
    }
}

#[test]
fn test_container_types_no_type_id() {
    for builtin in [
        BuiltinType::List,
        BuiltinType::Map,
        BuiltinType::Option,
        BuiltinType::Result,
        BuiltinType::Range,
        BuiltinType::Set,
        BuiltinType::Channel,
    ] {
        assert!(builtin.type_id().is_none());
        assert!(builtin.is_container());
        assert!(!builtin.is_primitive());
    }
}

#[test]
fn test_names() {
    assert_eq!(BuiltinType::Int.name(), "int");
    assert_eq!(BuiltinType::Duration.name(), "Duration");
    assert_eq!(BuiltinType::Ordering.name(), "Ordering");
    assert_eq!(BuiltinType::Unit.name(), "()");
}

#[test]
fn test_is_numeric() {
    assert!(BuiltinType::Int.is_numeric());
    assert!(BuiltinType::Float.is_numeric());
    assert!(BuiltinType::Byte.is_numeric());
    assert!(!BuiltinType::Bool.is_numeric());
    assert!(!BuiltinType::Str.is_numeric());
}

#[test]
fn test_is_comparable() {
    assert!(BuiltinType::Int.is_comparable());
    assert!(BuiltinType::Duration.is_comparable());
    assert!(BuiltinType::Ordering.is_comparable());
    assert!(!BuiltinType::Unit.is_comparable());
    assert!(!BuiltinType::Never.is_comparable());
}

#[test]
fn test_display() {
    assert_eq!(format!("{}", BuiltinType::Int), "int");
    assert_eq!(format!("{}", BuiltinType::Duration), "Duration");
}
