use super::*;

#[test]
fn tag_values_in_expected_ranges() {
    // Primitives: 0-15
    assert!((Tag::Int as u8) < 16);
    assert!((Tag::Ordering as u8) < 16);

    // Simple containers: 16-31
    assert!((16..32).contains(&(Tag::List as u8)));
    assert!((16..32).contains(&(Tag::Range as u8)));

    // Two-child containers: 32-47
    assert!((32..48).contains(&(Tag::Map as u8)));
    assert!((32..48).contains(&(Tag::Result as u8)));
    assert!((32..48).contains(&(Tag::Borrowed as u8)));

    // Complex types: 48-79
    assert!((48..80).contains(&(Tag::Function as u8)));
    assert!((48..80).contains(&(Tag::Enum as u8)));

    // Named types: 80-95
    assert!((80..96).contains(&(Tag::Named as u8)));
    assert!((80..96).contains(&(Tag::Alias as u8)));

    // Type variables: 96-111
    assert!((96..112).contains(&(Tag::Var as u8)));
    assert!((96..112).contains(&(Tag::RigidVar as u8)));

    // Schemes: 112-127
    assert!((112..128).contains(&(Tag::Scheme as u8)));
}

#[test]
fn uses_extra_is_correct() {
    // Primitives don't use extra
    assert!(!Tag::Int.uses_extra());
    assert!(!Tag::Bool.uses_extra());

    // Simple containers don't use extra (child in data)
    assert!(!Tag::List.uses_extra());
    assert!(!Tag::Option.uses_extra());

    // Two-child containers use extra
    assert!(Tag::Map.uses_extra());
    assert!(Tag::Result.uses_extra());
    assert!(Tag::Borrowed.uses_extra());

    // Complex types use extra
    assert!(Tag::Function.uses_extra());
    assert!(Tag::Tuple.uses_extra());
    assert!(Tag::Struct.uses_extra());

    // Variables don't use extra (id in data)
    assert!(!Tag::Var.uses_extra());

    // Schemes use extra
    assert!(Tag::Scheme.uses_extra());
}

#[test]
fn is_primitive_is_correct() {
    assert!(Tag::Int.is_primitive());
    assert!(Tag::Error.is_primitive());
    assert!(!Tag::List.is_primitive());
    assert!(!Tag::Function.is_primitive());
}

#[test]
fn is_type_variable_is_correct() {
    assert!(Tag::Var.is_type_variable());
    assert!(Tag::BoundVar.is_type_variable());
    assert!(Tag::RigidVar.is_type_variable());
    assert!(!Tag::Int.is_type_variable());
    assert!(!Tag::Function.is_type_variable());
}
