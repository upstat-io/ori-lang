// Pattern lowering for AST to TIR
// Handles match patterns and pattern expressions (fold, map, filter, etc.)

use crate::ast::{self, Pattern as AstPattern, PatternExpr};
use crate::ir::{IterDirection, OnError, TMatchPattern, TPattern, Type};
use super::Lowerer;

impl Lowerer {
    /// Lower a match pattern
    pub(super) fn lower_match_pattern(
        &mut self,
        pattern: &AstPattern,
        scrutinee_ty: &Type,
    ) -> Result<TMatchPattern, String> {
        match pattern {
            AstPattern::Wildcard => Ok(TMatchPattern::Wildcard),

            AstPattern::Literal(expr) => {
                let texpr = self.lower_expr(expr)?;
                Ok(TMatchPattern::Literal(texpr))
            }

            AstPattern::Binding(name) => {
                // Match bindings are immutable
                let local_id = self.locals.add(name.clone(), scrutinee_ty.clone(), false, false);
                self.local_scope.insert(name.clone(), local_id);
                Ok(TMatchPattern::Binding(local_id, scrutinee_ty.clone()))
            }

            AstPattern::Variant { name, fields } => {
                let bindings = fields
                    .iter()
                    .map(|(field_name, sub_pattern)| {
                        // For variant fields, we need to determine the field type
                        // For now, use Any
                        let field_ty = Type::Any;
                        if let AstPattern::Binding(binding_name) = sub_pattern {
                            // Match bindings are immutable
                            let local_id = self.locals.add(binding_name.clone(), field_ty.clone(), false, false);
                            self.local_scope.insert(binding_name.clone(), local_id);
                            Ok((field_name.clone(), local_id, field_ty))
                        } else {
                            Err("Nested patterns not yet supported".to_string())
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(TMatchPattern::Variant {
                    name: name.clone(),
                    bindings,
                })
            }

            AstPattern::Condition(expr) => {
                let texpr = self.lower_expr(expr)?;
                Ok(TMatchPattern::Condition(texpr))
            }
        }
    }

    /// Lower a pattern expression (fold, map, filter, etc.)
    pub(super) fn lower_pattern(&mut self, p: &PatternExpr) -> Result<TPattern, String> {
        match p {
            PatternExpr::Fold {
                collection,
                init,
                op,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let init_expr = self.lower_expr(init)?;
                let result_ty = init_expr.ty.clone();
                let op_expr = self.lower_expr(op)?;

                Ok(TPattern::Fold {
                    collection: coll,
                    elem_ty,
                    init: init_expr,
                    op: op_expr,
                    result_ty,
                })
            }

            PatternExpr::Map {
                collection,
                transform,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    Type::Range => Type::Int,
                    _ => Type::Any,
                };
                let transform_expr = self.lower_expr(transform)?;

                // Infer result element type from transform
                let result_elem_ty = match &transform_expr.ty {
                    Type::Function { ret, .. } => *ret.clone(),
                    _ => Type::Any,
                };

                Ok(TPattern::Map {
                    collection: coll,
                    elem_ty,
                    transform: transform_expr,
                    result_elem_ty,
                })
            }

            PatternExpr::Filter {
                collection,
                predicate,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let pred_expr = self.lower_expr(predicate)?;

                Ok(TPattern::Filter {
                    collection: coll,
                    elem_ty,
                    predicate: pred_expr,
                })
            }

            PatternExpr::Collect { range, transform } => {
                let range_expr = self.lower_expr(range)?;
                let transform_expr = self.lower_expr(transform)?;

                let result_elem_ty = match &transform_expr.ty {
                    Type::Function { ret, .. } => *ret.clone(),
                    _ => Type::Any,
                };

                Ok(TPattern::Collect {
                    range: range_expr,
                    transform: transform_expr,
                    result_elem_ty,
                })
            }

            PatternExpr::Recurse {
                condition,
                base_value,
                step,
                memo,
                parallel_threshold,
            } => {
                let cond = self.lower_expr(condition)?;
                let base = self.lower_expr(base_value)?;
                let result_ty = base.ty.clone();
                let step_expr = self.lower_expr(step)?;

                Ok(TPattern::Recurse {
                    cond,
                    base,
                    step: step_expr,
                    result_ty,
                    memo: *memo,
                    parallel_threshold: *parallel_threshold,
                })
            }

            PatternExpr::Iterate {
                over,
                direction,
                into,
                with,
            } => {
                let over_expr = self.lower_expr(over)?;
                let elem_ty = match &over_expr.ty {
                    Type::List(inner) => *inner.clone(),
                    Type::Range => Type::Int,
                    _ => Type::Any,
                };
                let into_expr = self.lower_expr(into)?;
                let result_ty = into_expr.ty.clone();
                let with_expr = self.lower_expr(with)?;

                let dir = match direction {
                    ast::IterDirection::Forward => IterDirection::Forward,
                    ast::IterDirection::Backward => IterDirection::Backward,
                };

                Ok(TPattern::Iterate {
                    over: over_expr,
                    elem_ty,
                    direction: dir,
                    into: into_expr,
                    with: with_expr,
                    result_ty,
                })
            }

            PatternExpr::Transform { input, steps } => {
                let input_expr = self.lower_expr(input)?;
                let steps_expr = steps
                    .iter()
                    .map(|s| self.lower_expr(s))
                    .collect::<Result<Vec<_>, _>>()?;

                // Compute result type by chaining through steps
                let mut current_ty = input_expr.ty.clone();
                for step in &steps_expr {
                    if let Type::Function { ret, .. } = &step.ty {
                        current_ty = *ret.clone();
                    }
                }

                Ok(TPattern::Transform {
                    input: input_expr,
                    steps: steps_expr,
                    result_ty: current_ty,
                })
            }

            PatternExpr::Count {
                collection,
                predicate,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let pred_expr = self.lower_expr(predicate)?;

                Ok(TPattern::Count {
                    collection: coll,
                    elem_ty,
                    predicate: pred_expr,
                })
            }

            PatternExpr::Parallel {
                branches,
                timeout,
                on_error,
            } => {
                let mut tbranches = Vec::new();
                let mut field_types = Vec::new();

                for (name, expr) in branches {
                    let texpr = self.lower_expr(expr)?;
                    let ty = texpr.ty.clone();
                    field_types.push((name.clone(), ty.clone()));
                    tbranches.push((name.clone(), texpr, ty));
                }

                let timeout_expr = timeout
                    .as_ref()
                    .map(|t| self.lower_expr(t))
                    .transpose()?;

                let err = match on_error {
                    ast::OnError::FailFast => OnError::FailFast,
                    ast::OnError::CollectAll => OnError::CollectAll,
                };

                let result_ty = Type::Record(field_types);

                Ok(TPattern::Parallel {
                    branches: tbranches,
                    timeout: timeout_expr,
                    on_error: err,
                    result_ty,
                })
            }

            PatternExpr::Find {
                collection,
                predicate,
                default,
            } => {
                let coll = self.lower_expr(collection)?;
                let elem_ty = match &coll.ty {
                    Type::List(inner) => *inner.clone(),
                    _ => Type::Any,
                };
                let pred_expr = self.lower_expr(predicate)?;
                let default_expr = default
                    .as_ref()
                    .map(|d| self.lower_expr(d))
                    .transpose()?;

                // Result type is Option<elem_ty> without default, elem_ty with default
                let result_ty = if default_expr.is_some() {
                    elem_ty.clone()
                } else {
                    Type::Option(Box::new(elem_ty.clone()))
                };

                Ok(TPattern::Find {
                    collection: coll,
                    elem_ty,
                    predicate: pred_expr,
                    default: default_expr,
                    result_ty,
                })
            }

            PatternExpr::Try { body, catch } => {
                let body_expr = self.lower_expr(body)?;
                let body_ty = body_expr.ty.clone();
                let catch_expr = catch
                    .as_ref()
                    .map(|c| self.lower_expr(c))
                    .transpose()?;

                // Result type is T with catch, Result<T, Error> without
                let result_ty = if catch_expr.is_some() {
                    body_ty
                } else {
                    Type::Result(Box::new(body_ty), Box::new(Type::Named("Error".to_string())))
                };

                Ok(TPattern::Try {
                    body: body_expr,
                    catch: catch_expr,
                    result_ty,
                })
            }

            PatternExpr::Retry {
                operation,
                max_attempts,
                backoff,
                delay_ms,
            } => {
                let op_expr = self.lower_expr(operation)?;
                let op_ty = op_expr.ty.clone();
                let attempts_expr = self.lower_expr(max_attempts)?;
                let delay_expr = delay_ms
                    .as_ref()
                    .map(|d| self.lower_expr(d))
                    .transpose()?;

                let tir_backoff = match backoff {
                    ast::RetryBackoff::None => crate::ir::RetryBackoff::None,
                    ast::RetryBackoff::Constant => crate::ir::RetryBackoff::Constant,
                    ast::RetryBackoff::Linear => crate::ir::RetryBackoff::Linear,
                    ast::RetryBackoff::Exponential => crate::ir::RetryBackoff::Exponential,
                };

                let result_ty = Type::Result(
                    Box::new(op_ty),
                    Box::new(Type::Named("Error".to_string())),
                );

                Ok(TPattern::Retry {
                    operation: op_expr,
                    max_attempts: attempts_expr,
                    backoff: tir_backoff,
                    delay_ms: delay_expr,
                    result_ty,
                })
            }

            PatternExpr::Validate { rules, then_value } => {
                // Lower each rule (condition, message) pair
                let trules = rules
                    .iter()
                    .map(|(cond, msg)| {
                        let tcond = self.lower_expr(cond)?;
                        let tmsg = self.lower_expr(msg)?;
                        Ok((tcond, tmsg))
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                let then_expr = self.lower_expr(then_value)?;
                let then_ty = then_expr.ty.clone();

                // Result type is Result<T, [str]>
                let result_ty = Type::Result(
                    Box::new(then_ty),
                    Box::new(Type::List(Box::new(Type::Str))),
                );

                Ok(TPattern::Validate {
                    rules: trules,
                    then_value: then_expr,
                    result_ty,
                })
            }
        }
    }
}
