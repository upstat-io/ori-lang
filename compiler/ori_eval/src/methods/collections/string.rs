//! Free-function method dispatch for `str` values.
//!
//! These dispatchers need no interpreter access (no user `@hash`/`@eq` calls).

use ori_ir::Name;
use ori_patterns::{no_such_method, EvalResult, IteratorValue, Value};

use super::super::arguments::{nonnegative_usize, require_args, require_int_arg, require_str_arg};
use super::super::compare::{fnv1a_hash, ordering_to_value};
use super::super::debug_format::escape_debug_str;
use super::super::length::len_to_value;
use super::super::DispatchCtx;

#[derive(Clone, Copy)]
enum PadSide {
    Start,
    End,
}

impl PadSide {
    const fn method_name(self) -> &'static str {
        match self {
            Self::Start => "pad_start",
            Self::End => "pad_end",
        }
    }
}

#[derive(Clone, Copy)]
enum Utf8DecodeMode {
    CheckedResult,
    LossyString,
}

impl Utf8DecodeMode {
    const fn method_name(self) -> &'static str {
        match self {
            Self::CheckedResult => "from_utf8",
            Self::LossyString => "from_utf8_unchecked",
        }
    }
}

/// Dispatch methods on string values.
#[expect(
    clippy::needless_pass_by_value,
    reason = "args: Vec<Value> for consistent dispatch signature across collection types"
)]
pub fn dispatch_string_method(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &DispatchCtx<'_>,
) -> EvalResult {
    let Value::Str(mut s) = receiver else {
        unreachable!("dispatch_string_method called with non-string receiver")
    };

    let n = ctx.names;

    if method == n.len || method == n.length {
        len_to_value(s.len(), "string")
    } else if method == n.is_empty {
        Ok(Value::Bool(s.is_empty()))
    } else if method == n.to_uppercase {
        Ok(Value::string(s.to_uppercase()))
    } else if method == n.to_lowercase {
        Ok(Value::string(s.to_lowercase()))
    } else if method == n.trim {
        Ok(Value::string(s.trim().to_string()))
    } else if method == n.contains {
        require_args("contains", 1, args.len())?;
        let needle = require_str_arg("contains", &args, 0)?;
        Ok(Value::Bool(s.contains(needle)))
    } else if method == n.starts_with {
        require_args("starts_with", 1, args.len())?;
        let prefix = require_str_arg("starts_with", &args, 0)?;
        Ok(Value::Bool(s.starts_with(prefix)))
    } else if method == n.ends_with {
        require_args("ends_with", 1, args.len())?;
        let suffix = require_str_arg("ends_with", &args, 0)?;
        Ok(Value::Bool(s.ends_with(suffix)))
    } else if method == n.add || method == n.concat {
        require_args("concat", 1, args.len())?;
        let other = require_str_arg("concat", &args, 0)?;
        s.make_mut().to_mut().push_str(other);
        Ok(Value::Str(s))
    } else if method == n.substring || method == n.slice {
        let name = if method == n.substring {
            "substring"
        } else {
            "slice"
        };
        eval_str_slice(&s, name, &args)
    // Comparable trait - lexicographic (Unicode codepoint)
    } else if method == n.compare {
        require_args("compare", 1, args.len())?;
        let other = require_str_arg("compare", &args, 0)?;
        Ok(ordering_to_value((**s).cmp(other)))
    // Eq trait
    } else if method == n.equals {
        require_args("equals", 1, args.len())?;
        let other = require_str_arg("equals", &args, 0)?;
        Ok(Value::Bool(&**s == other))
    // Iterable trait - create character iterator
    } else if method == n.iter {
        require_args("iter", 0, args.len())?;
        Ok(Value::iterator(IteratorValue::from_string(s)))
    // Clone trait
    } else if method == n.clone_ {
        require_args("clone", 0, args.len())?;
        Ok(Value::string(s.to_string()))
    // Printable trait - returns the string itself
    } else if method == n.to_str {
        require_args("to_str", 0, args.len())?;
        Ok(Value::string(s.to_string()))
    // escape - returns string with special characters escaped
    } else if method == n.escape {
        require_args("escape", 0, args.len())?;
        Ok(Value::string(escape_debug_str(&s)))
    // Debug trait - shows escaped string with quotes
    } else if method == n.debug {
        require_args("debug", 0, args.len())?;
        Ok(Value::string(format!("\"{}\"", escape_debug_str(&s))))
    // Hashable trait
    } else if method == n.hash {
        require_args("hash", 0, args.len())?;
        Ok(Value::int(fnv1a_hash(s.as_bytes())))
    // replace(pattern, replacement) -> str
    } else if method == n.replace {
        require_args("replace", 2, args.len())?;
        let pattern = require_str_arg("replace", &args, 0)?;
        let replacement = require_str_arg("replace", &args, 1)?;
        Ok(Value::string(s.replace(pattern, replacement)))
    // split(separator) -> [str]
    } else if method == n.split {
        str_split(&s, &args)
    // repeat(count) -> str
    } else if method == n.repeat {
        str_repeat(&s, &args)
    // Into trait: str -> Error (wraps string as error message)
    } else if method == n.into {
        require_args("into", 0, args.len())?;
        Ok(Value::error(s.to_string()))
    // Additional str methods (cold path — string-based dispatch)
    } else {
        let method_str = ctx.interner.lookup(method);
        dispatch_string_method_str(&s, method_str, &args)
    }
}

