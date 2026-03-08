//! Top-level type definition for the Ori type registry.
//!
//! [`TypeDef`] is the complete behavioral specification of a single builtin
//! type. It aggregates identity ([`TypeTag`]), memory strategy, type arity,
//! methods, and operator strategies into one `const`-constructible record.

use crate::method::MethodDef;
use crate::operator::OpDefs;
use crate::tags::{MemoryStrategy, TypeParamArity, TypeTag};

/// Complete behavioral specification of a single builtin type.
///
/// This is the single source of truth for everything the compiler needs
/// to know about a builtin type's behavior across all phases. One
/// `TypeDef` per type, all phases read from it, no phase hard-codes
/// type knowledge independently.
///
/// Stored as `static` constants in the binary's `.rodata` segment.
/// Zero runtime cost to access.
///
/// # Extensibility
///
/// New required fields produce compile errors in every `TypeDef`
/// definition (Sections 03-07), enforcing immediate full coverage.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeDef {
    /// The type's identity tag.
    pub tag: TypeTag,

    /// The type's name as it appears in Ori source code.
    ///
    /// Lowercase for primitives (`"int"`, `"str"`), `PascalCase` for
    /// composite types (`"Duration"`, `"Option"`).
    pub name: &'static str,

    /// How values of this type are managed in memory.
    pub memory: MemoryStrategy,

    /// Number of type parameters for generic types.
    pub type_params: TypeParamArity,

    /// All methods defined on this type.
    ///
    /// Includes inherent methods, trait implementations, and associated
    /// functions. The full method set that the type checker accepts,
    /// the evaluator dispatches, and the LLVM backend emits.
    pub methods: &'static [MethodDef],

    /// Operator lowering strategies for this type.
    pub operators: OpDefs,
}

/// Describes a variant of an enum type (e.g., Ordering's Less/Equal/Greater).
///
/// Variant structure is consumed by type checker registration, not by
/// method/operator dispatch. It is NOT part of `TypeDef` — it is a
/// standalone constant defined alongside the relevant `TypeDef`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VariantSpec {
    /// Variant name as it appears in Ori source code.
    pub name: &'static str,
    /// Discriminant tag value.
    pub tag: u8,
    /// Fields on this variant (empty for unit variants).
    pub fields: &'static [(&'static str, crate::ReturnTag)],
}

#[cfg(test)]
mod tests;
