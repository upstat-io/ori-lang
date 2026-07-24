//! Crate-internal declarative macros shared across `ori_ir` modules.

/// Define arena range newtypes (`start: u32, len: u16` = 8 bytes).
///
/// Each generated type has:
/// - `start: u32` and `len: u16` fields
/// - `EMPTY` constant
/// - `new()`, `is_empty()`, `len()` methods
/// - `Debug` showing the range as `TypeName(start..end)`
///
/// Derives `Copy, Clone, Eq, PartialEq, Hash, Default` for Salsa.
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

pub(crate) use define_range;
