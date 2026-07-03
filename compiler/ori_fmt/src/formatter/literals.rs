//! Literal Value Formatting
//!
//! Methods for emitting literal values (ints, floats, strings, chars, etc.).

use ori_ir::{ParsedType, StringLookup};

use super::Formatter;
use crate::context::FormatContext;
use crate::declarations::format_parsed_type;
use crate::emitter::StringEmitter;

/// Emit a string literal with surrounding quotes and escaped control / quote
/// characters. Free function so non-`Formatter` emitters share one escaping impl.
pub(crate) fn emit_escaped_str(ctx: &mut FormatContext<StringEmitter>, s: impl AsRef<str>) {
    let s = s.as_ref();
    ctx.emit("\"");
    for c in s.chars() {
        match c {
            '\\' => ctx.emit("\\\\"),
            '"' => ctx.emit("\\\""),
            '\n' => ctx.emit("\\n"),
            '\t' => ctx.emit("\\t"),
            '\r' => ctx.emit("\\r"),
            '\0' => ctx.emit("\\0"),
            _ => {
                let mut buf = [0; 4];
                ctx.emit(c.encode_utf8(&mut buf));
            }
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
        match c {
            '\\' => self.ctx.emit("\\\\"),
            '\'' => self.ctx.emit("\\'"),
            '\n' => self.ctx.emit("\\n"),
            '\t' => self.ctx.emit("\\t"),
            '\r' => self.ctx.emit("\\r"),
            '\0' => self.ctx.emit("\\0"),
            _ => {
                let mut buf = [0; 4];
                self.ctx.emit(c.encode_utf8(&mut buf));
            }
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