/// String-based dispatch for str methods not hot enough to warrant
/// pre-interned Name fields.
fn dispatch_string_method_str(s: &str, method: &str, args: &[Value]) -> EvalResult {
    match method {
        "as_bytes" | "to_bytes" | "bytes" => str_as_bytes(s, method, args),
        "byte_len" => {
            require_args("byte_len", 0, args.len())?;
            len_to_value(s.len(), "str")
        }
        "chars" => {
            require_args("chars", 0, args.len())?;
            Ok(Value::list(s.chars().map(Value::Char).collect()))
        }
        "index_of" => str_find_char_index(s, "index_of", args, |h, n| h.find(n)),
        "last_index_of" => str_find_char_index(s, "last_index_of", args, |h, n| h.rfind(n)),
        "lines" => str_lines(s, args),
        "pad_start" => str_pad(s, args, PadSide::Start),
        "pad_end" => str_pad(s, args, PadSide::End),
        "parse_int" | "to_int" => str_parse_int(s, method, args),
        "parse_float" | "to_float" => str_parse_float(s, method, args),
        "trim_start" => {
            require_args("trim_start", 0, args.len())?;
            Ok(Value::string(s.trim_start().to_string()))
        }
        "trim_end" => {
            require_args("trim_end", 0, args.len())?;
            Ok(Value::string(s.trim_end().to_string()))
        }
        "from_utf8" => str_from_utf8(args, Utf8DecodeMode::CheckedResult),
        "from_utf8_unchecked" => str_from_utf8(args, Utf8DecodeMode::LossyString),
        _ => Err(no_such_method(method, "str").into()),
    }
}

fn str_split(s: &str, args: &[Value]) -> EvalResult {
    require_args("split", 1, args.len())?;
    let sep = require_str_arg("split", args, 0)?;
    let parts: Vec<Value> = if sep.is_empty() {
        // Empty separator: split into individual characters
        s.chars().map(|c| Value::string(c.to_string())).collect()
    } else {
        s.split(sep).map(|p| Value::string(p.to_string())).collect()
    };
    Ok(Value::list(parts))
}

fn str_repeat(s: &str, args: &[Value]) -> EvalResult {
    require_args("repeat", 1, args.len())?;
    let count = require_int_arg("repeat", args, 0)?;
    let count = nonnegative_usize(count, "repeat", "non-negative int")?;
    Ok(Value::string(s.repeat(count)))
}

fn str_as_bytes(s: &str, method: &str, args: &[Value]) -> EvalResult {
    require_args(method, 0, args.len())?;
    Ok(Value::list(
        s.as_bytes().iter().map(|b| Value::Byte(*b)).collect(),
    ))
}

fn str_lines(s: &str, args: &[Value]) -> EvalResult {
    require_args("lines", 0, args.len())?;
    Ok(Value::list(
        s.lines()
            .map(|line| Value::string(line.to_string()))
            .collect(),
    ))
}

