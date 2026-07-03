//! Literal Value Formatting
//!
//! Methods for emitting literal values (ints, floats, strings, chars, etc.).

use ori_ir::{ParsedType, StringLookup};

use super::Formatter;
use crate::context::FormatContext;
use crate::declarations::format_parsed_type;
use crate::emitter::StringEmitter;

/// The escaped form of `c` inside a double-quoted string literal, or `None`
/// when `c` renders literally. SSOT for the string-escape set shared by
/// [`emit_escaped_str`] (emission) and `width::literals::string_width`
/// (width measurement) — the two must agree or line-fit decisions diverge
/// from the emitted text.
pub(crate) fn string_escape(c: char) -> Option<&'static str> {
    match c {
        '\\' => Some("\\\\"),
        '"' => Some("\\\""),
        '\n' => Some("\\n"),
        '\t' => Some("\\t"),
        '\r' => Some("\\r"),
        '\0' => Some("\\0"),
        _ => None,
    }
}

/// The escaped form of `c` inside a single-quoted char literal, or `None`
/// when `c` renders literally. Sibling of [`string_escape`] for char
/// literals (escapes `'` instead of `"`); shared by [`Formatter::emit_char`]
/// and `width::literals::char_width`.
pub(crate) fn char_escape(c: char) -> Option<&'static str> {
    match c {
        '\\' => Some("\\\\"),
        '\'' => Some("\\'"),
        '\n' => Some("\\n"),
        '\t' => Some("\\t"),
        '\r' => Some("\\r"),
        '\0' => Some("\\0"),
        _ => None,
    }
}

/// Emit a string literal with surrounding quotes and escaped control / quote
/// characters. Free function so non-`Formatter` emitters share one escaping impl.
pub(crate) fn emit_escaped_str(ctx: &mut FormatContext<StringEmitter>, s: impl AsRef<str>) {
    let s = s.as_ref();
    ctx.emit("\"");
    for c in s.chars() {
        if let Some(esc) = string_escape(c) {
            ctx.emit(esc);
        } else {
            let mut buf = [0; 4];
            ctx.emit(c.encode_utf8(&mut buf));
        }
    }
    ctx.emit("\"");
}

impl<I: StringLookup> Formatter<'_, I> {
    pub(super) fn emit_int(&mut self, n: i64) {
        // i64 max is 20 digits; itoa would be zero-alloc but isn't a dep.
        // format! is clear and fast enough for a formatter.
        self.ctx.emit(format!("{n}"));
    }

    pub(super) fn emit_float(&mut self, f: f64) {
        if f.fract() == 0.0 {
            self.ctx.emit(format!("{f:.1}"));
        } else {
            self.ctx.emit(format!("{f}"));
        }
    }

    pub(super) fn emit_string(&mut self, s: impl AsRef<str>) {
        emit_escaped_str(&mut self.ctx, s);
    }

    pub(super) fn emit_char(&mut self, c: char) {
        self.ctx.emit("'");
        if let Some(esc) = char_escape(c) {
            self.ctx.emit(esc);
        } else {
            let mut buf = [0; 4];
            self.ctx.emit(c.encode_utf8(&mut buf));
        }
        self.ctx.emit("'");
    }

    pub(super) fn emit_duration(&mut self, value: u64, unit: ori_ir::DurationUnit) {
        self.ctx.emit(format!("{value}"));
        self.ctx.emit(unit.suffix());
    }

    pub(super) fn emit_size(&mut self, value: u64, unit: ori_ir::SizeUnit) {
        self.ctx.emit(format!("{value}"));
        self.ctx.emit(unit.suffix());
    }

    /// Emit a parsed type annotation.
    ///
    /// Delegates to the canonical `format_parsed_type` in `declarations::parsed_types`
    /// to ensure consistent type rendering across expression and declaration contexts.
    pub(super) fn emit_type(&mut self, ty: &ParsedType) {
        format_parsed_type(ty, self.arena, self.interner, &mut self.ctx);
    }
}
