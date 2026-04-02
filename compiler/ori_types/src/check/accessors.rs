//! Accessor methods for `ModuleChecker`.
//!
//! Provides read-only and mutable access to the checker's internal
//! components (pool, registries, arena, interner, well-known names).

use ori_ir::{ExprArena, Name, StringInterner};

use crate::{Idx, MethodRegistry, Pool, TraitRegistry, TypeRegistry};

use super::well_known::WellKnownNames;
use super::ModuleChecker;

impl<'a> ModuleChecker<'a> {
    /// Get the expression arena.
    ///
    /// Returns with the original `'a` lifetime to avoid borrowing `self`.
    /// This allows using the arena while mutably borrowing other checker fields.
    #[inline]
    pub fn arena(&self) -> &'a ExprArena {
        self.arena
    }

    /// Get the string interner.
    ///
    /// Returns with the original `'a` lifetime to avoid borrowing `self`.
    #[inline]
    pub fn interner(&self) -> &'a StringInterner {
        self.interner
    }

    /// Get the pre-interned well-known type names cache.
    #[inline]
    pub(crate) fn well_known(&self) -> &WellKnownNames {
        &self.well_known
    }

    /// Resolve a primitive type name to its fixed `Idx` via the name cache.
    #[inline]
    pub fn resolve_primitive_name(&self, name: Name) -> Option<Idx> {
        self.well_known.resolve_primitive(name)
    }

    /// Resolve a well-known generic type name via the name cache.
    ///
    /// Split borrow: reads `well_known` (immutable) and writes `pool` (mutable)
    /// from the same `&mut self`. This is safe because they're independent fields.
    #[inline]
    pub fn resolve_well_known_generic_cached(&mut self, name: Name, args: &[Idx]) -> Option<Idx> {
        self.well_known.resolve_generic(&mut self.pool, name, args)
    }

    /// Check if a name is a well-known concrete type (not a trait object).
    #[inline]
    pub fn is_well_known_concrete_cached(&self, name: Name, num_args: usize) -> bool {
        self.well_known.is_concrete(name, num_args)
    }

    /// Resolve a registration-phase primitive (Ordering, Duration, Size).
    #[inline]
    pub fn resolve_registration_primitive(&self, name: Name) -> Option<Idx> {
        self.well_known.resolve_registration_primitive(name)
    }

    /// Get the type pool.
    #[inline]
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Get mutable access to the type pool.
    #[inline]
    pub fn pool_mut(&mut self) -> &mut Pool {
        &mut self.pool
    }

    /// Get the type registry.
    #[inline]
    pub fn type_registry(&self) -> &TypeRegistry {
        &self.types
    }

    /// Get mutable access to the type registry.
    #[inline]
    pub fn type_registry_mut(&mut self) -> &mut TypeRegistry {
        &mut self.types
    }

    /// Get the trait registry.
    #[inline]
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.traits
    }

    /// Get mutable access to the trait registry.
    #[inline]
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.traits
    }

    /// Get the method registry.
    #[inline]
    pub fn method_registry(&self) -> &MethodRegistry {
        &self.methods
    }
}
