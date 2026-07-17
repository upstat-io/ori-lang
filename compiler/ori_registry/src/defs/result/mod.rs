//! `Result` type definition.
//!
//! Result is a Structural wrapper (`Result<T, E>`) — its memory strategy
//! depends on both `T` and `E`. Contains `Ok(T)` or `Err(E)`.
//!
//! Supports monadic operations (`map`, `map_err`, `and_then`, `or_else`),
//! unwrapping (`unwrap`, `expect`, `unwrap_or`, `unwrap_err`, `expect_err`),
//! and projection (`ok`, `err` → `Option`).
//!
//! Includes `Traceable` trait methods (`has_trace`, `trace`, `trace_entries`)
//! for error trace inspection.

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, MethodRuntime, OpDefs, Ownership, ParamDef,
    ResultRuntime, ReturnTag, TypeDef, TypeParamArity, TypeProjection, TypeTag, ONE_SELF_OWNED,
};

use super::params::{CLOSURE_PARAM, MESSAGE_PARAM};

// Parameter arrays

/// `(default: T)` — for `unwrap_or`.
static DEFAULT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "default",
    ty: ReturnTag::OkType,
    ownership: Ownership::Owned,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const SELF: ReturnTag = ReturnTag::SelfType;
const FRESH: ReturnTag = ReturnTag::Fresh;

// All methods alphabetically sorted.
static RESULT_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "and_then",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::AndThen)),
    MethodDef::compound(
        "clone",
        &[],
        SELF,
        Some("Clone"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Clone)),
    MethodDef::compound(
        "compare",
        &ONE_SELF_OWNED,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Compare)),
    MethodDef::compound(
        "debug",
        &[],
        STR,
        Some("Debug"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Debug)),
    MethodDef::compound(
        "equals",
        &ONE_SELF_OWNED,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Equals)),
    MethodDef::compound(
        "err",
        &[],
        ReturnTag::OptionOf(TypeProjection::Err),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Err)),
    MethodDef::compound(
        "expect",
        &MESSAGE_PARAM,
        ReturnTag::OkType,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Expect)),
    MethodDef::compound(
        "expect_err",
        &MESSAGE_PARAM,
        ReturnTag::ErrType,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::ExpectErr)),
    MethodDef::compound(
        "has_trace",
        &[],
        BOOL,
        Some("Traceable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::HasTrace)),
    MethodDef::compound(
        "hash",
        &[],
        INT,
        Some("Hashable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Hash)),
    MethodDef::compound(
        "is_err",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::IsErr)),
    MethodDef::compound(
        "is_ok",
        &[],
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::IsOk)),
    MethodDef::compound(
        "map",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Map)),
    MethodDef::compound(
        "map_err",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::MapErr)),
    MethodDef::compound(
        "ok",
        &[],
        ReturnTag::OptionOf(TypeProjection::Ok),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Ok)),
    MethodDef::compound(
        "or_else",
        &CLOSURE_PARAM,
        FRESH,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::OrElse)),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::ToString),
    MethodDef::compound(
        "trace",
        &[],
        STR,
        Some("Traceable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Trace)),
    MethodDef::compound(
        "trace_entries",
        &[],
        FRESH,
        Some("Traceable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::TraceEntries)),
    MethodDef::compound(
        "unwrap",
        &[],
        ReturnTag::OkType,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::Unwrap)),
    MethodDef::compound(
        "unwrap_err",
        &[],
        ReturnTag::ErrType,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::UnwrapErr)),
    MethodDef::compound(
        "unwrap_or",
        &DEFAULT_PARAM,
        ReturnTag::OkType,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Result(ResultRuntime::UnwrapOr)),
];

pub static RESULT: TypeDef = TypeDef {
    tag: TypeTag::Result,
    name: "Result",
    memory: MemoryStrategy::Structural,
    type_params: TypeParamArity::Fixed(2),
    methods: RESULT_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Comparable", "Printable"],
};

#[cfg(test)]
mod tests;
