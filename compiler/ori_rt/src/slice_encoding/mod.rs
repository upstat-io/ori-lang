//! Seamless slice encoding primitives.
//!
//! Slices are zero-copy views into existing list/string buffers. The slice
//! flag is encoded in the sign bit of the `cap` field:
//!
//! - `cap >= 0` → regular collection (cap is capacity)
//! - `cap < 0`  → seamless slice (lower 63 bits = byte offset to original data)
//!
//! A slice's data pointer points *into* another allocation's data region.
//! The RC header for the *original* allocation starts at
//! `original_data - RC_HEADER_SIZE` (16 bytes), where
//! `original_data = slice_data - byte_offset`.
//!
//! Inspired by Roc's seamless slices (OWNERSHIP.md) and Go's slice model.

/// The sign bit of `cap` marks a seamless slice.
///
/// When `cap < 0`, the data pointer does NOT point to the start of an RC
/// allocation — it points somewhere *inside* one. The lower 63 bits of
/// `cap` encode the byte offset from the original allocation's data start.
pub(crate) const SLICE_FLAG: i64 = i64::MIN; // 0x8000_0000_0000_0000

/// Check whether a `cap` value indicates a seamless slice.
///
/// A negative cap means the data pointer is a view into another allocation.
/// Regular collections always have `cap >= 0`.
#[inline]
pub(crate) fn is_slice_cap(cap: i64) -> bool {
    cap < 0
}

/// Extract the byte offset from a slice's cap value.
///
/// The lower 63 bits of a slice cap encode the byte offset from the
/// original allocation's data start to the slice's data pointer.
///
/// # Panics
/// Debug-panics if `cap` is not a slice cap.
#[inline]
pub(crate) fn slice_byte_offset(cap: i64) -> usize {
    debug_assert!(
        is_slice_cap(cap),
        "slice_byte_offset called on non-slice cap: {cap}"
    );
    (cap & !SLICE_FLAG) as usize
}

/// Create a slice cap value from a byte offset to the original data.
///
/// Sets the sign bit (`SLICE_FLAG`) and stores the offset in the lower 63 bits.
/// The resulting value satisfies `is_slice_cap(result) == true` and
/// `slice_byte_offset(result) == byte_offset`.
#[inline]
pub(crate) fn make_slice_cap(byte_offset: usize) -> i64 {
    SLICE_FLAG | (byte_offset as i64)
}

/// Compute the original allocation's data pointer from a slice's data and cap.
///
/// Given a slice's data pointer (which is offset into the original buffer)
/// and its cap (which encodes the byte offset), returns the start of the
/// original allocation's data region.
///
/// The RC header starts at `result - 16` (standard 16-byte RC header layout).
///
/// # Safety
/// - `slice_data` must be a valid pointer into an RC-managed allocation
/// - `cap` must be a valid slice cap (`is_slice_cap(cap) == true`)
/// - The computed offset must not underflow the original allocation
#[inline]
pub(crate) fn slice_original_data(slice_data: *mut u8, cap: i64) -> *mut u8 {
    let offset = slice_byte_offset(cap);
    // SAFETY: The offset was stored when creating the slice, and the original
    // allocation is kept alive by the slice's RC increment. Subtracting the
    // offset from the slice data pointer recovers the original data start.
    unsafe { slice_data.sub(offset) }
}

#[cfg(test)]
mod tests;
