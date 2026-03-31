//! `List` type definition.
//!
//! List is an Arc-managed dynamic array (`[T]`). The largest builtin type
//! by method count (57 methods), spanning inherent operations, trait
//! implementations, and higher-order functional methods.
//!
//! Lists support COW (Copy-on-Write) mutations — `push`, `sort`, etc.
//! return a new list value. The runtime optimizes unique references
//! to mutate in-place.

use crate::{
    MemoryStrategy, MethodDef, OpDefs, OpStrategy, Ownership, ParamDef, ReturnTag, TypeDef,
    TypeParamArity, TypeProjection, TypeTag, ONE_SELF_BORROW,
};

use super::params::{
    CLOSURE_PARAM, ELEMENT_BORROW_PARAM, ELEMENT_OWNED_PARAM, INT_RANGE_PARAMS, SEPARATOR_PARAM,
};

// Parameter arrays

/// `(n: int)` — for `get`, `remove`, `take`, `skip`, `drop`, `chunk`, `window`.
static INT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "n",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

/// `(other: ?)` — for `zip` (different element type).
static FRESH_PARAM: [ParamDef; 1] = [ParamDef {
    name: "other",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Borrow,
}];

/// `(index: int, value: T)` — for `set`, `insert`.
static INDEX_ELEMENT_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "index",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: Ownership::Copy,
    },
    ParamDef {
        name: "value",
        ty: ReturnTag::ElementType,
        ownership: Ownership::Owned,
    },
];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);
const SELF: ReturnTag = ReturnTag::SelfType;
const FRESH: ReturnTag = ReturnTag::Fresh;
const OPT_ELEM: ReturnTag = ReturnTag::OptionOf(TypeProjection::Element);

// All 57 methods alphabetically sorted.
#[rustfmt::skip]
static LIST_METHODS: &[MethodDef] = &[
    MethodDef::compound("all",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("any",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("append",     &ONE_SELF_BORROW,           SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("chunk",      &INT_PARAM,            FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("clone",      &[],                   SELF,  Some("Clone"),        Ownership::Borrow, false),
    MethodDef::compound("compare",    &ONE_SELF_BORROW,           ORD,   Some("Comparable"),   Ownership::Borrow, false),
    MethodDef::compound("concat",     &ONE_SELF_BORROW,           SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("contains",   &ELEMENT_BORROW_PARAM, BOOL,  None,                Ownership::Borrow, false),
    MethodDef::compound("count",      &[],                   INT,   None,                Ownership::Borrow, false),
    MethodDef::compound("debug",      &[],                   STR,   Some("Debug"),        Ownership::Borrow, false),
    MethodDef::compound("drop",       &INT_PARAM,            SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("enumerate",  &[],                   ReturnTag::ListOfTupleIntElement, None, Ownership::Borrow, false),
    MethodDef::compound("equals",     &ONE_SELF_BORROW,           BOOL,  Some("Eq"),           Ownership::Borrow, false),
    MethodDef::compound("filter",     &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("find",       &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("first",      &[],                   OPT_ELEM, None,             Ownership::Borrow, false),
    MethodDef::compound("flat_map",   &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("flatten",    &[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("fold",       &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("for_each",   &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("get",        &INT_PARAM,            OPT_ELEM, None,             Ownership::Borrow, false),
    MethodDef::compound("group_by",   &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("hash",       &[],                   INT,   Some("Hashable"),     Ownership::Borrow, false),
    MethodDef::compound("insert",     &INDEX_ELEMENT_PARAMS,  SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("is_empty",   &[],                   BOOL,  None,                Ownership::Borrow, false),
    MethodDef::compound("iter",       &[],                   ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element), None, Ownership::Borrow, false),
    MethodDef::compound("join",       &SEPARATOR_PARAM,      STR,   None,                Ownership::Borrow, false),
    MethodDef::compound("last",       &[],                   OPT_ELEM, None,             Ownership::Borrow, false),
    MethodDef::compound("len",        &[],                   INT,   None,                Ownership::Borrow, false),
    MethodDef::compound("length",     &[],                   INT,   None,                Ownership::Borrow, false),
    MethodDef::compound("map",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("max",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("max_by",     &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("min",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("min_by",     &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("partition",  &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("pop",        &[],                   OPT_ELEM, None,             Ownership::Borrow, false),
    MethodDef::compound("prepend",    &ELEMENT_OWNED_PARAM,  SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("product",    &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("push",       &ELEMENT_OWNED_PARAM,  SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("reduce",     &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("remove",     &INT_PARAM,            SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("reverse",    &[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("set",        &INDEX_ELEMENT_PARAMS,  SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("skip",       &INT_PARAM,            SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("skip_while", &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("slice",      &INT_RANGE_PARAMS,     SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("sort",       &[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("sort_by",    &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("sort_stable",&[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("sorted",     &[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("sum",        &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("take",       &INT_PARAM,            SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("take_while", &CLOSURE_PARAM,        FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("unique",     &[],                   SELF,  None,                Ownership::Borrow, false),
    MethodDef::compound("window",     &INT_PARAM,            FRESH, None,                Ownership::Borrow, false),
    MethodDef::compound("zip",        &FRESH_PARAM,          FRESH, None,                Ownership::Borrow, false),
];

pub static LIST: TypeDef = TypeDef {
    tag: TypeTag::List,
    name: "List",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: LIST_METHODS,
    operators: OpDefs {
        add: OpStrategy::RuntimeCall {
            fn_name: "list_concat",
            returns_bool: false,
        },
        ..OpDefs::UNSUPPORTED
    },
};

#[cfg(test)]
mod tests;
