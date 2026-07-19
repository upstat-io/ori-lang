//! Internal list and element-layout carriers for COW slow paths.

use crate::rc::ori_rc_is_unique;
use crate::slice_encoding::is_slice_cap;

/// Typed interpretation of the integer COW mode carried by the runtime ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CowMode {
    /// Consult the runtime reference count.
    Dynamic,
    /// Trust the caller's static uniqueness proof.
    StaticUnique,
    /// Forbid mutation because sharing is statically known.
    StaticShared,
}

impl CowMode {
    /// Decode the stable ABI values. Unknown values retain the conservative
    /// historical behavior of a dynamic uniqueness check.
    pub(super) const fn from_abi(value: i32) -> Self {
        match value {
            1 => Self::StaticUnique,
            2 => Self::StaticShared,
            _ => Self::Dynamic,
        }
    }

    /// Returns whether a consuming operation may mutate this allocation.
    pub(super) fn allows_in_place(self, data: *mut u8, cap: i64) -> bool {
        !data.is_null()
            && !is_slice_cap(cap)
            && match self {
                Self::StaticUnique => true,
                Self::Dynamic => ori_rc_is_unique(data),
                Self::StaticShared => false,
            }
    }
}

/// Raw list storage carried together across COW strategy boundaries.
#[derive(Clone, Copy)]
pub(super) struct ListBuffer {
    pub(super) data: *mut u8,
    pub(super) len: i64,
    pub(super) cap: i64,
}

impl ListBuffer {
    /// Preserves the runtime ABI fields without dereferencing `data`.
    pub(super) const fn new(data: *mut u8, len: i64, cap: i64) -> Self {
        Self { data, len, cap }
    }
}

/// Element layout and retain operation required by copying strategies.
#[derive(Clone, Copy)]
pub(super) struct ElementOps {
    pub(super) size: usize,
    pub(super) align: usize,
    pub(super) inc: Option<extern "C" fn(*mut u8)>,
}

impl ElementOps {
    /// Bundles a validated layout with its optional element-retain function.
    pub(super) const fn new(
        size: usize,
        align: usize,
        inc: Option<extern "C" fn(*mut u8)>,
    ) -> Self {
        Self { size, align, inc }
    }
}
