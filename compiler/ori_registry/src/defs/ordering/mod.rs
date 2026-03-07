//! `Ordering` type definition.
//!
//! Ordering is a Copy enum (i8: Less=0, Equal=1, Greater=2) returned by
//! `compare` on all `Comparable` types. It supports `==`/`!=` but NOT
//! `<`/`>` (that would be circular — `<` desugars to `compare()` which
//! returns Ordering).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef, ReturnTag, TypeDef,
    TypeParamArity, TypeTag, VariantSpec,
};

// Shared parameter arrays

/// `(other: Self)` — for `equals`, `compare`.
static SELF_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::SelfType,
    ownership: Ownership::Copy,
}];

/// `(other: Ordering)` — for `then`.
static ORDERING_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::Concrete(TypeTag::Ordering),
    ownership: Ownership::Copy,
}];

/// `(f: () -> Ordering)` — closure param for `then_with`.
static THEN_WITH_PARAM: [ParamDef; 1] = [ParamDef {
    name: "f",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);

// All methods alphabetically sorted. `backend_required` = true for methods
// with eval+LLVM coverage, false for typeck-only methods.
static ORDERING_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "clone",
        &[],
        ReturnTag::SelfType,
        Some("Clone"),
        Ownership::Borrow,
        true,
    ),
    MethodDef::compound(
        "compare",
        &SELF_PARAM,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        true,
    ),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, true),
    MethodDef::compound(
        "equals",
        &SELF_PARAM,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        true,
    ),
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, true),
    MethodDef::compound("is_equal", &[], BOOL, None, Ownership::Borrow, true),
    MethodDef::compound("is_greater", &[], BOOL, None, Ownership::Borrow, true),
    MethodDef::compound(
        "is_greater_or_equal",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        true,
    ),
    MethodDef::compound("is_less", &[], BOOL, None, Ownership::Borrow, true),
    MethodDef::compound("is_less_or_equal", &[], BOOL, None, Ownership::Borrow, true),
    MethodDef::compound(
        "reverse",
        &[],
        ReturnTag::SelfType,
        None,
        Ownership::Borrow,
        true,
    ),
    MethodDef::compound("then", &ORDERING_PARAM, ORD, None, Ownership::Borrow, true),
    MethodDef::compound(
        "then_with",
        &THEN_WITH_PARAM,
        ORD,
        None,
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        true,
    ),
];

pub static ORDERING: TypeDef = TypeDef {
    tag: TypeTag::Ordering,
    name: "Ordering",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: ORDERING_METHODS,
    operators: OpDefs {
        add: OpStrategy::Unsupported,
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::IntInstr,
        neq: OpStrategy::IntInstr,
        lt: OpStrategy::Unsupported,
        gt: OpStrategy::Unsupported,
        lt_eq: OpStrategy::Unsupported,
        gt_eq: OpStrategy::Unsupported,
        neg: OpStrategy::Unsupported,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    },
};

/// Ordering enum variants: Less=0, Equal=1, Greater=2.
pub static ORDERING_VARIANTS: &[VariantSpec] = &[
    VariantSpec {
        name: "Less",
        tag: 0,
        fields: &[],
    },
    VariantSpec {
        name: "Equal",
        tag: 1,
        fields: &[],
    },
    VariantSpec {
        name: "Greater",
        tag: 2,
        fields: &[],
    },
];

#[cfg(test)]
mod tests;
