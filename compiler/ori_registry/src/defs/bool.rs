//! `bool` type definition.

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ReturnTag, TypeDef, TypeParamArity,
    TypeTag, ONE_SELF_COPY,
};

static BOOL_METHODS: &[MethodDef] = &[
    MethodDef::primitive(
        "clone",
        &[],
        ReturnTag::SelfType,
        Some("Clone"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "compare",
        &ONE_SELF_COPY,
        ReturnTag::Concrete(TypeTag::Ordering),
        Some("Comparable"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "debug",
        &[],
        ReturnTag::Concrete(TypeTag::Str),
        Some("Debug"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "equals",
        &ONE_SELF_COPY,
        ReturnTag::Concrete(TypeTag::Bool),
        Some("Eq"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "hash",
        &[],
        ReturnTag::Concrete(TypeTag::Int),
        Some("Hashable"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "not",
        &[],
        ReturnTag::Concrete(TypeTag::Bool),
        Some("Not"),
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_int",
        &[],
        ReturnTag::Concrete(TypeTag::Int),
        None,
        Ownership::Borrow,
    ),
    MethodDef::primitive(
        "to_str",
        &[],
        ReturnTag::Concrete(TypeTag::Str),
        Some("Printable"),
        Ownership::Borrow,
    ),
];

pub static BOOL: TypeDef = TypeDef {
    tag: TypeTag::Bool,
    name: "bool",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: BOOL_METHODS,
    operators: OpDefs {
        add: OpStrategy::Unsupported,
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::BoolLogic,
        neq: OpStrategy::BoolLogic,
        lt: OpStrategy::UnsignedCmp,
        gt: OpStrategy::UnsignedCmp,
        lt_eq: OpStrategy::UnsignedCmp,
        gt_eq: OpStrategy::UnsignedCmp,
        neg: OpStrategy::Unsupported,
        not: OpStrategy::BoolLogic,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
};
