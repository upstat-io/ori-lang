//! Format specification runtime for AOT-compiled template string interpolation.
//!
//! Provides `extern "C"` functions called by LLVM-generated code for `{value:spec}`
//! expressions. Each function receives the value and a format spec string, parses
//! the spec, and returns a formatted `OriStr`.
//!
//! The format spec parser and formatters are self-contained ports of the evaluator
//! logic (`ori_eval::interpreter::format`), with no external dependencies.

use crate::OriStr;

// =============================================================================
// Public FFI Entry Points
// =============================================================================

/// Format an integer with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_int(n: i64, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str);
    let result = format_int(n, &parsed);
    OriStr::from_owned(result)
}

/// Format a float with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_float(f: f64, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str);
    let result = format_float(f, &parsed);
    OriStr::from_owned(result)
}

/// Format a string with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_str(s: *const OriStr, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let input = unsafe { (*s).as_str() };
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str);
    let result = fmt_str(input, &parsed);
    OriStr::from_owned(result)
}

/// Format a boolean with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_bool(b: bool, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str);
    let s = if b { "true" } else { "false" };
    let result = fmt_str(s, &parsed);
    OriStr::from_owned(result)
}

/// Format a char (as i32 codepoint) with a format specification.
#[no_mangle]
pub extern "C" fn ori_format_char(c: i32, spec_ptr: *const u8, spec_len: i64) -> OriStr {
    let spec_str = unsafe { spec_from_raw(spec_ptr, spec_len) };
    let parsed = parse_format_spec(spec_str);
    let ch = char::from_u32(c as u32).unwrap_or('\u{FFFD}');
    let result = fmt_str(&ch.to_string(), &parsed);
    OriStr::from_owned(result)
}

// =============================================================================
// Format Spec Parser (self-contained, no ori_ir dependency)
// =============================================================================

#[derive(Clone, Debug)]
struct ParsedFormatSpec {
    fill: Option<char>,
    align: Option<Align>,
    sign: Option<Sign>,
    alternate: bool,
    zero_pad: bool,
    width: Option<usize>,
    precision: Option<usize>,
    format_type: Option<FormatType>,
}

impl ParsedFormatSpec {
    const EMPTY: Self = Self {
        fill: None,
        align: None,
        sign: None,
        alternate: false,
        zero_pad: false,
        width: None,
        precision: None,
        format_type: None,
    };
}

#[derive(Copy, Clone, Debug)]
enum Align {
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, Debug)]
enum Sign {
    Plus,
    Minus,
    Space,
}

