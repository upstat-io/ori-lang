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

/// Format a string with Debug semantics: wraps in quotes and escapes special chars.
///
/// `"hello"` → `"\"hello\""`, `"a\nb"` → `"\"a\\nb\""`.
/// Matches the evaluator's `escape_debug_str` + quote wrapping.
#[no_mangle]
pub extern "C" fn ori_str_debug_format(s: *const OriStr) -> OriStr {
    if s.is_null() {
        return OriStr::from_sso(b"\"\"");
    }
    // SAFETY: Caller ensures s points to a valid OriStr.
    let input = unsafe { &*s };
    let src = unsafe { input.as_str() };
    let mut result = String::with_capacity(src.len() + 2);
    result.push('"');
    for c in src.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\0' => result.push_str("\\0"),
            c => result.push(c),
        }
    }
    result.push('"');
    OriStr::from_owned(&result)
}

/// Format a char with Debug semantics: wraps in single quotes and escapes special chars.
///
/// `'a'` → `"'a'"`, `'\n'` → `"'\\n'"`.
#[no_mangle]
pub extern "C" fn ori_char_debug_format(ch: u32) -> OriStr {
    let c = char::from_u32(ch).unwrap_or('\u{FFFD}');
    let result = match c {
        '\n' => "'\\n'".to_string(),
        '\r' => "'\\r'".to_string(),
        '\t' => "'\\t'".to_string(),
        '\\' => "'\\\\'".to_string(),
        '\'' => "'\\''".to_string(),
        '\0' => "'\\0'".to_string(),
        c => format!("'{c}'"),
    };
    OriStr::from_owned(&result)
}

/// Format a byte with Debug semantics: `0x{:02x}`.
///
/// `65` → `"0x41"`, `0` → `"0x00"`.
#[no_mangle]
pub extern "C" fn ori_byte_debug_format(b: i64) -> OriStr {
    // Truncate to u8 range
    let byte = (b & 0xFF) as u8;
    let result = format!("0x{byte:02x}");
    OriStr::from_owned(&result)
}
