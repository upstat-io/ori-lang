//! Arena Range Types
//!
//! All range types for arena-allocated data. These are compact representations
//! that store start index and length, enabling efficient iteration over arena data.
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

/// Macro to define range types for arena-allocated data.
///
/// Each generated type has:
/// - `start: u32` and `len: u16` fields
/// - `EMPTY` constant
/// - `new()`, `is_empty()`, `len()` methods
/// - `Debug` implementation showing the range as `TypeName(start..end)`
macro_rules! define_range {
    ($($name:ident),* $(,)?) => { $(
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
        #[repr(C)]
        pub struct $name {
            pub start: u32,
            pub len: u16,
        }

        impl $name {
            pub const EMPTY: Self = Self { start: 0, len: 0 };

            #[inline]
            pub const fn new(start: u32, len: u16) -> Self {
                Self { start, len }
            }

            #[inline]
            pub const fn is_empty(&self) -> bool {
                self.len == 0
            }

            #[inline]
            pub const fn len(&self) -> usize {
                self.len as usize
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({}..{})", stringify!($name), self.start, self.start + u32::from(self.len))
            }
        }
    )* };
}

define_range!(
    ParamRange,
    GenericParamRange,
    ArmRange,
    MapEntryRange,
    FieldInitRange,
    SeqBindingRange,
    NamedExprRange,
    CallArgRange,
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Test that all range types work correctly using ParamRange as representative
    #[test]
    fn test_range_empty_constant() {
        assert_eq!(ParamRange::EMPTY.start, 0);
        assert_eq!(ParamRange::EMPTY.len, 0);
        assert!(ParamRange::EMPTY.is_empty());
        assert_eq!(ParamRange::EMPTY.len(), 0);

        // Also test other range types have EMPTY
        assert!(GenericParamRange::EMPTY.is_empty());
        assert!(ArmRange::EMPTY.is_empty());
        assert!(MapEntryRange::EMPTY.is_empty());
        assert!(FieldInitRange::EMPTY.is_empty());
        assert!(SeqBindingRange::EMPTY.is_empty());
        assert!(NamedExprRange::EMPTY.is_empty());
        assert!(CallArgRange::EMPTY.is_empty());
    }

    #[test]
    fn test_range_new() {
        let range = ParamRange::new(10, 5);
        assert_eq!(range.start, 10);
        assert_eq!(range.len, 5);
        assert_eq!(range.len(), 5);
        assert!(!range.is_empty());
    }

    #[test]
    fn test_range_len_conversion() {
        // Test that len() correctly converts u16 to usize
        let range = ParamRange::new(0, u16::MAX);
        assert_eq!(range.len(), u16::MAX as usize);
    }

    #[test]
    fn test_range_debug_format() {
        let range = ParamRange::new(5, 3);
        let debug = format!("{:?}", range);
        assert_eq!(debug, "ParamRange(5..8)");

        let arm_range = ArmRange::new(10, 2);
        let debug = format!("{:?}", arm_range);
        assert_eq!(debug, "ArmRange(10..12)");
    }

    #[test]
    fn test_range_debug_format_empty() {
        let empty = ParamRange::EMPTY;
        let debug = format!("{:?}", empty);
        assert_eq!(debug, "ParamRange(0..0)");
    }

    #[test]
    fn test_range_hash_in_hashset() {
        let mut set = HashSet::new();

        let r1 = ParamRange::new(0, 5);
        let r2 = ParamRange::new(0, 5); // same as r1
        let r3 = ParamRange::new(0, 6); // different len
        let r4 = ParamRange::new(1, 5); // different start

        set.insert(r1);
        set.insert(r2); // duplicate, should not increase size
        set.insert(r3);
        set.insert(r4);

        assert_eq!(set.len(), 3);
        assert!(set.contains(&ParamRange::new(0, 5)));
        assert!(set.contains(&ParamRange::new(0, 6)));
        assert!(set.contains(&ParamRange::new(1, 5)));
    }

    #[test]
    fn test_range_eq() {
        let r1 = ParamRange::new(10, 20);
        let r2 = ParamRange::new(10, 20);
        let r3 = ParamRange::new(10, 21);

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_range_copy_clone() {
        let original = ParamRange::new(5, 10);
        let copied = original; // Copy
        let cloned = original.clone(); // Clone

        assert_eq!(original, copied);
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_range_default() {
        let default: ParamRange = Default::default();
        assert_eq!(default, ParamRange::EMPTY);
    }
}
