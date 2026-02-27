//! Trait satisfaction checks for primitive and compound types.

use crate::{Idx, Pool, Tag};

/// Check if a type inherently satisfies a trait without needing an explicit impl.
///
/// Mirrors V1's `primitive_implements_trait()` from `bound_checking.rs`.
/// Primitive and built-in types have known trait implementations that don't
/// require explicit `impl` blocks in the trait registry.
#[expect(
    clippy::too_many_lines,
    reason = "per-primitive trait set lookup table"
)]
fn primitive_satisfies_trait(ty: Idx, trait_name: &str) -> bool {
    // Trait sets for each primitive type, matching V1's const arrays.
    const INT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const FLOAT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Neg",
    ];
    const BOOL_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Not",
    ];
    const STR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Len",
        "IsEmpty",
        "Add",
    ];
    const CHAR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
    ];
    const BYTE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const UNIT_TRAITS: &[&str] = &["Eq", "Clone", "Default", "Debug"];
    const DURATION_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "Neg",
    ];
    const SIZE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
    ];
    const ORDERING_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
    ];

    // Check primitive types by Idx constant
    if ty == Idx::INT {
        return INT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::FLOAT {
        return FLOAT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BOOL {
        return BOOL_TRAITS.contains(&trait_name);
    }
    if ty == Idx::STR {
        return STR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::CHAR {
        return CHAR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BYTE {
        return BYTE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::UNIT {
        return UNIT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::DURATION {
        return DURATION_TRAITS.contains(&trait_name);
    }
    if ty == Idx::SIZE {
        return SIZE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::ORDERING {
        return ORDERING_TRAITS.contains(&trait_name);
    }

    false
}

/// Extended trait satisfaction check that also handles compound types via Pool tags.
///
/// This extends `primitive_satisfies_trait` to handle List, Map, Option, Result,
/// Tuple, Set, and Range -- types that aren't simple Idx constants but can be
/// identified by their Pool tag.
pub(crate) fn type_satisfies_trait(ty: Idx, trait_name: &str, pool: &Pool) -> bool {
    const COLLECTION_TRAITS: &[&str] = &["Eq", "Clone", "Hashable", "Printable", "Len", "IsEmpty"];
    const WRAPPER_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Default",
    ];
    const RESULT_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Printable"];

    // First check primitives (no pool access needed)
    if primitive_satisfies_trait(ty, trait_name) {
        return true;
    }

    // Then check compound types by tag

    match pool.tag(ty) {
        Tag::List => {
            COLLECTION_TRAITS.contains(&trait_name)
                || trait_name == "Comparable"
                || trait_name == "Iterable"
        }
        Tag::Map | Tag::Set => COLLECTION_TRAITS.contains(&trait_name) || trait_name == "Iterable",
        Tag::Option => WRAPPER_TRAITS.contains(&trait_name),
        Tag::Result => RESULT_TRAITS.contains(&trait_name),
        Tag::Tuple => RESULT_TRAITS.contains(&trait_name) || trait_name == "Len",
        Tag::Range => matches!(trait_name, "Printable" | "Len" | "Iterable"),
        Tag::Str => trait_name == "Iterable",
        Tag::DoubleEndedIterator => trait_name == "Iterator" || trait_name == "DoubleEndedIterator",
        Tag::Iterator => trait_name == "Iterator",
        _ => false,
    }
}
