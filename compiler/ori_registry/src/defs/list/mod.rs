//! `List` type definition.
//!
//! List is an Arc-managed dynamic array (`[T]`). The largest builtin type
//! by method count, spanning inherent operations, trait implementations,
//! and higher-order functional methods.
//!
//! Lists support COW (Copy-on-Write) mutations — `push`, `sort`, etc.
//! return a new list value. The runtime optimizes unique references
//! to mutate in-place.

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, MethodRuntime, OpDefs, OpStrategy, Ownership,
    ParamDef, ReturnTag, RuntimeOperator, TypeDef, TypeParamArity, TypeProjection, TypeTag,
    ONE_SELF_BORROW,
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
const LIST_ELEM: ReturnTag = ReturnTag::ListOf(TypeProjection::Element);
const OPT_ELEM: ReturnTag = ReturnTag::OptionOf(TypeProjection::Element);
const IDX_PAIRS: ReturnTag = ReturnTag::ListOfTupleIntElement;
const DEI_ELEM: ReturnTag = ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element);
const BACKEND_REQUIRED: BackendRequirement = BackendRequirement::Required;
const BACKEND_NOT_REQUIRED: BackendRequirement = BackendRequirement::NotRequired;

// All methods alphabetically sorted.
#[rustfmt::skip]
static LIST_METHODS: &[MethodDef] = &[
    //                  name           params                 returns    trait               receiver           backend
    MethodDef::compound("all",         &CLOSURE_PARAM,        BOOL,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("any",         &CLOSURE_PARAM,        BOOL,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("append",      &ONE_SELF_BORROW,      SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("chunk",       &INT_PARAM,            FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("clone",       &[],                   SELF,      Some("Clone"),      Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("compare",     &ONE_SELF_BORROW,      ORD,       Some("Comparable"), Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("concat",      &ONE_SELF_BORROW,      SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("contains",    &ELEMENT_BORROW_PARAM, BOOL,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("count",       &[],                   INT,       None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("debug",       &[],                   STR,       Some("Debug"),      Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("drop",        &INT_PARAM,            SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("enumerate",   &[],                   IDX_PAIRS, None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("equals",      &ONE_SELF_BORROW,      BOOL,      Some("Eq"),         Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("filter",      &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("find",        &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("first",       &[],                   OPT_ELEM,  None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("flat_map",    &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("flatten",     &[],                   FRESH,     None,               Ownership::Borrow, BACKEND_REQUIRED),
    MethodDef::compound("fold",        &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("for_each",    &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("get",         &INT_PARAM,            OPT_ELEM,  None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("group_by",    &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("hash",        &[],                   INT,       Some("Hashable"),   Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("insert",      &INDEX_ELEMENT_PARAMS, SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListInsert),
    MethodDef::compound("is_empty",    &[],                   BOOL,      Some("IsEmpty"),    Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("iter",        &[],                   DEI_ELEM,  Some("Iterable"),   Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("join",        &SEPARATOR_PARAM,      STR,       None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("last",        &[],                   OPT_ELEM,  None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("len",         &[],                   INT,       Some("Len"),        Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::Length),
    MethodDef::compound("length",      &[],                   INT,       Some("Len"),        Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::Length),
    MethodDef::compound("map",         &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("max",         &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("max_by",      &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("min",         &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("min_by",      &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("partition",   &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("pop",         &[],                   OPT_ELEM,  None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("prepend",     &ELEMENT_OWNED_PARAM,  SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListPrepend),
    MethodDef::compound("product",     &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("push",        &ELEMENT_OWNED_PARAM,  SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListPush),
    MethodDef::compound("reduce",      &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("remove",      &INT_PARAM,            SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListRemove),
    MethodDef::compound("reverse",     &[],                   SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("set",         &INDEX_ELEMENT_PARAMS, SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListSet),
    MethodDef::compound("skip",        &INT_PARAM,            SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("skip_while",  &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("slice",       &INT_RANGE_PARAMS,     SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("sort",        &[],                   SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("sort_by",     &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("sort_stable", &[],                   SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("sorted",      &[],                   SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("sum",         &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("take",        &INT_PARAM,            SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("take_while",  &CLOSURE_PARAM,        FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("to_dynamic",  &[],                   LIST_ELEM, None,               Ownership::Owned,  BACKEND_NOT_REQUIRED),
    MethodDef::compound("to_fixed",    &[],                   LIST_ELEM, None,               Ownership::Owned,  BACKEND_NOT_REQUIRED),
    MethodDef::compound("to_str",      &[],                   STR,       Some("Printable"),  Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("unique",      &[],                   SELF,      None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("updated",     &INDEX_ELEMENT_PARAMS, SELF,      Some("IndexSet"),   Ownership::Borrow, BACKEND_NOT_REQUIRED).with_runtime(MethodRuntime::ListSet),
    MethodDef::compound("window",      &INT_PARAM,            FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
    MethodDef::compound("zip",         &FRESH_PARAM,          FRESH,     None,               Ownership::Borrow, BACKEND_NOT_REQUIRED),
];

/// Builtin `[T]` list type definition: methods, operators, memory strategy.
pub static LIST: TypeDef = TypeDef {
    tag: TypeTag::List,
    name: "List",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(1),
    methods: LIST_METHODS,
    operators: OpDefs {
        add: OpStrategy::RuntimeCall(RuntimeOperator::ListConcat),
        ..OpDefs::UNSUPPORTED
    },
    traits: &["Printable"],
};

#[cfg(test)]
mod tests;
