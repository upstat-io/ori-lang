//! Shared parameter definitions used by multiple type def modules.
//!
//! Parameters that appear identically in 2+ def files are extracted here
//! to eliminate duplication and ensure consistency. Each constant documents
//! which types use it.

use crate::{Ownership, ParamDef, ReturnTag, TypeTag};

/// `(f: closure)` — higher-order closure param.
///
/// Used by: List, Option, Result.
pub static CLOSURE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "f",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

/// `(count: int)` — integer count param.
///
/// Used by: Iterator (`take`, `skip`), str (`repeat`).
pub static COUNT_PARAM: [ParamDef; 1] = [ParamDef {
    name: "count",
    ty: ReturnTag::Concrete(TypeTag::Int),
    ownership: Ownership::Copy,
}];

/// `(separator: str)` — string separator param.
///
/// Used by: List (`join`), Iterator (`join`).
pub static SEPARATOR_PARAM: [ParamDef; 1] = [ParamDef {
    name: "separator",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(value: T)` — element param, borrowed.
///
/// Used by: List (`contains`), Set (`contains`, `remove`).
pub static ELEMENT_BORROW_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Borrow,
}];

/// `(value: T)` — element param, owned.
///
/// Used by: List (`push`, `prepend`), Set (`insert`).
pub static ELEMENT_OWNED_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::ElementType,
    ownership: Ownership::Owned,
}];

/// `(message: str)` — string message param.
///
/// Used by: Option (`expect`), Result (`expect`, `expect_err`).
pub static MESSAGE_PARAM: [ParamDef; 1] = [ParamDef {
    name: "message",
    ty: ReturnTag::Concrete(TypeTag::Str),
    ownership: Ownership::Borrow,
}];

/// `(start: int, end: int)` — integer range params.
///
/// Used by: List (`slice`), str (`slice`, `substring`).
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
