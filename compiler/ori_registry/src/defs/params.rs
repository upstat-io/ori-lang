//! Shared parameter definitions used by multiple type def modules.
//!
//! Parameters that appear identically in 2+ def files are extracted here
//! to eliminate duplication and ensure consistency. Each constant documents
//! which types use it.

use crate::{Ownership, ParamDef, ReturnTag, TypeTag};

/// `(f: closure)` — higher-order callback borrowed for the duration of the call.
/// A callee that stores the callback must retain its own closure-environment credit.
pub static CLOSURE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "f",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Borrow,
}];

/// `(count: int)` — integer count param.
pub static COUNT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "count",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

/// `(separator: str)` — string separator param.
pub static SEPARATOR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "separator",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(value: T)` — element param, borrowed.
pub static ELEMENT_BORROW_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Borrow,
}];

/// `(value: T)` — element param, owned.
pub static ELEMENT_OWNED_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Owned,
}];

/// `(message: str)` — string message param.
pub static MESSAGE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "message",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(start: int, end: int)` — integer range params.
pub static INT_RANGE_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "start",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: Ownership::Copy,
    },
    ParamDef {
        name: "end",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: Ownership::Copy,
    },
];
