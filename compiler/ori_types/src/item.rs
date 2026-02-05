//! Compact type item storage.
//!
//! Each type in the pool is stored as an `Item` with a tag and data field.
//! The interpretation of `data` depends on the tag.

use crate::{Idx, Tag};

/// A single type item in the pool.
///
/// This is the fundamental unit of type storage.
/// - `tag`: Identifies the type kind (see [`Tag`])
/// - `data`: Meaning depends on tag (child index, extra index, or var id)
///
/// # Memory Layout
///
/// 5 bytes total: 1 byte tag + 4 bytes data.
/// Padding may be added by the compiler, but the logical size is 5 bytes.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Item {
    /// Type kind discriminant.
    pub tag: Tag,
    /// Tag-dependent data field.
    pub data: u32,
}

impl Item {
    /// Create a new item with the given tag and data.
    #[inline]
    pub const fn new(tag: Tag, data: u32) -> Self {
        Self { tag, data }
    }

    /// Create a primitive type item.
    #[inline]
    pub const fn primitive(tag: Tag) -> Self {
        Self { tag, data: 0 }
    }

    /// Create a simple container item (List, Option, etc.).
    ///
    /// `child` is stored directly in the data field.
    #[inline]
    pub const fn simple_container(tag: Tag, child: Idx) -> Self {
        Self {
            tag,
            data: child.raw(),
        }
    }

    /// Create an item that references the extra array.
    ///
    /// `extra_idx` is the starting index in the extra array.
    #[inline]
    pub const fn with_extra(tag: Tag, extra_idx: u32) -> Self {
        Self {
            tag,
            data: extra_idx,
        }
    }

    /// Create a type variable item.
    ///
    /// `var_id` is the variable's index in the `var_states` array.
    #[inline]
    pub const fn var(tag: Tag, var_id: u32) -> Self {
        debug_assert!(matches!(tag, Tag::Var | Tag::BoundVar | Tag::RigidVar));
        Self { tag, data: var_id }
    }

    /// Get the child index for simple container types.
    ///
    /// Only valid for List, Option, Set, Channel, Range.
    #[inline]
    pub const fn child(self) -> Idx {
        Idx::from_raw(self.data)
    }

    /// Get the extra array index for complex types.
    ///
    /// Only valid for types where `tag.uses_extra()` returns true.
    #[inline]
    pub const fn extra_idx(self) -> u32 {
        self.data
    }

    /// Get the variable id for type variable items.
    ///
    /// Only valid for Var, `BoundVar`, `RigidVar`.
    #[inline]
    pub const fn var_id(self) -> u32 {
        self.data
    }
}

// Note: Item is 5 bytes logically but may be 8 bytes due to alignment.
// We use repr(C) for predictable layout.
// Compile-time assertion for packed size would need repr(packed).
#[cfg(test)]
const _: () = {
    // Tag is 1 byte, data is 4 bytes
    // With repr(C), actual size depends on alignment
    assert!(std::mem::size_of::<Item>() >= 5);
    assert!(std::mem::size_of::<Item>() <= 8);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_item() {
        let item = Item::primitive(Tag::Int);
        assert_eq!(item.tag, Tag::Int);
        assert_eq!(item.data, 0);
    }

    #[test]
    fn simple_container_item() {
        let item = Item::simple_container(Tag::List, Idx::INT);
        assert_eq!(item.tag, Tag::List);
        assert_eq!(item.child(), Idx::INT);
    }

    #[test]
    fn extra_item() {
        let item = Item::with_extra(Tag::Function, 100);
        assert_eq!(item.tag, Tag::Function);
        assert_eq!(item.extra_idx(), 100);
    }

    #[test]
    fn var_item() {
        let item = Item::var(Tag::Var, 42);
        assert_eq!(item.tag, Tag::Var);
        assert_eq!(item.var_id(), 42);
    }
}
