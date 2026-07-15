//! `Option` type definition.
//!
//! Option is a Structural wrapper (`Option<T>`) — its memory strategy
//! depends on `T`. Contains `Some(T)` or `None`.
//!
//! Supports monadic operations (`map`, `and_then`, `flat_map`, `filter`,
//! `or_else`), unwrapping (`unwrap`, `expect`, `unwrap_or`), and
//! conversion to `Result` via `ok_or`.

use crate::{
    MemoryStrategy, MethodDef, MethodRuntime, OpDefs, OptionRuntime, Ownership, ParamDef,
    ReturnTag, TypeDef, TypeParamArity, TypeProjection, TypeTag, ONE_SELF_OWNED,
};

use super::params::{CLOSURE_PARAM, MESSAGE_PARAM};

// Parameter arrays

/// `(default: T)` — for `unwrap_or`.
static DEFAULT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "default",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Owned,
}];

/// `(err: E)` — for `ok_or` (fresh error type).
static ERR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "err",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(other: Option<T>)` — for `or`.
static OR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::SelfType,
    ownership: Ownership::Owned,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const ELEM: ReturnTag = ReturnTag::ElementType;
const SELF: ReturnTag = ReturnTag::SelfType;
const FRESH: ReturnTag = ReturnTag::Fresh;

// All methods alphabetically sorted.
static OPTION_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "and_then",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::AndThen)),
    MethodDef::compound("clone", &[], SELF, Some("Clone"), Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Clone)),
    MethodDef::compound(
        "compare",
        &ONE_SELF_OWNED,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::Compare)),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Debug)),
    MethodDef::compound(
        "equals",
        &ONE_SELF_OWNED,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::Equals)),
    MethodDef::compound(
        "expect",
        &MESSAGE_PARAM,
        ELEM,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::Expect)),
    MethodDef::compound(
        "filter",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::Filter)),
    MethodDef::compound(
        "flat_map",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::AndThen)),
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Hash)),
    MethodDef::compound("is_none", &[], BOOL, None, Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::IsNone)),
    MethodDef::compound("is_some", &[], BOOL, None, Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::IsSome)),
    MethodDef::compound(
        "iter",
        &[],
        ReturnTag::IteratorOf(TypeProjection::Element),
        Some("Iterable"),
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Iter),
    MethodDef::compound("map", &CLOSURE_PARAM, FRESH, None, Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Map)),
    MethodDef::compound(
        "ok_or",
        &ERR_PARAM,
        ReturnTag::ResultOfProjectionFresh(TypeProjection::Element),
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::OkOr)),
    MethodDef::compound("or", &OR_PARAM, SELF, None, Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Or)),
    MethodDef::compound(
        "or_else",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::OrElse)),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::ToString),
    MethodDef::compound("unwrap", &[], ELEM, None, Ownership::Borrow, false)
        .with_runtime(MethodRuntime::Option(OptionRuntime::Unwrap)),
    MethodDef::compound(
        "unwrap_or",
        &DEFAULT_PARAM,
        ELEM,
        None,
        Ownership::Borrow,
        false,
    )
    .with_runtime(MethodRuntime::Option(OptionRuntime::UnwrapOr)),
];

pub static OPTION: TypeDef = TypeDef {
    tag: TypeTag::Option,
    name: "Option",
    memory: MemoryStrategy::Structural,
    type_params: TypeParamArity::Fixed(1),
    methods: OPTION_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Default", "Printable"],
};

#[cfg(test)]
mod tests;
