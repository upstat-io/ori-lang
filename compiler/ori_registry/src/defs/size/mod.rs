//! `Size` type definition.
//!
//! Size is stored as `i64` bytes (non-negative). Copy type with arithmetic operator support
//! via `IntInstr`. Has heterogeneous `mul`/`div` operators (take `int`, not
//! `Self`). No `neg` operator — Size is semantically non-negative.
//! Includes `format` (explicit Formattable entry). SI units (1000-based).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef, ReturnTag, TypeDef,
    TypeParamArity, TypeTag, ONE_SELF_COPY,
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
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const SELF: ReturnTag = ReturnTag::SelfType;

// b = backend_required
const B: bool = true;
const NB: bool = false;

// All 34 methods alphabetically sorted.
static SIZE_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "add",
        &ONE_SELF_COPY,
        SELF,
        Some("Add"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound("as_bytes", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::compound("bytes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("clone", &[], SELF, Some("Clone"), Ownership::Borrow, B),
    MethodDef::compound(
        "compare",
        &ONE_SELF_COPY,
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
        &ONE_SELF_COPY,
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
    MethodDef::associated("from_bytes", &INT_PARAM, SELF),
    MethodDef::associated("from_gb", &INT_PARAM, SELF),
    MethodDef::associated("from_gigabytes", &INT_PARAM, SELF),
    MethodDef::associated("from_kb", &INT_PARAM, SELF),
    MethodDef::associated("from_kilobytes", &INT_PARAM, SELF),
    MethodDef::associated("from_mb", &INT_PARAM, SELF),
    MethodDef::associated("from_megabytes", &INT_PARAM, SELF),
    MethodDef::associated("from_tb", &INT_PARAM, SELF),
    MethodDef::associated("from_terabytes", &INT_PARAM, SELF),
    MethodDef::compound("gigabytes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, B),
    MethodDef::compound("is_zero", &[], BOOL, None, Ownership::Borrow, NB),
    MethodDef::compound("kilobytes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("megabytes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound(
        "mul",
        &SCALAR_PARAM,
        SELF,
        Some("Mul"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound(
        "rem",
        &ONE_SELF_COPY,
        SELF,
        Some("Rem"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound(
        "sub",
        &ONE_SELF_COPY,
        SELF,
        Some("Sub"),
        Ownership::Borrow,
        B,
    ),
    MethodDef::compound("terabytes", &[], INT, None, Ownership::Borrow, B),
    MethodDef::compound("to_bytes", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_gb", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_kb", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_mb", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::compound("to_str", &[], STR, Some("Printable"), Ownership::Borrow, B),
    MethodDef::compound("to_tb", &[], INT, None, Ownership::Borrow, NB),
    MethodDef::associated("zero", &[], SELF),
];

pub static SIZE: TypeDef = TypeDef {
    tag: TypeTag::Size,
    name: "Size",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: SIZE_METHODS,
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
        neg: OpStrategy::Unsupported,
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
