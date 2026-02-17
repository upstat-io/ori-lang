//! Shared method metadata registry for built-in types.
//!
//! This module provides a single source of truth for built-in method signatures,
//! eliminating the need to maintain separate registries in typeck and eval.
//!
//! # Design
//!
//! Each built-in method is described by a `MethodDef` that specifies:
//! - The receiver type
//! - Method name
//! - Parameter types
//! - Return type
//! - Optional trait association
//!
//! # Usage
//!
//! ```ignore
//! use ori_ir::builtin_methods::{find_method, BuiltinType};
//!
//! if let Some(method) = find_method(BuiltinType::Int, "compare") {
//!     assert_eq!(method.returns, ReturnSpec::Ordering);
//!     assert_eq!(method.trait_name, Some("Comparable"));
//! }
//! ```

use crate::BuiltinType;

/// Specification for a method parameter.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParamSpec {
    /// Parameter has the same type as Self (receiver type).
    SelfType,
    /// Integer parameter.
    Int,
    /// String parameter.
    Str,
    /// Boolean parameter.
    Bool,
    /// Any type (for generic methods - the type checker handles this).
    Any,
    /// A closure/function parameter (for methods like map, filter).
    Closure,
}

/// Specification for a method's return type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ReturnSpec {
    /// Returns the same type as Self.
    SelfType,
    /// Returns a specific builtin type.
    Type(BuiltinType),
    /// Returns void/unit.
    Void,
    /// Returns the element type (for container methods).
    ElementType,
    /// Returns Option of the element type.
    OptionElement,
    /// Returns a list of the element type.
    ListElement,
    /// Returns the inner type (for Option/Result unwrap).
    InnerType,
}

/// Definition of a built-in method.
#[derive(Clone, Debug)]
pub struct MethodDef {
    /// The receiver type this method is defined on.
    pub receiver: BuiltinType,
    /// The method name.
    pub name: &'static str,
    /// The parameter specifications (excluding self).
    pub params: &'static [ParamSpec],
    /// The return type specification.
    pub returns: ReturnSpec,
    /// The trait this method belongs to, if any.
    pub trait_name: Option<&'static str>,
}

impl MethodDef {
    /// Create a new method definition.
    const fn new(
        receiver: BuiltinType,
        name: &'static str,
        params: &'static [ParamSpec],
        returns: ReturnSpec,
        trait_name: Option<&'static str>,
    ) -> Self {
        Self {
            receiver,
            name,
            params,
            returns,
            trait_name,
        }
    }

    /// Create a trait method with one Self parameter returning Ordering.
    const fn comparable(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "compare",
            &[ParamSpec::SelfType],
            ReturnSpec::Type(BuiltinType::Ordering),
            Some("Comparable"),
        )
    }

    /// Create an Eq trait method.
    const fn eq_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "equals",
            &[ParamSpec::SelfType],
            ReturnSpec::Type(BuiltinType::Bool),
            Some("Eq"),
        )
    }

    /// Create a Clone trait method.
    const fn clone_trait(receiver: BuiltinType) -> Self {
        Self::new(receiver, "clone", &[], ReturnSpec::SelfType, Some("Clone"))
    }

    /// Create a Hashable trait method.
    const fn hash_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "hash",
            &[],
            ReturnSpec::Type(BuiltinType::Int),
            Some("Hashable"),
        )
    }

    /// Create a Printable trait method.
    const fn to_str_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "to_str",
            &[],
            ReturnSpec::Type(BuiltinType::Str),
            Some("Printable"),
        )
    }

    /// Create a Debug trait method.
    const fn debug_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "debug",
            &[],
            ReturnSpec::Type(BuiltinType::Str),
            Some("Debug"),
        )
    }
}

