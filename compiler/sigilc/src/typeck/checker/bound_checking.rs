//! Trait Bound Checking
//!
//! Verifies that types satisfy trait bounds at call sites.

use crate::ir::{Name, Span};
use crate::types::Type;
use super::TypeChecker;

impl<'a> TypeChecker<'a> {
    /// Check if a type satisfies a trait bound.
    ///
    /// Returns true if the type implements the trait, false otherwise.
    /// This uses the trait registry to check for implementations.
    #[allow(dead_code)]
    pub(crate) fn type_satisfies_bound(&self, ty: &Type, trait_path: &[Name]) -> bool {
        // Get the trait name (last segment of path)
        let trait_name = match trait_path.last() {
            Some(name) => *name,
            None => return false,
        };

        // Check if the type implements the trait
        self.trait_registry.implements(ty, trait_name)
    }

    /// Check trait bounds for a function call.
    ///
    /// Given a function's generic bounds and the resolved types from a call,
    /// verifies that the types satisfy all required trait bounds.
    ///
    /// NOTE: Full constraint checking requires parser changes to preserve
    /// type annotation names (e.g., knowing that a param was annotated `: T`
    /// where `T` is a generic parameter). The current implementation stores
    /// bounds but cannot fully enforce them without that connection.
    ///
    /// For now, this is a stub that will be enhanced when the parser
    /// preserves type annotation names.
    #[allow(dead_code)]
    pub(crate) fn check_function_bounds(
        &mut self,
        _func_name: Name,
        _resolved_args: &[Type],
        _span: Span,
    ) {
        // TODO: Implement full constraint checking when parser preserves type names.
        //
        // The full implementation would:
        // 1. Look up the function's generics from function_sigs
        // 2. Map resolved types back to generic parameters
        // 3. Check that each resolved type satisfies its generic's bounds
        //
        // Currently blocked because the parser converts type annotations like `: T`
        // to `TypeId::INFER`, losing the information that `T` was used.
    }
}
