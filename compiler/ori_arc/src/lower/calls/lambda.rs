//! Lambda lowering and capture analysis.
//!
//! Lambda bodies become separate [`ArcFunction`]s with captures as leading
//! parameters followed by user parameters. The call site emits `PartialApply`
//! to pack the captured outer variables into a closure.

use rustc_hash::FxHashSet;

use ori_ir::canon::{
    CanExpr, CanFieldRange, CanId, CanMapEntryRange, CanNamedExprRange, CanParamRange, CanRange,
};
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

        let mut captures = Vec::new();
        let mut seen = FxHashSet::default();
        self.collect_captures(body, &param_names, &mut captures, &mut seen);
        tracing::debug!(
            params = param_names.len(),
            captures = captures.len(),
            "lambda: lower"
        );

        // Expose the outer Function/Scheme shape so its parameter list can be
        // read. Parameter children themselves retain their nominal identity:
        // collapsing a Named/Applied child through `resolve_fully` would make
        // the target signature differ from the PartialApply closure signature.
        // The shared post-lowering materializer owns deep Var/BoundVar link
        // substitution before AIMS and executable-artifact closure.
        let resolved_ty = self.pool.resolve_fully(ty);
        // Unwrap Scheme to reach the inner Function type.
        // Why: A polymorphic lambda's `Function` tag is inside its `Scheme`;
        // resolving parameters against the wrapper would default them to unit.
        let fn_ty = if self.pool.tag(resolved_ty) == Tag::Scheme {
            self.pool.scheme_body(resolved_ty)
        } else {
            resolved_ty
        };
        let (fn_param_types, declared_return_type) = if self.pool.tag(fn_ty) == Tag::Function {
            let params = self.pool.function_params(fn_ty);
            (
                params
                    .into_iter()
                    .map(|parameter| self.resolve_body_type(parameter))
                    .collect(),
                Some(self.resolve_body_type(self.pool.function_return(fn_ty))),
            )
        } else {
            (vec![Idx::UNIT; param_slice.len()], None)
        };

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

        // The target's result is the callable's declared result, not the
        // possibly narrower type inferred for its body expression (for example
        // `DoubleEndedIterator<T>` under a declared `Iterator<T>` result).
        // Exact agreement is required by every executable backend's closure ABI.
        let body_ty = declared_return_type.unwrap_or_else(|| {
            let raw_body_ty = self.expr_type(body);
            if self.pool.tag(raw_body_ty) == Tag::Scheme {
                self.pool.scheme_body(raw_body_ty)
            } else {
                raw_body_ty
            }
        });
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
                loop_ctx_stack: Vec::new(),
                problems: &mut lambda_problems,
                lambdas: self.lambdas,
                hash_length: None,
                block_let_names: rustc_hash::FxHashSet::default(),
                func_name: self.func_name,
                variant_ctors: self.variant_ctors,
                type_subst: self.type_subst,
                const_bindings: self.const_bindings,
                return_type: body_ty,
            };
            let result = lambda_lowerer.lower_expr(body);
            if !lambda_lowerer.builder.is_terminated() {
                lambda_lowerer.builder.terminate_return(result);
            }
        }

        self.problems.append(&mut lambda_problems);

        // Why: the parent function name in the lambda name keeps AIMS
        // contract-map entries for same-index lambdas in different parents
        // from colliding.
        let lambda_idx = self.lambdas.len();
        let parent = self.interner.lookup(self.func_name);
        let lambda_name = self
            .interner
            .intern(&format!("__lambda_{parent}_{lambda_idx}"));
        let mut lambda_func =
            lambda_builder.finish(lambda_name, lambda_params, body_ty, entry, false);
        lambda_func.num_captures = captures.len();
        self.lambdas.push(lambda_func);

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
    fn collect_captures(
        &self,
        expr_id: CanId,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        if !expr_id.is_valid() {
            return;
        }

        let kind = *self.arena.kind(expr_id);
        match kind {
            CanExpr::Ident(name) => self.record_lambda_capture(name, params, captures, seen),
            CanExpr::Binary {
                left: first,
                right: second,
                ..
            }
            | CanExpr::Index {
                receiver: first,
                index: second,
            }
            | CanExpr::Assign {
                target: first,
                value: second,
            }
            | CanExpr::WithCapability {
                provider: first,
                body: second,
                ..
            } => self.collect_two_captures([first, second], params, captures, seen),
            CanExpr::Unary { operand: child, .. }
            | CanExpr::Lambda { body: child, .. }
            | CanExpr::Loop { body: child, .. }
            | CanExpr::Field {
                receiver: child, ..
            }
            | CanExpr::Ok(child)
            | CanExpr::Err(child)
            | CanExpr::Some(child)
            | CanExpr::Try(child)
            | CanExpr::Await(child)
            | CanExpr::Unsafe(child)
            | CanExpr::Break { value: child, .. }
            | CanExpr::Continue { value: child, .. }
            | CanExpr::Cast { expr: child, .. }
            | CanExpr::FormatWith { expr: child, .. }
            | CanExpr::Let { init: child, .. } => {
                self.collect_captures(child, params, captures, seen);
            }
            CanExpr::Call { func: callee, args }
            | CanExpr::MethodCall {
                receiver: callee,
                args,
                ..
            } => self.collect_call_captures(callee, args, params, captures, seen),
            CanExpr::If {
                cond: first,
                then_branch: second,
                else_branch: third,
            }
            | CanExpr::For {
                iter: first,
                guard: second,
                body: third,
                ..
            }
            | CanExpr::Range {
                start: first,
                end: second,
                step: third,
                ..
            } => self.collect_three_captures([first, second, third], params, captures, seen),
            CanExpr::Block { stmts, result } => {
                self.collect_block_captures(stmts, result, params, captures, seen);
            }
            CanExpr::Match {
                scrutinee, arms, ..
            } => {
                self.collect_match_captures(scrutinee, arms, params, captures, seen);
            }
            CanExpr::Tuple(range) | CanExpr::List(range) => {
                self.collect_range_captures(range, params, captures, seen);
            }
            CanExpr::Struct { fields, .. } => self.collect_fields(fields, params, captures, seen),
            CanExpr::Map(entries) => self.collect_map_captures(entries, params, captures, seen),
            CanExpr::FunctionExp { props, .. } => {
                self.collect_named_captures(props, params, captures, seen);
            }
            CanExpr::Constant(_)
            | CanExpr::Int(_)
            | CanExpr::Float(_)
            | CanExpr::Bool(_)
            | CanExpr::Char(_)
            | CanExpr::Str(_)
            | CanExpr::FunctionRef(_)
            | CanExpr::TypeRef(_)
            | CanExpr::Const(_)
            | CanExpr::Duration { .. }
            | CanExpr::Size { .. }
            | CanExpr::Unit
            | CanExpr::None
            | CanExpr::Error
            | CanExpr::SelfRef
            | CanExpr::HashLength => {}
        }
    }

    fn record_lambda_capture(
        &self,
        name: Name,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        if params.contains(&name) || seen.contains(&name) {
            return;
        }
        if let Some(var) = self.scope.lookup(name) {
            seen.insert(name);
            captures.push((name, var));
        }
    }

    fn collect_call_captures(
        &self,
        callee: CanId,
        args: CanRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        self.collect_captures(callee, params, captures, seen);
        self.collect_range_captures(args, params, captures, seen);
    }

    fn collect_block_captures(
        &self,
        stmts: CanRange,
        result: CanId,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        self.collect_range_captures(stmts, params, captures, seen);
        self.collect_captures(result, params, captures, seen);
    }

    fn collect_match_captures(
        &self,
        scrutinee: CanId,
        arms: CanRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        self.collect_captures(scrutinee, params, captures, seen);
        self.collect_range_captures(arms, params, captures, seen);
    }

    fn collect_three_captures(
        &self,
        exprs: [CanId; 3],
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for expr in exprs {
            self.collect_captures(expr, params, captures, seen);
        }
    }

    fn collect_two_captures(
        &self,
        exprs: [CanId; 2],
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for expr in exprs {
            self.collect_captures(expr, params, captures, seen);
        }
    }

    fn collect_range_captures(
        &self,
        range: CanRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for &expr in self.arena.get_expr_list(range) {
            self.collect_captures(expr, params, captures, seen);
        }
    }

    fn collect_fields(
        &self,
        fields: CanFieldRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for field in self.arena.get_fields(fields) {
            self.collect_captures(field.value, params, captures, seen);
        }
    }

    fn collect_map_captures(
        &self,
        entries: CanMapEntryRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for entry in self.arena.get_map_entries(entries) {
            self.collect_captures(entry.key, params, captures, seen);
            self.collect_captures(entry.value, params, captures, seen);
        }
    }

    fn collect_named_captures(
        &self,
        props: CanNamedExprRange,
        params: &[Name],
        captures: &mut Vec<(Name, ArcVarId)>,
        seen: &mut FxHashSet<Name>,
    ) {
        for named in self.arena.get_named_exprs(props) {
            self.collect_captures(named.value, params, captures, seen);
        }
    }
}
