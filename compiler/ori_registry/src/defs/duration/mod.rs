//! `Duration` type definition.
//!
//! Duration is stored as `i64` nanoseconds. Copy type with full arithmetic
//! operator support via `IntInstr`. Has heterogeneous `mul`/`div` operators
//! (take `int`, not `Self`). Includes `format` (explicit Formattable entry).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef, ReturnTag, TypeDef,
    TypeParamArity, TypeTag,
};

// Shared parameter arrays

/// `(other: Self)` — for trait methods and homogeneous operators.
static SELF_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::SelfType,
    ownership: Ownership::Copy,
}];

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

// b = backend_required
const B: bool = true;
const NB: bool = false;

// All 41 methods alphabetically sorted.
static DURATION_METHODS: &[MethodDef] = &[
    MethodDef::compound("abs", &[], SELF, None, Ownership::Borrow, NB),
    MethodDef::compound("add", &SELF_PARAM, SELF, Some("Add"), Ownership::Borrow, B),
    MethodDef::compound("as_micros", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("as_millis", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("as_nanos", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("as_seconds", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("clone", &[], SELF, Some("Clone"), Ownership::Borrow, B),
    MethodDef::compound(
        "compare",
        &SELF_PARAM,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, B),
    MethodDef::compound(
        "div",
        &SCALAR_PARAM,
        SELF,
        Some("Div"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound(
        "equals",
        &SELF_PARAM,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound(
        "format",
        &[],
        STR,
        Some("Formattable"),
        Ownership::Borrow,
        NB,
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
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, B),
    MethodDef::compound("hours", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("is_negative", &[], BOOL, None, Ownership::Borrow, NB),
    MethodDef::compound("is_positive", &[], BOOL, None, Ownership::Borrow, NB),
    MethodDef::compound("is_zero", &[], BOOL, None, Ownership::Borrow, NB),
    MethodDef::compound("microseconds", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("milliseconds", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("minutes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound(
        "mul",
        &SCALAR_PARAM,
        SELF,
        Some("Mul"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound("nanoseconds", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("neg", &[], SELF, Some("Neg"), Ownership::Borrow, B),
    MethodDef::compound("rem", &SELF_PARAM, SELF, Some("Rem"), Ownership::Borrow, B),
    MethodDef::compound("seconds", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("sub", &SELF_PARAM, SELF, Some("Sub"), Ownership::Borrow, B),
    MethodDef::compound("to_micros", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_millis", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_nanos", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_seconds", &[], FLOAT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_str", &[], STR, Some("Printable"), Ownership::Borrow, B),
    MethodDef::associated("zero", &[], SELF),
];

pub static DURATION: TypeDef = TypeDef {
    tag: TypeTag::Duration,
    name: "Duration",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: DURATION_METHODS,
    operators: OpDefs {
        add: OpStrategy::IntInstr,
        sub: OpStrategy::IntInstr,
        mul: OpStrategy::IntInstr,
        div: OpStrategy::IntInstr,
        rem: OpStrategy::IntInstr,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::IntInstr,
        neq: OpStrategy::IntInstr,
        lt: OpStrategy::IntInstr,
        gt: OpStrategy::IntInstr,
        lt_eq: OpStrategy::IntInstr,
        gt_eq: OpStrategy::IntInstr,
        neg: OpStrategy::IntInstr,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
};

#[cfg(test)]
mod tests;