fn str_parse_int(s: &str, method: &str, args: &[Value]) -> EvalResult {
    require_args(method, 0, args.len())?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(Value::some(Value::int(n))),
        Err(_) => Ok(Value::None),
    }
}

fn str_parse_float(s: &str, method: &str, args: &[Value]) -> EvalResult {
    require_args(method, 0, args.len())?;
    match s.trim().parse::<f64>() {
        Ok(f) => Ok(Value::some(Value::Float(f))),
        Err(_) => Ok(Value::None),
    }
}

/// Find a substring and return its char index as `Option<int>`.
fn str_find_char_index(
    s: &str,
    method: &str,
    args: &[Value],
    finder: fn(&str, &str) -> Option<usize>,
) -> EvalResult {
    require_args(method, 1, args.len())?;
    let needle = require_str_arg(method, args, 0)?;
    match finder(s, needle) {
        Some(byte_idx) => {
            #[expect(clippy::cast_possible_wrap, reason = "char count fits in i64")]
            let char_idx = s[..byte_idx].chars().count() as i64;
            Ok(Value::some(Value::int(char_idx)))
        }
        None => Ok(Value::None),
    }
}

/// Pad a string to `width` characters using `fill` on the selected side.
fn str_pad(s: &str, args: &[Value], side: PadSide) -> EvalResult {
    let method = side.method_name();
    require_args(method, 2, args.len())?;
    let width = require_int_arg(method, args, 0)?;
    let fill = require_str_arg(method, args, 1)?;
    let current_len = s.chars().count();
    let width_usize = usize::try_from(width).unwrap_or(0);
    if current_len >= width_usize || fill.is_empty() {
        return Ok(Value::string(s.to_string()));
    }
    let pad_count = width_usize.saturating_sub(current_len);
    let pad: String = fill.chars().cycle().take(pad_count).collect();
    match side {
        PadSide::Start => Ok(Value::string(format!("{pad}{s}"))),
        PadSide::End => Ok(Value::string(format!("{s}{pad}"))),
    }
}

/// Parse a `[byte]` argument into a `Vec<u8>`, then convert to a string.
fn str_from_utf8(args: &[Value], mode: Utf8DecodeMode) -> EvalResult {
    let method = mode.method_name();
    require_args(method, 1, args.len())?;
    let Value::List(ref bytes) = args[0] else {
        return Err(ori_patterns::wrong_arg_type(method, "[byte]").into());
    };
    let byte_vec: Result<Vec<u8>, _> = bytes
        .iter()
        .map(|v| match v {
            Value::Byte(b) => Ok(*b),
            _ => Err(ori_patterns::wrong_arg_type(method, "[byte]")),
        })
        .collect();
    let bv = byte_vec?;
    match mode {
        Utf8DecodeMode::CheckedResult => match String::from_utf8(bv) {
            Ok(s) => Ok(Value::ok(Value::string(s))),
            Err(e) => Ok(Value::err(Value::error(e.to_string()))),
        },
        // Why: from_utf8_unchecked still validates in the interpreter (checked
        // decode + lossy fallback) rather than trusting the bytes.
        Utf8DecodeMode::LossyString => match String::from_utf8(bv) {
            Ok(s) => Ok(Value::string(s)),
            Err(e) => Ok(Value::string(
                String::from_utf8_lossy(e.as_bytes()).into_owned(),
            )),
        },
    }
}

/// `substring`/`slice` — extract char-indexed substring.
fn eval_str_slice(s: &str, name: &str, args: &[Value]) -> EvalResult {
    require_args(name, 2, args.len())?;
    let start = require_int_arg(name, args, 0)?;
    let end = require_int_arg(name, args, 1)?;
    let ustart = nonnegative_usize(start, name, "non-negative int")?;
    let uend = nonnegative_usize(end, name, "non-negative int")?;
    let chars: Vec<char> = s.chars().collect();
    let uend = uend.min(chars.len());
    let ustart = ustart.min(uend);
    let result: String = chars[ustart..uend].iter().collect();
    Ok(Value::string(result))
}

#[cfg(test)]
mod tests;
