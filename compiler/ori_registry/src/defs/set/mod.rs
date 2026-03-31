//! `Set` type definition.
//!
//! Set is an Arc-managed unique-element collection (`Set<T>` where `T: Hashable`).
//! Supports standard set operations (union, intersection, difference) and
//! iteration via `Iterator` (not `DoubleEndedIterator` — hash sets have no
//! inherent ordering).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, Ownership, ReturnTag, TypeDef, TypeParamArity,
    TypeProjection, TypeTag, ONE_SELF_BORROW,
};

use super::params::{ELEMENT_BORROW_PARAM, ELEMENT_OWNED_PARAM};

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;

// All 16 methods alphabetically sorted.
static SET_METHODS: &[MethodDef] = &[
    MethodDef::compound("clone", &[], SELF, Some("Clone"), Ownership::Borrow, false),
    MethodDef::compound(
        "contains",
        &ELEMENT_BORROW_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, false),
    MethodDef::compound(
        "difference",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "equals",
        &ONE_SELF_BORROW,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, false),
    MethodDef::compound(
        "insert",
        &ELEMENT_OWNED_PARAM,
        SELF,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "intersection",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "into",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        Some("Into"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "is_empty",
        &[],
        BOOL,
        Some("IsEmpty"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "iter",
        &[],
        ReturnTag::IteratorOf(TypeProjection::Element),
        Some("Iterable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("len", &[], INT, Some("Len"), Ownership::Borrow, false),
    MethodDef::compound("length", &[], INT, Some("Len"), Ownership::Borrow, false),
    MethodDef::compound(
        "remove",
        &ELEMENT_BORROW_PARAM,
        SELF,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "to_list",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "union",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        false,
    ),
];

pub static SET: TypeDef = TypeDef {
    tag: TypeTag::Set,
    name: "Set",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: SET_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &[],
};

#[cfg(test)]
mod tests;
