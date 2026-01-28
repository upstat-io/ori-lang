//! Method lookup types and caching.
//!
//! Contains the `MethodLookup` result type.

use ori_ir::Name;
use ori_types::Type;

/// Result of a method lookup.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MethodLookup {
    /// Trait providing the method (None for inherent methods).
    pub trait_name: Option<Name>,
    /// Method name.
    pub method_name: Name,
    /// Parameter types.
    pub params: Vec<Type>,
    /// Return type.
    pub return_ty: Type,
}
