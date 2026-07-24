//! `void` / unit type definition.

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ReturnTag, TypeDef, TypeParamArity,
    TypeTag, ONE_SELF_COPY,
};

const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;

static UNIT_METHODS: &[MethodDef] = &[
    MethodDef::primitive("clone", &[], SELF, Some("Clone"), Ownership::Borrow),
    MethodDef::primitive(
        "compare",
        &ONE_SELF_COPY,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
    ),
    MethodDef::primitive("debug", &[], STR, Some("Debug"), Ownership::Borrow),
    MethodDef::associated("default", &[], SELF),
    MethodDef::primitive(
        "equals",
        &ONE_SELF_COPY,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
    ),
    MethodDef::primitive("hash", &[], INT, Some("Hashable"), Ownership::Borrow),
];

pub static UNIT: TypeDef = TypeDef {
    tag: TypeTag::Unit,
    name: "void",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: UNIT_METHODS,
    operators: OpDefs {
        add: OpStrategy::Unsupported,
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::StructuralEquality,
        neq: OpStrategy::StructuralEquality,
        lt: OpStrategy::StructuralOrdering,
        gt: OpStrategy::StructuralOrdering,
        lt_eq: OpStrategy::StructuralOrdering,
        gt_eq: OpStrategy::StructuralOrdering,
        neg: OpStrategy::Unsupported,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
    traits: &["Default"],
};

#[cfg(test)]
mod tests;
