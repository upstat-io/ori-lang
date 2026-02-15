use super::*;

#[test]
fn test_expr_id_valid() {
    let id = ExprId::new(42);
    assert!(id.is_valid());
    assert_eq!(id.index(), 42);
}

#[test]
fn test_expr_id_invalid() {
    assert!(!ExprId::INVALID.is_valid());
    assert!(!ExprId::default().is_valid());
}

#[test]
fn test_expr_range() {
    let range = ExprRange::new(10, 5);
    assert!(!range.is_empty());
    assert_eq!(range.len(), 5);
    let indices: Vec<_> = range.indices().collect();
    assert_eq!(indices, vec![10, 11, 12, 13, 14]);
}

#[test]
fn test_expr_range_empty() {
    assert!(ExprRange::EMPTY.is_empty());
    assert!(ExprRange::default().is_empty());
}

#[test]
fn test_expr_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ExprId::new(1));
    set.insert(ExprId::new(1)); // duplicate
    set.insert(ExprId::new(2));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_memory_size() {
    // ExprId: 4 bytes (u32)
    assert_eq!(std::mem::size_of::<ExprId>(), 4);

    // ExprRange: Design spec says 6 bytes (u32 + u16), but Rust aligns
    // to 8 bytes due to u32 alignment requirements. Still better than
    // Vec<ExprId> at 24+ bytes.
    assert_eq!(std::mem::size_of::<ExprRange>(), 8);
}

#[test]
fn test_parsed_type_id_valid() {
    let id = ParsedTypeId::new(42);
    assert!(id.is_valid());
    assert_eq!(id.index(), 42);
    assert_eq!(id.raw(), 42);
}

#[test]
fn test_parsed_type_id_invalid() {
    assert!(!ParsedTypeId::INVALID.is_valid());
    assert!(!ParsedTypeId::default().is_valid());
}

#[test]
fn test_parsed_type_id_debug() {
    let valid = ParsedTypeId::new(10);
    assert_eq!(format!("{valid:?}"), "ParsedTypeId(10)");
    let invalid = ParsedTypeId::INVALID;
    assert_eq!(format!("{invalid:?}"), "ParsedTypeId::INVALID");
}

#[test]
fn test_parsed_type_range() {
    let range = ParsedTypeRange::new(5, 3);
    assert!(!range.is_empty());
    assert_eq!(range.len(), 3);
}

#[test]
fn test_parsed_type_range_empty() {
    assert!(ParsedTypeRange::EMPTY.is_empty());
    assert!(ParsedTypeRange::default().is_empty());
}

#[test]
fn test_match_pattern_id_valid() {
    let id = MatchPatternId::new(100);
    assert!(id.is_valid());
    assert_eq!(id.index(), 100);
    assert_eq!(id.raw(), 100);
}

#[test]
fn test_match_pattern_id_invalid() {
    assert!(!MatchPatternId::INVALID.is_valid());
    assert!(!MatchPatternId::default().is_valid());
}

#[test]
fn test_match_pattern_id_debug() {
    let valid = MatchPatternId::new(20);
    assert_eq!(format!("{valid:?}"), "MatchPatternId(20)");
    let invalid = MatchPatternId::INVALID;
    assert_eq!(format!("{invalid:?}"), "MatchPatternId::INVALID");
}

#[test]
fn test_match_pattern_range() {
    let range = MatchPatternRange::new(0, 5);
    assert!(!range.is_empty());
    assert_eq!(range.len(), 5);
}

#[test]
fn test_match_pattern_range_empty() {
    assert!(MatchPatternRange::EMPTY.is_empty());
    assert!(MatchPatternRange::default().is_empty());
}

#[test]
fn test_parsed_type_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ParsedTypeId::new(1));
    set.insert(ParsedTypeId::new(1)); // duplicate
    set.insert(ParsedTypeId::new(2));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_match_pattern_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(MatchPatternId::new(1));
    set.insert(MatchPatternId::new(1)); // duplicate
    set.insert(MatchPatternId::new(2));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_new_id_memory_sizes() {
    // ParsedTypeId and MatchPatternId: 4 bytes each
    assert_eq!(std::mem::size_of::<ParsedTypeId>(), 4);
    assert_eq!(std::mem::size_of::<MatchPatternId>(), 4);

    // Range types: 8 bytes each (with padding)
    assert_eq!(std::mem::size_of::<ParsedTypeRange>(), 8);
    assert_eq!(std::mem::size_of::<MatchPatternRange>(), 8);
}
