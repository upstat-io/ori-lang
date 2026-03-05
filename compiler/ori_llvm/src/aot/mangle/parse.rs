//! Demangling — symbol name parsing back to Ori-style form.
//!
//! Contains [`demangle()`] and the internal [`DemangleParser`] that converts
//! mangled symbol names back to human-readable Ori syntax.

use super::{MANGLE_PREFIX, MODULE_SEP};

/// Demangle an Ori symbol name back to its original Ori-style form.
///
/// # Arguments
///
/// * `mangled` - The mangled symbol name
///
/// # Returns
///
/// The demangled name in Ori syntax (e.g., `math.@add`), or `None` if not a valid Ori symbol.
///
/// # Output Format
///
/// - Module functions: `_ori_math$add` → `math.@add`
/// - Nested modules: `_ori_http$client$connect` → `http/client.@connect`
/// - Trait impls: `_ori_int$$Eq$equals` → `int::Eq.@equals`
/// - Associated fns: `_ori_Option$A$some` → `Option.@some`
#[must_use]
pub fn demangle(mangled: &str) -> Option<String> {
    let rest = mangled.strip_prefix(MANGLE_PREFIX)?;
    let parsed = DemangleParser::parse(rest)?;
    Some(parsed.format())
}

/// Internal parser state for demangling.
struct DemangleParser {
    segments: Vec<String>,
    is_trait_impl: bool,
    is_associated: bool,
}

impl DemangleParser {
    /// Parse a mangled symbol (without prefix) into segments and flags.
    fn parse(input: &str) -> Option<Self> {
        let mut parser = Self {
            segments: Vec::new(),
            is_trait_impl: false,
            is_associated: false,
        };
        let mut current = String::new();
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                MODULE_SEP => parser.handle_separator(&mut chars, &mut current),
                '_' if current.ends_with('<') || current.ends_with(", ") => {
                    current.push_str(", ");
                }
                c => current.push(c),
            }
        }

        if !current.is_empty() {
            parser.segments.push(current);
        }

        if parser.segments.is_empty() {
            return None;
        }
        Some(parser)
    }

    /// Handle `$` separator and its escape sequences.
    fn handle_separator(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        match chars.peek().copied() {
            Some('$') => {
                chars.next();
                self.push_segment(current);
                self.is_trait_impl = true;
            }
            Some('L') => Self::decode_left_bracket(chars, current),
            Some('G') => {
                chars.next();
                current.push('<');
            }
            Some('R') => Self::decode_right_bracket(chars, current),
            Some('C') => Self::decode_comma_or_colons(chars, current),
            Some('D') => {
                chars.next();
                current.push('-');
            }
            Some('A') => self.handle_associated_marker(chars, current),
            Some('e') => self.handle_extension_marker(chars, current),
            _ => self.push_segment(current),
        }
    }

    fn decode_left_bracket(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        match chars.next() {
            Some('T') => current.push('<'),
            Some('B') => current.push('['),
            Some('P') => current.push('('),
            _ => current.push_str("$L"),
        }
    }

    fn decode_right_bracket(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        match chars.next() {
            Some('B') => current.push(']'),
            Some('P') => current.push(')'),
            _ => current.push_str("$R"),
        }
    }

    fn decode_comma_or_colons(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        if chars.peek() == Some(&'C') {
            chars.next();
            current.push_str("::");
        } else {
            current.push(',');
        }
    }

    fn handle_associated_marker(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        chars.next();
        if chars.peek() == Some(&'$') {
            chars.next();
            self.push_segment(current);
            self.is_associated = true;
        }
    }

    fn handle_extension_marker(
        &mut self,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        current: &mut String,
    ) {
        let peek: String = chars.clone().take(3).collect();
        if peek == "xt$" {
            chars.next(); // x
            chars.next(); // t
            chars.next(); // $
            self.push_segment(current);
            self.is_trait_impl = true;
        } else {
            self.push_segment(current);
        }
    }

    fn push_segment(&mut self, current: &mut String) {
        if !current.is_empty() {
            self.segments.push(std::mem::take(current));
        }
    }

    /// Format parsed segments into Ori-style output.
    fn format(mut self) -> String {
        let mut result = String::with_capacity(self.segments.iter().map(String::len).sum());

        if self.segments.len() == 1 {
            result.push('@');
            result.push_str(&self.segments[0]);
        } else if self.is_trait_impl {
            self.format_trait_impl(&mut result);
        } else if self.is_associated {
            self.format_associated(&mut result);
        } else {
            self.format_module_function(&mut result);
        }

        if result.contains('<') && !result.contains('>') {
            result.push('>');
        }
        result
    }

    fn format_trait_impl(&mut self, result: &mut String) {
        let method = self.segments.pop().unwrap();
        let trait_name = self.segments.pop();
        let type_name = self.segments.pop();
        if let Some(tn) = type_name {
            result.push_str(&tn);
        }
        if let Some(tr) = trait_name {
            result.push_str("::");
            result.push_str(&tr);
        }
        result.push_str(".@");
        result.push_str(&method);
    }

    fn format_associated(&mut self, result: &mut String) {
        let method = self.segments.pop().unwrap();
        result.push_str(&self.segments.join("/"));
        result.push_str(".@");
        result.push_str(&method);
    }

    fn format_module_function(&mut self, result: &mut String) {
        let function = self.segments.pop().unwrap();
        if self.segments.is_empty() {
            result.push('@');
        } else {
            result.push_str(&self.segments.join("/"));
            result.push_str(".@");
        }
        result.push_str(&function);
    }
}
