//! `Set` type definition.
//!
//! Set is an Arc-managed unique-element collection (`Set<T>` where `T: Hashable`).
//! Supports standard set operations (union, intersection, difference) and
//! iteration via `Iterator` (not `DoubleEndedIterator` — hash sets have no
//! inherent ordering).

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, MethodRuntime, OpDefs, Ownership, ReturnTag,
    TypeDef, TypeParamArity, TypeProjection, TypeTag, ONE_SELF_BORROW,
};

use super::params::{CLOSURE_PARAM, ELEMENT_BORROW_PARAM, ELEMENT_OWNED_PARAM};

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const FRESH: ReturnTag = ReturnTag::Fresh;
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;

// All methods alphabetically sorted.
static SET_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "clone",
        &[],
        SELF,
        Some("Clone"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "contains",
        &ELEMENT_BORROW_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "debug",
        &[],
        STR,
        Some("Debug"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "difference",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "equals",
        &ONE_SELF_BORROW,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "fold",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "hash",
        &[],
        INT,
        Some("Hashable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "insert",
        &ELEMENT_OWNED_PARAM,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "intersection",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "into",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        Some("Into"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "is_empty",
        &[],
        BOOL,
        Some("IsEmpty"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "iter",
        &[],
        ReturnTag::IteratorOf(TypeProjection::Element),
        Some("Iterable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "len",
        &[],
        INT,
        Some("Len"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Length),
    MethodDef::compound(
        "length",
        &[],
        INT,
        Some("Len"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Length),
    MethodDef::compound(
        "remove",
        &ELEMENT_BORROW_PARAM,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "to_list",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "union",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
];

pub static SET: TypeDef = TypeDef {
    tag: TypeTag::Set,
    name: "Set",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: SET_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Printable"],
};

#[cfg(test)]
mod tests;
