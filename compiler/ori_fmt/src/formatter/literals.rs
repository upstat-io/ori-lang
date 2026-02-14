//! Literal Value Formatting
//!
//! Methods for emitting literal values (ints, floats, strings, chars, etc.).

use ori_ir::{ParsedType, StringLookup};

use super::Formatter;
use crate::declarations::format_const_expr;

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

    /// Emit a parsed type annotation.
    pub(super) fn emit_type(&mut self, ty: &ParsedType) {
        match ty {
            ParsedType::Primitive(type_id) => {
                // Use the type ID's display name
                let name = type_id.name().unwrap_or("?");
                self.ctx.emit(name);
            }
            ParsedType::Named { name, type_args } => {
                self.ctx.emit(self.interner.lookup(*name));
                if !type_args.is_empty() {
                    self.ctx.emit("<");
                    let type_ids = self.arena.get_parsed_type_list(*type_args);
                    for (i, type_id) in type_ids.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        let t = self.arena.get_parsed_type(*type_id);
                        self.emit_type(t);
                    }
                    self.ctx.emit(">");
                }
            }
            ParsedType::List(elem) => {
                self.ctx.emit("[");
                let elem_ty = self.arena.get_parsed_type(*elem);
                self.emit_type(elem_ty);
                self.ctx.emit("]");
            }
            ParsedType::FixedList { elem, capacity } => {
                self.ctx.emit("[");
                let elem_ty = self.arena.get_parsed_type(*elem);
                self.emit_type(elem_ty);
                self.ctx.emit(", max ");
                format_const_expr(*capacity, self.arena, self.interner, &mut self.ctx);
                self.ctx.emit("]");
            }
            ParsedType::Map { key, value } => {
                self.ctx.emit("{");
                let key_ty = self.arena.get_parsed_type(*key);
                self.emit_type(key_ty);
                self.ctx.emit(": ");
                let val_ty = self.arena.get_parsed_type(*value);
                self.emit_type(val_ty);
                self.ctx.emit("}");
            }
            ParsedType::Tuple(elems) => {
                self.ctx.emit("(");
                let type_ids = self.arena.get_parsed_type_list(*elems);
                for (i, type_id) in type_ids.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let t = self.arena.get_parsed_type(*type_id);
                    self.emit_type(t);
                }
                self.ctx.emit(")");
            }
            ParsedType::Function { params, ret } => {
                self.ctx.emit("(");
                let param_type_ids = self.arena.get_parsed_type_list(*params);
                for (i, type_id) in param_type_ids.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let t = self.arena.get_parsed_type(*type_id);
                    self.emit_type(t);
                }
                self.ctx.emit(") -> ");
                let ret_ty = self.arena.get_parsed_type(*ret);
                self.emit_type(ret_ty);
            }
            ParsedType::Infer => self.ctx.emit("_"),
            ParsedType::SelfType => self.ctx.emit("Self"),
            ParsedType::AssociatedType { base, assoc_name } => {
                let base_ty = self.arena.get_parsed_type(*base);
                self.emit_type(base_ty);
                self.ctx.emit(".");
                self.ctx.emit(self.interner.lookup(*assoc_name));
            }
            ParsedType::ConstExpr(expr_id) => {
                format_const_expr(*expr_id, self.arena, self.interner, &mut self.ctx);
            }
            ParsedType::TraitBounds(bounds) => {
                let bound_ids = self.arena.get_parsed_type_list(*bounds);
                for (i, bound_id) in bound_ids.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(" + ");
                    }
                    let bound = self.arena.get_parsed_type(*bound_id);
                    self.emit_type(bound);
                }
            }
        }
    }
}
