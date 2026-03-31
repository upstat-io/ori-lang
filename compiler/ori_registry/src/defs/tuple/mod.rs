//! `Tuple` type definition.
//!
//! Tuple is a fixed-size heterogeneous container with Structural memory
//! strategy — its memory management depends on the types of its elements.
//! A `(int, bool)` is Copy; a `(str, [int])` contains Arc types and
//! needs retain/release.
//!
//! Tuple uses `TypeParamArity::Variadic` because it can hold 0-N elements
//! of different types. Field access (`.0`, `.1`) is handled by the parser
//! and type checker, not by the method system.
//!
//! Tuple includes trait methods (Eq, Comparable, Hashable, Clone, Debug)
//! because the type checker resolves them directly via structural dispatch
//! rather than through the general trait machinery.

use crate::{
    MemoryStrategy, MethodDef, OpDefs, Ownership, ReturnTag, TypeDef, TypeParamArity, TypeTag,
    ONE_SELF_OWNED,
};

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const ORD: ReturnTag = ReturnTag::Concrete(TypeTag::Ordering);

// All 6 methods alphabetically sorted.
static TUPLE_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "clone",
        &[],
        ReturnTag::SelfType,
        Some("Clone"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound(
        "compare",
        &ONE_SELF_OWNED,
        ORD,
        Some("Comparable"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("debug", &[], STR, Some("Debug"), Ownership::Borrow, false),
    MethodDef::compound(
        "equals",
        &ONE_SELF_OWNED,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        false,
    ),
    MethodDef::compound("hash", &[], INT, Some("Hashable"), Ownership::Borrow, false),
    MethodDef::compound("len", &[], INT, Some("Len"), Ownership::Borrow, false),
];

pub static TUPLE: TypeDef = TypeDef {
    tag: TypeTag::Tuple,
    name: "Tuple",
    memory: MemoryStrategy::Structural,
    type_params: TypeParamArity::Variadic,
    methods: TUPLE_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &[],
};

#[cfg(test)]
mod tests;
