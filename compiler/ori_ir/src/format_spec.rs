//! Format specification parser for template string interpolation.
//!
//! Parses the format spec syntax `[[fill]align][sign][#][0][width][.precision][type]`
//! used in template strings like `` `{value:>10.2f}` ``.
//!
//! The parsed result ([`ParsedFormatSpec`]) is consumed by:
//! - Type checker: validates format type against expression type
//! - Evaluator: applies type-specific formatting at runtime
//! - LLVM codegen: emits runtime format calls

use std::fmt;

/// Parsed format specification from a template interpolation.
///
/// All fields are `Option` — an empty spec `{x:}` produces all-`None`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParsedFormatSpec {
    /// Padding character (default: space).
    pub fill: Option<char>,
    /// Alignment direction.
    pub align: Option<Align>,
    /// Sign display for numbers.
    pub sign: Option<Sign>,
    /// Alternate form (`#`): adds `0b`/`0o`/`0x` prefix.
    pub alternate: bool,
    /// Zero-pad (`0`): pads with zeros, implies right-align for numbers.
    pub zero_pad: bool,
    /// Minimum field width.
    pub width: Option<usize>,
    /// Decimal places (floats) or max length (strings).
    pub precision: Option<usize>,
    /// Type-specific format (binary, hex, scientific, etc.).
    pub format_type: Option<FormatType>,
}

impl ParsedFormatSpec {
    /// An empty spec with no formatting options set.
    pub const EMPTY: Self = Self {
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

/// Alignment direction for field padding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    /// `<` — pad on the right.
    Left,
    /// `^` — pad equally on both sides.
    Center,
    /// `>` — pad on the left.
    Right,
}

/// Sign display mode for numeric values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sign {
    /// `+` — always show sign.
    Plus,
    /// `-` — show sign only for negatives (default numeric behavior).
    Minus,
    /// ` ` — space for positive, `-` for negative.
    Space,
}

/// Type-specific formatting mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FormatType {
    /// `b` — binary integer.
    Binary,
    /// `o` — octal integer.
    Octal,
    /// `x` — lowercase hexadecimal.
    Hex,
    /// `X` — uppercase hexadecimal.
    HexUpper,
    /// `e` — lowercase scientific notation.
    Exp,
    /// `E` — uppercase scientific notation.
    ExpUpper,
    /// `f` — fixed-point decimal.
    Fixed,
    /// `%` — percentage (multiply by 100, append `%`).
    Percent,
}

impl FormatType {
    /// Returns `true` if this format type is only valid for integer values.
    pub fn is_integer_only(&self) -> bool {
        matches!(
            self,
            Self::Binary | Self::Octal | Self::Hex | Self::HexUpper
        )
    }

    /// Returns `true` if this format type is only valid for float values.
    pub fn is_float_only(&self) -> bool {
        matches!(
            self,
            Self::Exp | Self::ExpUpper | Self::Fixed | Self::Percent
        )
    }

    /// Human-readable name for error messages.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Octal => "octal",
            Self::Hex | Self::HexUpper => "hex",
            Self::Exp | Self::ExpUpper => "scientific",
            Self::Fixed => "fixed-point",
            Self::Percent => "percentage",
        }
    }
}

/// Error from parsing a format specification string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatSpecError {
    /// Unknown format type character (e.g., `{n:z}`).
    UnknownType(char),
    /// Trailing characters after a valid spec.
    TrailingCharacters(String),
    /// Width is not a valid number.
    InvalidWidth(String),
    /// Precision is not a valid number.
    InvalidPrecision(String),
}

impl fmt::Display for FormatSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(c) => write!(f, "unknown format type '{c}'"),
            Self::TrailingCharacters(s) => write!(f, "unexpected characters '{s}' in format spec"),
            Self::InvalidWidth(s) => write!(f, "invalid width '{s}'"),
            Self::InvalidPrecision(s) => write!(f, "invalid precision '{s}'"),
        }
    }
}

