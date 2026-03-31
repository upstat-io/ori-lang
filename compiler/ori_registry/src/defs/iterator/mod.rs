//! `Iterator` and `DoubleEndedIterator` type definition.
//!
//! Uses the "single `TypeDef` with `dei_only` flag" design:
//! all 24 user-callable methods live on one `TypeDef` keyed by `TypeTag::Iterator`.
//! `TypeTag::DoubleEndedIterator` aliases to `Iterator` via `TypeTag::base_type()`,
//! and the query API filters by `dei_only` to exclude DEI-specific methods when
//! the receiver is a plain Iterator.
//!
//! Iterator has NO trait methods — iterators are opaque lazy sequences that cannot
//! be printed, cloned, compared, hashed, or formatted. All 24 methods are either
//! protocol methods (`next`, `next_back`), adapters, or consumers.

use crate::{
    DeiPropagation, MemoryStrategy, MethodDef, MethodKind, OpDefs, Ownership, ParamDef, ReturnTag,
    TypeDef, TypeParamArity, TypeProjection, TypeTag,
};

use super::params::{COUNT_PARAM, SEPARATOR_PARAM};

// Shared return tag constants

const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);

// Parameter arrays

/// `(predicate: (T) -> bool)` — closure param for filter, find, any, all, rfind.
static PREDICATE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "predicate",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(transform: (T) -> U)` — closure param for map, `flat_map`.
static TRANSFORM_PARAM: [ParamDef; 1] = [ParamDef {
    name: "transform",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(f: (T) -> void)` — closure param for `for_each`.
static ACTION_PARAM: [ParamDef; 1] = [ParamDef {
    name: "f",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(other: Iterator<U>)` — iterator param for chain, zip.
static OTHER_ITER_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(initial: S, op: (S, T) -> S)` — init + closure for fold, rfold.
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

// Iterator method constructor.
//
// All iterator methods share: receiver=Borrow, trait_name=None, pure=true,
// kind=Instance. Only `dei_only`, `dei_propagation`, and `backend_required`
// vary per method.
const fn iter(
    name: &'static str,
    params: &'static [ParamDef],
    returns: ReturnTag,
    dei_only: bool,
    dei_propagation: DeiPropagation,
    backend_required: bool,
) -> MethodDef {
    MethodDef {
        name,
        receiver: Ownership::Borrow,
        params,
        returns,
        trait_name: None,
        pure: true,
        backend_required,
        kind: MethodKind::Instance,
        dei_only,
        dei_propagation,
    }
}

// Shorthand aliases for DeiPropagation variants.
const P: DeiPropagation = DeiPropagation::Propagate;
const D: DeiPropagation = DeiPropagation::Downgrade;
const NA: DeiPropagation = DeiPropagation::NotApplicable;

/// All 24 user-callable Iterator/DoubleEndedIterator methods.
///
/// Sorted alphabetically by name for deterministic iteration and binary search.
/// DEI-only methods are interleaved at their alphabetical position (not grouped
/// at the end) — the `dei_only` flag handles filtering.
///
/// Internal methods (`__iter_next`, `__collect_set`) are NOT included.
/// They are compiler-internal implementation details handled by the
/// LLVM backend and evaluator directly.
#[rustfmt::skip]
static ITERATOR_METHODS: &[MethodDef] = &[
    //                    name          params              returns                                        dei_only  dei_prop  backend
    iter("all",           &PREDICATE_PARAM,  BOOL,                                                        false,    NA,       true),
    iter("any",           &PREDICATE_PARAM,  BOOL,                                                        false,    NA,       true),
    iter("chain",         &OTHER_ITER_PARAM, ReturnTag::IteratorOf(TypeProjection::Element),               false,    D,        true),
    iter("collect",       &[],               ReturnTag::ListOf(TypeProjection::Element),                   false,    NA,       true),
    iter("count",         &[],               INT,                                                         false,    NA,       true),
    iter("cycle",         &[],               ReturnTag::IteratorOf(TypeProjection::Element),               false,    D,        false),
    iter("enumerate",     &[],               ReturnTag::IteratorOfTupleIntElement,                         false,    D,        true),
    iter("filter",        &PREDICATE_PARAM,  ReturnTag::SelfType,                                         false,    P,        true),
    iter("find",          &PREDICATE_PARAM,  ReturnTag::OptionOf(TypeProjection::Element),                 false,    NA,       true),
    iter("flat_map",      &TRANSFORM_PARAM,  ReturnTag::Fresh,                                            false,    D,        false),
    iter("flatten",       &[],               ReturnTag::Fresh,                                            false,    D,        false),
    iter("fold",          &FOLD_PARAMS,      ReturnTag::Fresh,                                            false,    NA,       true),
    iter("for_each",      &ACTION_PARAM,     ReturnTag::Unit,                                             false,    NA,       true),
    iter("join",          &SEPARATOR_PARAM,  STR,                                                         false,    NA,       false),
    iter("last",          &[],               ReturnTag::OptionOf(TypeProjection::Element),                 true,     NA,       false),
    iter("map",           &TRANSFORM_PARAM,  ReturnTag::Fresh,                                            false,    P,        true),
    iter("next",          &[],               ReturnTag::NextResult,                                       false,    NA,       false),
    iter("next_back",     &[],               ReturnTag::NextResult,                                       true,     NA,       false),
    iter("rev",           &[],               ReturnTag::SelfType,                                         true,     NA,       false),
    iter("rfind",         &PREDICATE_PARAM,  ReturnTag::OptionOf(TypeProjection::Element),                 true,     NA,       false),
    iter("rfold",         &FOLD_PARAMS,      ReturnTag::Fresh,                                            true,     NA,       false),
    iter("skip",          &COUNT_PARAM,      ReturnTag::IteratorOf(TypeProjection::Element),               false,    D,        true),
    iter("take",          &COUNT_PARAM,      ReturnTag::IteratorOf(TypeProjection::Element),               false,    D,        true),
    iter("zip",           &OTHER_ITER_PARAM, ReturnTag::Fresh,                                            false,    D,        true),
];

/// `Iterator<T>` — lazy sequence with adapter/consumer protocol.
///
/// `DoubleEndedIterator<T>` is not a separate `TypeDef`. It shares this
/// definition and is distinguished by `TypeTag::base_type()` aliasing
/// plus `dei_only` filtering on individual methods.
pub static ITERATOR: TypeDef = TypeDef {
    tag: TypeTag::Iterator,
    name: "Iterator",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: ITERATOR_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Iterator"],
};

#[cfg(test)]
mod tests;
