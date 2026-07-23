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

        if src_tag == tgt_tag {
            return Some(val);
        }

        match (src_tag, tgt_tag) {
            (Tag::Int, Tag::Float) => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(val, f64_ty, "cast.int.float"))
            }
            (Tag::Float, Tag::Int) => {
                // Why: Raw `fptosi` is poison for NaN and out-of-range inputs.
                let i64_ty = self.builder.i64_type();
                Some(self.builder.fp_to_si_sat(val, i64_ty, "cast.float.int"))
            }
            (Tag::Byte | Tag::Char, Tag::Int) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(val, i64_ty, "cast.widen.int"))
            }
            (Tag::Int, Tag::Byte) => {
                // Why: Truncation would wrap values outside the byte range.
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
                // Why: Pre-truncation checks prevent high bits from wrapping into a valid scalar.
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
            _ => None,
        }
    }

    /// Intercept `ori_format_*` calls and decompose the string spec argument.
    ///
    /// ARC IR emits `Apply("ori_format_int", [val, spec_str])` with 2 args.
    /// Runtime expects `ori_format_int(val, spec_ptr, spec_len)` — 3 args.
    /// The `spec_str` is `{i64 len, ptr data}` that needs decomposition.
    ///
    /// Dispatch is `Name`-keyed via [`super::FormatRtNames`] (pre-interned,
    /// per interning discipline) — the registry resolves a typed target whose
    /// runtime symbol and value ABI cannot disagree.
    pub(super) fn try_emit_format_call(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() < 2 {
            return None;
        }

        let target = self.format_rt_names.lookup(callee)?;
        let func_id = self.builder.runtime_fn(target.symbol());

        let value = self.var(args[0]);
        let spec_str = self.var(args[1]);

        let value_arg = if target.value_needs_pointer() {
            let val_ty = func.var_type(args[0]);
            self.coerce_aggregate_to_ptr(value, val_ty)
        } else {
            value
        };

        // Why: Field extraction reads inline bytes instead of length for SSO strings.
        let spec_str_ptr = self.str_to_ptr(spec_str, "fmt.spec");
        let len_fn = self.builder.runtime_fn("ori_str_len");
        let spec_len = self.builder.call(len_fn, &[spec_str_ptr], "fmt.spec_len")?;
        let data_fn = self.builder.runtime_fn("ori_str_data");
        let spec_ptr = self
            .builder
            .call(data_fn, &[spec_str_ptr], "fmt.spec_ptr")?;

        let str_ty = self.resolve_type(ori_types::Idx::STR);
        self.builder
            .call_with_sret(func_id, &[value_arg, spec_ptr, spec_len], str_ty, "fmt")
    }
}
