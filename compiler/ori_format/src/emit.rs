//! Scalar emission kernel — turns a value + parsed spec into the formatted
//! string. The single implementation both backends call.

use crate::spec::{Align, FormatType, ParsedFormatSpec, Sign};

/// Format an integer value according to the spec.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "subtraction guarded by `core_len < width` check"
)]
pub fn format_int(n: i64, spec: &ParsedFormatSpec) -> String {
    let (is_negative, abs_n) = if n < 0 {
        (true, n.unsigned_abs())
    } else {
        (false, n.cast_unsigned())
    };

    // Format the digits based on type
    let (digits, prefix) = match spec.format_type {
        Some(FormatType::Binary) => {
            let prefix = if spec.alternate { "0b" } else { "" };
            (format!("{abs_n:b}"), prefix)
        }
        Some(FormatType::Octal) => {
            let prefix = if spec.alternate { "0o" } else { "" };
            (format!("{abs_n:o}"), prefix)
        }
        Some(FormatType::Hex) => {
            let prefix = if spec.alternate { "0x" } else { "" };
            (format!("{abs_n:x}"), prefix)
        }
        Some(FormatType::HexUpper) => {
            let prefix = if spec.alternate { "0X" } else { "" };
            (format!("{abs_n:X}"), prefix)
        }
        _ => (format!("{abs_n}"), ""),
    };

    // Build sign string
    let sign = format_sign(is_negative, spec);

    // Assemble the number: sign + prefix + digits
    let core = format!("{sign}{prefix}{digits}");

    // Apply zero-padding if requested (padding goes between sign/prefix and digits)
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

/// Format a float value according to the spec.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "subtraction guarded by `core_len < width` check"
)]
pub fn format_float(f: f64, spec: &ParsedFormatSpec) -> String {
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
            // Default float formatting with optional precision
            if let Some(prec) = spec.precision {
                format!("{abs_f:.prec$}")
            } else {
                format!("{abs_f}")
            }
        }
    };

    let sign = format_sign(is_negative, spec);
    let core = format!("{sign}{digits}");

    // Zero-padding for floats
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

/// Format a string value according to the spec.
pub fn format_str(s: &str, spec: &ParsedFormatSpec) -> String {
    // No-op fast path: no precision truncation or width/alignment needed
    if spec.precision.is_none() && spec.width.is_none() {
        return s.to_string();
    }

    // Apply precision as max length for strings
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

/// Multiply by 100 and append `%`, honoring precision.
fn format_percent(abs_f: f64, precision: Option<usize>) -> String {
    let pct = abs_f * 100.0;
    if let Some(prec) = precision {
        format!("{pct:.prec$}%")
    } else {
        format!("{pct}%")
    }
}

/// Apply width and alignment to a formatted string.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "subtraction guarded by `len >= width` early return"
)]
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

/// Build the sign prefix for a numeric value.
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

/// Format a float in scientific notation.
///
/// When precision is specified, uses that many decimal places.
/// When precision is omitted, strips trailing zeros for compact output.
fn format_scientific(f: f64, uppercase: bool, precision: Option<usize>) -> String {
    let e = if uppercase { 'E' } else { 'e' };

    // Why: never run exponent math on a non-finite value. format_float does not
    // special-case NaN/Inf before dispatch (is_sign_negative only suppresses the
    // sign flag), so guard here. Rust Display gives "NaN"/"inf".
    if !f.is_finite() {
        return format!("{f}");
    }

    if f == 0.0 {
        return if let Some(prec) = precision {
            if prec > 0 {
                let zeros: String = "0".repeat(prec);
                format!("0.{zeros}{e}+00")
            } else {
                format!("0{e}+00")
            }
        } else {
            format!("0{e}+00")
        };
    }

    // Rust's own scientific formatter normalizes the mantissa into [1,10), rounds
    // correctly, performs the decade-carry, and handles subnormals and f64
    // extremes. f is already f.abs() per the dispatch; sign is the caller's.
    let rust_sci = match precision {
        Some(prec) => format!("{f:.prec$e}"),
        None => format!("{f:e}"),
    };

    // INVARIANT: Rust's {:e} emits "<mantissa>e<exp>" with <exp> a valid i32 for every
    // finite f (non-finite is guarded above). unreachable! keeps a broken invariant loud
    // rather than silently emitting exponent 0.
    let Some((mantissa_raw, exp_str)) = rust_sci.split_once('e') else {
        unreachable!("Rust scientific format lacks 'e' separator: {rust_sci}");
    };
    let raw_exp: i32 = match exp_str.parse() {
        Ok(exp) => exp,
        Err(_) => unreachable!("scientific exponent not a valid i32: {exp_str}"),
    };

    // No precision: Rust {:e} already gives the shortest round-trip mantissa;
    // trim any ".0" tail so e.g. "1.0" renders as "1".
    let mantissa_str = if precision.is_none() && mantissa_raw.contains('.') {
        mantissa_raw.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa_raw
    };

    // Render exponent C-printf-%e style: explicit sign + zero-padded min-2-digit
    // magnitude (>=3 digits render full width).
    let sign = if raw_exp >= 0 { '+' } else { '-' };
    let mag = raw_exp.unsigned_abs();
    format!("{mantissa_str}{e}{sign}{mag:02}")
}
