//! Format specification runtime for AOT-compiled template string interpolation.
//!
//! Provides `extern "C"` functions called by LLVM-generated code for `{value:spec}`
//! expressions. Each function receives the value and a format spec string, parses
//! the spec, and returns a formatted `OriStr`.
//!
//! The spec parser and the scalar emission kernel live in the `ori_format` leaf
//! crate — the single source of truth shared with the interpreter backend
//! (`ori_eval::interpreter::format`). These entry points are thin wrappers:
//! reconstruct the spec `&str` from the FFI pointer, parse + emit via
//! `ori_format`, and wrap the result in an `OriStr`.

use crate::OriStr;
use ori_format::{format_float, format_int, format_str, parse_format_spec, ParsedFormatSpec};

// Public FFI Entry Points

/// Format an integer with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_int(n: i64, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str).unwrap_or(ParsedFormatSpec::EMPTY);
    let result = format_int(n, &parsed);
    OriStr::from_owned(&result)
}

/// Format a float with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_float(f: f64, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str).unwrap_or(ParsedFormatSpec::EMPTY);
    let result = format_float(f, &parsed);
    OriStr::from_owned(&result)
}

/// Format a string with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_str(s: *const OriStr, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let input = unsafe { (*s).as_str() };
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str).unwrap_or(ParsedFormatSpec::EMPTY);
    let result = format_str(input, &parsed);
    OriStr::from_owned(&result)
}

/// Format a boolean with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_bool(b: bool, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str).unwrap_or(ParsedFormatSpec::EMPTY);
    let s = if b { "true" } else { "false" };
    let result = format_str(s, &parsed);
    OriStr::from_owned(&result)
}

/// Format a char (as i32 codepoint) with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_char(c: i32, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str).unwrap_or(ParsedFormatSpec::EMPTY);
    let ch = char::from_u32(c as u32).unwrap_or('\u{FFFD}');
    let result = format_str(&ch.to_string(), &parsed);
    OriStr::from_owned(&result)
}

/// Reconstruct a `&str` from FFI pointer + length.
///
/// # Safety
///
/// `ptr` must point to valid UTF-8 of at least `len` bytes.
unsafe fn spec_from_raw<'a>(ptr: *const u8, len: i64) -> &'a str {
    if len <= 0 || ptr.is_null() {
        return "";
    }
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len as usize) };
    core::str::from_utf8_unchecked(bytes)
}
