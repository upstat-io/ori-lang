use super::*;

#[test]
fn primitives_at_correct_indices() {
    let pool = Pool::new();

    assert_eq!(pool.tag(Idx::INT), Tag::Int);
    assert_eq!(pool.tag(Idx::FLOAT), Tag::Float);
    assert_eq!(pool.tag(Idx::BOOL), Tag::Bool);
    assert_eq!(pool.tag(Idx::STR), Tag::Str);
    assert_eq!(pool.tag(Idx::CHAR), Tag::Char);
    assert_eq!(pool.tag(Idx::BYTE), Tag::Byte);
    assert_eq!(pool.tag(Idx::UNIT), Tag::Unit);
    assert_eq!(pool.tag(Idx::NEVER), Tag::Never);
    assert_eq!(pool.tag(Idx::ERROR), Tag::Error);
    assert_eq!(pool.tag(Idx::DURATION), Tag::Duration);
    assert_eq!(pool.tag(Idx::SIZE), Tag::Size);
    assert_eq!(pool.tag(Idx::ORDERING), Tag::Ordering);
}

#[test]
fn primitive_flags_correct() {
    let pool = Pool::new();

    let int_flags = pool.flags(Idx::INT);
    assert!(int_flags.contains(TypeFlags::IS_PRIMITIVE));
    assert!(int_flags.contains(TypeFlags::IS_RESOLVED));
    assert!(int_flags.contains(TypeFlags::IS_MONO));
    assert!(!int_flags.has_errors());

    let error_flags = pool.flags(Idx::ERROR);
    assert!(error_flags.contains(TypeFlags::IS_PRIMITIVE));
    assert!(error_flags.has_errors());
}

#[test]
fn pool_starts_with_primitives() {
    let pool = Pool::new();
    assert_eq!(pool.len(), Idx::FIRST_DYNAMIC as usize);
}
