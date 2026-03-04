//! Operator emission for [`ArcIrEmitter`].
//!
//! Emits LLVM IR for binary and unary operations. Primitive types use direct
//! LLVM instructions; non-primitive types dispatch to operator trait methods
//! (e.g., `+` → `Add.add()`, `==` → `Eq.eq()`, `<` → `Comparable.compare()`).

use ori_ir::{BinaryOp, UnaryOp};
use ori_types::Idx;

use super::builtins;
use super::ArcIrEmitter;
use crate::codegen::abi::ReturnPassing;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a binary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `+` → `Add.add()`, `==` → `Eq.equals()`, `<` → `Comparable.compare()`).
    pub(super) fn emit_binary_op(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
        arc_func: &ori_arc::ir::ArcFunction,
    ) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !lhs_ty.is_primitive() {
            // Arithmetic operators (Add, Sub, Mul, etc.)
            if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
            // Comparison operators (==, !=, <, >, <=, >=)
            if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
        }

        let type_info = self.type_info.get(lhs_ty);
        let is_float = matches!(type_info, super::super::type_info::TypeInfo::Float);
        let is_str = matches!(type_info, super::super::type_info::TypeInfo::Str);

        // List + list → concat (same as str + str → concat)
        if matches!(op, BinaryOp::Add) {
            if let super::super::type_info::TypeInfo::List { element } = type_info {
                let cm = self.cow_mode_const(arc_func);
                if let Some(val) = self.emit_list_concat_cow(lhs, rhs, element, cm) {
                    return val;
                }
            }
        }

        match op {
            BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
            BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
            BinaryOp::Add => self.builder.checked_add(lhs, rhs, "add"),
            BinaryOp::Sub if is_float => self.builder.fsub(lhs, rhs, "sub"),
            BinaryOp::Sub => self.builder.checked_sub(lhs, rhs, "sub"),
            BinaryOp::Mul if is_float => self.builder.fmul(lhs, rhs, "mul"),
            BinaryOp::Mul => self.builder.checked_mul(lhs, rhs, "mul"),
            BinaryOp::Div if is_float => self.builder.fdiv(lhs, rhs, "div"),
            BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
            BinaryOp::Mod if is_float => self.builder.frem(lhs, rhs, "rem"),
            BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
            BinaryOp::Eq if is_float => self.builder.fcmp_oeq(lhs, rhs, "eq"),
            BinaryOp::Eq if is_str => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq if is_float => self.builder.fcmp_one(lhs, rhs, "ne"),
            BinaryOp::NotEq if is_str => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt if is_float => self.builder.fcmp_olt(lhs, rhs, "lt"),
            BinaryOp::Lt if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
                .unwrap_or_else(|| self.builder.icmp_slt(lhs, rhs, "lt")),
            BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
            BinaryOp::Gt if is_float => self.builder.fcmp_ogt(lhs, rhs, "gt"),
            BinaryOp::Gt if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
                .unwrap_or_else(|| self.builder.icmp_sgt(lhs, rhs, "gt")),
            BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
            BinaryOp::LtEq if is_float => self.builder.fcmp_ole(lhs, rhs, "le"),
            BinaryOp::LtEq if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
                .unwrap_or_else(|| self.builder.icmp_sle(lhs, rhs, "le")),
            BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
            BinaryOp::GtEq if is_float => self.builder.fcmp_oge(lhs, rhs, "ge"),
            BinaryOp::GtEq if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
                .unwrap_or_else(|| self.builder.icmp_sge(lhs, rhs, "ge")),
            BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
            BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
            BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
            BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
            BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
            BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
            BinaryOp::Coalesce => {
                // opt ?? default → extract tag, if Some(0) return payload else default
                // Result: same pattern — Ok(0) return payload else default
                let tag = self
                    .builder
                    .extract_value(lhs, 0, "coal.tag")
                    .unwrap_or(lhs);
                let payload = self
                    .builder
                    .extract_value(lhs, 1, "coal.val")
                    .unwrap_or(lhs);
                let zero = self.builder.const_i64(0);
                let is_some = self.builder.icmp_eq(tag, zero, "is_some");
                self.builder.select(is_some, payload, rhs, "coal")
            }
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
                // Range/matmul ops are desugared or trait-dispatched before reaching ARC IR
                tracing::warn!(?op, "ArcIrEmitter: desugared op in binary expression");
                self.builder.const_i64(0)
            }
        }
    }

    /// Emit a unary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `-` → `Negate.negate()`).
    pub(super) fn emit_unary_op(
        &mut self,
        op: UnaryOp,
        operand: ValueId,
        operand_ty: Idx,
    ) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !operand_ty.is_primitive() {
            if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
                return result;
            }
        }

        let is_float = matches!(
            self.type_info.get(operand_ty),
            super::super::type_info::TypeInfo::Float
        );

        match op {
            UnaryOp::Neg if is_float => self.builder.fneg(operand, "neg"),
            UnaryOp::Neg => self.builder.neg(operand, "neg"),
            UnaryOp::Not => self.builder.not(operand, "not"),
            UnaryOp::BitNot => {
                let all_ones = self.builder.const_i64(-1);
                self.builder.xor(operand, all_ones, "bitnot")
            }
            UnaryOp::Try => {
                // Try is desugared before reaching ARC IR
                tracing::warn!("ArcIrEmitter: try op in unary expression");
                self.builder.const_i64(0)
            }
        }
    }

    /// Dispatch a binary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `+` → `"add"`),
    /// looks up the compiled method function, and emits a method call.
    fn emit_binary_op_via_trait(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        // Scope the immutable borrow of method_functions: extract only what
        // we need so we can call &mut self methods below.
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "op_trait")
            }
        }
    }

    /// Dispatch comparison operators to Eq/Comparable trait methods.
    ///
    /// Comparison operators are not in `trait_method_name()` because they use
    /// a different dispatch model than arithmetic operators:
    /// - `==`/`!=` → `Eq.equals(self, other) -> bool`
    /// - `<`/`>`/`<=`/`>=` → `Comparable.compare(self, other) -> Ordering`
    ///   then check the i8 result against ordering constants.
    fn emit_comparison_via_trait(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        // Map comparison operators to their trait method and post-processing.
        // Note: Eq.method_name() is "eq" (not "equals") per DerivedTrait definition.
        let (method_name, negate) = match op {
            BinaryOp::Eq => ("eq", false),
            BinaryOp::NotEq => ("eq", true),
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
                return self.emit_ordering_comparison(op, lhs, rhs, lhs_ty);
            }
            _ => return None,
        };

        // Tuple equality: compare element-wise inline (no trait impl).
        // Tuples aren't in type_idx_to_name so trait dispatch won't find them.
        if let super::super::type_info::TypeInfo::Tuple { elements } = self.type_info.get(lhs_ty) {
            let result = self.emit_tuple_equals(lhs, rhs, &elements);
            return if negate {
                result.map(|r| self.builder.not(r, "neq"))
            } else {
                result
            };
        }

        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (*fid, abi.params.clone(), abi.return_abi.passing.clone())
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        let result = match &ret_passing {
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "eq_trait")
            }
            ReturnPassing::Sret { .. } => {
                // equals() returns bool — should always be Direct
                self.emit_rt_call(func_id, &passed_args, "eq_trait")
            }
        }?;

        if negate {
            Some(self.builder.not(result, "neq"))
        } else {
            Some(result)
        }
    }

    /// Emit `<`, `>`, `<=`, `>=` via `Comparable.compare()` + ordering check.
    ///
    /// `compare(self, other)` returns `Ordering` (i8): 0=Less, 1=Equal, 2=Greater.
    fn emit_ordering_comparison(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern("compare");
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        let ordering = match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "cmp_trait")?
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "cmp_trait")?
            }
        };

        // Ordering is i8: 0=Less, 1=Equal, 2=Greater
        // Map comparison operators to equality/inequality checks on the ordering value.
        let less = self.builder.const_i8(0);
        let greater = self.builder.const_i8(2);
        let result = match op {
            BinaryOp::Lt => self.builder.icmp_eq(ordering, less, "lt"),
            BinaryOp::Gt => self.builder.icmp_eq(ordering, greater, "gt"),
            BinaryOp::LtEq => self.builder.icmp_ne(ordering, greater, "le"),
            BinaryOp::GtEq => self.builder.icmp_ne(ordering, less, "ge"),
            _ => unreachable!("only Lt/Gt/LtEq/GtEq reach here"),
        };
        Some(result)
    }

    /// Dispatch a unary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `-` → `"negate"`),
    /// looks up the compiled method function, and emits a method call.
    fn emit_unary_op_via_trait(
        &mut self,
        op: UnaryOp,
        operand: ValueId,
        operand_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.ctx.type_idx_to_name.get(&operand_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [operand];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.emit_rt_call(func_id, &passed_args, "op_trait")
            }
        }
    }
}
