//! Prelude free-function signatures — canonical source for builtin identifiers.
//!
//! These are free functions registered in the evaluator's prelude that also
//! need type signatures in the type checker. Having the canonical name and
//! signature list in the registry eliminates the sync point between
//! `infer_ident()` (type checker) and `register_prelude()` (evaluator).

use crate::{ParamDef, ReturnTag, TypeTag};

/// A builtin prelude free-function signature.
///
/// Unlike [`crate::MethodDef`] (which describes methods on types), this
/// describes standalone functions like `hash_combine`, `repeat`, and
/// type conversion functions (`int`, `float`, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PreludeFunctionDef {
    /// Function name as it appears in Ori source code.
    pub name: &'static str,
    /// Parameter definitions. Use `ReturnTag::Fresh` for generic parameters.
    pub params: &'static [ParamDef],
    /// Return type. Use `ReturnTag::Fresh` for generic returns,
    /// `ReturnTag::IteratorOf` for iterator constructors, etc.
    pub returns: ReturnTag,
}

/// Parameter for `hash_combine`: `(seed: int, value: int)`.
static HASH_COMBINE_PARAMS: [ParamDef; 2] = [
    ParamDef {
        name: "seed",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: crate::Ownership::Copy,
    },
    ParamDef {
        name: "value",
        ty: ReturnTag::Concrete(TypeTag::Int),
        ownership: crate::Ownership::Copy,
    },
];

/// Parameter for generic single-arg functions: `(value: T)`.
static GENERIC_PARAM: [ParamDef; 1] = [ParamDef {
    name: "value",
    ty: ReturnTag::Fresh,
    ownership: crate::Ownership::Borrow,
}];

/// All builtin prelude free-function signatures.
///
/// Sorted alphabetically by name. The type checker and evaluator both
/// reference this list as the canonical source for prelude function names.
pub static PRELUDE_FUNCTIONS: &[PreludeFunctionDef] = &[
    // Conversion functions: (T) -> TargetType
    PreludeFunctionDef {
        name: "bool",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Bool),
    },
    PreludeFunctionDef {
        name: "byte",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Byte),
    },
    PreludeFunctionDef {
        name: "char",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Char),
    },
    PreludeFunctionDef {
        name: "float",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Float),
    },
    // hash_combine: (int, int) -> int
    PreludeFunctionDef {
        name: "hash_combine",
        params: &HASH_COMBINE_PARAMS,
        returns: ReturnTag::Concrete(TypeTag::Int),
    },
    PreludeFunctionDef {
        name: "int",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Int),
    },
    // repeat: (T) -> Iterator<T>
    PreludeFunctionDef {
        name: "repeat",
        params: &GENERIC_PARAM,
        returns: ReturnTag::IteratorOf(crate::TypeProjection::Element),
    },
    PreludeFunctionDef {
        name: "str",
        params: &GENERIC_PARAM,
        returns: ReturnTag::Concrete(TypeTag::Str),
    },
];

/// Look up a prelude function by name.
#[must_use]
pub fn find_prelude_function(name: &str) -> Option<&'static PreludeFunctionDef> {
    PRELUDE_FUNCTIONS.iter().find(|f| f.name == name)
}

#[cfg(test)]
mod tests;