/// All built-in methods for primitive types.
///
/// This is the single source of truth for which methods exist on which types.
/// The registry is organized by type for easy lookup.
pub static BUILTIN_METHODS: &[MethodDef] = &[
    // int methods
    MethodDef::comparable(BuiltinType::Int),
    MethodDef::eq_trait(BuiltinType::Int),
    MethodDef::clone_trait(BuiltinType::Int),
    MethodDef::hash_trait(BuiltinType::Int),
    MethodDef::to_str_trait(BuiltinType::Int),
    MethodDef::debug_trait(BuiltinType::Int),
    MethodDef::new(BuiltinType::Int, "abs", &[], ReturnSpec::SelfType, None),
    MethodDef::new(
        BuiltinType::Int,
        "min",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "max",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Int,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "mul",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Mul"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Div"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "floor_div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("FloorDiv"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
    ),
    // Bitwise
    MethodDef::new(
        BuiltinType::Int,
        "bit_and",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitAnd"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_or",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitOr"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_xor",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitXor"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_not",
        &[],
        ReturnSpec::SelfType,
        Some("BitNot"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "shl",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Shl"),
    ),
    MethodDef::new(
        BuiltinType::Int,
        "shr",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Shr"),
    ),
    // float methods
    MethodDef::comparable(BuiltinType::Float),
    MethodDef::eq_trait(BuiltinType::Float),
    MethodDef::clone_trait(BuiltinType::Float),
    MethodDef::to_str_trait(BuiltinType::Float),
    MethodDef::debug_trait(BuiltinType::Float),
    MethodDef::new(BuiltinType::Float, "abs", &[], ReturnSpec::SelfType, None),
    MethodDef::new(BuiltinType::Float, "floor", &[], ReturnSpec::SelfType, None),
    MethodDef::new(BuiltinType::Float, "ceil", &[], ReturnSpec::SelfType, None),
    MethodDef::new(BuiltinType::Float, "round", &[], ReturnSpec::SelfType, None),
    MethodDef::new(BuiltinType::Float, "sqrt", &[], ReturnSpec::SelfType, None),
    MethodDef::new(
        BuiltinType::Float,
        "min",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "max",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Float,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
    ),
    MethodDef::new(
        BuiltinType::Float,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
    ),
    MethodDef::new(
        BuiltinType::Float,
        "mul",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Mul"),
    ),
    MethodDef::new(
        BuiltinType::Float,
        "div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Div"),
    ),
    MethodDef::new(
        BuiltinType::Float,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
    ),
    // bool methods
    MethodDef::comparable(BuiltinType::Bool),
    MethodDef::eq_trait(BuiltinType::Bool),
    MethodDef::clone_trait(BuiltinType::Bool),
    MethodDef::hash_trait(BuiltinType::Bool),
    MethodDef::to_str_trait(BuiltinType::Bool),
    MethodDef::debug_trait(BuiltinType::Bool),
    MethodDef::new(
        BuiltinType::Bool,
        "not",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        Some("Not"),
    ),
    // char methods
    MethodDef::comparable(BuiltinType::Char),
    MethodDef::eq_trait(BuiltinType::Char),
    MethodDef::clone_trait(BuiltinType::Char),
    MethodDef::hash_trait(BuiltinType::Char),
    MethodDef::to_str_trait(BuiltinType::Char),
    MethodDef::debug_trait(BuiltinType::Char),
    // byte methods
    MethodDef::comparable(BuiltinType::Byte),
    MethodDef::eq_trait(BuiltinType::Byte),
    MethodDef::clone_trait(BuiltinType::Byte),
    MethodDef::hash_trait(BuiltinType::Byte),
    MethodDef::to_str_trait(BuiltinType::Byte),
    MethodDef::debug_trait(BuiltinType::Byte),
    // str methods
    MethodDef::comparable(BuiltinType::Str),
    MethodDef::eq_trait(BuiltinType::Str),
    MethodDef::clone_trait(BuiltinType::Str),
    MethodDef::hash_trait(BuiltinType::Str),
    MethodDef::debug_trait(BuiltinType::Str),
    MethodDef::new(
        BuiltinType::Str,
        "len",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "is_empty",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "contains",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "starts_with",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "ends_with",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "to_uppercase",
        &[],
        ReturnSpec::SelfType,
        None,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "to_lowercase",
        &[],
        ReturnSpec::SelfType,
        None,
    ),
    MethodDef::new(BuiltinType::Str, "trim", &[], ReturnSpec::SelfType, None),
    MethodDef::new(
        BuiltinType::Str,
        "add",
        &[ParamSpec::Str],
        ReturnSpec::SelfType,
        Some("Add"),
    ),
    MethodDef::new(
        BuiltinType::Str,
        "concat",
        &[ParamSpec::Str],
        ReturnSpec::SelfType,
        None,
    ),
    // Duration methods
    MethodDef::comparable(BuiltinType::Duration),
    MethodDef::eq_trait(BuiltinType::Duration),
    MethodDef::clone_trait(BuiltinType::Duration),
    MethodDef::hash_trait(BuiltinType::Duration),
    MethodDef::to_str_trait(BuiltinType::Duration),
    MethodDef::debug_trait(BuiltinType::Duration),
    MethodDef::new(
        BuiltinType::Duration,
        "nanoseconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "microseconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "milliseconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "seconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "minutes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "hours",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Duration,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "mul",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Mul"),
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "div",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Div"),
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
    ),
    // Size methods
    MethodDef::comparable(BuiltinType::Size),
    MethodDef::eq_trait(BuiltinType::Size),
    MethodDef::clone_trait(BuiltinType::Size),
    MethodDef::hash_trait(BuiltinType::Size),
    MethodDef::to_str_trait(BuiltinType::Size),
    MethodDef::debug_trait(BuiltinType::Size),
    MethodDef::new(
        BuiltinType::Size,
        "bytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "kilobytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "megabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "gigabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "terabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Size,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
    ),
    MethodDef::new(
        BuiltinType::Size,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
    ),
    MethodDef::new(
        BuiltinType::Size,
        "mul",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Mul"),
    ),
    MethodDef::new(
        BuiltinType::Size,
        "div",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Div"),
    ),
    MethodDef::new(
        BuiltinType::Size,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
    ),
    // Ordering methods
    MethodDef::comparable(BuiltinType::Ordering),
    MethodDef::eq_trait(BuiltinType::Ordering),
    MethodDef::clone_trait(BuiltinType::Ordering),
    MethodDef::hash_trait(BuiltinType::Ordering),
    MethodDef::to_str_trait(BuiltinType::Ordering),
    MethodDef::debug_trait(BuiltinType::Ordering),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_less",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_greater",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_less_or_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_greater_or_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "reverse",
        &[],
        ReturnSpec::SelfType,
        None,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "then",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
    ),
];

/// Find a method definition by receiver type and method name.
///
/// Returns `Some(&MethodDef)` if found, `None` otherwise.
#[must_use]
pub fn find_method(receiver: BuiltinType, name: &str) -> Option<&'static MethodDef> {
    BUILTIN_METHODS
        .iter()
        .find(|m| m.receiver == receiver && m.name == name)
}

/// Get all methods for a given receiver type.
///
/// Returns an iterator over all methods defined on the type.
pub fn methods_for(receiver: BuiltinType) -> impl Iterator<Item = &'static MethodDef> {
    BUILTIN_METHODS
        .iter()
        .filter(move |m| m.receiver == receiver)
}

/// Check if a method exists for a given receiver type.
#[must_use]
pub fn has_method(receiver: BuiltinType, name: &str) -> bool {
    find_method(receiver, name).is_some()
}

/// Get all method names for a given receiver type.
///
/// Useful for generating "did you mean?" suggestions.
pub fn method_names_for(receiver: BuiltinType) -> impl Iterator<Item = &'static str> {
    methods_for(receiver).map(|m| m.name)
}

#[cfg(test)]
mod tests;