#[derive(Copy, Clone, Debug)]
enum FormatType {
    Binary,
    Octal,
    Hex,
    HexUpper,
    Exp,
    ExpUpper,
    Fixed,
    Percent,
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

/// Parse a format spec string into structured options.
///
/// Mirrors `ori_ir::format_spec::parse_format_spec`. On invalid input,
/// falls back to empty spec (LLVM-compiled code has already been validated
/// by the type checker, so errors here indicate an internal bug, not user error).
fn parse_format_spec(spec: &str) -> ParsedFormatSpec {
    if spec.is_empty() {
        return ParsedFormatSpec::EMPTY;
    }

    let mut result = ParsedFormatSpec::EMPTY;
    let chars: Vec<char> = spec.chars().collect();
    let mut pos = 0;

    // [[fill]align]
    if chars.len() >= 2 && is_align_char(chars[1]) {
        result.fill = Some(chars[0]);
        result.align = Some(parse_align(chars[1]));
        pos = 2;
    } else if is_align_char(chars[0]) {
        result.align = Some(parse_align(chars[0]));
        pos = 1;
    }

    // [sign]
    if pos < chars.len() {
        match chars[pos] {
            '+' => {
                result.sign = Some(Sign::Plus);
                pos += 1;
            }
            '-' => {
                result.sign = Some(Sign::Minus);
                pos += 1;
            }
            ' ' => {
                result.sign = Some(Sign::Space);
                pos += 1;
            }
            _ => {}
        }
    }

    // [#]
    if pos < chars.len() && chars[pos] == '#' {
        result.alternate = true;
        pos += 1;
    }

    // [0]
    pos = parse_zero_pad(&chars, pos, &mut result);

    // [width]
    pos = parse_width(&chars, pos, &mut result);

    // [.precision]
    pos = parse_precision(&chars, pos, &mut result);

    // [type]
    parse_type(&chars, pos, &mut result);

    result
}

fn parse_zero_pad(chars: &[char], mut pos: usize, result: &mut ParsedFormatSpec) -> usize {
    if pos < chars.len() && chars[pos] == '0' {
        // '0' is zero-pad if followed by more digits, at end, or before precision/type.
        // Otherwise it's the start of a width number (handled in parse_width).
        let is_zero_pad = pos + 1 >= chars.len()
            || chars[pos + 1].is_ascii_digit()
            || chars[pos + 1] == '.'
            || is_format_type(chars[pos + 1]);
        if is_zero_pad {
            result.zero_pad = true;
            pos += 1;
        }
    }
    pos
}

fn parse_width(chars: &[char], mut pos: usize, result: &mut ParsedFormatSpec) -> usize {
    let start = pos;
    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos > start {
        let s: String = chars[start..pos].iter().collect();
        result.width = s.parse().ok();
    }
    pos
}

fn parse_precision(chars: &[char], mut pos: usize, result: &mut ParsedFormatSpec) -> usize {
    if pos < chars.len() && chars[pos] == '.' {
        pos += 1;
        let start = pos;
        while pos < chars.len() && chars[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos > start {
            let s: String = chars[start..pos].iter().collect();
            result.precision = s.parse().ok();
        } else {
            result.precision = Some(0);
        }
    }
    pos
}

fn parse_type(chars: &[char], pos: usize, result: &mut ParsedFormatSpec) {
    if pos < chars.len() {
        result.format_type = match chars[pos] {
            'b' => Some(FormatType::Binary),
            'o' => Some(FormatType::Octal),
            'x' => Some(FormatType::Hex),
            'X' => Some(FormatType::HexUpper),
            'e' => Some(FormatType::Exp),
            'E' => Some(FormatType::ExpUpper),
            'f' => Some(FormatType::Fixed),
            '%' => Some(FormatType::Percent),
            _ => None,
        };
    }
}

fn is_align_char(c: char) -> bool {
    matches!(c, '<' | '>' | '^')
}

fn parse_align(c: char) -> Align {
    match c {
        '<' => Align::Left,
        '^' => Align::Center,
        _ => Align::Right,
    }
}

fn is_format_type(c: char) -> bool {
    matches!(c, 'b' | 'o' | 'x' | 'X' | 'e' | 'E' | 'f' | '%')
}

// =============================================================================
// Integer Formatting
// =============================================================================

fn format_int(n: i64, spec: &ParsedFormatSpec) -> String {
    let (is_negative, abs_n) = if n < 0 {
        (true, n.unsigned_abs())
    } else {
        (false, n as u64)
    };

    let (digits, prefix) = match spec.format_type {
        Some(FormatType::Binary) => {
            let p = if spec.alternate { "0b" } else { "" };
            (format!("{abs_n:b}"), p)
        }
        Some(FormatType::Octal) => {
            let p = if spec.alternate { "0o" } else { "" };
            (format!("{abs_n:o}"), p)
        }
        Some(FormatType::Hex) => {
            let p = if spec.alternate { "0x" } else { "" };
            (format!("{abs_n:x}"), p)
        }
        Some(FormatType::HexUpper) => {
            let p = if spec.alternate { "0X" } else { "" };
            (format!("{abs_n:X}"), p)
        }
        _ => (format!("{abs_n}"), ""),
    };

    let sign = format_sign(is_negative, spec);
    let core = format!("{sign}{prefix}{digits}");

    if spec.zero_pad {
        if let Some(width) = spec.width {
            let core_len = core.chars().count();
            if core_len < width {
                let pad = width - sign.len() - prefix.len();
                return format!("{sign}{prefix}{digits:0>pad$}");
            }
        }
    }

    apply_alignment(&core, spec)
}

// =============================================================================
// Float Formatting
// =============================================================================

fn format_float(f: f64, spec: &ParsedFormatSpec) -> String {
    let is_negative = f.is_sign_negative() && !f.is_nan();
    let abs_f = f.abs();

    let digits = match spec.format_type {
        Some(FormatType::Exp) => format_scientific(abs_f, false, spec.precision),
        Some(FormatType::ExpUpper) => format_scientific(abs_f, true, spec.precision),
        Some(FormatType::Fixed) => {
            let prec = spec.precision.unwrap_or(6);
            format!("{abs_f:.prec$}")
        }
        Some(FormatType::Percent) => format_percent(abs_f, spec.precision),
        _ => {
            if let Some(prec) = spec.precision {
                format!("{abs_f:.prec$}")
            } else {
                format!("{abs_f}")
            }
        }
    };

    let sign = format_sign(is_negative, spec);
    let core = format!("{sign}{digits}");

    if spec.zero_pad {
        if let Some(width) = spec.width {
            let core_len = core.chars().count();
            if core_len < width {
                let pad = width - sign.len();
                return format!("{sign}{digits:0>pad$}");
            }
        }
    }

    apply_alignment(&core, spec)
}

fn format_percent(abs_f: f64, precision: Option<usize>) -> String {
    let pct = abs_f * 100.0;
    if let Some(prec) = precision {
        format!("{pct:.prec$}%")
    } else {
        format!("{pct}%")
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "log10 exponent fits in i32 for any finite f64"
)]
fn format_scientific(f: f64, uppercase: bool, precision: Option<usize>) -> String {
    let e = if uppercase { 'E' } else { 'e' };

    if f == 0.0 {
        return if let Some(prec) = precision {
            if prec > 0 {
                let zeros: String = "0".repeat(prec);
                format!("0.{zeros}{e}0")
            } else {
                format!("0{e}0")
            }
        } else {
            format!("0{e}0")
        };
    }

    let exp = f.abs().log10().floor() as i32;
    let mantissa = f / 10f64.powi(exp);

    let mantissa_str = if let Some(prec) = precision {
        format!("{mantissa:.prec$}")
    } else {
        let s = format!("{mantissa:.15}");
        let trimmed = s.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        trimmed.to_string()
    };

    format!("{mantissa_str}{e}{exp}")
}

// =============================================================================
// String Formatting
// =============================================================================

fn fmt_str(s: &str, spec: &ParsedFormatSpec) -> String {
    let truncated = if let Some(prec) = spec.precision {
        if s.chars().count() > prec {
            s.chars().take(prec).collect::<String>()
        } else {
            s.to_string()
        }
    } else {
        s.to_string()
    };

    apply_alignment(&truncated, spec)
}

// =============================================================================
// Shared Helpers
// =============================================================================

fn format_sign(is_negative: bool, spec: &ParsedFormatSpec) -> &'static str {
    if is_negative {
        "-"
    } else {
        match spec.sign {
            Some(Sign::Plus) => "+",
            Some(Sign::Space) => " ",
            _ => "",
        }
    }
}

fn apply_alignment(s: &str, spec: &ParsedFormatSpec) -> String {
    let Some(width) = spec.width else {
        return s.to_string();
    };

    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }

    let fill = spec.fill.unwrap_or(' ');
    let padding = width - len;

    match spec.align.unwrap_or(Align::Left) {
        Align::Left => {
            let right_pad: String = std::iter::repeat_n(fill, padding).collect();
            format!("{s}{right_pad}")
        }
        Align::Right => {
            let left_pad: String = std::iter::repeat_n(fill, padding).collect();
            format!("{left_pad}{s}")
        }
        Align::Center => {
            let left = padding / 2;
            let right = padding - left;
            let left_pad: String = std::iter::repeat_n(fill, left).collect();
            let right_pad: String = std::iter::repeat_n(fill, right).collect();
            format!("{left_pad}{s}{right_pad}")
        }
    }
}
