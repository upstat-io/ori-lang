//! Type-specific accessor methods for [`Pool`].
//!
//! Provides read access to compound type structures (functions, tuples, structs,
//! enums, etc.) stored in the pool's extra array.
//!
//! # Submodules
//!
//! - `resolution` — resolution chain following (`resolve`, `resolve_fully`,
//!   `chase_var_links`, `resolve_applied_via_matching_args`, `var_idx_for_id`)
//!   plus Named/newtype resolution registration.
//! - `nominal` — accessors for nominal user-defined types (`Struct`, `Enum`).
//!
//! This `mod.rs` holds the pure compound-type accessors (function / tuple /
//! map / result / borrowed / simple-container / scheme / applied / named).

mod nominal;
mod resolution;

use crate::{Idx, LifetimeId, Tag};

use super::Pool;

impl Pool {
    // === Function Accessors ===

    /// Get function parameter count.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_param_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx] as usize
    }

    /// Get a function parameter type by index.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type or if `param_idx` is out of bounds.
    pub fn function_param(&self, idx: Idx, param_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        debug_assert!(param_idx < count);
        Idx::from_raw(self.extra[extra_idx + 1 + param_idx])
    }

    /// Get function parameter types as a Vec.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_params(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 1 + i]))
            .collect()
    }

    /// Get function return type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_return(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx::from_raw(self.extra[extra_idx + 1 + count])
    }

    // === Tuple Accessors ===

    /// Get tuple element count.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type.
    pub fn tuple_elem_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx] as usize
    }

    /// Get a tuple element type by index.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type or if `elem_idx` is out of bounds.
    pub fn tuple_elem(&self, idx: Idx, elem_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        debug_assert!(elem_idx < count);
        Idx::from_raw(self.extra[extra_idx + 1 + elem_idx])
    }

    /// Get tuple element types as a Vec.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type.
    pub fn tuple_elems(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 1 + i]))
            .collect()
    }

    /// Look up an existing tuple type without creating it.
    ///
    /// Returns `Some(idx)` if the tuple `(elems...)` was previously interned
    /// (e.g. by the type checker), `None` otherwise.
    pub fn find_tuple(&self, elems: &[Idx]) -> Option<Idx> {
        if elems.is_empty() {
            return Some(Idx::UNIT);
        }
        let mut extra = Vec::with_capacity(elems.len() + 1);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count fits u32 — pool layout uses u32 words"
        )]
        extra.push(elems.len() as u32);
        for &e in elems {
            extra.push(e.raw());
        }
        let hash = self.merkle_hash(Tag::Tuple, 0, &extra);
        self.intern_map.get(&hash).copied()
    }

    // === Map/Result Accessors ===

    /// Get map key type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Map type.
    pub fn map_key(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Map);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get map value type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Map type.
    pub fn map_value(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Map);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx + 1])
    }

    /// Get result ok type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Result type.
    pub fn result_ok(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Result);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get result error type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Result type.
    pub fn result_err(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Result);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx + 1])
    }

    // === Borrowed & Container Accessors ===

    /// Get the inner type of a borrowed reference.
    ///
    /// For `&T`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Borrowed type.
    pub fn borrowed_inner(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Borrowed);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get the lifetime of a borrowed reference.
    ///
    /// # Panics
    /// Panics if `idx` is not a Borrowed type.
    pub fn borrowed_lifetime(&self, idx: Idx) -> LifetimeId {
        debug_assert_eq!(self.tag(idx), Tag::Borrowed);
        let extra_idx = self.data(idx) as usize;
        LifetimeId::from_raw(self.extra[extra_idx + 1])
    }

    /// Get option inner type.
    ///
    /// For `Option<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not an Option type.
    pub fn option_inner(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Option);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get list element type.
    ///
    /// For `[T]`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a List type.
    pub fn list_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::List);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get range element type.
    ///
    /// For `Range<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Range type.
    pub fn range_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Range);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get set element type.
    ///
    /// For `Set<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Set type.
    pub fn set_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Set);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get channel element type.
    ///
    /// For `chan<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Channel type.
    pub fn channel_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Channel);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get iterator element type.
    ///
    /// Works for both `Iterator<T>` and `DoubleEndedIterator<T>`.
    ///
    /// # Panics
    /// Panics if `idx` is not an iterator type.
    pub fn iterator_elem(&self, idx: Idx) -> Idx {
        debug_assert!(
            self.tag(idx).is_iterator(),
            "expected Iterator or DoubleEndedIterator, got {:?}",
            self.tag(idx)
        );
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    // === Scheme/Generic Accessors ===

    /// Get scheme quantified variable IDs.
    ///
    /// # Panics
    /// Panics if `idx` is not a Scheme type.
    pub fn scheme_vars(&self, idx: Idx) -> &[u32] {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        &self.extra[extra_idx + 1..extra_idx + 1 + count]
    }

    /// Get scheme body type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Scheme type.
    pub fn scheme_body(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx::from_raw(self.extra[extra_idx + 1 + count])
    }

    /// Get the name of an applied generic type.
    ///
    /// For `List<int>`, returns the `Name` for "List".
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        // Name is stored as two u32s for future 64-bit expansion,
        // but currently only uses the low 32 bits
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }

    /// Get the number of type arguments for an applied type.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_arg_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx + 2] as usize
    }

    /// Get a specific type argument by index.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type or `arg_idx` is out of bounds.
    pub fn applied_arg(&self, idx: Idx, arg_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;
        debug_assert!(arg_idx < count);
        Idx::from_raw(self.extra[extra_idx + 3 + arg_idx])
    }

    /// Get all type arguments for an applied type.
    ///
    /// For `Map<str, int>`, returns `[Idx::STR, Idx::INT]`.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_args(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 3 + i]))
            .collect()
    }

    /// Get the name of a named type reference.
    ///
    /// # Panics
    /// Panics if `idx` is not a Named type.
    pub fn named_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Named);
        let extra_idx = self.data(idx) as usize;
        // Name is stored as two u32s for future 64-bit expansion,
        // but currently only uses the low 32 bits
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }
}
