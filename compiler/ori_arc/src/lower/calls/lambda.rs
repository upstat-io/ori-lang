//! Lambda lowering and capture analysis.
//!
//! Lambda bodies become separate [`ArcFunction`]s with captures as leading
//! parameters followed by user parameters. The call site emits `PartialApply`
//! to pack the captured outer variables into a closure.

use std::collections::HashSet;

use ori_ir::canon::{CanExpr, CanId, CanParamRange};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcParam, ArcVarId};
use crate::Ownership;

use super::super::expr::ArcLowerer;
use super::super::scope::ArcScope;
use super::super::ArcIrBuilder;

impl ArcLowerer<'_> {
    // Lambda

    /// Lower a lambda expression.
    ///
    /// The lambda body becomes a separate `ArcFunction` with captures as
    /// leading parameters followed by user parameters. The lambda expression
    /// emits `PartialApply` to bind the captured outer variables.
    pub(crate) fn lower_lambda(
        &mut self,
        params: CanParamRange,
        body: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let param_slice: Vec<_> = self.arena.get_params(params).to_vec();
        let param_names: Vec<Name> = param_slice.iter().map(|p| p.name).collect();

        // Step 1: Capture analysis — find free variables in the lambda body
        let mut captures = Vec::new();
        let mut seen = HashSet::new();
        self.collect_captures(body, &param_names, &mut captures, &mut seen);
        tracing::debug!(
            params = param_names.len(),
            captures = captures.len(),
            "lambda: lower"
        );

        // Step 2: Get actual param types from the function type in the pool
        let fn_param_types = if self.pool.tag(ty) == Tag::Function {
            self.pool.function_params(ty)
        } else {
            vec![Idx::UNIT; param_slice.len()]
        };

        // Step 3: Build the lambda function body
        let mut lambda_builder = ArcIrBuilder::new();
        let mut lambda_scope = ArcScope::new();
        let mut lambda_params = Vec::with_capacity(captures.len() + param_slice.len());

        // Capture params come first — the lambda receives them as regular parameters
        for &(name, outer_var) in &captures {
            let cap_ty = self.builder.var_type(outer_var);
            let var = lambda_builder.fresh_var(cap_ty);
            lambda_scope.bind(name, var);
            lambda_params.push(ArcParam {
                var,
                ty: cap_ty,
                ownership: Ownership::Owned,
            });
        }

        // User params follow
        for (i, param) in param_slice.iter().enumerate() {
            let param_ty = fn_param_types.get(i).copied().unwrap_or(Idx::UNIT);
            let var = lambda_builder.fresh_var(param_ty);
            lambda_scope.bind(param.name, var);
            lambda_params.push(ArcParam {
                var,
                ty: param_ty,
                ownership: Ownership::Owned,
            });
        }

        let body_ty = self.expr_type(body);
        let entry = lambda_builder.entry_block();

        // Lower the lambda body
        let mut lambda_problems = Vec::new();
        {
            let mut lambda_lowerer = ArcLowerer {
                builder: &mut lambda_builder,
                arena: self.arena,
                canon: self.canon,
                interner: self.interner,
                pool: self.pool,
                scope: lambda_scope,
                loop_ctx: None,
                problems: &mut lambda_problems,
                lambdas: self.lambdas,
                hash_length: None,
                block_let_names: rustc_hash::FxHashSet::default(),
                func_name: self.func_name,
                variant_ctors: self.variant_ctors,
                type_subst: self.type_subst,
            };
            let result = lambda_lowerer.lower_expr(body);
            if !lambda_lowerer.builder.is_terminated() {
                lambda_lowerer.builder.terminate_return(result);
            }
        }

        self.problems.append(&mut lambda_problems);

        // Step 4: Assign unique name (after inner lambdas are pushed, so index is unique)
        let lambda_idx = self.lambdas.len();
        let lambda_name = self.interner.intern(&format!("__lambda_{lambda_idx}"));
        let mut lambda_func =
            lambda_builder.finish(lambda_name, lambda_params, body_ty, entry, false);
        lambda_func.num_captures = captures.len();
        self.lambdas.push(lambda_func);

        // Step 5: Emit PartialApply with the outer capture variable IDs
        let capture_outer_vars: Vec<ArcVarId> =
            captures.iter().map(|&(_, outer_var)| outer_var).collect();
        self.builder
            .emit_partial_apply(ty, lambda_name, capture_outer_vars, Some(span))
    }

    // Capture analysis

    /// Collect free variables (captures) from a lambda body.
    ///
    /// Walks the canonical expression tree. For each `Ident(name)` not in the
    /// lambda's parameter list and present in the outer scope, records
    /// `(name, outer_arc_var_id)`.
    #[expect(
        clippy::too_many_lines,
        reason = "dispatch table — each arm is 1-3 lines"
    )]
    fn collect_captures(
        &self,
        expr_id: CanId,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut HashSet<Name>,
    ) {
        if !expr_id.is_valid() {
            return;
        }

        let kind = *self.arena.kind(expr_id);
        match kind {
            CanExpr::Ident(name) => {
                if !params.contains(&name) && !seen.contains(&name) {
                    if let Some(var) = self.scope.lookup(name) {
                        seen.insert(name);
                        captures.push((name, var));
                    }
                }
            }
            CanExpr::Binary { left, right, .. } => {
                self.collect_captures(left, params, captures, seen);
                self.collect_captures(right, params, captures, seen);
            }
            CanExpr::Unary { operand, .. } => {
                self.collect_captures(operand, params, captures, seen);
            }
            CanExpr::Call { func, args } => {
                self.collect_captures(func, params, captures, seen);
                for &arg in self.arena.get_expr_list(args) {
                    self.collect_captures(arg, params, captures, seen);
                }
            }
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_captures(cond, params, captures, seen);
                self.collect_captures(then_branch, params, captures, seen);
                self.collect_captures(else_branch, params, captures, seen);
            }
            CanExpr::Block { stmts, result } => {
                for &stmt in self.arena.get_expr_list(stmts) {
                    self.collect_captures(stmt, params, captures, seen);
                }
                self.collect_captures(result, params, captures, seen);
            }
            CanExpr::Lambda { body, .. } | CanExpr::Loop { body, .. } => {
                self.collect_captures(body, params, captures, seen);
            }
            CanExpr::Field { receiver, .. } => {
                self.collect_captures(receiver, params, captures, seen);
            }
            CanExpr::Index { receiver, index } => {
                self.collect_captures(receiver, params, captures, seen);
                self.collect_captures(index, params, captures, seen);
            }
            CanExpr::For {
                iter, body, guard, ..
            } => {
                self.collect_captures(iter, params, captures, seen);
                self.collect_captures(guard, params, captures, seen);
                self.collect_captures(body, params, captures, seen);
            }
            CanExpr::Match {
                scrutinee, arms, ..
            } => {
                self.collect_captures(scrutinee, params, captures, seen);
                for &arm_body in self.arena.get_expr_list(arms) {
                    self.collect_captures(arm_body, params, captures, seen);
                }
            }
            CanExpr::Ok(e)
            | CanExpr::Err(e)
            | CanExpr::Some(e)
            | CanExpr::Try(e)
            | CanExpr::Await(e)
            | CanExpr::Unsafe(e)
            | CanExpr::Break { value: e, .. }
            | CanExpr::Continue { value: e, .. } => {
                self.collect_captures(e, params, captures, seen);
            }
            CanExpr::Assign { target, value } => {
                self.collect_captures(target, params, captures, seen);
                self.collect_captures(value, params, captures, seen);
            }
            CanExpr::Cast { expr, .. } | CanExpr::FormatWith { expr, .. } => {
                self.collect_captures(expr, params, captures, seen);
            }
            CanExpr::Tuple(range) | CanExpr::List(range) => {
                for &e in self.arena.get_expr_list(range) {
                    self.collect_captures(e, params, captures, seen);
                }
            }
            CanExpr::MethodCall { receiver, args, .. } => {
                self.collect_captures(receiver, params, captures, seen);
                for &arg in self.arena.get_expr_list(args) {
                    self.collect_captures(arg, params, captures, seen);
                }
            }
            CanExpr::WithCapability { body, provider, .. } => {
                self.collect_captures(provider, params, captures, seen);
                self.collect_captures(body, params, captures, seen);
            }
            CanExpr::Let { init, .. } => {
                self.collect_captures(init, params, captures, seen);
            }
            CanExpr::Range {
                start, end, step, ..
            } => {
                self.collect_captures(start, params, captures, seen);
                self.collect_captures(end, params, captures, seen);
                self.collect_captures(step, params, captures, seen);
            }
            CanExpr::Struct { fields, .. } => {
                for fi in self.arena.get_fields(fields) {
                    self.collect_captures(fi.value, params, captures, seen);
                }
            }
            CanExpr::Map(entries) => {
                for entry in self.arena.get_map_entries(entries) {
                    self.collect_captures(entry.key, params, captures, seen);
                    self.collect_captures(entry.value, params, captures, seen);
                }
            }
            CanExpr::FunctionExp { props, .. } => {
                for ne in self.arena.get_named_exprs(props) {
                    self.collect_captures(ne.value, params, captures, seen);
                }
            }
            // Leaf expressions — no free variables
            CanExpr::Constant(_)
            | CanExpr::Int(_)
            | CanExpr::Float(_)
            | CanExpr::Bool(_)
            | CanExpr::Char(_)
            | CanExpr::Str(_)
            | CanExpr::Unit
            | CanExpr::None
            | CanExpr::Error
            | CanExpr::SelfRef
            | CanExpr::FunctionRef(_)
            | CanExpr::TypeRef(_)
            | CanExpr::Const(_)
            | CanExpr::HashLength
            | CanExpr::Duration { .. }
            | CanExpr::Size { .. } => {}
        }
    }
}
