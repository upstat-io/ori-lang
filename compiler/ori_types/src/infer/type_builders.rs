//! Simple type-building helpers on [`InferEngine`].
//!
//! Literal inference returns the pre-interned primitive `Idx`s
//! (`types.md §TY-5`); collection/compound helpers delegate straight to the
//! pool constructors. Grouped here to keep `infer/mod.rs` as the thin engine
//! facade.

use crate::Idx;

use super::InferEngine;

impl InferEngine<'_> {
    // Literal Inference Helpers

    /// Infer the type of an integer literal.
    #[inline]
    pub fn infer_int(&self) -> Idx {
        Idx::INT
    }

    /// Infer the type of a float literal.
    #[inline]
    pub fn infer_float(&self) -> Idx {
        Idx::FLOAT
    }

    /// Infer the type of a boolean literal.
    #[inline]
    pub fn infer_bool(&self) -> Idx {
        Idx::BOOL
    }

    /// Infer the type of a string literal.
    #[inline]
    pub fn infer_str(&self) -> Idx {
        Idx::STR
    }

    /// Infer the type of a character literal.
    #[inline]
    pub fn infer_char(&self) -> Idx {
        Idx::CHAR
    }

    /// Infer the type of a byte literal.
    #[inline]
    pub fn infer_byte(&self) -> Idx {
        Idx::BYTE
    }

    /// Infer the type of a unit literal.
    #[inline]
    pub fn infer_unit(&self) -> Idx {
        Idx::UNIT
    }

    // Collection Inference Helpers

    /// Infer the type of an empty list.
    ///
    /// Returns `[?a]` where `?a` is a fresh type variable.
    pub fn infer_empty_list(&mut self) -> Idx {
        let elem = self.fresh_var();
        self.pool_mut().list(elem)
    }

    /// Infer the type of a list with a known element type.
    pub fn infer_list(&mut self, elem_ty: Idx) -> Idx {
        self.pool_mut().list(elem_ty)
    }

    /// Infer the type of an empty map.
    ///
    /// Returns `{?k: ?v}` where `?k` and `?v` are fresh type variables.
    pub fn infer_empty_map(&mut self) -> Idx {
        let key = self.fresh_var();
        let value = self.fresh_var();
        self.pool_mut().map(key, value)
    }

    /// Infer the type of a map with known key and value types.
    pub fn infer_map(&mut self, key_ty: Idx, value_ty: Idx) -> Idx {
        self.pool_mut().map(key_ty, value_ty)
    }

    /// Infer the type of a tuple.
    pub fn infer_tuple(&mut self, elem_types: &[Idx]) -> Idx {
        self.pool_mut().tuple(elem_types)
    }

    /// Infer the type of an option with known inner type.
    pub fn infer_option(&mut self, inner_ty: Idx) -> Idx {
        self.pool_mut().option(inner_ty)
    }

    /// Infer the type of a result with known ok and error types.
    pub fn infer_result(&mut self, ok_ty: Idx, err_ty: Idx) -> Idx {
        self.pool_mut().result(ok_ty, err_ty)
    }

    /// Infer the type of a function.
    pub fn infer_function(&mut self, params: &[Idx], ret: Idx) -> Idx {
        self.pool_mut().function(params, ret)
    }

    /// Infer the type of an iterator with known element type.
    pub fn infer_iterator(&mut self, elem_ty: Idx) -> Idx {
        self.pool_mut().iterator(elem_ty)
    }
}
