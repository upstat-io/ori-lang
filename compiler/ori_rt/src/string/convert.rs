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

/// Unit conversion constants for Duration/Size formatting.
///
/// `ori_rt` is a zero-dependency C-ABI staticlib (no `ori_ir` prod-dep), so the
/// canonical `ori_ir::builtin_constants` values are copied here. The dual-execution
/// parity test exercises every unit boundary, so a copy typo diverges interp<->AOT
/// and fails parity.
mod units {
    pub const NS_PER_US: u64 = 1_000;
    pub const NS_PER_MS: u64 = 1_000_000;
    pub const NS_PER_S: u64 = 1_000_000_000;
    pub const NS_PER_M: u64 = 60 * NS_PER_S;
    pub const NS_PER_H: u64 = 60 * NS_PER_M;

    pub const BYTES_PER_KB: u64 = 1_000;
    pub const BYTES_PER_MB: u64 = 1_000_000;
    pub const BYTES_PER_GB: u64 = 1_000_000_000;
    pub const BYTES_PER_TB: u64 = 1_000_000_000_000;
}

/// Convert a Duration (i64 nanoseconds) to its unit string.
///
/// Mirrors `ori_eval::methods::units::format_duration`: sign-aware, picks the
/// largest whole unit among ns/us/ms/s/m/h, `"0ns"` for zero.
#[no_mangle]
pub extern "C" fn ori_str_from_duration(ns: i64) -> OriStr {
    let abs_ns = ns.unsigned_abs();
    let sign = if ns < 0 { "-" } else { "" };

    if abs_ns == 0 {
        return OriStr::from_owned("0ns");
    }

    let s = if abs_ns.is_multiple_of(units::NS_PER_H) {
        format!("{sign}{}h", abs_ns / units::NS_PER_H)
    } else if abs_ns.is_multiple_of(units::NS_PER_M) {
        format!("{sign}{}m", abs_ns / units::NS_PER_M)
    } else if abs_ns.is_multiple_of(units::NS_PER_S) {
        format!("{sign}{}s", abs_ns / units::NS_PER_S)
    } else if abs_ns.is_multiple_of(units::NS_PER_MS) {
        format!("{sign}{}ms", abs_ns / units::NS_PER_MS)
    } else if abs_ns.is_multiple_of(units::NS_PER_US) {
        format!("{sign}{}us", abs_ns / units::NS_PER_US)
    } else {
        format!("{sign}{abs_ns}ns")
    };
    OriStr::from_owned(&s)
}

/// Convert a Size to its unit string.
///
/// Mirrors `ori_eval::methods::units::format_size`: largest whole unit among
/// b/kb/mb/gb/tb (SI 1000-based), `"0b"` for zero. The codegen ABI passes the
/// byte count as i64; Size values are non-negative, so cast to u64 internally.
#[no_mangle]
pub extern "C" fn ori_str_from_size(bytes_i64: i64) -> OriStr {
    let bytes = bytes_i64 as u64;

    if bytes == 0 {
        return OriStr::from_owned("0b");
    }

    let s = if bytes.is_multiple_of(units::BYTES_PER_TB) {
        format!("{}tb", bytes / units::BYTES_PER_TB)
    } else if bytes.is_multiple_of(units::BYTES_PER_GB) {
        format!("{}gb", bytes / units::BYTES_PER_GB)
    } else if bytes.is_multiple_of(units::BYTES_PER_MB) {
        format!("{}mb", bytes / units::BYTES_PER_MB)
    } else if bytes.is_multiple_of(units::BYTES_PER_KB) {
        format!("{}kb", bytes / units::BYTES_PER_KB)
    } else {
        format!("{bytes}b")
    };
    OriStr::from_owned(&s)
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

/// Convert an `Ordering` tag to its variant name.
///
/// Tag convention (Spec: Clause 8.4.1): `Less=0`, `Equal=1`, `Greater=2`.
/// All three names fit in SSO -- no heap allocation.
#[no_mangle]
pub extern "C" fn ori_str_from_ordering(tag: i64) -> OriStr {
    match tag {
        0 => OriStr::from_sso(b"Less"),
        1 => OriStr::from_sso(b"Equal"),
        _ => OriStr::from_sso(b"Greater"),
    }
}

/// Convert a float to a string.
#[no_mangle]
pub extern "C" fn ori_str_from_float(f: f64) -> OriStr {
    OriStr::from_owned(&f.to_string())
}

/// Append `c` to `out`, escaping the control/quote characters that Debug and
/// Printable string rendering both escape (`\n`, `\r`, `\t`, `\`, `"`, `\0`).
///
/// Shared by `ori_str_debug_format` and `ori_str_escape_control` so the escape
/// table has one home. Matches the evaluator's `escape_debug_str`.
fn push_escaped_str_char(out: &mut String, c: char) {
    match c {
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\\' => out.push_str("\\\\"),
        '"' => out.push_str("\\\""),
        '\0' => out.push_str("\\0"),
        c => out.push(c),
    }
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
    // SAFETY: OriStr bytes are valid UTF-8 by construction.
    let src = unsafe { input.as_str() };
    let mut result = String::with_capacity(src.len() + 2);
    result.push('"');
    for c in src.chars() {
        push_escaped_str_char(&mut result, c);
    }
    result.push('"');
    OriStr::from_owned(&result)
}

/// Escape control characters in a string without adding quotes.
///
/// `"a\nb"` → `"a\\nb"`, `"hello"` → `"hello"` (unchanged).
/// Used for map key formatting in `Map.debug()` — keys are displayed with
/// Printable semantics but control characters must be escaped for readability.
/// Matches the evaluator's `escape_debug_str` behavior.
#[no_mangle]
pub extern "C" fn ori_str_escape_control(s: *const OriStr) -> OriStr {
    if s.is_null() {
        return OriStr::from_sso(b"");
    }
    // SAFETY: Caller ensures s points to a valid OriStr.
    let input = unsafe { &*s };
    // SAFETY: OriStr bytes are valid UTF-8 by construction.
    let src = unsafe { input.as_str() };
    // Fast path: if no control chars, return as-is
    if !src
        .chars()
        .any(|c| matches!(c, '\n' | '\r' | '\t' | '\\' | '"' | '\0'))
    {
        return OriStr::from_owned(src);
    }
    let mut result = String::with_capacity(src.len());
    for c in src.chars() {
        push_escaped_str_char(&mut result, c);
    }
    OriStr::from_owned(&result)
}

/// Convert a char to its Printable string representation (no quotes).
///
/// `'a'` → `"a"`, `'\n'` → `"\n"` (the actual newline character).
#[no_mangle]
pub extern "C" fn ori_str_from_char(ch: u32) -> OriStr {
    let c = char::from_u32(ch).unwrap_or('\u{FFFD}');
    let mut buf = [0u8; 4];
    let s = c.encode_utf8(&mut buf);
    OriStr::from_owned(s)
}

/// Return whether `ch` is an alphabetic Unicode scalar.
///
/// Matches the evaluator's `char.is_alpha` (`char::is_alphabetic`).
#[no_mangle]
pub extern "C" fn ori_char_is_alpha(ch: u32) -> bool {
    char::from_u32(ch).is_some_and(char::is_alphabetic)
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
