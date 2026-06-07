//! Cast and format-call intercepts for `Apply` emission.
//!
//! Two intercepts that fire before normal callee resolution in
//! [`super::apply`]:
//!
//! - [`ArcIrEmitter::try_emit_cast`] — the `__cast` protocol (primitive `as`
//!   conversions emitted inline)
//! - [`ArcIrEmitter::try_emit_format_call`] — `ori_format_*` runtime calls
//!   (string-spec argument decomposed into `(ptr, len)`)

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::Tag;

use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a primitive `as` cast (the `__cast` intercept). Returns the
    /// converted value, or `None` for source/target pairs handled elsewhere
    /// (str parse, value→str) so the caller falls through.
    ///
    /// Matches `ori_eval::eval_can_cast` for the conversions emitted here:
    /// identity (same primitive — no-op), int→float (sitofp), float→int
    /// (fptosi), byte→int / char→int (zext — lossless for valid values),
    /// int→byte / int→char (range-checked; panics on out-of-range).
    pub(super) fn try_emit_cast(
        &mut self,
        dst: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() != 1 {
            return None;
        }
        let val = self.var(args[0]);
        let src_tag = self
            .pool
            .tag(self.pool.resolve_fully(func.var_type(args[0])));
        let tgt_tag = self.pool.tag(self.pool.resolve_fully(func.var_type(dst)));

        // Identity conversion (same primitive type) — no-op.
        if src_tag == tgt_tag {
            return Some(val);
        }

        match (src_tag, tgt_tag) {
            (Tag::Int, Tag::Float) => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(val, f64_ty, "cast.int.float"))
            }
            (Tag::Float, Tag::Int) => {
                // Saturating conversion — NaN -> 0, out-of-range clamps to
                // i64 MIN/MAX (parity with eval's Rust `as` semantics; raw
                // fptosi is poison on those inputs).
                let i64_ty = self.builder.i64_type();
                Some(self.builder.fp_to_si_sat(val, i64_ty, "cast.float.int"))
            }
            (Tag::Byte | Tag::Char, Tag::Int) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(val, i64_ty, "cast.widen.int"))
            }
            (Tag::Int, Tag::Byte) => {
                // Range check 0..=255, panic out of range (parity with
                // eval's "value N out of range for byte (0-255)").
                let lo = self.builder.const_i64(0);
                let hi = self.builder.const_i64(255);
                let ge = self.builder.icmp_sge(val, lo, "cast.byte.ge");
                let le = self.builder.icmp_sle(val, hi, "cast.byte.le");
                let valid = self.builder.and(ge, le, "cast.byte.valid");
                self.emit_unwrap_branch(valid, "value out of range for byte (0-255)", "cast.byte")?;
                let i8_ty = self.builder.i8_type();
                Some(self.builder.trunc(val, i8_ty, "cast.int.byte"))
            }
            (Tag::Int, Tag::Char) => {
                // Valid Unicode scalar: [0, 0xD7FF] or [0xE000, 0x10FFFF].
                // Checked on the full i64 BEFORE truncation (parity with
                // eval — high bits must not wrap into the valid range).
                let zero = self.builder.const_i64(0);
                let surrogate_lo = self.builder.const_i64(0xD7FF);
                let surrogate_hi = self.builder.const_i64(0xE000);
                let max_scalar = self.builder.const_i64(0x0010_FFFF);
                let ge_zero = self.builder.icmp_sge(val, zero, "cast.char.ge0");
                let below_surrogates = self.builder.icmp_sle(val, surrogate_lo, "cast.char.bmp");
                let low_ok = self.builder.and(ge_zero, below_surrogates, "cast.char.low");
                let above_surrogates = self.builder.icmp_sge(val, surrogate_hi, "cast.char.astral");
                let le_max = self.builder.icmp_sle(val, max_scalar, "cast.char.max");
                let high_ok = self.builder.and(above_surrogates, le_max, "cast.char.high");
                let valid = self.builder.or(low_ok, high_ok, "cast.char.valid");
                self.emit_unwrap_branch(
                    valid,
                    "value is not a valid Unicode codepoint",
                    "cast.char",
                )?;
                let i32_ty = self.builder.i32_type();
                Some(self.builder.trunc(val, i32_ty, "cast.int.char"))
            }
            // str→int / str→float (parse) and value→str (to_str) need the
            // cast runtime surface — not emitted here; caller falls through.
            _ => None,
        }
    }

    // Format call decomposition

    /// Intercept `ori_format_*` calls and decompose the string spec argument.
    ///
    /// ARC IR emits `Apply("ori_format_int", [val, spec_str])` with 2 args.
    /// Runtime expects `ori_format_int(val, spec_ptr, spec_len)` — 3 args.
    /// The `spec_str` is `{i64 len, ptr data}` that needs decomposition.
    ///
    /// Dispatch is `Name`-keyed via [`super::FormatRtNames`] (pre-interned,
    /// per interning discipline) — the registry carries the per-function
    /// `value_is_str` flag alongside the runtime symbol.
    pub(super) fn try_emit_format_call(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() < 2 {
            return None;
        }

        // Single dispatch point: resolve the runtime function AND whether the
        // formatted value itself is a string struct needing ptr coercion.
        let (symbol, value_is_str) = self.format_rt_names.lookup(callee)?;
        let func_id = self.builder.runtime_fn(symbol);

        // args[0] = the value to format
        let value = self.var(args[0]);
        // args[1] = spec string {i64 len, ptr data}
        let spec_str = self.var(args[1]);

        // For ori_format_str, the value arg is also a string struct — coerce to ptr.
        let value_arg = if value_is_str {
            let val_ty = func.var_type(args[0]);
            self.coerce_aggregate_to_ptr(value, val_ty)
        } else {
            value
        };

        // Decompose spec string via SSO-safe runtime helpers.
        // Field extraction is WRONG for SSO strings (field 0 = inline bytes, not len).
        let spec_str_ptr = self.str_to_ptr(spec_str, "fmt.spec");
        let len_fn = self.builder.runtime_fn("ori_str_len");
        let spec_len = self
            .builder
            .call(len_fn, &[spec_str_ptr], "fmt.spec_len")
            .expect("ori_str_len is non-void; builder.call returns Some");
        let data_fn = self.builder.runtime_fn("ori_str_data");
        let spec_ptr = self
            .builder
            .call(data_fn, &[spec_str_ptr], "fmt.spec_ptr")
            .expect("ori_str_data is non-void; builder.call returns Some");

        // Call runtime: ori_format_*(value, spec_ptr, spec_len) → Str via sret
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[value_arg, spec_ptr, spec_len], str_ty, "fmt")
    }
}
