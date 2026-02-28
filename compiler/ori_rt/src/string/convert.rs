//! String conversion functions for AOT-compiled Ori programs.
//!
//! Contains extern "C" functions for converting various types to `OriStr`:
//! integers, floats, booleans, and raw byte pointers.

use super::OriStr;

/// Convert an integer to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_int(n: i64) -> OriStr {
    OriStr::from_owned(&n.to_string())
}

/// Create an `OriStr` from a raw pointer + length (e.g., string literals).
///
/// Uses SSO for strings <= 23 bytes (no heap allocation). For longer strings,
/// copies into an RC-managed heap buffer for ARC `RcInc`/`RcDec` operations.
#[no_mangle]
pub extern "C" fn ori_str_from_raw(src: *const u8, len: i64) -> OriStr {
    if src.is_null() || len <= 0 {
        return OriStr::EMPTY;
    }
    let size = len as usize;
    // SAFETY: Caller ensures src is valid for `size` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(src, size) };
    OriStr::from_bytes(bytes)
}

/// Convert a boolean to a string.
///
/// Both "true" (4 bytes) and "false" (5 bytes) fit in SSO -- no heap allocation.
#[no_mangle]
pub extern "C" fn ori_str_from_bool(b: bool) -> OriStr {
    if b {
        OriStr::from_sso(b"true")
    } else {
        OriStr::from_sso(b"false")
    }
}

/// Convert a float to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_float(f: f64) -> OriStr {
    OriStr::from_owned(&f.to_string())
}
