// Expression translation for TIR-based C code generation
// Converts TExpr to C code, using embedded types directly

use super::TirCodeGen;
use crate::ast::BinaryOp;
use crate::ir::{FuncRef, TExpr, TExprKind, TMatch, TMatchPattern, TPattern};

impl TirCodeGen {
    /// Convert a typed expression to C code
    pub(super) fn expr_to_c(&self, expr: &TExpr) -> Result<String, String> {
        match &expr.kind {
            // Literals
            TExprKind::Int(n) => Ok(format!("{}", n)),
            TExprKind::Float(f) => Ok(format!("{}", f)),
            TExprKind::String(s) => Ok(format!("str_new(\"{}\")", s)),
            TExprKind::Bool(b) => Ok(if *b { "true" } else { "false" }.to_string()),
            TExprKind::Nil => Ok("NULL".to_string()),

            // Variables
            TExprKind::Local(id) => Ok(format!("__local_{}", id.0)),
            TExprKind::Param(idx) => {
                // Parameters are referenced by their index
                // In generated code, they're named param_N
                Ok(format!("__param_{}", idx))
            }
            TExprKind::Config(name) => Ok(name.clone()),

            // Collections
            TExprKind::List(elems) => {
                let elems_c: Result<Vec<String>, String> =
                    elems.iter().map(|e| self.expr_to_c(e)).collect();
                // For now, return a simple representation
                Ok(format!("{{ {} }}", elems_c?.join(", ")))
            }

            TExprKind::MapLiteral(entries) => {
                let entries_c: Result<Vec<String>, String> = entries
                    .iter()
                    .map(|(k, v)| Ok(format!("{{ {}, {} }}", self.expr_to_c(k)?, self.expr_to_c(v)?)))
                    .collect();
                Ok(format!("{{ {} }}", entries_c?.join(", ")))
            }

            TExprKind::Tuple(elems) => {
                let elems_c: Result<Vec<String>, String> =
                    elems.iter().map(|e| self.expr_to_c(e)).collect();
                Ok(format!("{{ {} }}", elems_c?.join(", ")))
            }

            TExprKind::Struct { name, fields } => {
                let fields_c: Result<Vec<String>, String> = fields
                    .iter()
                    .map(|(n, e)| Ok(format!(".{} = {}", n, self.expr_to_c(e)?)))
                    .collect();
                Ok(format!("({}){{ {} }}", name, fields_c?.join(", ")))
            }

            // Operations
            TExprKind::Binary { op, left, right } => {
                let l = self.expr_to_c(left)?;
                let r = self.expr_to_c(right)?;

                // Check if string concatenation using the type from TExpr
                if matches!(op, BinaryOp::Add) && self.is_string_type(&left.ty) {
                    return Ok(format!("str_concat({}, {})", l, r));
                }

                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::IntDiv => "/",
                    BinaryOp::Mod => "%",
                    BinaryOp::Eq => "==",
                    BinaryOp::NotEq => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::LtEq => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::GtEq => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    BinaryOp::Pipe => {
                        return Err("Pipe operator not yet supported in codegen".to_string())
                    }
                };
                Ok(format!("({} {} {})", l, op_str, r))
            }

            TExprKind::Unary { op, operand } => {
                let o = self.expr_to_c(operand)?;
                let op_str = match op {
                    crate::ast::UnaryOp::Neg => "-",
                    crate::ast::UnaryOp::Not => "!",
                };
                Ok(format!("({}{})", op_str, o))
            }

            // Access
            TExprKind::Field(obj, field) => {
                let obj_c = self.expr_to_c(obj)?;
                Ok(format!("{}.{}", obj_c, field))
            }

            TExprKind::Index(obj, idx) => {
                let obj_c = self.expr_to_c(obj)?;
                let idx_c = self.expr_to_c(idx)?;
                Ok(format!("{}[{}]", obj_c, idx_c))
            }

            TExprKind::LengthOf(obj) => {
                let obj_c = self.expr_to_c(obj)?;
                Ok(format!("len({})", obj_c))
            }

            // Calls
            TExprKind::Call { func, args } => {
                let func_name = match func {
                    FuncRef::User(name) => name.clone(),
                    FuncRef::Builtin(name) => {
                        // Map builtin names to C equivalents
                        match name.as_str() {
                            "str" => "int_to_str".to_string(),
                            "print" => "printf".to_string(),
                            "len" => "len".to_string(),
                            _ => name.clone(),
                        }
                    }
                    FuncRef::Operator(op) => {
                        // For operator-as-function, return the operator
                        match op {
                            BinaryOp::Add => "+".to_string(),
                            BinaryOp::Sub => "-".to_string(),
                            BinaryOp::Mul => "*".to_string(),
                            BinaryOp::Div => "/".to_string(),
                            _ => format!("{:?}", op),
                        }
                    }
                };

                let args_c: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_to_c(a)).collect();
                Ok(format!("{}({})", func_name, args_c?.join(", ")))
            }

            TExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv_c = self.expr_to_c(receiver)?;
                let args_c: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_to_c(a)).collect();

                // Handle common methods
                match method.as_str() {
                    "push" => Ok(format!("array_push(&{}, {})", recv_c, args_c?.join(", "))),
                    "pop" => Ok(format!("array_pop(&{})", recv_c)),
                    "len" => Ok(format!("len({})", recv_c)),
                    _ => Ok(format!("{}.{}({})", recv_c, method, args_c?.join(", "))),
                }
            }

            // Lambdas
            TExprKind::Lambda { params, body, .. } => {
                // For now, just generate inline code
                // A full implementation would create function pointers
                let body_c = self.expr_to_c(body)?;
                let params_str: Vec<_> = params.iter().map(|(n, _)| n.clone()).collect();
                Ok(format!("/* lambda({}) */ {}", params_str.join(", "), body_c))
            }

            // Control flow
            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_c = self.expr_to_c(cond)?;
                let then_c = self.expr_to_c(then_branch)?;
                let else_c = self.expr_to_c(else_branch)?;
                Ok(format!("({} ? {} : {})", cond_c, then_c, else_c))
            }

            TExprKind::Match(m) => self.match_to_c(m),

            TExprKind::Block(stmts, result) => {
                // For blocks used as expressions, use GCC statement expressions
                // or just return the final value
                if stmts.is_empty() {
                    self.expr_to_c(result)
                } else {
                    // Complex blocks need statement expressions
                    let result_c = self.expr_to_c(result)?;
                    Ok(format!("/* block */ {}", result_c))
                }
            }

            TExprKind::For { .. } => {
                // For loops as expressions don't make sense in C
                // They should be lowered to statements before codegen
                Ok("/* for loop */".to_string())
            }

            TExprKind::Assign { target, value } => {
                let val = self.expr_to_c(value)?;
                Ok(format!("(__local_{} = {})", target.0, val))
            }

            TExprKind::Range { start, end } => {
                let start_c = self.expr_to_c(start)?;
                let end_c = self.expr_to_c(end)?;
                Ok(format!("range({}, {})", start_c, end_c))
            }

            // Patterns (should be lowered before codegen)
            TExprKind::Pattern(pattern) => self.pattern_to_c(pattern),

            // Result/Option
            TExprKind::Ok(inner) => {
                let inner_c = self.expr_to_c(inner)?;
                Ok(format!("Result_Ok({})", inner_c))
            }
            TExprKind::Err(inner) => {
                let inner_c = self.expr_to_c(inner)?;
                Ok(format!("Result_Err({})", inner_c))
            }
            TExprKind::Some(inner) => {
                let inner_c = self.expr_to_c(inner)?;
                Ok(format!("Option_Some({})", inner_c))
            }
            TExprKind::None_ => Ok("Option_None()".to_string()),

            TExprKind::Coalesce { value, default } => {
                let val_c = self.expr_to_c(value)?;
                let def_c = self.expr_to_c(default)?;
                Ok(format!("coalesce({}, {})", val_c, def_c))
            }

            TExprKind::Unwrap(inner) => {
                let inner_c = self.expr_to_c(inner)?;
                Ok(format!("unwrap({})", inner_c))
            }
        }
    }

    /// Convert a match expression to C
    fn match_to_c(&self, m: &TMatch) -> Result<String, String> {
        let scrutinee = self.expr_to_c(&m.scrutinee)?;

        let mut result = String::new();
        for (i, arm) in m.arms.iter().enumerate() {
            match &arm.pattern {
                TMatchPattern::Wildcard => {
                    let body = self.expr_to_c(&arm.body)?;
                    result.push_str(&body);
                }
                TMatchPattern::Condition(cond) => {
                    let cond_c = self.expr_to_c(cond)?;
                    let body = self.expr_to_c(&arm.body)?;
                    if i < m.arms.len() - 1 {
                        result.push_str(&format!("({} ? {} : ", cond_c, body));
                    } else {
                        result.push_str(&body);
                    }
                }
                TMatchPattern::Literal(lit) => {
                    let lit_c = self.expr_to_c(lit)?;
                    let body = self.expr_to_c(&arm.body)?;
                    if i < m.arms.len() - 1 {
                        result.push_str(&format!("({} == {} ? {} : ", scrutinee, lit_c, body));
                    } else {
                        result.push_str(&body);
                    }
                }
                TMatchPattern::Binding(_, _) => {
                    let body = self.expr_to_c(&arm.body)?;
                    result.push_str(&body);
                }
                TMatchPattern::Variant { name, .. } => {
                    let body = self.expr_to_c(&arm.body)?;
                    if i < m.arms.len() - 1 {
                        result.push_str(&format!(
                            "({}.tag == TAG_{} ? {} : ",
                            scrutinee, name, body
                        ));
                    } else {
                        result.push_str(&body);
                    }
                }
            }
        }

        // Close parentheses for nested ternaries
        for arm in &m.arms[..m.arms.len().saturating_sub(1)] {
            if !matches!(arm.pattern, TMatchPattern::Wildcard | TMatchPattern::Binding(_, _)) {
                result.push(')');
            }
        }

        Ok(result)
    }

    /// Convert a pattern to C (patterns should typically be lowered first)
    fn pattern_to_c(&self, pattern: &TPattern) -> Result<String, String> {
        match pattern {
            // Recurse is kept in TIR for special handling
            TPattern::Recurse {
                cond,
                base,
                step,
                memo,
                ..
            } => {
                let cond_c = self.expr_to_c(cond)?;
                let base_c = self.expr_to_c(base)?;
                let step_c = self.expr_to_c(step)?;

                // For simple non-memoized recursion, use a ternary
                if !*memo {
                    Ok(format!("({} ? {} : {})", cond_c, base_c, step_c))
                } else {
                    // Memoized recursion needs special handling
                    Ok(format!(
                        "/* memoized recurse */ ({} ? {} : {})",
                        cond_c, base_c, step_c
                    ))
                }
            }

            // Other patterns should be lowered before reaching codegen
            TPattern::Fold { .. } => {
                Err("Fold pattern should be lowered before codegen".to_string())
            }
            TPattern::Map { .. } => {
                Err("Map pattern should be lowered before codegen".to_string())
            }
            TPattern::Filter { .. } => {
                Err("Filter pattern should be lowered before codegen".to_string())
            }
            TPattern::Collect { .. } => {
                Err("Collect pattern should be lowered before codegen".to_string())
            }
            TPattern::Count { .. } => {
                Err("Count pattern should be lowered before codegen".to_string())
            }
            TPattern::Iterate { .. } => {
                Err("Iterate pattern should be lowered before codegen".to_string())
            }
            TPattern::Transform { .. } => {
                Err("Transform pattern should be lowered before codegen".to_string())
            }
            TPattern::Parallel { .. } => {
                // Parallel execution would need runtime support
                Err("Parallel pattern not yet supported in codegen".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Type;

    #[test]
    fn test_literal_to_c() {
        let cg = TirCodeGen::new();

        let int_expr = TExpr::new(TExprKind::Int(42), Type::Int, 0..1);
        assert_eq!(cg.expr_to_c(&int_expr).unwrap(), "42");

        let bool_expr = TExpr::new(TExprKind::Bool(true), Type::Bool, 0..1);
        assert_eq!(cg.expr_to_c(&bool_expr).unwrap(), "true");

        let str_expr = TExpr::new(TExprKind::String("hello".to_string()), Type::Str, 0..1);
        assert_eq!(cg.expr_to_c(&str_expr).unwrap(), "str_new(\"hello\")");
    }

    #[test]
    fn test_binary_to_c() {
        let cg = TirCodeGen::new();

        let add_expr = TExpr::new(
            TExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(TExpr::new(TExprKind::Int(1), Type::Int, 0..1)),
                right: Box::new(TExpr::new(TExprKind::Int(2), Type::Int, 0..1)),
            },
            Type::Int,
            0..1,
        );
        assert_eq!(cg.expr_to_c(&add_expr).unwrap(), "(1 + 2)");
    }
}
