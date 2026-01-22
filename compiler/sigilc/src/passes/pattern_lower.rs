// Pattern lowering pass for Sigil TIR
// Transforms high-level patterns (fold, map, filter, etc.) into loops
//
// Uses the Folder trait - only overrides fold_pattern to transform patterns.
// All other expression types use the default recursion.

use super::{Pass, PassContext, PassError, PassResult};
use crate::ast::BinaryOp;
use crate::ir::{Folder, FuncRef, LocalTable, TExpr, TExprKind, TModule, TPattern, TStmt, Type};

/// Pattern lowering pass
/// Transforms: fold, map, filter, collect, count → loops
/// Keeps: recurse (handled specially in codegen), parallel (runtime feature)
pub struct PatternLoweringPass;

impl Pass for PatternLoweringPass {
    fn name(&self) -> &'static str {
        "pattern_lowering"
    }

    fn required(&self) -> bool {
        true // Pattern lowering is required for codegen
    }

    fn run(&self, ir: &mut TModule, _ctx: &mut PassContext) -> Result<PassResult, PassError> {
        let mut changed = false;
        let mut total_count = 0;

        // Lower configs
        for config in &mut ir.configs {
            let mut lowerer = PatternLowerer::new(LocalTable::new());
            config.value = lowerer.fold_expr(config.value.clone());
            if lowerer.count > 0 {
                changed = true;
                total_count += lowerer.count;
            }
        }

        // Lower functions
        for func in &mut ir.functions {
            let mut lowerer = PatternLowerer::new(func.locals.clone());
            func.body = lowerer.fold_expr(func.body.clone());
            if lowerer.count > 0 {
                func.locals = lowerer.locals;
                changed = true;
                total_count += lowerer.count;
            }
        }

        // Lower tests
        for test in &mut ir.tests {
            let mut lowerer = PatternLowerer::new(test.locals.clone());
            test.body = lowerer.fold_expr(test.body.clone());
            if lowerer.count > 0 {
                test.locals = lowerer.locals;
                changed = true;
                total_count += lowerer.count;
            }
        }

        if changed {
            Ok(PassResult::changed(total_count))
        } else {
            Ok(PassResult::unchanged())
        }
    }
}

struct PatternLowerer {
    locals: LocalTable,
    count: usize,
}

impl PatternLowerer {
    fn new(locals: LocalTable) -> Self {
        PatternLowerer { locals, count: 0 }
    }

