//! Inherent impl blocks for [`VariantFields`] and [`TypeKind`].
//!
//! Extracted from `mod.rs` to keep the main module under the 500-line
//! production file limit (TPR-01-043).

use super::{FieldDef, TypeKind, VariantFields};

use crate::Idx;

impl VariantFields {
    /// Check if this is a unit variant.
    #[inline]
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    /// Check if this is a tuple variant.
    #[inline]
    pub fn is_tuple(&self) -> bool {
        matches!(self, Self::Tuple(_))
    }

    /// Check if this is a record variant.
    #[inline]
    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(_))
    }

    /// Get the arity (number of fields) of this variant.
    pub fn arity(&self) -> usize {
        match self {
            Self::Unit => 0,
            Self::Tuple(fields) => fields.len(),
            Self::Record(fields) => fields.len(),
        }
    }

    /// Get tuple field types if this is a tuple variant.
    pub fn tuple_types(&self) -> Option<&[Idx]> {
        match self {
            Self::Tuple(types) => Some(types),
            _ => None,
        }
    }

    /// Get record fields if this is a record variant.
    pub fn record_fields(&self) -> Option<&[FieldDef]> {
        match self {
            Self::Record(fields) => Some(fields),
            _ => None,
        }
    }
}

impl TypeKind {
    /// Check if this is a struct.
    #[inline]
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct(_))
    }

    /// Check if this is an enum.
    #[inline]
    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum { .. })
    }

    /// Check if this is a newtype.
    #[inline]
    pub fn is_newtype(&self) -> bool {
        matches!(self, Self::Newtype { .. })
    }

    /// Check if this is an alias.
    #[inline]
    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias { .. })
    }
}
