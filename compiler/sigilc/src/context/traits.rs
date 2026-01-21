// Context traits for Sigil compiler phases
//
// These traits abstract over phase-specific contexts, allowing patterns
// and other components to work uniformly across type checking, lowering,
// and evaluation phases.

use crate::ast::TypeExpr;

/// Trait for looking up type information.
///
/// Implemented by contexts that track type definitions.
pub trait TypeLookup {
    /// Look up a type definition by name.
    fn lookup_type(&self, name: &str) -> Option<&crate::ast::TypeDef>;

    /// Check if a type exists.
    fn has_type(&self, name: &str) -> bool {
        self.lookup_type(name).is_some()
    }
}

/// Trait for looking up function signatures (type checking) or definitions (evaluation).
///
/// This is parameterized by the function info type since type checking uses
/// signatures while evaluation uses full definitions.
pub trait FunctionLookup<F> {
    /// Look up a function by name.
    fn lookup_function(&self, name: &str) -> Option<&F>;

    /// Check if a function exists.
    fn has_function(&self, name: &str) -> bool {
        self.lookup_function(name).is_some()
    }
}

/// Trait for looking up configuration values.
///
/// For type checking, this returns types. For evaluation, this returns values.
pub trait ConfigLookup<T> {
    /// Look up a config variable by name.
    fn lookup_config(&self, name: &str) -> Option<&T>;

    /// Check if a config variable exists.
    fn has_config(&self, name: &str) -> bool {
        self.lookup_config(name).is_some()
    }
}

/// Trait for variable scope management.
///
/// Implemented by contexts that track local variables.
pub trait VariableScope {
    /// The type of variable bindings (TypeExpr for checking, Value for eval).
    type Binding;

    /// Define a new variable in the current scope.
    fn define_variable(&mut self, name: String, binding: Self::Binding, mutable: bool);

    /// Look up a variable by name.
    fn lookup_variable(&self, name: &str) -> Option<&Self::Binding>;

    /// Check if a variable exists.
    fn has_variable(&self, name: &str) -> bool {
        self.lookup_variable(name).is_some()
    }

    /// Check if a variable is mutable.
    fn is_variable_mutable(&self, name: &str) -> Option<bool>;
}

/// Trait for scoped execution with automatic cleanup.
///
/// Allows entering and exiting scopes with RAII semantics.
pub trait ScopedContext {
    /// The guard type returned when entering a scope.
    type Guard<'a>
    where
        Self: 'a;

    /// Enter a new scope and return a guard that restores on drop.
    fn enter_scope(&mut self) -> Self::Guard<'_>;
}

/// Trait for contexts that track the current function's return type.
///
/// Used by recursive patterns to know the expected return type.
pub trait ReturnTypeContext {
    /// Get the current function's expected return type.
    fn current_return_type(&self) -> Option<TypeExpr>;

    /// Set the current function's return type.
    fn set_return_type(&mut self, ty: TypeExpr);

    /// Clear the current function's return type.
    fn clear_return_type(&mut self);
}

/// Marker trait for read-only context access.
///
/// Contexts implementing this provide immutable access patterns.
pub trait ReadOnlyContext: TypeLookup {}

/// Combined trait for full type checking context.
///
/// Provides all capabilities needed for type checking.
pub trait CheckingContext:
    TypeLookup
    + FunctionLookup<crate::types::FunctionSig>
    + ConfigLookup<TypeExpr>
    + VariableScope<Binding = TypeExpr>
    + ReturnTypeContext
{
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify traits are object-safe where applicable
    fn _assert_type_lookup_object_safe(_: &dyn TypeLookup) {}
}