    /// Lower a pattern to its loop-based equivalent
    fn lower_pattern(&mut self, pattern: TPattern, span: std::ops::Range<usize>) -> TExpr {
        match pattern {
            // fold(coll, init, op) →
            //   run(acc := init, for item in coll { acc := op(acc, item) }, acc)
            TPattern::Fold {
                collection,
                elem_ty,
                init,
                op,
                result_ty,
            } => self.lower_fold(collection, elem_ty, init, op, result_ty, span),

            // map(coll, transform) →
            //   run(result := [], for item in coll { result.push(transform(item)) }, result)
            TPattern::Map {
                collection,
                elem_ty,
                transform,
                result_elem_ty,
            } => self.lower_map(collection, elem_ty, transform, result_elem_ty, span),

            // filter(coll, pred) →
            //   run(result := [], for item in coll { if pred(item) then result.push(item) }, result)
            TPattern::Filter {
                collection,
                elem_ty,
                predicate,
            } => self.lower_filter(collection, elem_ty, predicate, span),

            // collect(range, transform) →
            //   run(result := [], for i in range { result.push(transform(i)) }, result)
            TPattern::Collect {
                range,
                transform,
                result_elem_ty,
            } => self.lower_collect(range, transform, result_elem_ty, span),

            // count(coll, pred) →
            //   run(cnt := 0, for item in coll { if pred(item) then cnt := cnt + 1 }, cnt)
            TPattern::Count {
                collection,
                elem_ty,
                predicate,
            } => self.lower_count(collection, elem_ty, predicate, span),

            // iterate is similar to fold but with direction
            TPattern::Iterate {
                over,
                elem_ty,
                direction: _,
                into,
                with,
                result_ty,
            } => {
                // For now, just treat it like fold (direction handled at codegen)
                self.lower_fold(over, elem_ty, into, with, result_ty, span)
            }

            // transform(input, step1, step2, ...) → step2(step1(input))
            TPattern::Transform {
                input,
                steps,
                result_ty,
            } => self.lower_transform(input, steps, result_ty, span),

            // recurse and parallel are kept as patterns for now
            // (recurse needs special handling, parallel is a runtime feature)
            TPattern::Recurse {
                cond,
                base,
                step,
                result_ty,
                memo,
                parallel_threshold,
            } => {
                // Keep as pattern - codegen handles it specially
                TExpr::new(
                    TExprKind::Pattern(Box::new(TPattern::Recurse {
                        cond,
                        base,
                        step,
                        result_ty: result_ty.clone(),
                        memo,
                        parallel_threshold,
                    })),
                    result_ty,
                    span,
                )
            }

            TPattern::Parallel {
                branches,
                timeout,
                on_error,
                result_ty,
            } => {
                // Keep as pattern - it's a runtime feature
                TExpr::new(
                    TExprKind::Pattern(Box::new(TPattern::Parallel {
                        branches,
                        timeout,
                        on_error,
                        result_ty: result_ty.clone(),
                    })),
                    result_ty,
                    span,
                )
            }

            // find(coll, pred) →
            //   run(result := None, for item in coll { if pred(item) then { result := Some(item); break } }, result)
            TPattern::Find {
                collection,
                elem_ty,
                predicate,
                default,
                result_ty,
            } => self.lower_find(collection, elem_ty, predicate, default, result_ty, span),

            // try(body) → keeps as pattern for now (needs runtime support)
            TPattern::Try {
                body,
                catch,
                result_ty,
            } => {
                // Keep as pattern - it's a runtime feature
                TExpr::new(
                    TExprKind::Pattern(Box::new(TPattern::Try {
                        body,
                        catch,
                        result_ty: result_ty.clone(),
                    })),
                    result_ty,
                    span,
                )
            }

            // retry(op, times) → keeps as pattern for now (needs runtime support)
            TPattern::Retry {
                operation,
                max_attempts,
                backoff,
                delay_ms,
                result_ty,
            } => {
                // Keep as pattern - it's a runtime feature
                TExpr::new(
                    TExprKind::Pattern(Box::new(TPattern::Retry {
                        operation,
                        max_attempts,
                        backoff,
                        delay_ms,
                        result_ty: result_ty.clone(),
                    })),
                    result_ty,
                    span,
                )
            }

            // validate(rules, then) → keeps as pattern for runtime accumulation
            TPattern::Validate {
                rules,
                then_value,
                result_ty,
            } => {
                // Keep as pattern - validation accumulation is a runtime feature
                TExpr::new(
                    TExprKind::Pattern(Box::new(TPattern::Validate {
                        rules,
                        then_value,
                        result_ty: result_ty.clone(),
                    })),
                    result_ty,
                    span,
                )
            }
        }
    }

    /// Lower fold(coll, init, op) to a loop
    fn lower_fold(
        &mut self,
        collection: TExpr,
        elem_ty: Type,
        init: TExpr,
        op: TExpr,
        result_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        // Create locals for accumulator and item
        let acc_id = self
            .locals
            .add("__acc".to_string(), result_ty.clone(), false, true); // mutable
        let item_id = self
            .locals
            .add("__item".to_string(), elem_ty.clone(), false, false); // immutable

        // Build: acc := op(acc, item)
        let acc_ref = TExpr::new(TExprKind::Local(acc_id), result_ty.clone(), span.clone());
        let item_ref = TExpr::new(TExprKind::Local(item_id), elem_ty.clone(), span.clone());

        // Apply the operator
        let apply_op = self.apply_op(
            op,
            acc_ref.clone(),
            item_ref,
            result_ty.clone(),
            span.clone(),
        );

        // Build the loop body: acc := apply_op
        let loop_body = TExpr::new(
            TExprKind::Assign {
                target: acc_id,
                value: Box::new(apply_op),
            },
            Type::Void,
            span.clone(),
        );

        // Build the for loop
        let for_loop = TExpr::new(
            TExprKind::For {
                binding: item_id,
                iter: Box::new(collection),
                body: Box::new(loop_body),
            },
            Type::Void,
            span.clone(),
        );

        // Build: run(acc := init, for ..., acc)
        TExpr::new(
            TExprKind::Block(
                vec![TStmt::Let {
                    local: acc_id,
                    value: init,
                }],
                Box::new(TExpr::new(
                    TExprKind::Block(vec![TStmt::Expr(for_loop)], Box::new(acc_ref)),
                    result_ty.clone(),
                    span.clone(),
                )),
            ),
            result_ty,
            span,
        )
    }

