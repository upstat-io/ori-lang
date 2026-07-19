//! `Duration` type definition.
//!
//! Duration is stored as `i64` nanoseconds. Copy type with full arithmetic
//! operator support via `SignedInteger`. Has heterogeneous `mul`/`div` operators
//! (take `int`, not `Self`). Includes `format` (explicit Formattable entry).

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef,
    ReturnTag, TypeDef, TypeParamArity, TypeTag, ONE_SELF_COPY,
};

// Shared parameter arrays

/// `(val: int)` — for associated factory functions.
static INT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "val",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

/// `(scalar: int)` — for heterogeneous mul/div operators.
static SCALAR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "scalar",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const FLOAT: ReturnTag = ReturnTag::Concrete(TypeTag::Float);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const SELF: ReturnTag = ReturnTag::SelfType;

const BACKEND_REQUIRED: BackendRequirement = BackendRequirement::Required;
const BACKEND_NOT_REQUIRED: BackendRequirement = BackendRequirement::NotRequired;

// All methods alphabetically sorted.
static DURATION_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "abs",
        &[],
        SELF,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "add",
        &ONE_SELF_COPY,
        SELF,
        Some("Add"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "as_micros",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "as_millis",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "as_nanos",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "as_seconds",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "clone",
        &[],
        SELF,
        Some("Clone"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "compare",
        &ONE_SELF_COPY,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "debug",
        &[],
        STR,
        Some("Debug"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    // Default::default() -> Self (0ns). Associated (no receiver); the Default
    // trait it satisfies is also listed in TypeDef.traits.
    MethodDef::associated("default", &[], SELF),
    MethodDef::compound(
        "div",
        &SCALAR_PARAM,
        SELF,
        Some("Div"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "equals",
        &ONE_SELF_COPY,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "format",
        &[],
        STR,
        Some("Formattable"),
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::associated("from_hours", &INT_PARAM, SELF),
    MethodDef::associated("from_micros", &INT_PARAM, SELF),
    MethodDef::associated("from_microseconds", &INT_PARAM, SELF),
    MethodDef::associated("from_millis", &INT_PARAM, SELF),
    MethodDef::associated("from_milliseconds", &INT_PARAM, SELF),
    MethodDef::associated("from_minutes", &INT_PARAM, SELF),
    MethodDef::associated("from_nanos", &INT_PARAM, SELF),
    MethodDef::associated("from_nanoseconds", &INT_PARAM, SELF),
    MethodDef::associated("from_seconds", &INT_PARAM, SELF),
    MethodDef::compound(
        "hash",
        &[],
        INT,
        Some("Hashable"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound("hours", &[], INT, None, Ownership::Borrow, BACKEND_REQUIRED),
    MethodDef::compound(
        "is_negative",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "is_positive",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "is_zero",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "microseconds",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "milliseconds",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "minutes",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "mul",
        &SCALAR_PARAM,
        SELF,
        Some("Mul"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "nanoseconds",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "neg",
        &[],
        SELF,
        Some("Neg"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "rem",
        &ONE_SELF_COPY,
        SELF,
        Some("Rem"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "seconds",
        &[],
        INT,
        None,
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "sub",
        &ONE_SELF_COPY,
        SELF,
        Some("Sub"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::compound(
        "to_micros",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "to_millis",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "to_nanos",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "to_seconds",
        &[],
        FLOAT,
        None,
        Ownership::Borrow,
        BACKEND_NOT_REQUIRED,
    ),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        BACKEND_REQUIRED,
    ),
    MethodDef::associated("zero", &[], SELF),
];

pub static DURATION: TypeDef = TypeDef {
    tag: TypeTag::Duration,
    name: "Duration",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: DURATION_METHODS,
    operators: OpDefs {
        add: OpStrategy::SignedInteger,
        sub: OpStrategy::SignedInteger,
        mul: OpStrategy::SignedInteger,
        div: OpStrategy::SignedInteger,
        rem: OpStrategy::SignedInteger,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::SignedInteger,
        neq: OpStrategy::SignedInteger,
        lt: OpStrategy::SignedInteger,
        gt: OpStrategy::SignedInteger,
        lt_eq: OpStrategy::SignedInteger,
        gt_eq: OpStrategy::SignedInteger,
        neg: OpStrategy::SignedInteger,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
    traits: &["Default", "Sendable"],
};

#[cfg(test)]
mod tests;
