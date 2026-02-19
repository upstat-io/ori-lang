//! Format specification evaluation for template string interpolation.
//!
//! Handles `CanExpr::FormatWith { expr, spec }` by evaluating the expression,
//! parsing the format spec, and applying type-specific formatting.
//!
//! Supports:
//! - Integer formatting: decimal, binary, octal, hex, sign, alternate, zero-pad
//! - Float formatting: default, scientific, fixed-point, percentage, precision
//! - String formatting: precision truncation, width, alignment
//! - Blanket: Printable values with width/alignment via `to_str()` fallback

use ori_ir::canon::CanId;
use ori_ir::format_spec::{parse_format_spec, Align, FormatType, ParsedFormatSpec, Sign};
use ori_ir::Name;
use ori_patterns::{EvalError, EvalResult, StructValue, Value};
use rustc_hash::FxHashMap;

use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a `FormatWith` expression: format `expr` using `spec`.
    ///
    /// Dispatch order:
    /// 1. Built-in types (int, float, str, bool, char) — fast path, no struct construction
    /// 2. User types with `Formattable` impl — construct `FormatSpec` value, call `format()`
    /// 3. Fallback — `display_value()` + alignment (blanket impl behavior)
    pub(super) fn eval_format_with(
        &mut self,
        can_id: CanId,
        expr: CanId,
        spec: Name,
    ) -> EvalResult {
        let value = self.eval_can(expr)?;
        let spec_str = self.interner.lookup(spec);

        let parsed = parse_format_spec(spec_str).map_err(|e| {
            let span = self.can_span(can_id);
            Self::attach_span(
                EvalError::new(format!("invalid format spec: {e}")).into(),
                span,
            )
        })?;

        let result = match &value {
            // Fast path: built-in type formatting
            Value::Int(n) => format_int(n.raw(), &parsed),
            Value::Float(f) => format_float(*f, &parsed),
            Value::Str(s) => format_str(s, &parsed),
            Value::Bool(b) => {
                let s = if *b { "true" } else { "false" };
                format_str(s, &parsed)
            }
            Value::Char(c) => format_str(&c.to_string(), &parsed),
            // User types: check for Formattable impl, then blanket fallback
            _ => {
                let format_method = self.interner.intern("format");
                let type_name = self.get_value_type_name(&value);
                let has_user_impl = self
                    .user_method_registry
                    .read()
                    .has_method(type_name, format_method);

                if has_user_impl {
                    let spec_value = build_format_spec_value(&parsed, self.interner);
                    let result = self.eval_method_call(value, format_method, vec![spec_value])?;
                    return Ok(result);
                }

                // Blanket fallback: display_value() + alignment
                let base = value.display_value();
                apply_alignment(&base, &parsed)
            }
        };

        Ok(Value::string(result))
    }
}

/// Build a `Value::Struct(FormatSpec{...})` from a parsed format spec.
///
/// Converts the Rust-side `ParsedFormatSpec` to an Ori-side `FormatSpec` struct value
/// for passing to user-defined `Formattable::format()` implementations.
fn build_format_spec_value(parsed: &ParsedFormatSpec, interner: &ori_ir::StringInterner) -> Value {
    let type_name = interner.intern("FormatSpec");
    let fill_name = interner.intern("fill");
    let align_name = interner.intern("align");
    let sign_name = interner.intern("sign");
    let width_name = interner.intern("width");
    let precision_name = interner.intern("precision");
    let format_type_name = interner.intern("format_type");

    let fill_val = match parsed.fill {
        Some(c) => Value::some(Value::Char(c)),
        None => Value::None,
    };

    let align_val = match parsed.align {
        Some(align) => {
            let alignment_type = interner.intern("Alignment");
            let variant = match align {
                Align::Left => interner.intern("Left"),
                Align::Center => interner.intern("Center"),
                Align::Right => interner.intern("Right"),
            };
            Value::some(Value::variant(alignment_type, variant, vec![]))
        }
        None => Value::None,
    };

    let sign_val = match parsed.sign {
        Some(sign) => {
            let sign_type = interner.intern("Sign");
            let variant = match sign {
                Sign::Plus => interner.intern("Plus"),
                Sign::Minus => interner.intern("Minus"),
                Sign::Space => interner.intern("Space"),
            };
            Value::some(Value::variant(sign_type, variant, vec![]))
        }
        None => Value::None,
    };

    let width_val = match parsed.width {
        Some(w) => Value::some(Value::int(i64::try_from(w).unwrap_or(i64::MAX))),
        None => Value::None,
    };

    let precision_val = match parsed.precision {
        Some(p) => Value::some(Value::int(i64::try_from(p).unwrap_or(i64::MAX))),
        None => Value::None,
    };

    let format_type_val = match parsed.format_type {
        Some(ft) => {
            let ft_type = interner.intern("FormatType");
            let variant = match ft {
                FormatType::Binary => interner.intern("Binary"),
                FormatType::Octal => interner.intern("Octal"),
                FormatType::Hex => interner.intern("Hex"),
                FormatType::HexUpper => interner.intern("HexUpper"),
                FormatType::Exp => interner.intern("Exp"),
                FormatType::ExpUpper => interner.intern("ExpUpper"),
                FormatType::Fixed => interner.intern("Fixed"),
                FormatType::Percent => interner.intern("Percent"),
            };
            Value::some(Value::variant(ft_type, variant, vec![]))
        }
        None => Value::None,
    };

    let mut fields = FxHashMap::default();
    fields.insert(fill_name, fill_val);
    fields.insert(align_name, align_val);
    fields.insert(sign_name, sign_val);
    fields.insert(width_name, width_val);
    fields.insert(precision_name, precision_val);
    fields.insert(format_type_name, format_type_val);

    Value::Struct(StructValue::new(type_name, fields))
}

/// Format an integer value according to the spec.
#[expect(
    clippy::arithmetic_side_effects,
    reason = "subtraction guarded by `core_len < width` check"
)]
fn format_int(n: i64, spec: &ParsedFormatSpec) -> String {
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
        Some(FormatType::Percent) => {
            let pct = abs_f * 100.0;
            if let Some(prec) = spec.precision {
                format!("{pct:.prec$}%")
            } else {
                // No precision: strip trailing zeros
                let s = format!("{pct}");
                format!("{s}%")
            }
        }
        _ => {
            // Default float formatting with optional precision
            if let Some(prec) = spec.precision {
                format!("{abs_f:.prec$}")
            } else {
                format_float_default(abs_f)
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
fn format_str(s: &str, spec: &ParsedFormatSpec) -> String {
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
#[expect(
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
        // No precision: strip trailing zeros
        let s = format!("{mantissa:.15}");
        let trimmed = s.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        trimmed.to_string()
    };

    format!("{mantissa_str}{e}{exp}")
}

/// Format a float with default representation (no trailing zeros beyond one decimal).
fn format_float_default(f: f64) -> String {
    // Use Rust's default float formatting which already handles this well
    let s = format!("{f}");
    s
}

#[cfg(test)]
mod tests;
