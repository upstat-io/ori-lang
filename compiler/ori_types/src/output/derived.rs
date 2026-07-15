//! Type-checker accepted compiler-generated implementation facts.

use ori_ir::{DerivedImplId, DerivedTrait, Name, Span};

use crate::{FunctionSig, Idx};

/// One derived implementation accepted by type checking and coherence.
///
/// This is the semantic source for generated Canon roots. Downstream phases
/// must not rescan raw derive attributes: absence here means the derive was not
/// accepted and therefore has no executable body.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AcceptedDerivedImpl {
    /// Stable module-local identity.
    pub id: DerivedImplId,
    /// Declared owner name, retained across concrete generic instances.
    pub owner_name: Name,
    /// Declared owner type before concrete generic substitution.
    pub owner_type: Idx,
    /// Exact registered trait type.
    pub trait_type: Idx,
    /// Closed derivation strategy identity.
    pub trait_kind: DerivedTrait,
    /// Exact registered method name.
    pub method_name: Name,
    /// Authoritative declaration-template signature.
    pub signature: FunctionSig,
    /// Source span of the derive-bearing declaration.
    pub span: Span,
}
