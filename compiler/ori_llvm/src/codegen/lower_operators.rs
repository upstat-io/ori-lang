//! Binary and unary operator lowering for V2 codegen.
//!
//! Uses TypeInfo-driven dispatch to select integer vs float operations.
//! Short-circuit operators (`&&`, `||`, `??`) use conditional branching
//! with phi nodes — they do NOT eagerly evaluate both operands.

use ori_ir::{BinaryOp, ExprId, UnaryOp};
use ori_types::Idx;

use super::expr_lowerer::ExprLowerer;
use super::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    // -----------------------------------------------------------------------
    // Binary operators — top-level dispatch
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Binary { op, left, right }`.
    pub(crate) fn lower_binary(
        &mut self,
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        // Short-circuit operators must NOT evaluate right before branching.
        match op {
            BinaryOp::And => return self.lower_short_circuit_and(left, right),
            BinaryOp::Or => return self.lower_short_circuit_or(left, right),
            BinaryOp::Coalesce => return self.lower_coalesce(left, right, expr_id),
            _ => {}
        }

        // Eager operators: evaluate both sides, then combine.
        let lhs = self.lower(left)?;
        let rhs = self.lower(right)?;
        let left_type = self.expr_type(left);

        self.lower_binary_op(op, lhs, rhs, left_type)
    }

    /// Emit the actual binary operation given evaluated operands.
    fn lower_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        left_type: Idx,
    ) -> Option<ValueId> {
        let is_float = left_type == Idx::FLOAT;
        let is_str = left_type == Idx::STR;

        match op {
            // Arithmetic
            BinaryOp::Add if is_float => Some(self.builder.fadd(lhs, rhs, "fadd")),
            BinaryOp::Add if is_str => self.lower_str_concat(lhs, rhs),
            BinaryOp::Add => Some(self.builder.add(lhs, rhs, "add")),

            BinaryOp::Sub if is_float => Some(self.builder.fsub(lhs, rhs, "fsub")),
            BinaryOp::Sub => Some(self.builder.sub(lhs, rhs, "sub")),

            BinaryOp::Mul if is_float => Some(self.builder.fmul(lhs, rhs, "fmul")),
            BinaryOp::Mul => Some(self.builder.mul(lhs, rhs, "mul")),

            BinaryOp::Div if is_float => Some(self.builder.fdiv(lhs, rhs, "fdiv")),
            BinaryOp::Div => Some(self.builder.sdiv(lhs, rhs, "sdiv")),

            BinaryOp::Mod if is_float => Some(self.builder.frem(lhs, rhs, "frem")),
            BinaryOp::Mod => Some(self.builder.srem(lhs, rhs, "srem")),

            BinaryOp::FloorDiv => Some(self.lower_floor_div(lhs, rhs)),

            // Comparisons
            BinaryOp::Eq if is_float => Some(self.builder.fcmp_oeq(lhs, rhs, "feq")),
            BinaryOp::Eq if is_str => self.lower_str_eq(lhs, rhs),
            BinaryOp::Eq => Some(self.builder.icmp_eq(lhs, rhs, "eq")),

            BinaryOp::NotEq if is_float => Some(self.builder.fcmp_une(lhs, rhs, "fne")),
            BinaryOp::NotEq if is_str => self.lower_str_ne(lhs, rhs),
            BinaryOp::NotEq => Some(self.builder.icmp_ne(lhs, rhs, "ne")),

            BinaryOp::Lt if is_float => Some(self.builder.fcmp_olt(lhs, rhs, "flt")),
            BinaryOp::Lt => Some(self.builder.icmp_slt(lhs, rhs, "slt")),

            BinaryOp::LtEq if is_float => Some(self.builder.fcmp_ole(lhs, rhs, "fle")),
            BinaryOp::LtEq => Some(self.builder.icmp_sle(lhs, rhs, "sle")),

            BinaryOp::Gt if is_float => Some(self.builder.fcmp_ogt(lhs, rhs, "fgt")),
            BinaryOp::Gt => Some(self.builder.icmp_sgt(lhs, rhs, "sgt")),

            BinaryOp::GtEq if is_float => Some(self.builder.fcmp_oge(lhs, rhs, "fge")),
            BinaryOp::GtEq => Some(self.builder.icmp_sge(lhs, rhs, "sge")),

            // Bitwise
            BinaryOp::BitAnd => Some(self.builder.and(lhs, rhs, "bitand")),
            BinaryOp::BitOr => Some(self.builder.or(lhs, rhs, "bitor")),
            BinaryOp::BitXor => Some(self.builder.xor(lhs, rhs, "bitxor")),
            BinaryOp::Shl => Some(self.builder.shl(lhs, rhs, "shl")),
            BinaryOp::Shr => Some(self.builder.ashr(lhs, rhs, "shr")),

            // Range operators produce range structs (handled in lower_collections)
            BinaryOp::Range | BinaryOp::RangeInclusive => {
                let inclusive = matches!(op, BinaryOp::RangeInclusive);
                Some(self.build_range_struct(lhs, rhs, inclusive))
            }

            // Short-circuit handled above; this arm is unreachable.
            BinaryOp::And | BinaryOp::Or | BinaryOp::Coalesce => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // FloorDiv correction
    // -----------------------------------------------------------------------

    /// Integer floor division: rounds toward negative infinity.
    ///
    /// LLVM's `sdiv` truncates toward zero. For negative results with
    /// a non-zero remainder, we subtract 1 to get floor semantics.
    ///
    /// ```text
    /// floor_div(a, b) = sdiv(a, b) - (has_remainder && signs_differ ? 1 : 0)
    /// ```
    fn lower_floor_div(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let quotient = self.builder.sdiv(lhs, rhs, "quot");
        let remainder = self.builder.srem(lhs, rhs, "rem");

        // Check if remainder is non-zero
        let zero = self.builder.const_i64(0);
        let has_rem = self.builder.icmp_ne(remainder, zero, "has_rem");

        // Check if signs differ: (lhs ^ rhs) < 0
        let sign_xor = self.builder.xor(lhs, rhs, "sign_xor");
        let signs_differ = self.builder.icmp_slt(sign_xor, zero, "signs_differ");

        // Correction needed when both conditions hold
        let needs_correction = self.builder.and(has_rem, signs_differ, "needs_corr");

        // Subtract 1 if correction needed (select avoids branching)
        let one = self.builder.const_i64(1);
        let corrected = self.builder.sub(quotient, one, "corrected");
        self.builder
            .select(needs_correction, corrected, quotient, "floordiv")
    }

    // -----------------------------------------------------------------------
    // Short-circuit And / Or
    // -----------------------------------------------------------------------

    /// Lower `a && b` with short-circuit evaluation.
    ///
    /// ```text
    /// entry:
    ///   %a = ...
    ///   cond_br %a, rhs_bb, merge_bb
    /// rhs:
    ///   %b = ...
    ///   br merge_bb
    /// merge:
    ///   %result = phi [false, entry], [%b, rhs]
    /// ```
    fn lower_short_circuit_and(&mut self, left: ExprId, right: ExprId) -> Option<ValueId> {
        let lhs = self.lower(left)?;

        let rhs_bb = self.builder.append_block(self.current_function, "and.rhs");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "and.merge");
        let entry_bb = self.builder.current_block()?;

        self.builder.cond_br(lhs, rhs_bb, merge_bb);

        // Evaluate right operand only if left is true
        self.builder.position_at_end(rhs_bb);
        let rhs = self
            .lower(right)
            .unwrap_or_else(|| self.builder.const_bool(false));
        let rhs_exit_bb = self.builder.current_block()?;
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Merge: false from entry, rhs value from rhs block
        self.builder.position_at_end(merge_bb);
        let false_val = self.builder.const_bool(false);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[(false_val, entry_bb), (rhs, rhs_exit_bb)],
            "and.result",
        )
    }

    /// Lower `a || b` with short-circuit evaluation.
    ///
    /// ```text
    /// entry:
    ///   %a = ...
    ///   cond_br %a, merge_bb, rhs_bb
    /// rhs:
    ///   %b = ...
    ///   br merge_bb
    /// merge:
    ///   %result = phi [true, entry], [%b, rhs]
    /// ```
    fn lower_short_circuit_or(&mut self, left: ExprId, right: ExprId) -> Option<ValueId> {
        let lhs = self.lower(left)?;

        let rhs_bb = self.builder.append_block(self.current_function, "or.rhs");
        let merge_bb = self.builder.append_block(self.current_function, "or.merge");
        let entry_bb = self.builder.current_block()?;

        // If true, skip right operand
        self.builder.cond_br(lhs, merge_bb, rhs_bb);

        // Evaluate right operand only if left is false
        self.builder.position_at_end(rhs_bb);
        let rhs = self
            .lower(right)
            .unwrap_or_else(|| self.builder.const_bool(true));
        let rhs_exit_bb = self.builder.current_block()?;
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Merge: true from entry, rhs value from rhs block
        self.builder.position_at_end(merge_bb);
        let true_val = self.builder.const_bool(true);
        let bool_ty = self.builder.bool_type();
        self.builder.phi_from_incoming(
            bool_ty,
            &[(true_val, entry_bb), (rhs, rhs_exit_bb)],
            "or.result",
        )
    }

    // -----------------------------------------------------------------------
    // Coalesce (??)
    // -----------------------------------------------------------------------

    /// Lower `a ?? b` — unwrap Option/Result or use fallback.
    ///
    /// For `Option`: check `tag != 0` (is Some), extract payload or eval `b`.
    /// For `Result`: check `tag == 0` (is Ok), extract payload or eval `b`.
    fn lower_coalesce(&mut self, left: ExprId, right: ExprId, expr_id: ExprId) -> Option<ValueId> {
        let left_type = self.expr_type(left);
        let type_info = self.type_info.get(left_type);

        let lhs = self.lower(left)?;

        let is_option = matches!(type_info, super::type_info::TypeInfo::Option { .. });

        // Extract tag (field 0 of the tagged union)
        let tag = self.builder.extract_value(lhs, 0, "coal.tag")?;

        // Determine "has value" condition:
        // Option: tag != 0 (Some=1)
        // Result: tag == 0 (Ok=0)
        let zero_tag = self.builder.const_i8(0);
        let has_value = if is_option {
            self.builder.icmp_ne(tag, zero_tag, "is_some")
        } else {
            self.builder.icmp_eq(tag, zero_tag, "is_ok")
        };

        let unwrap_bb = self
            .builder
            .append_block(self.current_function, "coal.unwrap");
        let fallback_bb = self
            .builder
            .append_block(self.current_function, "coal.fallback");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "coal.merge");
        self.builder.cond_br(has_value, unwrap_bb, fallback_bb);

        // Unwrap: extract payload from the tagged union
        self.builder.position_at_end(unwrap_bb);
        let payload = self.builder.extract_value(lhs, 1, "coal.payload")?;

        // Coerce payload to result type if needed
        let result_type = self.expr_type(expr_id);
        let payload_val = self.coerce_payload(payload, result_type);
        let unwrap_exit = self.builder.current_block()?;
        self.builder.br(merge_bb);

        // Fallback: evaluate right operand
        self.builder.position_at_end(fallback_bb);
        let fallback = self.lower(right)?;
        let fallback_exit = self.builder.current_block()?;
        if !self.builder.current_block_terminated() {
            self.builder.br(merge_bb);
        }

        // Merge
        self.builder.position_at_end(merge_bb);
        let result_llvm_ty = self.resolve_type(result_type);
        self.builder.phi_from_incoming(
            result_llvm_ty,
            &[(payload_val, unwrap_exit), (fallback, fallback_exit)],
            "coal.result",
        )
    }

    /// Coerce a payload value to the expected type.
    ///
    /// With `TypeLayoutResolver`, Option payloads match the inner type exactly,
    /// so extraction usually needs no coercion. For Result, the payload is
    /// `max(ok, err)` — if the target type differs (e.g., extracting `bool`
    /// from an `i64` payload), we reinterpret via alloca+store+load.
    pub(crate) fn coerce_payload(&mut self, payload: ValueId, target_idx: Idx) -> ValueId {
        let target_ty = self.type_resolver.resolve(target_idx);
        let raw_payload = self.builder.raw_value(payload);

        // If types match (common: Option, and Result where ok_ty == err_ty)
        if raw_payload.get_type() == target_ty {
            return payload;
        }

        // Result mismatch: alloca payload, store, load as target.
        // Example: payload is i64 (max of ok=i64, err=bool), target is bool.
        let payload_ty = self.builder.register_type(raw_payload.get_type());
        let ptr =
            self.builder
                .create_entry_alloca(self.current_function, "payload.cast", payload_ty);
        self.builder.store(payload, ptr);
        let target_ty_id = self.builder.register_type(target_ty);
        self.builder
            .load(target_ty_id, ptr, "payload.reinterpreted")
    }

    // -----------------------------------------------------------------------
    // String operations (via runtime calls)
    // -----------------------------------------------------------------------

    /// Lower `str + str` → `ori_str_concat(a, b)`.
    fn lower_str_concat(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let ptr_ty = self.builder.ptr_type();
        let str_ty = self.resolve_type(Idx::STR);
        let func =
            self.builder
                .get_or_declare_function("ori_str_concat", &[ptr_ty, ptr_ty], str_ty);

        // String values are {i64, ptr} structs — we need to pass pointers.
        // Alloca, store, and pass the pointer.
        let lhs_ptr = self.alloca_and_store(lhs, "str_concat.lhs");
        let rhs_ptr = self.alloca_and_store(rhs, "str_concat.rhs");
        self.builder.call(func, &[lhs_ptr, rhs_ptr], "str_concat")
    }

    /// Lower `str == str` → `ori_str_eq(a, b)`.
    fn lower_str_eq(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let func = self
            .builder
            .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
        let lhs_ptr = self.alloca_and_store(lhs, "str_eq.lhs");
        let rhs_ptr = self.alloca_and_store(rhs, "str_eq.rhs");
        self.builder.call(func, &[lhs_ptr, rhs_ptr], "str_eq")
    }

    /// Lower `str != str` → `ori_str_ne(a, b)`.
    fn lower_str_ne(&mut self, lhs: ValueId, rhs: ValueId) -> Option<ValueId> {
        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let func = self
            .builder
            .get_or_declare_function("ori_str_ne", &[ptr_ty, ptr_ty], bool_ty);
        let lhs_ptr = self.alloca_and_store(lhs, "str_ne.lhs");
        let rhs_ptr = self.alloca_and_store(rhs, "str_ne.rhs");
        self.builder.call(func, &[lhs_ptr, rhs_ptr], "str_ne")
    }

    // -----------------------------------------------------------------------
    // Unary operators
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Unary { op, operand }`.
    pub(crate) fn lower_unary(
        &mut self,
        op: UnaryOp,
        operand: ExprId,
        _expr_id: ExprId,
    ) -> Option<ValueId> {
        let val = self.lower(operand)?;
        let operand_type = self.expr_type(operand);

        match op {
            UnaryOp::Neg => {
                if operand_type == Idx::FLOAT {
                    Some(self.builder.fneg(val, "fneg"))
                } else {
                    Some(self.builder.neg(val, "neg"))
                }
            }
            UnaryOp::Not => {
                // Logical NOT: i1 → i1 (xor with 1)
                Some(self.builder.not(val, "not"))
            }
            UnaryOp::BitNot => {
                // Bitwise NOT: int → int (xor with -1)
                Some(self.builder.not(val, "bitnot"))
            }
            UnaryOp::Try => {
                // Parser emits ExprKind::Try for `?`, not UnaryOp::Try.
                // This arm should be unreachable.
                tracing::warn!("UnaryOp::Try reached codegen — should be ExprKind::Try");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Cast
    // -----------------------------------------------------------------------

    /// Lower `ExprKind::Cast { expr, fallible }`.
    ///
    /// Infallible (`as`): direct value, type checker ensures validity.
    /// Fallible (`as?`): wraps result in `Option` (Some on success).
    pub(crate) fn lower_cast(
        &mut self,
        inner: ExprId,
        fallible: bool,
        expr_id: ExprId,
    ) -> Option<ValueId> {
        let val = self.lower(inner)?;
        let source_type = self.expr_type(inner);
        let target_type = self.expr_type(expr_id);

        if fallible {
            // `as?` wraps in Some — full cast validation is future work.
            // With TypeLayoutResolver, the Option payload type matches the
            // cast result type, so the value can be used directly.
            let tag = self.builder.const_i8(1); // Some
            let opt_ty = self.resolve_type(target_type);
            let result = self.builder.build_struct(opt_ty, &[tag, val], "cast_some");
            Some(result)
        } else {
            // `as` — emit appropriate LLVM conversion
            self.emit_cast(val, source_type, target_type)
        }
    }

    /// Emit LLVM type conversion for infallible `as` cast.
    fn emit_cast(&mut self, val: ValueId, source: Idx, target: Idx) -> Option<ValueId> {
        match (source, target) {
            // int → float
            (Idx::INT, Idx::FLOAT) => {
                let f64_ty = self.builder.f64_type();
                Some(self.builder.si_to_fp(val, f64_ty, "i2f"))
            }
            // float → int
            (Idx::FLOAT, Idx::INT) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.fp_to_si(val, i64_ty, "f2i"))
            }
            // char → int
            (Idx::CHAR, Idx::INT) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "char2int"))
            }
            // int → char (truncate)
            (Idx::INT, Idx::CHAR) => {
                let i32_ty = self.builder.i32_type();
                Some(self.builder.trunc(val, i32_ty, "int2char"))
            }
            // byte → int
            (Idx::BYTE, Idx::INT) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.sext(val, i64_ty, "byte2int"))
            }
            // int → byte
            (Idx::INT, Idx::BYTE) => {
                let i8_ty = self.builder.i8_type();
                Some(self.builder.trunc(val, i8_ty, "int2byte"))
            }
            // bool → int
            (Idx::BOOL, Idx::INT) => {
                let i64_ty = self.builder.i64_type();
                Some(self.builder.zext(val, i64_ty, "bool2int"))
            }
            // Same type or no conversion needed
            _ if source == target => Some(val),
            // Unknown cast — pass through
            _ => {
                tracing::debug!(
                    ?source,
                    ?target,
                    "cast between non-primitive types, passing through"
                );
                Some(val)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Build a range struct `{i64 start, i64 end, i1 inclusive}` from
    /// pre-evaluated operands.
    fn build_range_struct(&mut self, start: ValueId, end: ValueId, inclusive: bool) -> ValueId {
        let incl_val = self.builder.const_bool(inclusive);
        // Build the struct manually since Range uses {i64, i64, i1}
        let range_llvm = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i1().into(),
                    ],
                    false,
                )
                .into(),
        );
        self.builder
            .build_struct(range_llvm, &[start, end, incl_val], "range")
    }

    /// Alloca a value on the stack and store it, returning the pointer.
    ///
    /// Used for passing struct values to runtime functions that expect
    /// pointers (e.g., `ori_str_concat`).
    pub(crate) fn alloca_and_store(&mut self, val: ValueId, name: &str) -> ValueId {
        // Determine the LLVM type from the value itself
        let raw_val = self.builder.raw_value(val);
        let val_ty = self.builder.register_type(raw_val.get_type());
        let ptr = self
            .builder
            .create_entry_alloca(self.current_function, name, val_ty);
        self.builder.store(val, ptr);
        ptr
    }

    /// Coerce a value to i64 for storing in tagged union payloads.
    ///
    /// Different source types need different coercion:
    /// - i64 (int, duration, size): identity
    /// - f64 (float): bitcast to i64
    /// - i1 (bool): zero-extend
    /// - i32 (char): sign-extend
    /// - i8 (byte): sign-extend
    pub(crate) fn coerce_to_i64(&mut self, val: ValueId, source_type: Idx) -> ValueId {
        let i64_ty = self.builder.i64_type();
        match source_type {
            Idx::FLOAT => self.builder.bitcast(val, i64_ty, "f2bits"),
            Idx::BOOL => self.builder.zext(val, i64_ty, "b2i"),
            Idx::CHAR => self.builder.sext(val, i64_ty, "c2i"),
            Idx::BYTE | Idx::ORDERING => self.builder.sext(val, i64_ty, "b2i"),
            // INT, DURATION, SIZE, UNIT, NEVER — already i64
            _ => val,
        }
    }
}
