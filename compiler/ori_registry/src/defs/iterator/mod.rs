//! `Iterator` and `DoubleEndedIterator` type definition.
//!
//! Uses the "single `TypeDef` with `dei_only` flag" design:
//! every user-callable method lives on one `TypeDef` keyed by `TypeTag::Iterator`.
//! `TypeTag::DoubleEndedIterator` aliases to `Iterator` via `TypeTag::base_type()`,
//! and the query API filters by `dei_only` to exclude DEI-specific methods when
//! the receiver is a plain Iterator.
//!
//! Iterator has NO trait methods — iterators are opaque lazy sequences that cannot
//! be printed, cloned, compared, hashed, or formatted. Every method is either
//! a protocol method (`next`, `next_back`), an adapter, or a consumer.

use crate::{
    DeiPropagation, MemoryStrategy, MethodDef, MethodKind, OpDefs, Ownership, ParamDef, ReturnTag,
    TypeDef, TypeParamArity, TypeProjection, TypeTag,
};

use super::params::{COUNT_PARAM, SEPARATOR_PARAM};

// Shared return tag constants

const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;
const FRESH: ReturnTag = ReturnTag::Fresh;
const UNIT: ReturnTag = ReturnTag::Unit;
const NEXT: ReturnTag = ReturnTag::NextResult;
const OPT_ELEM: ReturnTag = ReturnTag::OptionOf(TypeProjection::Element);
const ITER_ELEM: ReturnTag = ReturnTag::IteratorOf(TypeProjection::Element);
const LIST_ELEM: ReturnTag = ReturnTag::ListOf(TypeProjection::Element);
const IDX_PAIRS: ReturnTag = ReturnTag::IteratorOfTupleIntElement;

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
// All iterator methods share: receiver=Owned, trait_name=None, pure=true,
// kind=Instance. Only `dei_only`, `dei_propagation`, and `backend_required`
// vary per method.
//
// Iterator methods consume their receiver. Every adapter
// (`map`, `filter`, `take`, ...) internally calls
// `Box::from_raw(iter.cast::<IterState>())` on the source iterator to
// wrap it into an adapter variant; every consumer (`count`, `collect`,
// `fold`, ...) drains the iterator and then drops the `Box<IterState>`
// explicitly. The registry reports `Ownership::Owned` so ARC
// treats the call as a consumption event — but because ARC's borrow
// inference matches method names without type qualification, the
// borrow layer ALSO needs to check the receiver's type tag to avoid
// List/Map methods with the same name being mistakenly upgraded to
// Owned. That per-receiver-type disambiguation lives in the ARC
// pipeline, not the registry.
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
        receiver: Ownership::Owned,
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

/// The user-callable Iterator/DoubleEndedIterator methods.
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
    //   name         params             returns    dei_only  dei_prop  backend
    iter("all",       &PREDICATE_PARAM,  BOOL,      false,    NA,       true),
    iter("any",       &PREDICATE_PARAM,  BOOL,      false,    NA,       true),
    iter("chain",     &OTHER_ITER_PARAM, ITER_ELEM, false,    D,        true),
    iter("collect",   &[],               LIST_ELEM, false,    NA,       true),
    iter("count",     &[],               INT,       false,    NA,       true),
    iter("cycle",     &[],               ITER_ELEM, false,    D,        false),
    iter("enumerate", &[],               IDX_PAIRS, false,    D,        true),
    iter("filter",    &PREDICATE_PARAM,  SELF,      false,    P,        true),
    iter("find",      &PREDICATE_PARAM,  OPT_ELEM,  false,    NA,       true),
    iter("flat_map",  &TRANSFORM_PARAM,  FRESH,     false,    D,        false),
    iter("flatten",   &[],               FRESH,     false,    D,        false),
    iter("fold",      &FOLD_PARAMS,      FRESH,     false,    NA,       true),
    iter("for_each",  &ACTION_PARAM,     UNIT,      false,    NA,       true),
    iter("join",      &SEPARATOR_PARAM,  STR,       false,    NA,       false),
    iter("last",      &[],               OPT_ELEM,  true,     NA,       false),
    iter("map",       &TRANSFORM_PARAM,  FRESH,     false,    P,        true),
    iter("next",      &[],               NEXT,      false,    NA,       false),
    iter("next_back", &[],               NEXT,      true,     NA,       false),
    iter("rev",       &[],               SELF,      true,     NA,       false),
    iter("rfind",     &PREDICATE_PARAM,  OPT_ELEM,  true,     NA,       false),
    iter("rfold",     &FOLD_PARAMS,      FRESH,     true,     NA,       false),
    iter("skip",      &COUNT_PARAM,      ITER_ELEM, false,    D,        true),
    iter("take",      &COUNT_PARAM,      ITER_ELEM, false,    D,        true),
    iter("zip",       &OTHER_ITER_PARAM, FRESH,     false,    D,        true),
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
