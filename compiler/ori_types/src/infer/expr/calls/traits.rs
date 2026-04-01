//! Trait satisfaction checks for primitive and compound types.
//!
//! Delegates to the registry bridge (`registry_bridge`) for all trait
//! satisfaction queries. The registry is the single source of truth
//! for which traits each builtin type satisfies.

use crate::{Idx, Pool};

use super::super::registry_bridge::{registry_satisfies_trait, registry_type_satisfies_trait};

/// Check if a type inherently satisfies a trait without needing an explicit impl.
///
/// Queries the registry bridge for builtin type trait satisfaction.
/// Primitive types (identified by fixed `Idx` constants) are mapped to
/// `TypeTag`s and checked against the registry.
fn primitive_satisfies_trait(ty: Idx, trait_name: &str) -> bool {
    use ori_registry::TypeTag;

    // Map Idx constants to TypeTags
    let type_tag = if ty == Idx::INT {
        TypeTag::Int
    } else if ty == Idx::FLOAT {
        TypeTag::Float
    } else if ty == Idx::BOOL {
        TypeTag::Bool
    } else if ty == Idx::STR {
        TypeTag::Str
    } else if ty == Idx::CHAR {
        TypeTag::Char
    } else if ty == Idx::BYTE {
        TypeTag::Byte
    } else if ty == Idx::UNIT {
        TypeTag::Unit
    } else if ty == Idx::DURATION {
        TypeTag::Duration
    } else if ty == Idx::SIZE {
        TypeTag::Size
    } else if ty == Idx::ORDERING {
        TypeTag::Ordering
    } else {
        return false;
    };

    registry_satisfies_trait(type_tag, trait_name)
}

/// Extended trait satisfaction check that also handles compound types via Pool tags.
///
/// Queries the registry bridge for both primitive and compound type trait
/// satisfaction. The bridge derives trait information from `TypeDef.operators`,
/// `MethodDef.trait_name`, and `TypeDef.traits`.
pub(crate) fn type_satisfies_trait(ty: Idx, trait_name: &str, pool: &Pool) -> bool {
    // First check primitives (no pool access needed)
    if primitive_satisfies_trait(ty, trait_name) {
        return true;
    }

    // Then check compound types by tag via the registry bridge
    let tag = pool.tag(ty);
    if let Some(result) = registry_type_satisfies_trait(tag, trait_name) {
        return result;
    }

    false
}
