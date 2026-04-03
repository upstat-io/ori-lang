//! Short-circuit and lazy-evaluation lowering for `&&`, `||`, and `??`.
//!
//! These operators cannot use eager (`PrimOp`) evaluation because the RHS
//! must only be evaluated when the LHS doesn't determine the result.
//! ARC lowering converts them to control-flow IR (branch → blocks → merge).

use ori_ir::Span;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use ori_ir::canon::CanId;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::ArcLowerer;
use crate::lower::scope::merge_mutable_vars;

impl ArcLowerer<'_> {
    /// Lower `lhs ?? rhs` with lazy RHS evaluation.
    ///
    /// Generates: extract tag from LHS → branch on tag == 0 (Some/Ok) →
    /// then: extract payload (or pass-through if chaining) → merge;
    /// else: evaluate RHS → merge.
    ///
    /// Chaining detection: the result type `ty` must equal the LHS type
    /// (same Idx via pool interning). This correctly distinguishes:
    /// - `Option<T> ?? Option<T> -> Option<T>` (CHAIN: ty == `lhs_ty`)
    /// - `Option<Option<T>> ?? Option<T> -> Option<T>` (UNWRAP: ty != `lhs_ty`)
    pub(in crate::lower) fn lower_coalesce(
        &mut self,
        left: CanId,
        right: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let lhs = self.lower_expr(left);

        // Detect chaining: result type equals LHS type (both are the same wrapper).
        // Pool interning guarantees structural equality via Idx identity.
        // Uses expr_type() to correctly convert TypeId→Idx with type substitution.
        let lhs_ty = self.pool.resolve_fully(self.expr_type(left));
        let resolved_ty = self.pool.resolve_fully(ty);
        let is_chaining = lhs_ty == resolved_ty;

        // Extract tag (field 0) and compare to Some/Ok tag value.
        let tag = self.builder.emit_project(Idx::INT, lhs, 0, Some(span));
        let some_tag = self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(ori_ir::OPTION_TAG_SOME)),
            Some(span),
        );
        let is_some = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag, some_tag],
            },
            Some(span),
        );

        // Create blocks for the two branches + merge.
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        let merge_block = self.builder.new_block();
        self.builder
            .terminate_branch(is_some, some_block, none_block);

        let pre_scope = self.scope.clone();
        let mut mutable_var_types = FxHashMap::default();
        for (name, var) in pre_scope.mutable_bindings() {
            mutable_var_types.insert(name, self.builder.var_type_or_unit(var));
        }

        // Some/Ok branch: pass-through LHS if chaining, extract payload otherwise.
        self.builder.position_at(some_block);
        self.scope = pre_scope.clone();
        let some_val = if is_chaining {
            lhs
        } else {
            self.builder.emit_project(ty, lhs, 1, Some(span))
        };
        let some_scope = self.scope.clone();
        let some_terminated = self.builder.is_terminated();
        let some_exit = self.builder.current_block();

        // None/Err branch: evaluate RHS lazily.
        self.builder.position_at(none_block);
        self.scope = pre_scope.clone();
        let rhs_val = self.lower_expr(right);
        let none_scope = self.scope.clone();
        let none_terminated = self.builder.is_terminated();
        let none_exit = self.builder.current_block();

        // Merge block: result param + mutable variable merge params.
        let result_param = self.builder.add_block_param(merge_block, ty);
        let rebindings = merge_mutable_vars(
            self.builder,
            merge_block,
            &pre_scope,
            &[some_scope.clone(), none_scope.clone()],
            &mutable_var_types,
        );

        if !some_terminated {
            self.builder.position_at(some_exit);
            let mut jump_args = vec![some_val];
            for (name, _) in &rebindings {
                let var = some_scope.lookup(*name).unwrap_or(some_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }
        if !none_terminated {
            self.builder.position_at(none_exit);
            let mut jump_args = vec![rhs_val];
            for (name, _) in &rebindings {
                let var = none_scope.lookup(*name).unwrap_or(rhs_val);
                jump_args.push(var);
            }
            self.builder.terminate_jump(merge_block, jump_args);
        }

        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        for (name, merge_var) in &rebindings {
            self.scope.bind_mutable(*name, *merge_var);
        }
        result_param
    }

    /// Lower `a && b` with short-circuit evaluation.
    ///
    /// Generates: evaluate a → branch on a → then: evaluate b → merge;
    /// else: false → merge. Same pattern as `lower_coalesce`.
    pub(in crate::lower) fn lower_short_circuit_and(
        &mut self,
        left: CanId,
        right: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let lhs = self.lower_expr(left);

        let then_block = self.builder.new_block();
        let else_block = self.builder.new_block();
        let merge_block = self.builder.new_block();
        self.builder.terminate_branch(lhs, then_block, else_block);

        let pre_scope = self.scope.clone();

        // Then: evaluate RHS (only when LHS is true)
        self.builder.position_at(then_block);
        self.scope = pre_scope.clone();
        let rhs = self.lower_expr(right);
        let then_terminated = self.builder.is_terminated();
        let then_exit = self.builder.current_block();

        // Else: false
        self.builder.position_at(else_block);
        self.scope = pre_scope.clone();
        let false_val = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::Literal(LitValue::Bool(false)),
            Some(span),
        );
        let else_exit = self.builder.current_block();

        // Merge
        let result = self.builder.add_block_param(merge_block, ty);
        if !then_terminated {
            self.builder.position_at(then_exit);
            self.builder.terminate_jump(merge_block, vec![rhs]);
        }
        self.builder.position_at(else_exit);
        self.builder.terminate_jump(merge_block, vec![false_val]);

        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        result
    }

    /// Lower `a || b` with short-circuit evaluation.
    ///
    /// Generates: evaluate a → branch on a → then: true → merge;
    /// else: evaluate b → merge. Same pattern as `lower_coalesce`.
    pub(in crate::lower) fn lower_short_circuit_or(
        &mut self,
        left: CanId,
        right: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let lhs = self.lower_expr(left);

        let then_block = self.builder.new_block();
        let else_block = self.builder.new_block();
        let merge_block = self.builder.new_block();
        self.builder.terminate_branch(lhs, then_block, else_block);

        let pre_scope = self.scope.clone();

        // Then: true
        self.builder.position_at(then_block);
        self.scope = pre_scope.clone();
        let true_val = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::Literal(LitValue::Bool(true)),
            Some(span),
        );
        let then_exit = self.builder.current_block();

        // Else: evaluate RHS (only when LHS is false)
        self.builder.position_at(else_block);
        self.scope = pre_scope.clone();
        let rhs = self.lower_expr(right);
        let else_terminated = self.builder.is_terminated();
        let else_exit = self.builder.current_block();

        // Merge
        let result = self.builder.add_block_param(merge_block, ty);
        self.builder.position_at(then_exit);
        self.builder.terminate_jump(merge_block, vec![true_val]);
        if !else_terminated {
            self.builder.position_at(else_exit);
            self.builder.terminate_jump(merge_block, vec![rhs]);
        }

        self.builder.position_at(merge_block);
        self.scope = pre_scope;
        result
    }
}