    /// Lower map(coll, transform) to a loop
    fn lower_map(
        &mut self,
        collection: TExpr,
        elem_ty: Type,
        transform: TExpr,
        result_elem_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        let result_ty = Type::List(Box::new(result_elem_ty.clone()));

        // Create locals
        let result_id = self
            .locals
            .add("__result".to_string(), result_ty.clone(), false, true); // mutable
        let item_id = self
            .locals
            .add("__item".to_string(), elem_ty.clone(), false, false); // immutable

        let result_ref = TExpr::new(TExprKind::Local(result_id), result_ty.clone(), span.clone());
        let item_ref = TExpr::new(TExprKind::Local(item_id), elem_ty.clone(), span.clone());

        // Apply transform
        let transformed =
            self.apply_transform(transform, item_ref, result_elem_ty.clone(), span.clone());

        // Build: result.push(transformed)
        let push_call = TExpr::new(
            TExprKind::MethodCall {
                receiver: Box::new(result_ref.clone()),
                method: "push".to_string(),
                args: vec![transformed],
            },
            Type::Void,
            span.clone(),
        );

        // Build the for loop
        let for_loop = TExpr::new(
            TExprKind::For {
                binding: item_id,
                iter: Box::new(collection),
                body: Box::new(push_call),
            },
            Type::Void,
            span.clone(),
        );

        // Build: run(result := [], for ..., result)
        TExpr::new(
            TExprKind::Block(
                vec![TStmt::Let {
                    local: result_id,
                    value: TExpr::new(TExprKind::List(vec![]), result_ty.clone(), span.clone()),
                }],
                Box::new(TExpr::new(
                    TExprKind::Block(vec![TStmt::Expr(for_loop)], Box::new(result_ref)),
                    result_ty.clone(),
                    span.clone(),
                )),
            ),
            result_ty,
            span,
        )
    }

    /// Lower filter(coll, predicate) to a loop
    fn lower_filter(
        &mut self,
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        let result_ty = Type::List(Box::new(elem_ty.clone()));

        // Create locals
        let result_id = self
            .locals
            .add("__result".to_string(), result_ty.clone(), false, true); // mutable
        let item_id = self
            .locals
            .add("__item".to_string(), elem_ty.clone(), false, false); // immutable

        let result_ref = TExpr::new(TExprKind::Local(result_id), result_ty.clone(), span.clone());
        let item_ref = TExpr::new(TExprKind::Local(item_id), elem_ty.clone(), span.clone());

        // Apply predicate
        let pred_result =
            self.apply_transform(predicate, item_ref.clone(), Type::Bool, span.clone());

        // Build: result.push(item)
        let push_call = TExpr::new(
            TExprKind::MethodCall {
                receiver: Box::new(result_ref.clone()),
                method: "push".to_string(),
                args: vec![item_ref],
            },
            Type::Void,
            span.clone(),
        );

        // Build: if pred(item) then result.push(item) else nil
        let if_push = TExpr::new(
            TExprKind::If {
                cond: Box::new(pred_result),
                then_branch: Box::new(push_call),
                else_branch: Box::new(TExpr::nil(span.clone())),
            },
            Type::Void,
            span.clone(),
        );

        // Build the for loop
        let for_loop = TExpr::new(
            TExprKind::For {
                binding: item_id,
                iter: Box::new(collection),
                body: Box::new(if_push),
            },
            Type::Void,
            span.clone(),
        );

        // Build: run(result := [], for ..., result)
        TExpr::new(
            TExprKind::Block(
                vec![TStmt::Let {
                    local: result_id,
                    value: TExpr::new(TExprKind::List(vec![]), result_ty.clone(), span.clone()),
                }],
                Box::new(TExpr::new(
                    TExprKind::Block(vec![TStmt::Expr(for_loop)], Box::new(result_ref)),
                    result_ty.clone(),
                    span.clone(),
                )),
            ),
            result_ty,
            span,
        )
    }

    /// Lower collect(range, transform) to a loop
    fn lower_collect(
        &mut self,
        range: TExpr,
        transform: TExpr,
        result_elem_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        // Collect is essentially map over a range
        self.lower_map(range, Type::Int, transform, result_elem_ty, span)
    }

