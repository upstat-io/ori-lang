//! `FunctionSeq` evaluation methods for the Evaluator.

use crate::ir::{FunctionSeq, SeqBinding};
use super::{Evaluator, EvalResult, EvalError};
use super::super::value::Value;

impl Evaluator<'_> {
    /// Evaluate a `function_seq` expression (run, try, match).
    pub(super) fn eval_function_seq(&mut self, func_seq: &FunctionSeq) -> EvalResult {
        match func_seq {
            FunctionSeq::Run { bindings, result, .. } => {
                // Evaluate bindings and statements in sequence
                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, mutable, .. } => {
                            let val = self.eval(*value)?;
                            self.bind_pattern(pattern, val, *mutable)?;
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Evaluate for side effects (e.g., assignment)
                            self.eval(*expr)?;
                        }
                    }
                }
                // Evaluate and return result
                self.eval(*result)
            }

            FunctionSeq::Try { bindings, result, .. } => {
                // Evaluate bindings, unwrapping Result/Option and short-circuiting on error
                let seq_bindings = self.arena.get_seq_bindings(*bindings);
                for binding in seq_bindings {
                    match binding {
                        SeqBinding::Let { pattern, value, mutable, .. } => {
                            match self.eval(*value) {
                                Ok(value) => {
                                    // Unwrap Result/Option types per spec:
                                    // "If any binding expression returns a Result<T, E>, the binding variable has type T"
                                    let unwrapped = match value {
                                        Value::Err(e) => {
                                            // Early return with the error
                                            return Ok(Value::Err(e));
                                        }
                                        Value::None => {
                                            // Early return with None
                                            return Ok(Value::None);
                                        }
                                        Value::Ok(inner) | Value::Some(inner) => (*inner).clone(),
                                        other => other,
                                    };
                                    self.bind_pattern(pattern, unwrapped, *mutable)?;
                                }
                                Err(e) => {
                                    // If this is a propagated error, return the value
                                    if let Some(propagated) = e.propagated_value {
                                        return Ok(propagated);
                                    }
                                    return Err(e);
                                }
                            }
                        }
                        SeqBinding::Stmt { expr, .. } => {
                            // Evaluate for side effects
                            self.eval(*expr)?;
                        }
                    }
                }
                // Evaluate and return result
                self.eval(*result)
            }

            FunctionSeq::Match { scrutinee, arms, .. } => {
                let value = self.eval(*scrutinee)?;
                self.eval_match(&value, *arms)
            }

            FunctionSeq::ForPattern { over, map, arm, default, .. } => {
                // Evaluate the collection to iterate over
                let items = self.eval(*over)?;

                let Value::List(items_list) = items else {
                    return Err(EvalError::new(format!(
                        "for pattern requires a list, got {}",
                        items.type_name()
                    )));
                };

                // Iterate and find first match
                for item in items_list.iter() {
                    // Optionally apply map function
                    let match_item = if let Some(map_expr) = map {
                        let map_fn = self.eval(*map_expr)?;
                        self.eval_call_value(map_fn, std::slice::from_ref(item))?
                    } else {
                        item.clone()
                    };

                    // Try to match against the arm pattern
                    if let Some(bindings) = super::super::exec::control::try_match(
                        &arm.pattern,
                        &match_item,
                        self.arena,
                        self.interner,
                    )? {
                        // Pattern matched - bind variables and evaluate body
                        self.env.push_scope();
                        for (name, value) in bindings {
                            self.env.define(name, value, false);
                        }
                        let result = self.eval(arm.body);
                        self.env.pop_scope();
                        return result;
                    }
                }

                // No match found - return default
                self.eval(*default)
            }
        }
    }
}
