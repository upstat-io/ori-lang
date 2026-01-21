// Expression translation for C code generation
// Converts Sigil expressions to C code

use super::CodeGen;
use crate::ast::*;

impl CodeGen {
    pub(super) fn expr_to_c(&self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Int(n) => Ok(format!("{}", n)),
            Expr::Float(f) => Ok(format!("{}", f)),
            Expr::String(s) => Ok(format!("str_new(\"{}\")", s)),
            Expr::Bool(b) => Ok(if *b { "true" } else { "false" }.to_string()),
            Expr::Nil => Ok("NULL".to_string()),

            Expr::Ident(name) => Ok(name.clone()),

            Expr::Config(name) => Ok(name.clone()),

            Expr::Binary { op, left, right } => {
                let l = self.expr_to_c(left)?;
                let r = self.expr_to_c(right)?;

                // Check if string concatenation
                if matches!(op, BinaryOp::Add)
                    && (self.is_string_expr(left) || self.is_string_expr(right))
                {
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

            Expr::Unary { op, operand } => {
                let o = self.expr_to_c(operand)?;
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                Ok(format!("({}{})", op_str, o))
            }

            Expr::Call { func, args } => {
                let func_name = match func.as_ref() {
                    Expr::Ident(name) => name.clone(),
                    _ => return Err("Complex function calls not yet supported".to_string()),
                };

                // Handle built-in functions
                match func_name.as_str() {
                    "str" => {
                        if let Some(arg) = args.first() {
                            let arg_c = self.expr_to_c(arg)?;
                            return Ok(format!("int_to_str({})", arg_c));
                        }
                    }
                    "print" => {
                        // Handled in emit_statement
                    }
                    _ => {}
                }

                let args_c: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_to_c(a)).collect();
                Ok(format!("{}({})", func_name, args_c?.join(", ")))
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.expr_to_c(condition)?;
                let then_c = self.expr_to_c(then_branch)?;
                if let Some(else_b) = else_branch {
                    let else_c = self.expr_to_c(else_b)?;
                    Ok(format!("({} ? {} : {})", cond, then_c, else_c))
                } else {
                    Ok(format!("({} ? {} : 0)", cond, then_c))
                }
            }

            Expr::Match(m) => self.match_to_c(m),

            Expr::Block(exprs) => {
                if let Some(last) = exprs.last() {
                    self.expr_to_c(last)
                } else {
                    Ok("0".to_string())
                }
            }

            _ => Err(format!(
                "Expression not yet supported in codegen: {:?}",
                expr
            )),
        }
    }

    pub(super) fn match_to_c(&self, m: &MatchExpr) -> Result<String, String> {
        // For simple conditional matches, generate nested ternaries
        let scrutinee = self.expr_to_c(&m.scrutinee)?;

        let mut result = String::new();
        for (i, arm) in m.arms.iter().enumerate() {
            match &arm.pattern {
                Pattern::Wildcard => {
                    // Default case
                    let body = self.expr_to_c(&arm.body)?;
                    result.push_str(&body);
                }
                Pattern::Condition(cond) => {
                    let cond_c = self.expr_to_c(cond)?;
                    let body = self.expr_to_c(&arm.body)?;
                    if i < m.arms.len() - 1 {
                        result.push_str(&format!("({} ? {} : ", cond_c, body));
                    } else {
                        result.push_str(&body);
                    }
                }
                Pattern::Literal(lit) => {
                    let lit_c = self.expr_to_c(lit)?;
                    let body = self.expr_to_c(&arm.body)?;
                    if i < m.arms.len() - 1 {
                        result.push_str(&format!("({} == {} ? {} : ", scrutinee, lit_c, body));
                    } else {
                        result.push_str(&body);
                    }
                }
                _ => return Err("Complex patterns not yet supported in codegen".to_string()),
            }
        }

        // Close parentheses for nested ternaries
        for arm in &m.arms[..m.arms.len().saturating_sub(1)] {
            if !matches!(arm.pattern, Pattern::Wildcard) {
                result.push(')');
            }
        }

        Ok(result)
    }
}