    /// Lower count(coll, predicate) to a loop
    fn lower_count(
        &mut self,
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        // Create locals
        let count_id = self
            .locals
            .add("__count".to_string(), Type::Int, false, true); // mutable
        let item_id = self
            .locals
            .add("__item".to_string(), elem_ty.clone(), false, false); // immutable

        let count_ref = TExpr::new(TExprKind::Local(count_id), Type::Int, span.clone());
        let item_ref = TExpr::new(TExprKind::Local(item_id), elem_ty.clone(), span.clone());

        // Apply predicate
        let pred_result = self.apply_transform(predicate, item_ref, Type::Bool, span.clone());

        // Build: count := count + 1
        let inc_count = TExpr::new(
            TExprKind::Assign {
                target: count_id,
                value: Box::new(TExpr::new(
                    TExprKind::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(count_ref.clone()),
                        right: Box::new(TExpr::int(1, span.clone())),
                    },
                    Type::Int,
                    span.clone(),
                )),
            },
            Type::Void,
            span.clone(),
        );

        // Build: if pred(item) then count := count + 1 else nil
        let if_inc = TExpr::new(
            TExprKind::If {
                cond: Box::new(pred_result),
                then_branch: Box::new(inc_count),
                else_branch: Box::new(TExpr::nil(span.clone())),
            },
            Type::Void,
            span.clone(),
        );

        // Build the for loop
        let for_loop = TExpr::new(
            TExprKind::For {
                binding: item_id,
                iter: Box::new(collection),
                body: Box::new(if_inc),
            },
            Type::Void,
            span.clone(),
        );

