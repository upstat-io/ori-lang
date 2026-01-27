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
