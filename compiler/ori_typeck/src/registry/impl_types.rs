//! Implementation definition types.
//!
//! Contains types for representing impl blocks in the registry.

use ori_ir::{Name, Span, TypeId};
use ori_types::Type;

/// Implementation method.
///
/// Parameter and return types are stored as `TypeId` for efficient equality comparisons.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplMethodDef {
    /// Method name.
    pub name: Name,
    /// Parameter types.
    pub params: Vec<TypeId>,
    /// Return type.
    pub return_ty: TypeId,
}

/// Associated type definition in an impl block (e.g., `type Item = T`).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplAssocTypeDef {
    /// Associated type name (e.g., `Item`).
    pub name: Name,
    /// Concrete type assigned (e.g., `T` or `int`), stored as `TypeId`.
    pub ty: TypeId,
}

/// Entry for an implementation block.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplEntry {
    /// The trait being implemented (None for inherent impl).
    pub trait_name: Option<Name>,
    /// The type implementing the trait.
    pub self_ty: Type,
    /// Source location.
    pub span: Span,
    /// Generic parameters.
    pub type_params: Vec<Name>,
    /// Methods in this impl block.
    pub methods: Vec<ImplMethodDef>,
    /// Associated type definitions (e.g., `type Item = T`).
    pub assoc_types: Vec<ImplAssocTypeDef>,
}

/// Error when coherence rules are violated.
#[derive(Clone, Debug)]
pub struct CoherenceError {
    /// Description of the conflict.
    pub message: String,
    /// Span of the conflicting impl.
    pub span: Span,
    /// Span of the existing impl.
    pub existing_span: Span,
}
