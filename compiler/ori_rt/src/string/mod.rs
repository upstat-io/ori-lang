//! String types and operations for AOT-compiled Ori programs.
//!
//! Ori strings use a 24-byte union with Small String Optimization (SSO):
//! - Strings <= 23 bytes are stored inline (no heap allocation, no RC)
//! - Strings > 23 bytes use a heap-allocated RC-managed buffer
//!
//! The discriminator is byte 23: high bit = 1 -> SSO, high bit = 0 -> heap.

mod convert;
mod methods;
mod ops;

pub use convert::*;
pub use methods::*;
pub use ops::*;

use crate::rc::ori_rc_realloc;

/// SSO discriminator: high bit of byte 23 (the last byte of the 24-byte struct).
///
/// On 64-bit platforms, user-space pointers have the high bit clear (canonical
/// addressing), so byte 23 of a heap string (the MSB of the data pointer) is
/// always 0. For SSO strings, we set it to 1.
const SSO_FLAG: u8 = 0x80;

/// Mask to extract SSO string length from the flags byte (low 7 bits).
const SSO_LEN_MASK: u8 = 0x7F;

/// Maximum byte length for SSO (inline) strings.
const SSO_MAX_LEN: usize = 23;

/// Heap string layout: `{len, cap, data}` -- 24 bytes.
///
/// Data pointer points to RC-managed memory with a refcount header
/// at `data_ptr - 8`, allocated by `ori_rc_alloc`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OriStrHeap {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

/// SSO (Small String Optimization) layout: `{bytes, flags}` -- 24 bytes.
///
/// String data is stored inline in `bytes[0..len]`. No heap allocation,
/// no RC header, no refcount operations needed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OriStrSSO {
    pub bytes: [u8; 23],
    /// High bit (0x80) = SSO flag; low 7 bits = length (0..=23).
    pub flags: u8,
}

/// Ori string representation -- 24-byte union with Small String Optimization.
///
/// Strings <= 23 bytes are stored inline (SSO): no heap allocation, no RC.
/// Strings > 23 bytes use a heap-allocated RC-managed buffer with capacity
/// tracking for COW (Copy-on-Write) growth.
///
/// The discriminator is byte 23: high bit = 1 -> SSO, high bit = 0 -> heap.
/// This works because user-space pointers on 64-bit platforms have the MSB
/// clear (canonical addressing), so the heap data pointer's MSB is always 0.
///
/// ```text
/// Heap:  +----------+----------+----------+
///        | len (i64)| cap (i64)|data (ptr)|  24 bytes
///        +----------+----------+----------+
///
/// SSO:   +--------------------------+-----+
///        | string data (23 bytes)   |flags|  24 bytes
///        +--------------------------+-----+
///                                    high bit=1 -> SSO, low 7 bits=length
/// ```
#[repr(C)]
#[derive(Clone, Copy)]
pub union OriStr {
    pub heap: OriStrHeap,
    pub sso: OriStrSSO,
}

impl OriStr {
    /// Empty string constant (SSO, zero length, no allocation).
    pub const EMPTY: Self = Self {
        sso: OriStrSSO {
            bytes: [0u8; 23],
            flags: SSO_FLAG,
        },
    };

    /// Check whether this string uses SSO (inline storage).
    #[inline]
    #[must_use]
    pub fn is_sso(&self) -> bool {
        // SAFETY: The flags byte (byte 23) is at the same position in both
        // union variants -- it's the MSB of the data pointer for heap strings,
        // and the explicit flags field for SSO strings.
        unsafe { self.sso.flags & SSO_FLAG != 0 }
    }

