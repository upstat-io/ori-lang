//! `Range` type definition.
//!
//! Range is a Copy generic type (`Range<T>`) representing integer ranges
//! with start, end, step, and inclusive flag. It supports iteration
//! (producing a `DoubleEndedIterator`), eager higher-order operations, and
//! conversion to lists.
//!
//! Range has no operator support — arithmetic on ranges is not defined.
//! Float ranges exist but cannot iterate (`iter`, `to_list`, `collect`
//! are rejected for `Range<float>` by the type checker).

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, MethodRuntime, OpDefs, Ownership, ParamDef,
    ReturnTag, TypeDef, TypeParamArity, TypeProjection, TypeTag,
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

/// `(predicate: (T) -> bool)` — closure param for eager `filter`.
static PREDICATE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "predicate",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(transform: (T) -> U)` — closure param for eager `map`.
static TRANSFORM_PARAM: [ParamDef; 1] = [ParamDef {
    name: "transform",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(initial: U, op: (U, T) -> U)` — accumulator and closure for eager `fold`.
static FOLD_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "initial",
        ty: ReturnTag::Fresh,
        ownership: Ownership::Owned,
    },
    ParamDef {
        name: "op",
        ty: ReturnTag::Fresh,
        ownership: Ownership::Owned,
    },
];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);

// All methods alphabetically sorted.
// backend_required: false for all — Range methods are typeck+eval only.
static RANGE_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "collect",
        &[],
        ReturnTag::ListOf(TypeProjection::Element),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "contains",
        &ELEMENT_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "count",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "filter",
        &PREDICATE_PARAM,
        ReturnTag::ListOf(TypeProjection::Element),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "fold",
        &FOLD_PARAMS,
        ReturnTag::Fresh,
        None,
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
        ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element),
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
        "map",
        &TRANSFORM_PARAM,
        ReturnTag::Fresh,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "step_by",
        &STEP_PARAM,
        ReturnTag::SelfType,
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
];

pub static RANGE: TypeDef = TypeDef {
    tag: TypeTag::Range,
    name: "Range",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(1),
    methods: RANGE_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Printable"],
};

#[cfg(test)]
mod tests;
