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
    /// Whether this method borrows its receiver (reads without consuming).
    ///
    /// When `true`, the ARC pipeline treats the receiver as borrowed — no
    /// `RcDec` at the call site, no ownership transfer. When `false`, the
    /// receiver is consumed (ownership transferred to the callee).
    ///
    /// This is the single source of truth for builtin method ownership,
    /// consumed by `ori_arc` borrow inference and `ori_llvm` codegen.
    pub receiver_borrows: bool,
}

impl MethodDef {
    /// Create a new method definition.
    pub const fn new(
        receiver: BuiltinType,
        name: &'static str,
        params: &'static [ParamSpec],
        returns: ReturnSpec,
        trait_name: Option<&'static str>,
        receiver_borrows: bool,
    ) -> Self {
        Self {
            receiver,
            name,
            params,
            returns,
            trait_name,
            receiver_borrows,
        }
    }

    /// Create a trait method with one Self parameter returning Ordering.
    /// Receiver is borrowed (reads fields for comparison).
    const fn comparable(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "compare",
            &[ParamSpec::SelfType],
            ReturnSpec::Type(BuiltinType::Ordering),
            Some("Comparable"),
            true,
        )
    }

    /// Create an Eq trait method.
    /// Receiver is borrowed (reads fields for equality check).
    const fn eq_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "equals",
            &[ParamSpec::SelfType],
            ReturnSpec::Type(BuiltinType::Bool),
            Some("Eq"),
            true,
        )
    }

    /// Create a Clone trait method.
    /// Receiver is borrowed (reads to produce a copy).
    const fn clone_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "clone",
            &[],
            ReturnSpec::SelfType,
            Some("Clone"),
            true,
        )
    }

    /// Create a Hashable trait method.
    /// Receiver is borrowed (reads fields for hashing).
    const fn hash_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "hash",
            &[],
            ReturnSpec::Type(BuiltinType::Int),
            Some("Hashable"),
            true,
        )
    }

    /// Create a Printable trait method.
    /// Receiver is borrowed (reads to format).
    const fn to_str_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "to_str",
            &[],
            ReturnSpec::Type(BuiltinType::Str),
            Some("Printable"),
            true,
        )
    }

    /// Create a Debug trait method.
    /// Receiver is borrowed (reads to format).
    const fn debug_trait(receiver: BuiltinType) -> Self {
        Self::new(
            receiver,
            "debug",
            &[],
            ReturnSpec::Type(BuiltinType::Str),
            Some("Debug"),
            true,
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
    MethodDef::new(
        BuiltinType::Int,
        "abs",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "min",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "max",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Int,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "mul",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Mul"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Div"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "floor_div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("FloorDiv"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
        true,
    ),
    // Bitwise
    MethodDef::new(
        BuiltinType::Int,
        "bit_and",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitAnd"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_or",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitOr"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_xor",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("BitXor"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "bit_not",
        &[],
        ReturnSpec::SelfType,
        Some("BitNot"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "shl",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Shl"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Int,
        "shr",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Shr"),
        true,
    ),
    // float methods
    MethodDef::comparable(BuiltinType::Float),
    MethodDef::eq_trait(BuiltinType::Float),
    MethodDef::clone_trait(BuiltinType::Float),
    MethodDef::to_str_trait(BuiltinType::Float),
    MethodDef::debug_trait(BuiltinType::Float),
    MethodDef::new(
        BuiltinType::Float,
        "abs",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "floor",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "ceil",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "round",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "sqrt",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "min",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "max",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Float,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "mul",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Mul"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "div",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Div"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Float,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
        true,
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
        true,
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
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "is_empty",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "contains",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "starts_with",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "ends_with",
        &[ParamSpec::Str],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "to_uppercase",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "to_lowercase",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "trim",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "escape",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "add",
        &[ParamSpec::Str],
        ReturnSpec::SelfType,
        Some("Add"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "concat",
        &[ParamSpec::Str],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "replace",
        &[ParamSpec::Str, ParamSpec::Str],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Str,
        "repeat",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        None,
        true,
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
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "microseconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "milliseconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "seconds",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "minutes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "hours",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Duration,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "mul",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Mul"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "div",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Div"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Duration,
        "neg",
        &[],
        ReturnSpec::SelfType,
        Some("Neg"),
        true,
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
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "kilobytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "megabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "gigabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "terabytes",
        &[],
        ReturnSpec::Type(BuiltinType::Int),
        None,
        true,
    ),
    // Operator methods
    MethodDef::new(
        BuiltinType::Size,
        "add",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Add"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "sub",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Sub"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "mul",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Mul"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "div",
        &[ParamSpec::Int],
        ReturnSpec::SelfType,
        Some("Div"),
        true,
    ),
    MethodDef::new(
        BuiltinType::Size,
        "rem",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        Some("Rem"),
        true,
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
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_greater",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_less_or_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "is_greater_or_equal",
        &[],
        ReturnSpec::Type(BuiltinType::Bool),
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "reverse",
        &[],
        ReturnSpec::SelfType,
        None,
        true,
    ),
    MethodDef::new(
        BuiltinType::Ordering,
        "then",
        &[ParamSpec::SelfType],
        ReturnSpec::SelfType,
        None,
        true,
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

/// All builtin method names whose receiver is borrowed.
///
/// Used by `ori_arc` borrow inference to build the `borrowing_builtins` set.
/// Yields deduplicated names (multiple types may share a method name like
/// `"clone"`, but the iterator yields it once per `MethodDef`).
pub fn borrowing_method_names() -> impl Iterator<Item = &'static str> {
    BUILTIN_METHODS
        .iter()
        .filter(|m| m.receiver_borrows)
        .map(|m| m.name)
}

/// Check if a specific method borrows its receiver.
///
/// Returns `None` if the method doesn't exist in the registry.
#[must_use]
pub fn method_borrows_receiver(receiver: BuiltinType, name: &str) -> Option<bool> {
    find_method(receiver, name).map(|m| m.receiver_borrows)
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