/// Parse a format specification string.
///
/// Syntax: `[[fill]align][sign][#][0][width][.precision][type]`
///
/// # Examples
///
/// ```ignore
/// parse_format_spec(">10")     // align=Right, width=10
/// parse_format_spec("08x")     // zero_pad, width=8, type=Hex
/// parse_format_spec("*^20.5f") // fill='*', align=Center, width=20, precision=5, type=Fixed
/// parse_format_spec("")        // all defaults (empty spec)
/// ```
pub fn parse_format_spec(spec: &str) -> Result<ParsedFormatSpec, FormatSpecError> {
    if spec.is_empty() {
        return Ok(ParsedFormatSpec::EMPTY);
    }

    let mut result = ParsedFormatSpec::EMPTY;
    let chars: Vec<char> = spec.chars().collect();
    let mut pos = 0;

    // Parse [[fill]align]
    // Look ahead: if chars[1] is an alignment char, then chars[0] is fill.
    // Otherwise, if chars[0] is an alignment char, it's align with no fill.
    if chars.len() >= 2 && is_align_char(chars[1]) {
        result.fill = Some(chars[0]);
        result.align = Some(parse_align(chars[1]));
        pos = 2;
    } else if !chars.is_empty() && is_align_char(chars[0]) {
        result.align = Some(parse_align(chars[0]));
        pos = 1;
    }

    // Parse [sign]
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

    // Parse [#]
    if pos < chars.len() && chars[pos] == '#' {
        result.alternate = true;
        pos += 1;
    }

    // Parse [0] — zero-pad (must come before width digits)
    if pos < chars.len() && chars[pos] == '0' {
        // Only treat as zero-pad if followed by more digits (width) or end/type/precision.
        // A lone "0" as a width is handled below.
        if pos + 1 < chars.len() && chars[pos + 1].is_ascii_digit() {
            result.zero_pad = true;
            pos += 1;
        } else if pos + 1 >= chars.len() || chars[pos + 1] == '.' || is_format_type(chars[pos + 1])
        {
            // "0" alone or before precision/type means zero-pad with no width
            result.zero_pad = true;
            pos += 1;
        }
        // Otherwise it's the start of a width number
    }

    // Parse [width]
    let width_start = pos;
    while pos < chars.len() && chars[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos > width_start {
        let width_str: String = chars[width_start..pos].iter().collect();
        result.width = Some(
            width_str
                .parse()
                .map_err(|_| FormatSpecError::InvalidWidth(width_str))?,
        );
    }

    // Parse [.precision]
    if pos < chars.len() && chars[pos] == '.' {
        pos += 1; // skip '.'
        let prec_start = pos;
        while pos < chars.len() && chars[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos > prec_start {
            let prec_str: String = chars[prec_start..pos].iter().collect();
            result.precision = Some(
                prec_str
                    .parse()
                    .map_err(|_| FormatSpecError::InvalidPrecision(prec_str))?,
            );
        } else {
            // `.` with no digits means precision 0
            result.precision = Some(0);
        }
    }

    // Parse [type]
    if pos < chars.len() {
        let type_char = chars[pos];
        result.format_type = Some(match type_char {
            'b' => FormatType::Binary,
            'o' => FormatType::Octal,
            'x' => FormatType::Hex,
            'X' => FormatType::HexUpper,
            'e' => FormatType::Exp,
            'E' => FormatType::ExpUpper,
            'f' => FormatType::Fixed,
            '%' => FormatType::Percent,
            c => return Err(FormatSpecError::UnknownType(c)),
        });
        pos += 1;
    }

    // Check for trailing characters
    if pos < chars.len() {
        let trailing: String = chars[pos..].iter().collect();
        return Err(FormatSpecError::TrailingCharacters(trailing));
    }

    Ok(result)
}

fn is_align_char(c: char) -> bool {
    matches!(c, '<' | '>' | '^')
}

fn parse_align(c: char) -> Align {
    match c {
        '<' => Align::Left,
        '^' => Align::Center,
        '>' => Align::Right,
        _ => unreachable!("is_align_char check failed"),
    }
}

fn is_format_type(c: char) -> bool {
    matches!(c, 'b' | 'o' | 'x' | 'X' | 'e' | 'E' | 'f' | '%')
}

#[cfg(test)]
mod tests;
