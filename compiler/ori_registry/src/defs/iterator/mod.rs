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
    BackendRequirement, DeiPropagation, MemoryStrategy, MethodDef, MethodKind, OpDefs, Ownership,
    ParamDef, ReturnTag, TypeDef, TypeParamArity, TypeProjection, TypeTag,
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

#[derive(Clone, Copy)]
enum IteratorAvailability {
    AnyIterator,
    DoubleEndedOnly,
}

impl IteratorAvailability {
    const fn is_dei_only(self) -> bool {
        matches!(self, Self::DoubleEndedOnly)
    }
}

const ANY_ITERATOR: IteratorAvailability = IteratorAvailability::AnyIterator;
const DOUBLE_ENDED_ONLY: IteratorAvailability = IteratorAvailability::DoubleEndedOnly;
const BACKEND_REQUIRED: BackendRequirement = BackendRequirement::Required;
const BACKEND_NOT_REQUIRED: BackendRequirement = BackendRequirement::NotRequired;

// Iterator method constructor.
//
// All iterator methods share: receiver=Owned, trait_name=None, pure=true,
// kind=Instance. Only availability, `dei_propagation`, and backend requirement
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
    availability: IteratorAvailability,
    dei_propagation: DeiPropagation,
    backend_requirement: BackendRequirement,
) -> MethodDef {
    MethodDef {
        name,
        receiver: Ownership::Owned,
        params,
        returns,
        runtime: None,
        trait_name: None,
        pure: true,
        backend_required: backend_requirement.is_required(),
        kind: MethodKind::Instance,
        dei_only: availability.is_dei_only(),
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
/// They are compiler-internal implementation details handled through each
/// admitted executor's internal-operation projection.
#[rustfmt::skip]
static ITERATOR_METHODS: &[MethodDef] = &[
    //   name         params             returns    availability       dei_prop  backend
    iter("all",       &PREDICATE_PARAM,  BOOL,      ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("any",       &PREDICATE_PARAM,  BOOL,      ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("chain",     &OTHER_ITER_PARAM, ITER_ELEM, ANY_ITERATOR,      D,        BACKEND_REQUIRED),
    iter("collect",   &[],               LIST_ELEM, ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("count",     &[],               INT,       ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("cycle",     &[],               ITER_ELEM, ANY_ITERATOR,      D,        BACKEND_NOT_REQUIRED),
    iter("enumerate", &[],               IDX_PAIRS, ANY_ITERATOR,      D,        BACKEND_REQUIRED),
    iter("filter",    &PREDICATE_PARAM,  SELF,      ANY_ITERATOR,      P,        BACKEND_REQUIRED),
    iter("find",      &PREDICATE_PARAM,  OPT_ELEM,  ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("flat_map",  &TRANSFORM_PARAM,  FRESH,     ANY_ITERATOR,      D,        BACKEND_NOT_REQUIRED),
    iter("flatten",   &[],               FRESH,     ANY_ITERATOR,      D,        BACKEND_NOT_REQUIRED),
    iter("fold",      &FOLD_PARAMS,      FRESH,     ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("for_each",  &ACTION_PARAM,     UNIT,      ANY_ITERATOR,      NA,       BACKEND_REQUIRED),
    iter("join",      &SEPARATOR_PARAM,  STR,       ANY_ITERATOR,      NA,       BACKEND_NOT_REQUIRED),
    iter("last",      &[],               OPT_ELEM,  DOUBLE_ENDED_ONLY, NA,       BACKEND_NOT_REQUIRED),
    iter("map",       &TRANSFORM_PARAM,  FRESH,     ANY_ITERATOR,      P,        BACKEND_REQUIRED),
    iter("next",      &[],               NEXT,      ANY_ITERATOR,      NA,       BACKEND_NOT_REQUIRED),
    iter("next_back", &[],               NEXT,      DOUBLE_ENDED_ONLY, NA,       BACKEND_NOT_REQUIRED),
    iter("rev",       &[],               SELF,      DOUBLE_ENDED_ONLY, NA,       BACKEND_NOT_REQUIRED),
    iter("rfind",     &PREDICATE_PARAM,  OPT_ELEM,  DOUBLE_ENDED_ONLY, NA,       BACKEND_NOT_REQUIRED),
    iter("rfold",     &FOLD_PARAMS,      FRESH,     DOUBLE_ENDED_ONLY, NA,       BACKEND_NOT_REQUIRED),
    iter("skip",      &COUNT_PARAM,      ITER_ELEM, ANY_ITERATOR,      D,        BACKEND_REQUIRED),
    iter("take",      &COUNT_PARAM,      ITER_ELEM, ANY_ITERATOR,      D,        BACKEND_REQUIRED),
    iter("zip",       &OTHER_ITER_PARAM, FRESH,     ANY_ITERATOR,      D,        BACKEND_REQUIRED),
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
