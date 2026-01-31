//! Literal Value Formatting
//!
//! Methods for emitting literal values (ints, floats, strings, chars, etc.).

use ori_ir::StringLookup;

use super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    pub(super) fn emit_int(&mut self, n: i64) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{n}");
        self.ctx.emit(&buf);
    }

    pub(super) fn emit_float(&mut self, f: f64) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        if f.fract() == 0.0 {
            let _ = write!(buf, "{f:.1}");
        } else {
            let _ = write!(buf, "{f}");
        }
        self.ctx.emit(&buf);
    }

    pub(super) fn emit_string(&mut self, s: &str) {
        self.ctx.emit("\"");
        for c in s.chars() {
            match c {
                '\\' => self.ctx.emit("\\\\"),
                '"' => self.ctx.emit("\\\""),
                '\n' => self.ctx.emit("\\n"),
                '\t' => self.ctx.emit("\\t"),
                '\r' => self.ctx.emit("\\r"),
                '\0' => self.ctx.emit("\\0"),
                _ => {
                    let mut buf = [0; 4];
                    self.ctx.emit(c.encode_utf8(&mut buf));
                }
            }
        }
        self.ctx.emit("\"");
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
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{value}");
        self.ctx.emit(&buf);
        self.ctx.emit(unit.suffix());
    }

    pub(super) fn emit_size(&mut self, value: u64, unit: ori_ir::SizeUnit) {
        use std::fmt::Write;
        let mut buf = String::new();
        // Writing to a String is infallible
        let _ = write!(buf, "{value}");
        self.ctx.emit(&buf);
        self.ctx.emit(unit.suffix());
    }
}
