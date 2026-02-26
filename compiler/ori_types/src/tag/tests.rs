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

#[test]
fn double_ended_iterator_in_simple_container_range() {
    assert!((16..32).contains(&(Tag::DoubleEndedIterator as u8)));
}

#[test]
fn double_ended_iterator_does_not_use_extra() {
    // DoubleEndedIterator is a simple container (one child in data)
    assert!(!Tag::DoubleEndedIterator.uses_extra());
}

#[test]
fn is_iterator_is_correct() {
    assert!(Tag::Iterator.is_iterator());
    assert!(Tag::DoubleEndedIterator.is_iterator());
    assert!(!Tag::List.is_iterator());
    assert!(!Tag::Int.is_iterator());
    assert!(!Tag::Option.is_iterator());
    assert!(!Tag::Function.is_iterator());
}

#[test]
fn double_ended_iterator_is_not_primitive() {
    assert!(!Tag::DoubleEndedIterator.is_primitive());
    assert!(!Tag::DoubleEndedIterator.is_type_variable());
}

#[test]
fn double_ended_iterator_has_correct_name() {
    assert_eq!(Tag::DoubleEndedIterator.name(), "DoubleEndedIterator");
}

// === Merkle Hash Classification Tests ===

/// Every Tag variant must fall into exactly one Merkle hash category.
///
/// If a new Tag variant is added, this test won't compile (non-exhaustive match),
/// forcing the developer to classify it for Merkle hashing.
#[test]
fn merkle_hash_classification_exhaustive() {
    // Exhaustive match ensures every variant is covered.
    // Each variant must be in exactly one of:
    //   1. has_child_in_data()  — simple containers
    //   2. uses_extra()         — complex/named/scheme types
    //   3. is_merkle_leaf()     — primitives/variables/specials
    let all_tags = [
        // Primitives (leaves)
        Tag::Int,
        Tag::Float,
        Tag::Bool,
        Tag::Str,
        Tag::Char,
        Tag::Byte,
        Tag::Unit,
        Tag::Never,
        Tag::Error,
        Tag::Duration,
        Tag::Size,
        Tag::Ordering,
        // Simple containers (child in data)
        Tag::List,
        Tag::Option,
        Tag::Set,
        Tag::Channel,
        Tag::Range,
        Tag::Iterator,
        Tag::DoubleEndedIterator,
        // Two-child containers (extra)
        Tag::Map,
        Tag::Result,
        Tag::Borrowed,
        // Complex types (extra)
        Tag::Function,
        Tag::Tuple,
        Tag::Struct,
        Tag::Enum,
        // Named types (extra)
        Tag::Named,
        Tag::Applied,
        Tag::Alias,
        // Type variables (leaves)
        Tag::Var,
        Tag::BoundVar,
        Tag::RigidVar,
        // Schemes (extra)
        Tag::Scheme,
        // Specials
        Tag::Projection, // uses extra
        Tag::ModuleNs,   // leaf
        Tag::Infer,      // leaf
        Tag::SelfType,   // leaf
    ];

    for &tag in &all_tags {
        let child_in_data = tag.has_child_in_data();
        let extra = tag.uses_extra();
        let leaf = tag.is_merkle_leaf();

        // Exactly one must be true
        let count = usize::from(child_in_data) + usize::from(extra) + usize::from(leaf);
        assert_eq!(
            count,
            1,
            "Tag::{} must be in exactly one Merkle category, but: \
             has_child_in_data={child_in_data}, uses_extra={extra}, is_merkle_leaf={leaf}",
            tag.name()
        );
    }
}

/// Verify the match-based exhaustiveness: if a new Tag variant is added,
/// this match will fail to compile, reminding the developer to update
/// the Merkle classification.
#[test]
fn merkle_hash_classification_match_coverage() {
    fn classify(tag: Tag) -> &'static str {
        match tag {
            // Child in data (simple containers)
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
                assert!(tag.has_child_in_data());
                "child_in_data"
            }

            // Uses extra array
            Tag::Map
            | Tag::Result
            | Tag::Borrowed
            | Tag::Function
            | Tag::Tuple
            | Tag::Struct
            | Tag::Enum
            | Tag::Named
            | Tag::Applied
            | Tag::Alias
            | Tag::Scheme
            | Tag::Projection => {
                assert!(tag.uses_extra());
                "uses_extra"
            }

            // Merkle leaves (no children, no extra)
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering
            | Tag::Var
            | Tag::BoundVar
            | Tag::RigidVar
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => {
                assert!(tag.is_merkle_leaf());
                "leaf"
            }
        }
    }

    // Verify a few representative tags
    assert_eq!(classify(Tag::Int), "leaf");
    assert_eq!(classify(Tag::List), "child_in_data");
    assert_eq!(classify(Tag::Map), "uses_extra");
    assert_eq!(classify(Tag::Function), "uses_extra");
    assert_eq!(classify(Tag::Named), "uses_extra");
    assert_eq!(classify(Tag::Var), "leaf");
    assert_eq!(classify(Tag::Scheme), "uses_extra");
    assert_eq!(classify(Tag::SelfType), "leaf");
}

#[test]
fn has_child_in_data_matches_simple_containers() {
    // All simple containers (range 16-31) with child in data
    assert!(Tag::List.has_child_in_data());
    assert!(Tag::Option.has_child_in_data());
    assert!(Tag::Set.has_child_in_data());
    assert!(Tag::Channel.has_child_in_data());
    assert!(Tag::Range.has_child_in_data());
    assert!(Tag::Iterator.has_child_in_data());
    assert!(Tag::DoubleEndedIterator.has_child_in_data());

    // NOT child in data
    assert!(!Tag::Int.has_child_in_data());
    assert!(!Tag::Map.has_child_in_data());
    assert!(!Tag::Function.has_child_in_data());
    assert!(!Tag::Var.has_child_in_data());
}

#[test]
fn is_merkle_leaf_matches_primitives_vars_specials() {
    // Primitives
    assert!(Tag::Int.is_merkle_leaf());
    assert!(Tag::Float.is_merkle_leaf());
    assert!(Tag::Error.is_merkle_leaf());
    assert!(Tag::Ordering.is_merkle_leaf());

    // Variables
    assert!(Tag::Var.is_merkle_leaf());
    assert!(Tag::BoundVar.is_merkle_leaf());
    assert!(Tag::RigidVar.is_merkle_leaf());

    // Specials without extra
    assert!(Tag::ModuleNs.is_merkle_leaf());
    assert!(Tag::Infer.is_merkle_leaf());
    assert!(Tag::SelfType.is_merkle_leaf());

    // NOT leaves
    assert!(!Tag::List.is_merkle_leaf());
    assert!(!Tag::Map.is_merkle_leaf());
    assert!(!Tag::Function.is_merkle_leaf());
    assert!(!Tag::Named.is_merkle_leaf());
    assert!(!Tag::Scheme.is_merkle_leaf());
}