        // Build: run(count := 0, for ..., count)
        TExpr::new(
            TExprKind::Block(
                vec![TStmt::Let {
                    local: count_id,
                    value: TExpr::int(0, span.clone()),
                }],
                Box::new(TExpr::new(
                    TExprKind::Block(vec![TStmt::Expr(for_loop)], Box::new(count_ref)),
                    Type::Int,
                    span.clone(),
                )),
            ),
            Type::Int,
            span,
        )
    }

    /// Lower find(coll, predicate) to a loop that returns first match
    fn lower_find(
        &mut self,
        collection: TExpr,
        elem_ty: Type,
        predicate: TExpr,
        default: Option<TExpr>,
        result_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        // Create locals
        let result_id = self
            .locals
            .add("__result".to_string(), result_ty.clone(), false, true); // mutable
        let item_id = self
            .locals
            .add("__item".to_string(), elem_ty.clone(), false, false); // immutable
        let found_id = self
            .locals
            .add("__found".to_string(), Type::Bool, false, true); // mutable

        let result_ref = TExpr::new(TExprKind::Local(result_id), result_ty.clone(), span.clone());
        let item_ref = TExpr::new(TExprKind::Local(item_id), elem_ty.clone(), span.clone());
        let found_ref = TExpr::new(TExprKind::Local(found_id), Type::Bool, span.clone());

        // Apply predicate
        let pred_result =
            self.apply_transform(predicate, item_ref.clone(), Type::Bool, span.clone());

        // Build: result := Some(item) (or just item if we have a default)
        let set_result = if default.is_some() {
            TExpr::new(
                TExprKind::Assign {
                    target: result_id,
                    value: Box::new(item_ref.clone()),
                },
                Type::Void,
                span.clone(),
            )
        } else {
            TExpr::new(
                TExprKind::Assign {
                    target: result_id,
                    value: Box::new(TExpr::new(
                        TExprKind::Some(Box::new(item_ref.clone())),
                        Type::Option(Box::new(elem_ty.clone())),
                        span.clone(),
                    )),
                },
                Type::Void,
                span.clone(),
            )
        };

        // Build: found := true
        let set_found = TExpr::new(
            TExprKind::Assign {
                target: found_id,
                value: Box::new(TExpr::new(TExprKind::Bool(true), Type::Bool, span.clone())),
            },
            Type::Void,
            span.clone(),
        );

        // Build: if pred(item) && !found then { result := Some(item); found := true }
        let if_found = TExpr::new(
            TExprKind::If {
                cond: Box::new(TExpr::new(
                    TExprKind::Binary {
                        op: BinaryOp::And,
                        left: Box::new(pred_result),
                        right: Box::new(TExpr::new(
                            TExprKind::Unary {
                                op: crate::ast::UnaryOp::Not,
                                operand: Box::new(found_ref.clone()),
                            },
                            Type::Bool,
                            span.clone(),
                        )),
                    },
                    Type::Bool,
                    span.clone(),
                )),
                then_branch: Box::new(TExpr::new(
                    TExprKind::Block(
                        vec![TStmt::Expr(set_result), TStmt::Expr(set_found)],
                        Box::new(TExpr::nil(span.clone())),
                    ),
                    Type::Void,
                    span.clone(),
                )),
                else_branch: Box::new(TExpr::nil(span.clone())),
            },
            Type::Void,
            span.clone(),
        );

        // Build the for loop
        let for_loop = TExpr::new(
            TExprKind::For {
                binding: item_id,
                iter: Box::new(collection),
                body: Box::new(if_found),
            },
            Type::Void,
            span.clone(),
        );

        // Initial value: default or None
        let init_value = if let Some(def) = default {
            def
        } else {
            TExpr::new(
                TExprKind::None_,
                Type::Option(Box::new(elem_ty)),
                span.clone(),
            )
        };

        // Build: run(result := None, found := false, for ..., result)
        TExpr::new(
            TExprKind::Block(
                vec![
                    TStmt::Let {
                        local: result_id,
                        value: init_value,
                    },
                    TStmt::Let {
                        local: found_id,
                        value: TExpr::new(TExprKind::Bool(false), Type::Bool, span.clone()),
                    },
                ],
                Box::new(TExpr::new(
                    TExprKind::Block(vec![TStmt::Expr(for_loop)], Box::new(result_ref)),
                    result_ty.clone(),
                    span.clone(),
                )),
            ),
            result_ty,
            span,
        )
    }

    /// Lower transform(input, steps) to nested applications
    fn lower_transform(
        &mut self,
        input: TExpr,
        steps: Vec<TExpr>,
        result_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        // Build: step_n(...step_2(step_1(input))...)
        let mut current = input;
        for step in steps {
            current = self.apply_transform(step, current, result_ty.clone(), span.clone());
        }
        current
    }

    /// Apply an operator to two arguments (for fold)
    fn apply_op(
        &self,
        op: TExpr,
        left: TExpr,
        right: TExpr,
        result_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        match &op.kind {
            // If it's a lambda, call it
            TExprKind::Lambda { .. } => TExpr::new(
                TExprKind::Call {
                    func: FuncRef::User("__lambda".to_string()),
                    args: vec![left, right],
                },
                result_ty,
                span,
            ),

            // If it's a user function reference
            TExprKind::Call { func, .. } => TExpr::new(
                TExprKind::Call {
                    func: func.clone(),
                    args: vec![left, right],
                },
                result_ty,
                span,
            ),

            // If it's a builtin operator like +, -, *, /
            TExprKind::Local(_) | TExprKind::Param(_) => {
                // Function is stored as a variable reference - need to call it
                TExpr::new(
                    TExprKind::Call {
                        func: FuncRef::Builtin("apply".to_string()),
                        args: vec![op.clone(), left, right],
                    },
                    result_ty,
                    span,
                )
            }

            // Check if it's a known operator symbol
            _ => {
                // Default: assume it's callable
                TExpr::new(
                    TExprKind::Call {
                        func: FuncRef::Builtin("apply".to_string()),
                        args: vec![op, left, right],
                    },
                    result_ty,
                    span,
                )
            }
        }
    }

    /// Apply a transform function to an argument
    fn apply_transform(
        &self,
        transform: TExpr,
        arg: TExpr,
        result_ty: Type,
        span: std::ops::Range<usize>,
    ) -> TExpr {
        match &transform.kind {
            // Lambda: just call it
            TExprKind::Lambda { .. } => TExpr::new(
                TExprKind::Call {
                    func: FuncRef::User("__lambda".to_string()),
                    args: vec![arg],
                },
                result_ty,
                span,
            ),

            // Function reference
            TExprKind::Call { func, .. } => TExpr::new(
                TExprKind::Call {
                    func: func.clone(),
                    args: vec![arg],
                },
                result_ty,
                span,
            ),

            // Default: assume callable
            _ => TExpr::new(
                TExprKind::Call {
                    func: FuncRef::Builtin("apply".to_string()),
                    args: vec![transform, arg],
                },
                result_ty,
                span,
            ),
        }
    }
}

impl Folder for PatternLowerer {
    // Only override fold_pattern - all other methods use defaults for recursion
    fn fold_pattern(&mut self, pattern: TPattern, _ty: Type, span: crate::ast::Span) -> TExpr {
        // First, recursively fold subexpressions within the pattern
        let pattern = self.fold_tpattern(pattern);

        // Then lower the pattern to loops
        self.count += 1;
        self.lower_pattern(pattern, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_lowering_pass_name() {
        let pass = PatternLoweringPass;
        assert_eq!(pass.name(), "pattern_lowering");
    }

    #[test]
    fn test_pattern_lowering_pass_required() {
        let pass = PatternLoweringPass;
        assert!(pass.required());
    }
}
