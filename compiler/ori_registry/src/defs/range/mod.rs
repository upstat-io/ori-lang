//! `Range` type definition.
//!
//! Range is a Copy generic type (`Range<T>`) representing integer ranges
//! with start, end, step, and inclusive flag. It supports iteration
//! (producing a `DoubleEndedIterator`) and conversion to lists.
//!
//! Range has no operator support — arithmetic on ranges is not defined.
//! Float ranges exist but cannot iterate (`iter`, `to_list`, `collect`
//! are rejected for `Range<float>` by the type checker).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, Ownership, ParamDef, ReturnTag, TypeDef, TypeParamArity,
    TypeProjection, TypeTag,
};

// Parameter arrays

/// `(value: T)` — element-typed param for `contains`.
static ELEMENT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Borrow,
}];

/// `(step: int)` — step size for `step_by`.
static STEP_PARAM: [ParamDef; 1] = [ParamDef {
    name: "step",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);

// All 8 methods alphabetically sorted.
// backend_required: false for all — Range methods are typeck+eval only.
static RANGE_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "collect",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "contains",
        &ELEMENT_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("count", &[], INT, None, Ownership::Borrow, false),
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
        ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element),
        Some("Iterable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("len", &[], INT, Some("Len"), Ownership::Borrow, false),
    MethodDef::compound(
        "step_by",
        &STEP_PARAM,
        ReturnTag::SelfType,
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
];

pub static RANGE: TypeDef = TypeDef {
    tag: TypeTag::Range,
    name: "Range",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(1),
    methods: RANGE_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &[],
};

#[cfg(test)]
mod tests;
