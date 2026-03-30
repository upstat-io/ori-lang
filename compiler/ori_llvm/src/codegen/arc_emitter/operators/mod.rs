//! Operator emission for [`ArcIrEmitter`].
//!
//! Emits LLVM IR for binary and unary operations. Primitive/builtin types
//! use [`OpStrategy`] dispatch from the registry; non-primitive types dispatch
//! to operator trait methods (e.g., `+` → `Add.add()`, `==` → `Eq.eq()`).
//!
//! Strategy dispatch helpers live in [`strategy`].

mod strategy;

use ori_ir::{BinaryOp, UnaryOp};
use ori_registry::OpStrategy;
use ori_types::Idx;

use super::ArcIrEmitter;
use crate::codegen::abi::ReturnPassing;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit a binary operation.
    ///
    /// For non-primitive types, dispatches to trait methods. For primitive/builtin
    /// types, uses [`OpStrategy`] from the registry to select the correct LLVM
    /// instruction family — eliminating ad-hoc type guards.
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
            if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
            if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
        }

        // List + list → COW concat (type-info-driven, not OpStrategy)
        if matches!(op, BinaryOp::Add) {
            if let TypeInfo::List { element } = self.type_info.get(lhs_ty) {
                let cm = self.cow_mode_const(arc_func);
                if let Some(val) = self.emit_list_concat_cow(lhs, rhs, element, cm, lhs_ty) {
                    return val;
                }
            }
        }

        // Registry-driven dispatch for primitive/builtin types.
        let Some(type_tag) = self.idx_to_type_tag(lhs_ty) else {
            unreachable!(
                "binary op {op:?} on unmapped type idx {lhs_ty:?} — \
                 should have used trait dispatch"
            );
        };
        let strategy = Self::op_strategy_for_binary(type_tag, op);

        match strategy {
            OpStrategy::IntInstr => self.emit_int_binary_op(op, lhs, rhs),
            OpStrategy::FloatInstr => self.emit_float_binary_op(op, lhs, rhs),
            OpStrategy::UnsignedCmp => self.emit_unsigned_binary_op(op, lhs, rhs),
            OpStrategy::BoolLogic => self.emit_bool_binary_op(op, lhs, rhs),
            OpStrategy::RuntimeCall { .. } => self.emit_runtime_binary_op(op, lhs, rhs),
            OpStrategy::Unsupported => {
                // Fallback: types with no registry entry (Unit, Never, Error,
                // Function) or Unsupported operators still reach here because
                // the type checker silently accepts some unknown methods by
                // returning Idx::ERROR. Use integer instructions as a safe
                // fallback — these types are either uninhabited (Never) or
                // have i64-compatible representation.
                // TODO(typeck): register missing methods so Error type doesn't
                // propagate to codegen. See roadmap section-07A (core built-ins).
                tracing::warn!(
                    ?op,
                    ?type_tag,
                    "binary op on type with Unsupported strategy — \
                     falling back to IntInstr"
                );
                self.emit_int_binary_op(op, lhs, rhs)
            }
        }
    }

    /// Emit a unary operation.
    ///
    /// For non-primitive types, dispatches to the corresponding operator trait
    /// method (e.g., `-` → `Negate.negate()`). For primitive/builtin types,
    /// uses [`OpStrategy`] from the registry to select the correct LLVM
    /// instruction family.
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

        // Registry-driven dispatch for primitive/builtin types.
        let Some(type_tag) = self.idx_to_type_tag(operand_ty) else {
            unreachable!(
                "unary op {op:?} on unmapped type idx {operand_ty:?} — \
                 should have used trait dispatch"
            );
        };
        let strategy = Self::op_strategy_for_unary(type_tag, op);

        match strategy {
            OpStrategy::IntInstr | OpStrategy::BoolLogic | OpStrategy::UnsignedCmp => match op {
                UnaryOp::Neg => self.builder.checked_neg(operand, "neg"),
                UnaryOp::Not => self.builder.not(operand, "not"),
                UnaryOp::BitNot => {
                    let all_ones = self.builder.const_i64(-1);
                    self.builder.xor(operand, all_ones, "bitnot")
                }
                UnaryOp::Try => unreachable!("try desugared before ARC IR"),
            },
            OpStrategy::FloatInstr => match op {
                UnaryOp::Neg => self.builder.fneg(operand, "neg"),
                _ => unreachable!("unsupported float unary op {op:?}"),
            },
            OpStrategy::Unsupported => {
                // Try is desugared before reaching ARC IR. If it slips
                // through, warn and return a zero constant.
                tracing::warn!(?op, ?type_tag, "unary op with Unsupported strategy");
                self.builder.const_i64(0)
            }
            OpStrategy::RuntimeCall { .. } => {
                unreachable!("unary op {op:?} on type {type_tag:?} has RuntimeCall strategy")
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
                abi.return_abi.passing,
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

        // Compound type equality: inline comparison for built-in generic
        // types (Tuple, Option, Result, List) that don't have compiled
        // derived Eq methods in LLVM. Uses recursive element comparison
        // via emit_element_equals().
        if let Some(result) = self.emit_element_equals(lhs, rhs, lhs_ty) {
            return if negate {
                Some(self.builder.not(result, "neq"))
            } else {
                Some(result)
            };
        }

        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (*fid, abi.params.clone(), abi.return_abi.passing)
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
                abi.return_abi.passing,
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
                abi.return_abi.passing,
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
