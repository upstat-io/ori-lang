//! `Map` type definition.
//!
//! Map is an Arc-managed key-value collection (`{K: V}` / `Map<K, V>`)
//! where keys must be `Hashable`. It has two type parameters (`Fixed(2)`).
//!
//! Map iteration produces `Iterator<(K, V)>` tuples (not DEI — hash maps
//! have no inherent ordering). Methods that return projected types use
//! `KeyType` and `ValueType` projections.

use crate::{
    BackendRequirement, MemoryStrategy, MethodDef, MethodRuntime, OpDefs, Ownership, ParamDef,
    ReturnTag, TypeDef, TypeParamArity, TypeProjection, TypeTag, ONE_SELF_BORROW,
};

// Parameter arrays

/// `(key: K)` — for `get`, `contains_key`, `remove`.
static KEY_BORROW_PARAM: [ParamDef; 1] = [ParamDef {
    name: "key",
    ty: ReturnTag::KeyType,
    ownership: Ownership::Borrow,
}];

/// `(key: K, value: V)` — for `insert`, `updated`.
static INSERT_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "key",
        ty: ReturnTag::KeyType,
        ownership: Ownership::Owned,
    },
    ParamDef {
        name: "value",
        ty: ReturnTag::ValueType,
        ownership: Ownership::Owned,
    },
];

/// `(key: K, f: closure)` — for `update`.
static UPDATE_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "key",
        ty: ReturnTag::KeyType,
        ownership: Ownership::Owned,
    },
    ParamDef {
        name: "f",
        ty: ReturnTag::Fresh,
        ownership: Ownership::Owned,
    },
];

// Helper aliases
const BOOL: ReturnTag = ReturnTag::Concrete(TypeTag::Bool);
const INT: ReturnTag = ReturnTag::Concrete(TypeTag::Int);
const STR: ReturnTag = ReturnTag::Concrete(TypeTag::Str);
const SELF: ReturnTag = ReturnTag::SelfType;

// All methods alphabetically sorted.
static MAP_METHODS: &[MethodDef] = &[
    MethodDef::compound(
        "clone",
        &[],
        SELF,
        Some("Clone"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "contains",
        &KEY_BORROW_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "contains_key",
        &KEY_BORROW_PARAM,
        BOOL,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "debug",
        &[],
        STR,
        Some("Debug"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "entries",
        &[],
        ReturnTag::ListKeyValue,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "equals",
        &ONE_SELF_BORROW,
        BOOL,
        Some("Eq"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "get",
        &KEY_BORROW_PARAM,
        ReturnTag::OptionOf(TypeProjection::Value),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "hash",
        &[],
        INT,
        Some("Hashable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "insert",
        &INSERT_PARAMS,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "is_empty",
        &[],
        BOOL,
        Some("IsEmpty"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "iter",
        &[],
        ReturnTag::MapIterator,
        Some("Iterable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "keys",
        &[],
        ReturnTag::ListOf(TypeProjection::Key),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "len",
        &[],
        INT,
        Some("Len"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Length),
    MethodDef::compound(
        "length",
        &[],
        INT,
        Some("Len"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    )
    .with_runtime(MethodRuntime::Length),
    MethodDef::compound(
        "merge",
        &ONE_SELF_BORROW,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "remove",
        &KEY_BORROW_PARAM,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "to_str",
        &[],
        STR,
        Some("Printable"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "update",
        &UPDATE_PARAMS,
        SELF,
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "updated",
        &INSERT_PARAMS,
        SELF,
        Some("IndexSet"),
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
    MethodDef::compound(
        "values",
        &[],
        ReturnTag::ListOf(TypeProjection::Value),
        None,
        Ownership::Borrow,
        BackendRequirement::NotRequired,
    ),
];

pub static MAP: TypeDef = TypeDef {
    tag: TypeTag::Map,
    name: "Map",
    memory: MemoryStrategy::Arc,
    type_params: TypeParamArity::Fixed(2),
    methods: MAP_METHODS,
    operators: OpDefs::UNSUPPORTED,
    traits: &["Printable"],
};

#[cfg(test)]
mod tests;
