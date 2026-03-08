//! `Error` type definition.
//!
//! Error is an Arc type (heap-allocated, reference-counted) containing a
//! message string and optional trace. No operators. All methods are
//! `backend_required: false` (no LLVM coverage yet). `trace_entries` and
//! `with_trace` use `ReturnTag::Fresh` for `TraceEntry` (stdlib struct, no
//! `TypeTag`).

use crate::{
    MemoryStrategy, MethodDef, OpDefs, Ownership, ParamDef, ReturnTag, TypeDef, TypeParamArity,
    TypeTag,
};

// Shared parameter arrays

/// `(entry: TraceEntry)` — for `with_trace`.
static TRACE_ENTRY_PARAM: [ParamDef; 1] = [ParamDef {
    name: "entry",
    ty: ReturnTag::Fresh,
    ownership: Ownership::Owned,
}];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;

// All 8 methods alphabetically sorted.
static ERROR_METHODS: &[MethodDef] = &[
    MethodDef::compound("clone", &[], SELF, Some("Clone"), Ownership::Borrow, false),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, false),
    MethodDef::compound(
        "has_trace",
        &[],
        BOOL,
        Some("Traceable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("message", &[], STR, None, Ownership::Borrow, false),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "trace",
        &[],
        STR,
        Some("Traceable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "trace_entries",
        &[],
        ReturnTag::Fresh,
        Some("Traceable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "with_trace",
        &TRACE_ENTRY_PARAM,
        SELF,
        Some("Traceable"),
        Ownership::Borrow,
        false,
    ),
];

pub static ERROR: TypeDef = TypeDef {
    tag: TypeTag::Error,
    name: "Error",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(0),
    methods: ERROR_METHODS,
    operators: OpDefs::UNSUPPORTED,
};

#[cfg(test)]
mod tests;
