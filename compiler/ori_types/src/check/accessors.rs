//! Accessor methods for `ModuleChecker`.
//!
//! Provides read-only and mutable access to the checker's internal
//! components (pool, registries, arena, interner, well-known names).

use ori_ir::{ExprArena, Module, Name, StringInterner};

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

    /// Resolve an FFI type name to its concrete primitive Idx.
    #[inline]
    pub fn resolve_ffi_concrete(&self, name: Name) -> Option<Idx> {
        self.well_known.resolve_ffi_concrete(name)
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

    /// Pre-intern tuple types for multi-clause function scrutinees.
    ///
    /// Multi-clause functions (e.g., `@f (0) = 1; @f (n) = n * f(n-1)`)
    /// with >1 parameter are lowered to a match on a synthetic tuple
    /// scrutinee. Canonical lowering (`ori_canon`) looks up this tuple type
    /// via `pool.find_tuple()`, so it must be interned before lowering.
    ///
    /// Only targets actual multi-clause groups — single-clause functions
    /// are excluded to avoid polluting the pool with tuples that collide
    /// with type-variable-bearing tuples created during inference (e.g.,
    /// `zip()` creates `(int, Var(T))` that resolves to `(int, int)`).
    pub fn intern_multi_clause_tuples(&mut self, module: &Module) {
        let mut counts: rustc_hash::FxHashMap<Name, usize> = rustc_hash::FxHashMap::default();
        for func in &module.functions {
            *counts.entry(func.name).or_default() += 1;
        }
        for (name, count) in &counts {
            if *count > 1 {
                if let Some(sig) = self.signatures.get(name) {
                    if sig.param_types.len() > 1 {
                        self.pool.tuple(&sig.param_types);
                    }
                }
            }
        }
    }
}