    /// Byte length of the string.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        if self.is_sso() {
            unsafe { (self.sso.flags & SSO_LEN_MASK) as usize }
        } else {
            unsafe { self.heap.len as usize }
        }
    }

    /// Whether the string is empty (zero bytes).
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Raw byte content of the string.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        if self.is_sso() {
            let len = self.len();
            unsafe { &self.sso.bytes[..len] }
        } else {
            unsafe {
                if self.heap.data.is_null() || self.heap.len <= 0 {
                    &[]
                } else {
                    std::slice::from_raw_parts(self.heap.data, self.heap.len as usize)
                }
            }
        }
    }

    /// Convert to Rust string slice.
    ///
    /// # Safety
    /// Caller must ensure the bytes are valid UTF-8.
    #[inline]
    #[must_use]
    pub unsafe fn as_str(&self) -> &str {
        std::str::from_utf8_unchecked(self.as_bytes())
    }

    /// Get the heap data pointer for RC operations.
    ///
    /// Returns `None` for SSO strings (no heap allocation to manage).
    #[inline]
    #[must_use]
    pub fn heap_data_ptr(&self) -> Option<*mut u8> {
        if self.is_sso() {
            None
        } else {
            unsafe {
                if self.heap.data.is_null() {
                    None
                } else {
                    Some(self.heap.data)
                }
            }
        }
    }

    /// Create an SSO string from a byte slice.
    ///
    /// # Panics
    /// Debug-panics if `bytes.len() > 23`.
    #[must_use]
    pub fn from_sso(bytes: &[u8]) -> Self {
        debug_assert!(
            bytes.len() <= SSO_MAX_LEN,
            "SSO string too long: {} > {SSO_MAX_LEN}",
            bytes.len()
        );
        let mut sso = OriStrSSO {
            bytes: [0u8; 23],
            flags: SSO_FLAG | bytes.len() as u8,
        };
        sso.bytes[..bytes.len()].copy_from_slice(bytes);
        Self { sso }
    }

    /// Create a heap string from a byte slice with RC-managed allocation.
    ///
    /// The data is copied into an `ori_rc_alloc` buffer so that the ARC
    /// pipeline's `RcInc`/`RcDec` operations work correctly.
    #[must_use]
    pub fn from_heap(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            return Self::EMPTY;
        }
        let len = bytes.len();
        let data = crate::rc::ori_rc_alloc(len, 1);
        if !data.is_null() {
            // SAFETY: data is freshly allocated with at least `len` bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, len);
            }
        }
        Self {
            heap: OriStrHeap {
                len: len as i64,
                cap: len as i64,
                data,
            },
        }
    }

    /// Create a string, automatically choosing SSO (<= 23 bytes) or heap.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() <= SSO_MAX_LEN {
            Self::from_sso(bytes)
        } else {
            Self::from_heap(bytes)
        }
    }

    /// Create from an owned `String`, using SSO when possible.
    ///
    /// For strings <= 23 bytes, stores inline (no heap allocation).
    /// For longer strings, copies into an RC-managed heap buffer.
    #[must_use]
    pub fn from_owned(s: &str) -> Self {
        Self::from_bytes(s.as_bytes())
    }

    /// Create a heap string with a pre-allocated capacity.
    ///
    /// Allocates a buffer of `cap` bytes but sets length to 0.
    /// Used for building strings incrementally (concat chains).
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        if cap == 0 {
            return Self::EMPTY;
        }
        let data = crate::rc::ori_rc_alloc(cap, 1);
        Self {
            heap: OriStrHeap {
                len: 0,
                cap: cap as i64,
                data,
            },
        }
    }

    /// Promote an SSO string to a heap string with at least `min_cap` capacity.
    ///
    /// Copies the inline bytes into an RC-managed heap buffer. Returns a heap
    /// variant with the original content and room to grow.
    ///
    /// # Precondition
    /// `self` must be an SSO string. Calling on a heap string is a logic error
    /// (debug-asserted).
    #[must_use]
    pub fn promote_to_heap(&self, min_cap: usize) -> Self {
        debug_assert!(self.is_sso(), "promote_to_heap called on heap string");
        let len = self.len();
        let cap = crate::next_capacity(0, min_cap);
        let data = crate::rc::ori_rc_alloc(cap, 1);
        if !data.is_null() && len > 0 {
            // SAFETY: SSO bytes are inline and valid for `len` bytes.
            unsafe {
                std::ptr::copy_nonoverlapping(self.sso.bytes.as_ptr(), data, len);
            }
        }
        Self {
            heap: OriStrHeap {
                len: len as i64,
                cap: cap as i64,
                data,
            },
        }
    }

    /// Ensure a heap string has capacity for at least `required` bytes.
    ///
    /// If the current capacity is sufficient, this is a no-op.
    /// Otherwise, reallocates the buffer with amortized growth.
    ///
    /// # Precondition
    /// `self` must be a uniquely-owned heap string (not SSO, not shared).
    pub fn ensure_capacity(&mut self, required: usize) {
        debug_assert!(!self.is_sso(), "ensure_capacity called on SSO string");
        unsafe {
            if self.heap.cap as usize >= required {
                return;
            }
            let old_len = self.heap.len as usize;
            let new_cap = crate::next_capacity(self.heap.cap as usize, required);
            let new_data = ori_rc_realloc(self.heap.data, old_len, new_cap, 1);
            self.heap.cap = new_cap as i64;
            self.heap.data = new_data;
        }
    }
}

/// Return an empty string sentinel (SSO, no allocation).
#[no_mangle]
pub extern "C" fn ori_str_empty() -> OriStr {
    OriStr::EMPTY
}

/// Ensure a heap string has capacity for at least `required` bytes.
///
/// If the string is SSO or already has sufficient capacity, this is a no-op.
/// Otherwise, reallocates the heap buffer using `next_capacity()` for
/// amortized O(1) growth.
///
/// # Precondition
/// If the string is heap, it must be uniquely owned (`ori_rc_is_unique`).
/// The caller (LLVM codegen) is responsible for checking uniqueness before
/// calling this function.
#[no_mangle]
pub extern "C" fn ori_str_ensure_capacity(s: *mut OriStr, required: i64) {
    if s.is_null() || required <= 0 {
        return;
    }
    let str_ref = unsafe { &mut *s };
    if str_ref.is_sso() {
        // SSO strings don't have capacity -- promotion handled by caller
        return;
    }
    str_ref.ensure_capacity(required as usize);
}

/// Safely dereference an `OriStr` pointer, returning `""` for null.
///
/// # Safety
///
/// If non-null, `ptr` must point to a valid `OriStr`.
pub(crate) unsafe fn deref_str<'a>(ptr: *const OriStr) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        (*ptr).as_str()
    }
}

/// Result of decoding a single UTF-8 character from a string.
///
/// Returned by `ori_str_next_char`. The codepoint is the Unicode scalar value
/// (as i32/char), and `next_offset` is the byte offset of the next character.
#[repr(C)]
pub struct OriCharResult {
    /// Unicode codepoint (i32). -1 on error or end-of-string.
    pub codepoint: i32,
    /// Byte offset of the next character. Equals `byte_offset + char_width`.
    pub next_offset: i64,
}
