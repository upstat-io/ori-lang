//! Pattern and parameter lowering — match, multi-clause, binding patterns, params, `function_exp`.
//!
//! Handles lowering of match expressions (including pattern compilation and
//! exhaustiveness checking), multi-clause function definitions, binding
//! pattern destructuring, parameter lists, and function expressions.

use ori_ir::canon::{
    CanBindingPattern, CanBindingPatternId, CanExpr, CanId, CanNamedExpr, CanParam,
};
use ori_ir::{ExprId, Name, Span, TypeId};

use super::Lowerer;

impl Lowerer<'_> {
    // Match Lowering

    /// Lower a match expression.
    ///
    /// Compiles arm patterns into a decision tree using the Maranget (2008)
    /// algorithm, then lowers arm bodies into the canonical arena.
    pub(super) fn lower_match(
        &mut self,
        scrutinee: ExprId,
        arms: ori_ir::ArmRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let scrutinee_id = self.lower_expr(scrutinee);

        // Get scrutinee type for pattern flattening.
        let scrutinee_ty = self
            .typed
            .expr_type(scrutinee.index())
            .unwrap_or(ori_types::Idx::UNIT);

        // Extract arm data to avoid borrow conflict (patterns + guards + bodies).
        // We separate bodies from patterns+guards so patterns can be consumed
        // (moved) into pattern_data without a second clone.
        let src_arms = self.src.get_arms(arms);
        let mut patterns_and_guards: Vec<_> = src_arms
            .iter()
            .map(|arm| (arm.pattern.clone(), arm.guard))
            .collect();
        let bodies: Vec<_> = src_arms.iter().map(|arm| arm.body).collect();

        // Lower guards from ExprId → CanId and take ownership of patterns
        // in a single pass. Guards must be lowered here (where we have &mut self)
        // rather than inside compile_patterns (which only borrows &self).
        let pattern_data: Vec<_> = patterns_and_guards
            .drain(..)
            .map(|(pat, guard)| {
                let can_guard = guard.map(|g| self.lower_expr(g));
                (pat, can_guard)
            })
            .collect();
        let tree = crate::patterns::compile_patterns(self, &pattern_data, arms.start, scrutinee_ty);

        // Exhaustiveness check: capture arm spans from the source arms (still
        // accessible since src_arms borrows the read-only source arena).
        let arm_spans: Vec<Span> = src_arms.iter().map(|arm| arm.span).collect();
        let check = crate::exhaustiveness::check_exhaustiveness(
            &tree,
            src_arms.len(),
            span,
            &arm_spans,
            scrutinee_ty,
            self.pool,
            self.interner,
        );
        self.problems.extend(check.problems);

        let dt_id = self.decision_trees.push(tree);

        // Lower arm bodies BEFORE building the expr list. lower_expr may
        // recursively lower nested match expressions, which would push their
        // own arm bodies into the flat expr_lists array, corrupting our range.
        let lowered_bodies: Vec<CanId> = bodies.iter().map(|body| self.lower_expr(*body)).collect();
        let start = self.arena.start_expr_list();
        for can_body in lowered_bodies {
            self.arena.push_expr_list_item(can_body);
        }
        let arms_range = self.arena.finish_expr_list(start);

        self.push(
            CanExpr::Match {
                scrutinee: scrutinee_id,
                decision_tree: dt_id,
                arms: arms_range,
            },
            span,
            ty,
        )
    }

    // Multi-Clause Function Lowering

    /// Lower a group of same-name functions into a single body with a
    /// synthesized match expression.
    ///
    /// Transforms:
    /// ```text
    /// @fib (0: int) -> int = 0
    /// @fib (1: int) -> int = 1
    /// @fib (n: int) -> int = fib(n - 1) + fib(n - 2)
    /// ```
    /// Into a single function body equivalent to:
    /// ```text
    /// match param0 {
    ///   0 -> 0
    ///   1 -> 1
    ///   n -> fib(n - 1) + fib(n - 2)
    /// }
    /// ```
    ///
    /// For multi-parameter functions, the scrutinee is a tuple of the
    /// parameters and each arm pattern is a tuple of the param patterns.
    pub(super) fn lower_multi_clause(&mut self, clauses: &[&ori_ir::Function]) -> CanId {
        debug_assert!(clauses.len() >= 2);

        // Use the first clause's span/type as the overall function span/type.
        let span = clauses[0].span;
        let ty = self.expr_type(clauses[0].body);

        // Determine parameter names from the first clause.
        let first_params = self.src.get_params(clauses[0].params);
        let param_count = first_params.len();
        let param_names: Vec<Name> = first_params.iter().map(|p| p.name).collect();

        // Build the scrutinee: Ident for single-param, Tuple for multi-param.
        // Types use ERROR because these are synthetic nodes — the evaluator
        // dispatches on values, not types. Codegen (LLVM) would need real types,
        // but multi-clause functions aren't supported there yet.
        let scrutinee_id = if param_count == 1 {
            self.push(CanExpr::Ident(param_names[0]), span, TypeId::ERROR)
        } else {
            let idents: Vec<CanId> = param_names
                .iter()
                .map(|&name| self.push(CanExpr::Ident(name), span, TypeId::ERROR))
                .collect();
            let range = self.arena.push_expr_list(&idents);
            self.push(CanExpr::Tuple(range), span, TypeId::ERROR)
        };

        // Flatten each clause's parameter patterns into a FlatPattern row.
        // Guards are lowered from ExprId → CanId before decision tree compilation.
        let mut flat_rows: Vec<Vec<ori_ir::canon::tree::FlatPattern>> =
            Vec::with_capacity(clauses.len());
        let mut guards: Vec<Option<CanId>> = Vec::with_capacity(clauses.len());

        for clause in clauses {
            let params = self.src.get_params(clause.params).to_vec();
            let row: Vec<_> = params
                .iter()
                .map(|p| crate::patterns::flatten_param_pattern(self, p))
                .collect();
            flat_rows.push(row);
            guards.push(clause.guard.map(|g| self.lower_expr(g)));
        }

        // Compile the multi-column pattern matrix into a decision tree.
        let tree = crate::patterns::compile_multi_clause_patterns(&flat_rows, &guards);

        // Exhaustiveness check: use clause spans as "arm" spans.
        let clause_spans: Vec<Span> = clauses.iter().map(|c| c.span).collect();
        let check = crate::exhaustiveness::check_exhaustiveness(
            &tree,
            clauses.len(),
            span,
            &clause_spans,
            ori_types::Idx::UNIT,
            self.pool,
            self.interner,
        );
        self.problems.extend(check.problems);

        let dt_id = self.decision_trees.push(tree);

        // Lower each clause body.
        let lowered_bodies: Vec<CanId> = clauses
            .iter()
            .map(|clause| self.lower_expr(clause.body))
            .collect();

        // Build the arms CanRange.
        let start = self.arena.start_expr_list();
        for can_body in lowered_bodies {
            self.arena.push_expr_list_item(can_body);
        }
        let arms_range = self.arena.finish_expr_list(start);

        self.push(
            CanExpr::Match {
                scrutinee: scrutinee_id,
                decision_tree: dt_id,
                arms: arms_range,
            },
            span,
            ty,
        )
    }

    // Binding Pattern Lowering

    /// Lower a `BindingPatternId` (`ExprArena` reference) to `CanBindingPatternId`.
    ///
    /// Recursively lowers `BindingPattern` → `CanBindingPattern`, storing
    /// sub-patterns in the canonical arena.
    pub(super) fn lower_binding_pattern(
        &mut self,
        bp_id: ori_ir::BindingPatternId,
    ) -> CanBindingPatternId {
        let bp = self.src.get_binding_pattern(bp_id).clone();
        self.lower_binding_pattern_value(&bp)
    }

    /// Lower a `BindingPattern` value to canonical form.
    fn lower_binding_pattern_value(&mut self, bp: &ori_ir::BindingPattern) -> CanBindingPatternId {
        let can_bp = match bp {
            ori_ir::BindingPattern::Name { name, mutable } => CanBindingPattern::Name {
                name: *name,
                mutable: *mutable,
            },
            ori_ir::BindingPattern::Wildcard => CanBindingPattern::Wildcard,
            ori_ir::BindingPattern::Tuple(children) => {
                let child_ids: Vec<_> = children
                    .iter()
                    .map(|c| self.lower_binding_pattern_value(c))
                    .collect();
                let range = self.arena.push_binding_pattern_list(&child_ids);
                CanBindingPattern::Tuple(range)
            }
            ori_ir::BindingPattern::Struct { fields } => {
                let field_bindings: Vec<_> = fields
                    .iter()
                    .map(|field| {
                        let sub = match &field.pattern {
                            Some(p) => self.lower_binding_pattern_value(p),
                            // Field shorthand: `{ x }` or `{ $x }` → bind name directly
                            None => self.arena.push_binding_pattern(CanBindingPattern::Name {
                                name: field.name,
                                mutable: field.mutable,
                            }),
                        };
                        ori_ir::canon::CanFieldBinding {
                            name: field.name,
                            pattern: sub,
                        }
                    })
                    .collect();
                let range = self.arena.push_field_bindings(&field_bindings);
                CanBindingPattern::Struct { fields: range }
            }
            ori_ir::BindingPattern::List { elements, rest } => {
                let child_ids: Vec<_> = elements
                    .iter()
                    .map(|c| self.lower_binding_pattern_value(c))
                    .collect();
                let range = self.arena.push_binding_pattern_list(&child_ids);
                CanBindingPattern::List {
                    elements: range,
                    rest: *rest,
                }
            }
        };
        self.arena.push_binding_pattern(can_bp)
    }

    // Param Lowering

    /// Lower a `ParamRange` (`ExprArena` reference) to `CanParamRange`.
    pub(super) fn lower_params(
        &mut self,
        param_range: ori_ir::ParamRange,
    ) -> ori_ir::canon::CanParamRange {
        let src_params = self.src.get_params(param_range);
        if src_params.is_empty() {
            return ori_ir::canon::CanParamRange::EMPTY;
        }

        // Copy out to avoid borrow conflict.
        let param_data: Vec<_> = src_params.iter().map(|p| (p.name, p.default)).collect();

        let can_params: Vec<_> = param_data
            .into_iter()
            .map(|(name, default)| CanParam {
                name,
                default: match default {
                    Some(expr_id) => self.lower_expr(expr_id),
                    None => CanId::INVALID,
                },
            })
            .collect();

        self.arena.push_params(&can_params)
    }

    /// Lower parameter defaults from a `ParamRange` to `Vec<Option<CanId>>`.
    ///
    /// Unlike `lower_params` (which produces a `CanParamRange` for lambda parameters),
    /// this extracts only the default expressions for module-level functions, where
    /// defaults are stored separately from the parameter list in `CanonRoot.defaults`.
    pub(super) fn lower_param_defaults(
        &mut self,
        param_range: ori_ir::ParamRange,
    ) -> Vec<Option<CanId>> {
        let src_params = self.src.get_params(param_range);
        // Copy out to avoid borrow conflict with `self.lower_expr`.
        let defaults: Vec<_> = src_params.iter().map(|p| p.default).collect();
        defaults
            .into_iter()
            .map(|default| default.map(|expr_id| self.lower_expr(expr_id)))
            .collect()
    }

    // FunctionExp Lowering

    /// Lower a `FunctionExp` (`ExprArena` side-table) to inline `CanExpr::FunctionExp`.
    pub(super) fn lower_function_exp(
        &mut self,
        exp_id: ori_ir::FunctionExpId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func_exp = self.src.get_function_exp(exp_id).clone();
        let kind = func_exp.kind;

        // Lower each named prop value.
        let src_props = self.src.get_named_exprs(func_exp.props);
        let prop_data: Vec<_> = src_props.iter().map(|p| (p.name, p.value)).collect();
        let can_props: Vec<_> = prop_data
            .into_iter()
            .map(|(name, value)| CanNamedExpr {
                name,
                value: self.lower_expr(value),
            })
            .collect();
        let props = self.arena.push_named_exprs(&can_props);

        self.push(CanExpr::FunctionExp { kind, props }, span, ty)
    }
}
