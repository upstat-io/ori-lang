//! Built-in method dispatch for V2 codegen.
//!
//! Handles type-specific method calls that are compiled inline rather than
//! dispatched to user-defined functions. Extracted from `lower_calls.rs` to
//! keep files under 500 lines.
//!
//! # Supported Types
//!
//! - **Primitives**: int, float, bool, byte, char (compare, hash, abs, clone)
//! - **Ordering**: `is_less`, `is_equal`, `is_greater`, `reverse`, `compare`, `equals`, `hash`
//! - **Str**: `len`, `is_empty`, `compare`, `equals`, `hash`, `clone`
//! - **Option**: `is_some`, `is_none`, `unwrap`, `unwrap_or`, `compare`, `equals`, `hash`, `clone`
//! - **Result**: `is_ok`, `is_err`, `unwrap`, `compare`, `equals`, `hash`, `clone`
//! - **Tuple**: `len`, `compare`, `equals`, `hash`, `clone`
//! - **List**: `len`, `is_empty`, `clone`

use ori_ir::canon::CanRange;
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::type_info::TypeInfo;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Top-level dispatch
    // -----------------------------------------------------------------------

    /// Dispatch built-in methods based on receiver type.
    ///
    /// Returns `None` if the method is not a built-in, allowing fallthrough
    /// to user-defined method lookup.
    pub(crate) fn lower_builtin_method(
        &mut self,
        recv_val: ValueId,
        recv_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match recv_type {
            Idx::INT | Idx::DURATION | Idx::SIZE => self.lower_int_method(recv_val, method, args),
            Idx::FLOAT => self.lower_float_method(recv_val, method, args),
            Idx::BOOL => self.lower_bool_method(recv_val, method, args),
            Idx::ORDERING => self.lower_ordering_method(recv_val, method, args),
            Idx::STR => self.lower_str_method(recv_val, method, args),
            Idx::BYTE => self.lower_byte_method(recv_val, method, args),
            Idx::CHAR => self.lower_char_method(recv_val, method, args),
            _ => {
                let type_info = self.type_info.get(recv_type);
                match &type_info {
                    TypeInfo::Option { inner } => {
                        let inner = *inner;
                        self.lower_option_method(recv_val, recv_type, inner, method, args)
                    }
                    TypeInfo::Result { ok, err } => {
                        let ok = *ok;
                        let err = *err;
                        self.lower_result_method(recv_val, recv_type, ok, err, method, args)
                    }
                    TypeInfo::List { .. } => self.lower_list_method(recv_val, method),
                    TypeInfo::Tuple { elements } => {
                        let elements = elements.clone();
                        self.lower_tuple_method(recv_val, &elements, method, args)
                    }
                    // Map/Set are ARC-managed — clone is identity
                    TypeInfo::Map { .. } | TypeInfo::Set { .. } if method == "clone" => {
                        Some(recv_val)
                    }
                    _ => None,
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Int methods
    // -----------------------------------------------------------------------

    fn lower_int_method(&mut self, recv: ValueId, method: &str, args: CanRange) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "cmp", true))
            }
            "abs" => {
                let zero = self.builder.const_i64(0);
                let is_neg = self.builder.icmp_slt(recv, zero, "abs.neg");
                let negated = self.builder.neg(recv, "abs.negated");
                Some(self.builder.select(is_neg, negated, recv, "abs"))
            }
            // Value types: clone/hash are identity operations
            "clone" | "hash" => Some(recv),
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "int.eq"))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Float methods
    // -----------------------------------------------------------------------

    fn lower_float_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_fcmp_ordering(recv, other, "fcmp"))
            }
            "abs" => {
                let zero = self.builder.const_f64(0.0);
                let is_neg = self.builder.fcmp_olt(recv, zero, "fabs.neg");
                let negated = self.builder.fneg(recv, "fabs.negated");
                Some(self.builder.select(is_neg, negated, recv, "fabs"))
            }
            "hash" => {
                // Float hash: normalize ±0.0 → +0.0, NaN → canonical, then bitcast to i64
                let normalized = self.normalize_float_for_hash(recv);
                let i64_ty = self.builder.i64_type();
                Some(self.builder.bitcast(normalized, i64_ty, "float.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.fcmp_oeq(recv, other, "float.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// Normalize a float for hashing: ±0.0 → +0.0.
    ///
    /// This ensures that `(-0.0).hash() == (0.0).hash()` as required by
    /// the contract `a.equals(b) → a.hash() == b.hash()`.
    fn normalize_float_for_hash(&mut self, val: ValueId) -> ValueId {
        let pos_zero = self.builder.const_f64(0.0);
        let is_zero = self.builder.fcmp_oeq(val, pos_zero, "hash.is_zero");
        self.builder
            .select(is_zero, pos_zero, val, "hash.normalized")
    }

    // -----------------------------------------------------------------------
    // Bool methods
    // -----------------------------------------------------------------------

    fn lower_bool_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // false < true: zext to i8 then unsigned compare
                let i8_ty = self.builder.i8_type();
                let lhs = self.builder.zext(recv, i8_ty, "b2i8.self");
                let i8_ty2 = self.builder.i8_type();
                let rhs = self.builder.zext(other, i8_ty2, "b2i8.other");
                Some(self.emit_icmp_ordering(lhs, rhs, "bcmp", false))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(recv, i64_ty, "bool.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "bool.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Byte methods
    // -----------------------------------------------------------------------

    fn lower_byte_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "byte.cmp", false))
            }
            "hash" => {
                // Byte is unsigned (8-bit) — use zext to match evaluator semantics
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(recv, i64_ty, "byte.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "byte.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Char methods
    // -----------------------------------------------------------------------

    fn lower_char_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                // Char is i32 (Unicode scalar) — unsigned comparison
                Some(self.emit_icmp_ordering(recv, other, "char.cmp", false))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(recv, i64_ty, "char.hash"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "char.eq"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Ordering methods
    // -----------------------------------------------------------------------

    fn lower_ordering_method(
        &mut self,
        recv: ValueId,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let less = self.builder.const_i8(0);
        let equal = self.builder.const_i8(1);
        let greater = self.builder.const_i8(2);

        match method {
            "is_less" => Some(self.builder.icmp_eq(recv, less, "ord.is_less")),
            "is_equal" => Some(self.builder.icmp_eq(recv, equal, "ord.is_equal")),
            "is_greater" => Some(self.builder.icmp_eq(recv, greater, "ord.is_greater")),
            "is_less_or_equal" => Some(self.builder.icmp_ne(recv, greater, "ord.is_le")),
            "is_greater_or_equal" => Some(self.builder.icmp_ne(recv, less, "ord.is_ge")),
            "reverse" => {
                // 2 - tag: Less(0)↔Greater(2), Equal(1) unchanged
                Some(self.builder.sub(greater, recv, "ord.reverse"))
            }
            "compare" => {
                // Ordering.compare(): unsigned i8 comparison
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_icmp_ordering(recv, other, "ord.cmp", false))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.builder.icmp_eq(recv, other, "ord.eq"))
            }
            "hash" => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(recv, i64_ty, "ord.hash"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // String methods
    // -----------------------------------------------------------------------

    fn lower_str_method(&mut self, recv: ValueId, method: &str, args: CanRange) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "str.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "str.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "str.is_empty"))
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_str_runtime_call(recv, other, "ori_str_compare", "str.cmp"))
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                Some(self.emit_str_eq_call(recv, other, "str.eq"))
            }
            "hash" => Some(self.emit_str_hash_call(recv, "str.hash")),
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Option methods
    // -----------------------------------------------------------------------

    /// Built-in Option methods.
    ///
    /// Option is `{i8 tag, T payload}` where tag=0 is None, tag=1 is Some.
    fn lower_option_method(
        &mut self,
        recv: ValueId,
        recv_type: Idx,
        inner_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "opt.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_some" => Some(self.builder.icmp_ne(tag, zero, "opt.is_some")),
            "is_none" => Some(self.builder.icmp_eq(tag, zero, "opt.is_none")),
            "unwrap" => self.builder.extract_value(recv, 1, "opt.unwrap"),
            "unwrap_or" => {
                let is_some = self.builder.icmp_ne(tag, zero, "opt.is_some");
                let payload = self.builder.extract_value(recv, 1, "opt.payload")?;
                let arg_ids = self.canon.arena.get_expr_list(args);
                let default_val = self.lower(*arg_ids.first()?)?;
                Some(
                    self.builder
                        .select(is_some, payload, default_val, "opt.unwrap_or"),
                )
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_option_compare(recv, other, recv_type, inner_type)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_option_equals(recv, other, recv_type, inner_type)
            }
            "hash" => self.emit_option_hash(recv, inner_type),
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// `Option.compare()`: None < Some, then compare payloads.
    fn emit_option_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        _recv_type: Idx,
        inner_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "opt.cmp.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "opt.cmp.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "opt.cmp.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.check");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.diff");

        self.builder.cond_br(tags_eq, check_bb, diff_bb);

        // Tags differ: None(0) < Some(1), so compare tags unsigned
        self.builder.position_at_end(diff_bb);
        let diff_ord = self.emit_icmp_ordering(tag_self, tag_other, "opt.cmp.tag", false);
        self.builder.br(merge_bb);
        let diff_bb_final = self.builder.current_block()?;

        // Tags equal: if None, Equal; if Some, compare payloads
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag_self, zero, "opt.cmp.is_none");

        let payload_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.payload");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.cmp.none");

        self.builder.cond_br(is_none, none_bb, payload_bb);

        // Both None → Equal
        self.builder.position_at_end(none_bb);
        let equal_val = self.builder.const_i8(1);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Both Some → compare payloads
        self.builder.position_at_end(payload_bb);
        let pay_self = self.builder.extract_value(lhs, 1, "opt.cmp.pay.self")?;
        let pay_other = self.builder.extract_value(rhs, 1, "opt.cmp.pay.other")?;
        let payload_ord = self.emit_inner_compare(pay_self, pay_other, inner_type, "opt.cmp.inner");
        self.builder.br(merge_bb);
        let payload_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder.phi_from_incoming(
            i8_ty,
            &[
                (diff_ord, diff_bb_final),
                (equal_val, none_bb_final),
                (payload_ord, payload_bb_final),
            ],
            "opt.cmp.result",
        )
    }

    /// `Option.equals()`: tags must match, then compare payloads.
    fn emit_option_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        _recv_type: Idx,
        inner_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "opt.eq.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "opt.eq.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "opt.eq.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.check");
        let false_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.false");

        self.builder.cond_br(tags_eq, check_bb, false_bb);

        // Tags differ → false
        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        // Tags equal: if None → true, if Some → compare payloads
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag_self, zero, "opt.eq.is_none");

        let payload_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.payload");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.eq.none");

        self.builder.cond_br(is_none, none_bb, payload_bb);

        // Both None → true
        self.builder.position_at_end(none_bb);
        let true_val = self.builder.const_bool(true);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Both Some → compare payloads
        self.builder.position_at_end(payload_bb);
        let pay_self = self.builder.extract_value(lhs, 1, "opt.eq.pay.self")?;
        let pay_other = self.builder.extract_value(rhs, 1, "opt.eq.pay.other")?;
        let payload_eq = self.emit_inner_eq(pay_self, pay_other, inner_type, "opt.eq.inner");
        self.builder.br(merge_bb);
        let payload_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[
                (false_val, false_bb_final),
                (true_val, none_bb_final),
                (payload_eq, payload_bb_final),
            ],
            "opt.eq.result",
        )
    }

    /// `Option.hash()`: None → 0, Some(x) → `hash_combine(1, x.hash())`.
    fn emit_option_hash(&mut self, recv: ValueId, inner_type: Idx) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "opt.hash.tag")?;
        let zero = self.builder.const_i8(0);
        let is_none = self.builder.icmp_eq(tag, zero, "opt.hash.is_none");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.merge");
        let some_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.some");
        let none_bb = self
            .builder
            .append_block(self.current_function, "opt.hash.none");

        self.builder.cond_br(is_none, none_bb, some_bb);

        // None → 0
        self.builder.position_at_end(none_bb);
        let none_hash = self.builder.const_i64(0);
        self.builder.br(merge_bb);
        let none_bb_final = self.builder.current_block()?;

        // Some → hash_combine(1, payload.hash())
        self.builder.position_at_end(some_bb);
        let payload = self.builder.extract_value(recv, 1, "opt.hash.payload")?;
        let inner_hash = self.emit_inner_hash(payload, inner_type, "opt.hash.inner");
        let seed = self.builder.const_i64(1);
        let some_hash = self.emit_hash_combine(seed, inner_hash, "opt.hash.combine");
        self.builder.br(merge_bb);
        let some_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i64_ty = self.builder.i64_type();
        self.builder.phi_from_incoming(
            i64_ty,
            &[(none_hash, none_bb_final), (some_hash, some_bb_final)],
            "opt.hash.result",
        )
    }

    // -----------------------------------------------------------------------
    // Result methods
    // -----------------------------------------------------------------------

    /// Built-in Result methods.
    ///
    /// Result is `{i8 tag, max(T, E) payload}` where tag=0 is Ok, tag=1 is Err.
    fn lower_result_method(
        &mut self,
        recv: ValueId,
        recv_type: Idx,
        ok_type: Idx,
        err_type: Idx,
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.tag")?;
        let zero = self.builder.const_i8(0);

        match method {
            "is_ok" => Some(self.builder.icmp_eq(tag, zero, "res.is_ok")),
            "is_err" => Some(self.builder.icmp_ne(tag, zero, "res.is_err")),
            "unwrap" => {
                let payload = self.builder.extract_value(recv, 1, "res.unwrap")?;
                Some(self.coerce_payload(payload, ok_type))
            }
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_result_compare(recv, other, recv_type, ok_type, err_type)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_result_equals(recv, other, recv_type, ok_type, err_type)
            }
            "hash" => self.emit_result_hash(recv, ok_type, err_type),
            "clone" => Some(recv),
            _ => None,
        }
    }

    /// `Result.compare()`: Ok < Err, then compare payloads.
    fn emit_result_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        _recv_type: Idx,
        ok_type: Idx,
        err_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "res.cmp.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "res.cmp.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "res.cmp.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.check");
        let diff_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.diff");

        self.builder.cond_br(tags_eq, check_bb, diff_bb);

        // Tags differ: Ok(0) < Err(1), so compare tags unsigned
        self.builder.position_at_end(diff_bb);
        let diff_ord = self.emit_icmp_ordering(tag_self, tag_other, "res.cmp.tag", false);
        self.builder.br(merge_bb);
        let diff_bb_final = self.builder.current_block()?;

        // Tags equal: compare payloads (coerced to the correct variant type)
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag_self, zero, "res.cmp.is_ok");

        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.cmp.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Both Ok → compare ok payloads
        self.builder.position_at_end(ok_bb);
        let pay_self_ok = self.builder.extract_value(lhs, 1, "res.cmp.pay.self.ok")?;
        let pay_other_ok = self.builder.extract_value(rhs, 1, "res.cmp.pay.other.ok")?;
        let self_ok = self.coerce_payload(pay_self_ok, ok_type);
        let other_ok = self.coerce_payload(pay_other_ok, ok_type);
        let ok_ord = self.emit_inner_compare(self_ok, other_ok, ok_type, "res.cmp.ok");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Both Err → compare err payloads
        self.builder.position_at_end(err_bb);
        let pay_self_err = self.builder.extract_value(lhs, 1, "res.cmp.pay.self.err")?;
        let pay_other_err = self
            .builder
            .extract_value(rhs, 1, "res.cmp.pay.other.err")?;
        let self_err = self.coerce_payload(pay_self_err, err_type);
        let other_err = self.coerce_payload(pay_other_err, err_type);
        let err_ord = self.emit_inner_compare(self_err, other_err, err_type, "res.cmp.err");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder.phi_from_incoming(
            i8_ty,
            &[
                (diff_ord, diff_bb_final),
                (ok_ord, ok_bb_final),
                (err_ord, err_bb_final),
            ],
            "res.cmp.result",
        )
    }

    /// `Result.equals()`: tags must match, then compare payloads.
    fn emit_result_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        _recv_type: Idx,
        ok_type: Idx,
        err_type: Idx,
    ) -> Option<ValueId> {
        let tag_self = self.builder.extract_value(lhs, 0, "res.eq.tag.self")?;
        let tag_other = self.builder.extract_value(rhs, 0, "res.eq.tag.other")?;

        let tags_eq = self.builder.icmp_eq(tag_self, tag_other, "res.eq.tags_eq");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.eq.merge");
        let check_bb = self
            .builder
            .append_block(self.current_function, "res.eq.check");
        let false_bb = self
            .builder
            .append_block(self.current_function, "res.eq.false");

        self.builder.cond_br(tags_eq, check_bb, false_bb);

        // Tags differ → false
        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        // Tags equal: dispatch on Ok vs Err
        self.builder.position_at_end(check_bb);
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag_self, zero, "res.eq.is_ok");

        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.eq.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.eq.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Both Ok → compare ok payloads
        self.builder.position_at_end(ok_bb);
        let pay_self_ok = self.builder.extract_value(lhs, 1, "res.eq.pay.self.ok")?;
        let pay_other_ok = self.builder.extract_value(rhs, 1, "res.eq.pay.other.ok")?;
        let self_ok = self.coerce_payload(pay_self_ok, ok_type);
        let other_ok = self.coerce_payload(pay_other_ok, ok_type);
        let ok_eq = self.emit_inner_eq(self_ok, other_ok, ok_type, "res.eq.ok");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Both Err → compare err payloads
        self.builder.position_at_end(err_bb);
        let pay_self_err = self.builder.extract_value(lhs, 1, "res.eq.pay.self.err")?;
        let pay_other_err = self.builder.extract_value(rhs, 1, "res.eq.pay.other.err")?;
        let self_err = self.coerce_payload(pay_self_err, err_type);
        let other_err = self.coerce_payload(pay_other_err, err_type);
        let err_eq = self.emit_inner_eq(self_err, other_err, err_type, "res.eq.err");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[
                (false_val, false_bb_final),
                (ok_eq, ok_bb_final),
                (err_eq, err_bb_final),
            ],
            "res.eq.result",
        )
    }

    /// `Result.hash()`: Ok(x) → `hash_combine(2, x.hash())`, Err(x) → `hash_combine(3, x.hash())`.
    fn emit_result_hash(&mut self, recv: ValueId, ok_type: Idx, err_type: Idx) -> Option<ValueId> {
        let tag = self.builder.extract_value(recv, 0, "res.hash.tag")?;
        let zero = self.builder.const_i8(0);
        let is_ok = self.builder.icmp_eq(tag, zero, "res.hash.is_ok");

        let merge_bb = self
            .builder
            .append_block(self.current_function, "res.hash.merge");
        let ok_bb = self
            .builder
            .append_block(self.current_function, "res.hash.ok");
        let err_bb = self
            .builder
            .append_block(self.current_function, "res.hash.err");

        self.builder.cond_br(is_ok, ok_bb, err_bb);

        // Ok → hash_combine(2, ok_payload.hash())
        self.builder.position_at_end(ok_bb);
        let pay_ok = self.builder.extract_value(recv, 1, "res.hash.pay.ok")?;
        let pay_ok_coerced = self.coerce_payload(pay_ok, ok_type);
        let ok_inner = self.emit_inner_hash(pay_ok_coerced, ok_type, "res.hash.ok");
        let seed_ok = self.builder.const_i64(2);
        let ok_hash = self.emit_hash_combine(seed_ok, ok_inner, "res.hash.ok.combine");
        self.builder.br(merge_bb);
        let ok_bb_final = self.builder.current_block()?;

        // Err → hash_combine(3, err_payload.hash())
        self.builder.position_at_end(err_bb);
        let pay_err = self.builder.extract_value(recv, 1, "res.hash.pay.err")?;
        let pay_err_coerced = self.coerce_payload(pay_err, err_type);
        let err_inner = self.emit_inner_hash(pay_err_coerced, err_type, "res.hash.err");
        let seed_err = self.builder.const_i64(3);
        let err_hash = self.emit_hash_combine(seed_err, err_inner, "res.hash.err.combine");
        self.builder.br(merge_bb);
        let err_bb_final = self.builder.current_block()?;

        // Merge
        self.builder.position_at_end(merge_bb);
        let i64_ty = self.builder.i64_type();
        self.builder.phi_from_incoming(
            i64_ty,
            &[(ok_hash, ok_bb_final), (err_hash, err_bb_final)],
            "res.hash.result",
        )
    }

    // -----------------------------------------------------------------------
    // Tuple methods
    // -----------------------------------------------------------------------

    fn lower_tuple_method(
        &mut self,
        recv: ValueId,
        elements: &[Idx],
        method: &str,
        args: CanRange,
    ) -> Option<ValueId> {
        match method {
            "clone" => Some(recv),
            "len" => Some(self.builder.const_i64(elements.len() as i64)),
            "compare" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_tuple_compare(recv, other, elements)
            }
            "equals" => {
                let arg_ids = self.canon.arena.get_expr_list(args);
                let other = self.lower(*arg_ids.first()?)?;
                self.emit_tuple_equals(recv, other, elements)
            }
            "hash" => self.emit_tuple_hash(recv, elements),
            _ => None,
        }
    }

    /// `Tuple.compare()`: lexicographic field comparison.
    ///
    /// Uses phi merging at a final block — we can't emit `ret` since this
    /// is inline in the caller's function, not a standalone derived method.
    fn emit_tuple_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_i8(1)); // Equal
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "tup.cmp.merge");
        let mut incoming = Vec::with_capacity(elements.len() + 1);

        for (i, &elem_type) in elements.iter().enumerate() {
            let lhs_field = self
                .builder
                .extract_value(lhs, i as u32, &format!("tup.cmp.l.{i}"))?;
            let rhs_field = self
                .builder
                .extract_value(rhs, i as u32, &format!("tup.cmp.r.{i}"))?;

            let ord =
                self.emit_inner_compare(lhs_field, rhs_field, elem_type, &format!("tup.cmp.{i}"));

            let one = self.builder.const_i8(1);
            let is_eq = self
                .builder
                .icmp_eq(ord, one, &format!("tup.cmp.{i}.is_eq"));

            if i + 1 < elements.len() {
                let early_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.early.{i}"));
                let next_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.next.{}", i + 1));
                self.builder.cond_br(is_eq, next_bb, early_bb);

                // Non-equal: jump to merge with this ordering
                self.builder.position_at_end(early_bb);
                self.builder.br(merge_bb);
                incoming.push((ord, self.builder.current_block()?));

                self.builder.position_at_end(next_bb);
            } else {
                let early_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.cmp.early.{i}"));
                let equal_bb = self
                    .builder
                    .append_block(self.current_function, "tup.cmp.equal");
                self.builder.cond_br(is_eq, equal_bb, early_bb);

                // Non-equal: jump to merge
                self.builder.position_at_end(early_bb);
                self.builder.br(merge_bb);
                incoming.push((ord, self.builder.current_block()?));

                // All fields equal
                self.builder.position_at_end(equal_bb);
                let equal_val = self.builder.const_i8(1);
                self.builder.br(merge_bb);
                incoming.push((equal_val, self.builder.current_block()?));
            }
        }

        self.builder.position_at_end(merge_bb);
        let i8_ty = self.builder.i8_type();
        self.builder
            .phi_from_incoming(i8_ty, &incoming, "tup.cmp.result")
    }

    /// `Tuple.equals()`: field-wise equality with short-circuit.
    fn emit_tuple_equals(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        elements: &[Idx],
    ) -> Option<ValueId> {
        if elements.is_empty() {
            return Some(self.builder.const_bool(true));
        }

        let true_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.true");
        let false_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.false");

        for (i, &elem_type) in elements.iter().enumerate() {
            let lhs_field = self
                .builder
                .extract_value(lhs, i as u32, &format!("tup.eq.l.{i}"))?;
            let rhs_field = self
                .builder
                .extract_value(rhs, i as u32, &format!("tup.eq.r.{i}"))?;

            let eq = self.emit_inner_eq(lhs_field, rhs_field, elem_type, &format!("tup.eq.{i}"));

            if i + 1 < elements.len() {
                let next_bb = self
                    .builder
                    .append_block(self.current_function, &format!("tup.eq.next.{}", i + 1));
                self.builder.cond_br(eq, next_bb, false_bb);
                self.builder.position_at_end(next_bb);
            } else {
                self.builder.cond_br(eq, true_bb, false_bb);
            }
        }

        let merge_bb = self
            .builder
            .append_block(self.current_function, "tup.eq.merge");

        self.builder.position_at_end(true_bb);
        let true_val = self.builder.const_bool(true);
        self.builder.br(merge_bb);
        let true_bb_final = self.builder.current_block()?;

        self.builder.position_at_end(false_bb);
        let false_val = self.builder.const_bool(false);
        self.builder.br(merge_bb);
        let false_bb_final = self.builder.current_block()?;

        self.builder.position_at_end(merge_bb);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[(true_val, true_bb_final), (false_val, false_bb_final)],
            "tup.eq.result",
        )
    }

    /// `Tuple.hash()`: FNV-1a over all field hashes.
    fn emit_tuple_hash(&mut self, recv: ValueId, elements: &[Idx]) -> Option<ValueId> {
        let mut hash = self.builder.const_i64(0);

        for (i, &elem_type) in elements.iter().enumerate() {
            let field_val = self
                .builder
                .extract_value(recv, i as u32, &format!("tup.hash.{i}"))?;
            let field_hash = self.emit_inner_hash(field_val, elem_type, &format!("tup.hash.{i}"));
            hash = self.emit_hash_combine(hash, field_hash, &format!("tup.hash.combine.{i}"));
        }

        Some(hash)
    }

    // -----------------------------------------------------------------------
    // List methods
    // -----------------------------------------------------------------------

    fn lower_list_method(&mut self, recv: ValueId, method: &str) -> Option<ValueId> {
        match method {
            "len" | "length" => self.builder.extract_value(recv, 0, "list.len"),
            "is_empty" => {
                let len = self.builder.extract_value(recv, 0, "list.len")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(len, zero, "list.is_empty"))
            }
            "clone" => Some(recv),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Inner-type dispatch helpers
    // -----------------------------------------------------------------------

    /// Emit equality comparison for an inner value, dispatching on `TypeInfo`.
    fn emit_inner_eq(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
        name: &str,
    ) -> ValueId {
        let info = self.type_info.get(inner_type);
        match &info {
            TypeInfo::Float => self.builder.fcmp_oeq(lhs, rhs, name),
            TypeInfo::Str => self.emit_str_eq_call(lhs, rhs, name),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_equals(lhs, rhs, inner_type, inner)
                    .unwrap_or_else(|| self.builder.const_bool(false))
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_equals(lhs, rhs, inner_type, ok, err)
                    .unwrap_or_else(|| self.builder.const_bool(false))
            }
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_equals(lhs, rhs, &elements)
                    .unwrap_or_else(|| self.builder.const_bool(false))
            }
            TypeInfo::Struct { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let eq_name = self.interner.intern("eq");
                    if let Some((func_id, _abi)) = self.method_functions.get(&(type_name, eq_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[lhs, rhs], name)
                            .unwrap_or_else(|| self.builder.const_bool(false));
                    }
                }
                self.builder.icmp_eq(lhs, rhs, name)
            }
            // All integer-representable types (int, bool, char, byte, ordering, etc.)
            _ => self.builder.icmp_eq(lhs, rhs, name),
        }
    }

    /// Emit three-way comparison for an inner value, returning Ordering (i8).
    fn emit_inner_compare(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        inner_type: Idx,
        name: &str,
    ) -> ValueId {
        let info = self.type_info.get(inner_type);
        match &info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                self.emit_icmp_ordering(lhs, rhs, name, true)
            }
            TypeInfo::Char | TypeInfo::Byte | TypeInfo::Ordering => {
                self.emit_icmp_ordering(lhs, rhs, name, false)
            }
            TypeInfo::Bool => {
                // false(0) < true(1): zext to i8 then unsigned
                let i8_ty = self.builder.i8_type();
                let l = self.builder.zext(lhs, i8_ty, &format!("{name}.l.ext"));
                let i8_ty2 = self.builder.i8_type();
                let r = self.builder.zext(rhs, i8_ty2, &format!("{name}.r.ext"));
                self.emit_icmp_ordering(l, r, name, false)
            }
            TypeInfo::Float => self.emit_fcmp_ordering(lhs, rhs, name),
            TypeInfo::Str => self.emit_str_runtime_call(lhs, rhs, "ori_str_compare", name),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_compare(lhs, rhs, inner_type, inner)
                    .unwrap_or_else(|| self.builder.const_i8(1))
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_compare(lhs, rhs, inner_type, ok, err)
                    .unwrap_or_else(|| self.builder.const_i8(1))
            }
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_compare(lhs, rhs, &elements)
                    .unwrap_or_else(|| self.builder.const_i8(1))
            }
            TypeInfo::Struct { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let compare_name = self.interner.intern("compare");
                    if let Some((func_id, _abi)) =
                        self.method_functions.get(&(type_name, compare_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[lhs, rhs], name)
                            .unwrap_or_else(|| self.builder.const_i8(1));
                    }
                }
                // Fallback: Equal
                self.builder.const_i8(1)
            }
            _ => self.builder.const_i8(1),
        }
    }

    /// Emit hash computation for an inner value, producing i64.
    fn emit_inner_hash(&mut self, val: ValueId, inner_type: Idx, name: &str) -> ValueId {
        let info = self.type_info.get(inner_type);
        match &info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => val,
            TypeInfo::Byte => {
                // Byte is unsigned (8-bit) — use zext to match evaluator semantics
                let i64_ty = self.builder.i64_type();
                self.builder.zext(val, i64_ty, name)
            }
            TypeInfo::Char | TypeInfo::Ordering => {
                let i64_ty = self.builder.i64_type();
                self.builder.sext(val, i64_ty, name)
            }
            TypeInfo::Bool => {
                let i64_ty = self.builder.i64_type();
                self.builder.zext(val, i64_ty, name)
            }
            TypeInfo::Float => {
                let normalized = self.normalize_float_for_hash(val);
                let i64_ty = self.builder.i64_type();
                self.builder.bitcast(normalized, i64_ty, name)
            }
            TypeInfo::Str => self.emit_str_hash_call(val, name),
            TypeInfo::Option { inner } => {
                let inner = *inner;
                self.emit_option_hash(val, inner)
                    .unwrap_or_else(|| self.builder.const_i64(0))
            }
            TypeInfo::Result { ok, err } => {
                let ok = *ok;
                let err = *err;
                self.emit_result_hash(val, ok, err)
                    .unwrap_or_else(|| self.builder.const_i64(0))
            }
            TypeInfo::Tuple { elements } => {
                let elements = elements.clone();
                self.emit_tuple_hash(val, &elements)
                    .unwrap_or_else(|| self.builder.const_i64(0))
            }
            TypeInfo::Struct { .. } => {
                if let Some(&type_name) = self.type_idx_to_name.get(&inner_type) {
                    let hash_name = self.interner.intern("hash");
                    if let Some((func_id, _abi)) =
                        self.method_functions.get(&(type_name, hash_name))
                    {
                        let func_id = *func_id;
                        return self
                            .invoke_user_function(func_id, &[val], name)
                            .unwrap_or_else(|| self.builder.const_i64(0));
                    }
                }
                self.builder.const_i64(0)
            }
            _ => self.builder.const_i64(0),
        }
    }

    // -----------------------------------------------------------------------
    // Shared emit helpers
    // -----------------------------------------------------------------------

    /// Emit `icmp slt/sgt → select` chain returning Ordering i8.
    ///
    /// Delegates to `IrBuilder::emit_icmp_ordering`.
    fn emit_icmp_ordering(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        signed: bool,
    ) -> ValueId {
        self.builder.emit_icmp_ordering(lhs, rhs, name, signed)
    }

    /// Emit `fcmp olt/ogt → select` chain returning Ordering i8.
    ///
    /// Delegates to `IrBuilder::emit_fcmp_ordering`.
    fn emit_fcmp_ordering(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.builder.emit_fcmp_ordering(lhs, rhs, name)
    }

    /// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
    fn emit_str_eq_call(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let lhs_ptr = self.alloca_and_store(lhs, &format!("{name}.lhs"));
        let rhs_ptr = self.alloca_and_store(rhs, &format!("{name}.rhs"));

        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let eq_fn = self
            .builder
            .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
        self.builder
            .call(eq_fn, &[lhs_ptr, rhs_ptr], name)
            .unwrap_or_else(|| self.builder.const_bool(false))
    }

    /// Call a string runtime function (compare or eq) via alloca+store pattern.
    ///
    /// `func_name` should be `"ori_str_compare"` (returns i8) or similar.
    fn emit_str_runtime_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        func_name: &str,
        name: &str,
    ) -> ValueId {
        let lhs_ptr = self.alloca_and_store(lhs, &format!("{name}.lhs"));
        let rhs_ptr = self.alloca_and_store(rhs, &format!("{name}.rhs"));

        let ptr_ty = self.builder.ptr_type();
        let i8_ty = self.builder.i8_type();
        let cmp_fn = self
            .builder
            .get_or_declare_function(func_name, &[ptr_ty, ptr_ty], i8_ty);
        self.builder
            .call(cmp_fn, &[lhs_ptr, rhs_ptr], name)
            .unwrap_or_else(|| self.builder.const_i8(1))
    }

    /// Call `ori_str_hash(s: ptr) -> i64` via alloca+store pattern.
    fn emit_str_hash_call(&mut self, val: ValueId, name: &str) -> ValueId {
        let val_ptr = self.alloca_and_store(val, &format!("{name}.str"));

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let hash_fn = self
            .builder
            .get_or_declare_function("ori_str_hash", &[ptr_ty], i64_ty);
        self.builder
            .call(hash_fn, &[val_ptr], name)
            .unwrap_or_else(|| self.builder.const_i64(0))
    }

    /// Emit inline `hash_combine`: `seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))`.
    ///
    /// This is the Boost hash combine algorithm, matching the evaluator's
    /// `function_val_hash_combine` and `hash_combine` in `ori_eval`.
    pub(crate) fn emit_hash_combine(
        &mut self,
        seed: ValueId,
        value: ValueId,
        name: &str,
    ) -> ValueId {
        let golden = self.builder.const_i64(0x9e37_79b9_i64);
        let six = self.builder.const_i64(6);
        let two = self.builder.const_i64(2);

        let seed_shl6 = self.builder.shl(seed, six, &format!("{name}.shl6"));
        let seed_shr2 = self.builder.lshr(seed, two, &format!("{name}.shr2"));

        // value + golden + (seed << 6) + (seed >> 2)
        let sum1 = self.builder.add(value, golden, &format!("{name}.add1"));
        let sum2 = self.builder.add(sum1, seed_shl6, &format!("{name}.add2"));
        let sum3 = self.builder.add(sum2, seed_shr2, &format!("{name}.add3"));

        // seed XOR sum
        self.builder.xor(seed, sum3, &format!("{name}.xor"))
    }

    // -----------------------------------------------------------------------
    // Built-in function: hash_combine
    // -----------------------------------------------------------------------

    /// Lower `hash_combine(seed, value)` → inline Boost hash combine.
    ///
    /// This is a free function (not a method), called from `lower_call` dispatch.
    pub(crate) fn lower_builtin_hash_combine(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let seed = self.lower(*arg_ids.first()?)?;
        let value = self.lower(*arg_ids.get(1)?)?;
        Some(self.emit_hash_combine(seed, value, "hash_combine"))
    }
}
