// Declaration emission for TIR-based C code generation
// Handles forward declarations, config variables, and function definitions

use super::TirCodeGen;
use crate::ir::{TConfig, TExpr, TExprKind, TFunction, TStmt, Type};

impl TirCodeGen {
    /// Emit forward declaration for a function
    pub(super) fn emit_forward_decl(&mut self, f: &TFunction) -> Result<(), String> {
        let ret_type = self.type_to_c(&f.return_type);
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
            .collect();

        self.emit_line(&format!("{} {}({});", ret_type, f.name, params.join(", ")));
        Ok(())
    }

    /// Emit a config variable
    pub(super) fn emit_config(&mut self, c: &TConfig) -> Result<(), String> {
        let ty = self.type_to_c(&c.ty);
        let value = self.expr_to_c(&c.value)?;

        // Use const for configs
        if ty == "String" {
            // Handle string configs specially
            if let TExprKind::String(s) = &c.value.kind {
                self.emit_line(&format!(
                    "String {} = {{ .data = \"{}\", .len = {} }};",
                    c.name,
                    s,
                    s.len()
                ));
            } else {
                self.emit_line(&format!("String {} = {};", c.name, value));
            }
        } else {
            self.emit_line(&format!("const {} {} = {};", ty, c.name, value));
        }
        Ok(())
    }

    /// Emit a function definition
    pub(super) fn emit_function(&mut self, f: &TFunction) -> Result<(), String> {
        let ret_type = self.type_to_c(&f.return_type);
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
            .collect();

        let params_str = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };

        // main is special
        if f.name == "main" {
            self.emit_line("int main(void) {");
            self.indent();
            self.emit_block(&f.body)?;
            self.emit_line("return 0;");
            self.dedent();
            self.emit_line("}");
        } else {
            self.emit_line(&format!("{} {}({}) {{", ret_type, f.name, params_str));
            self.indent();

            // Emit local variable declarations
            for (_id, info) in f.locals.iter() {
                if !info.is_param {
                    let ty = self.type_to_c(&info.ty);
                    let default = self.default_value(&info.ty);
                    self.emit_line(&format!("{} {} = {};", ty, info.name, default));
                }
            }

            if ret_type != "void" {
                let body = self.expr_to_c(&f.body)?;
                self.emit_line(&format!("return {};", body));
            } else {
                self.emit_block(&f.body)?;
            }

            self.dedent();
            self.emit_line("}");
        }
        Ok(())
    }

    /// Emit a block of expressions
    pub(super) fn emit_block(&mut self, expr: &TExpr) -> Result<(), String> {
        match &expr.kind {
            TExprKind::Block(stmts, result) => {
                for stmt in stmts {
                    self.emit_stmt(stmt)?;
                }
                // If the result is not nil/void, emit it as a statement
                if !matches!(result.ty, Type::Void) {
                    self.emit_statement(result)?;
                }
            }
            _ => {
                self.emit_statement(expr)?;
            }
        }
        Ok(())
    }

    /// Emit a statement
    fn emit_stmt(&mut self, stmt: &TStmt) -> Result<(), String> {
        match stmt {
            TStmt::Expr(expr) => self.emit_statement(expr),
            TStmt::Let { local, value } => {
                // Local variable is already declared, just assign
                let val = self.expr_to_c(value)?;
                // Get the local name from the function's local table
                // For now, we'll use a generic name format
                self.emit_line(&format!("__local_{} = {};", local.0, val));
                Ok(())
            }
        }
    }

    /// Emit a statement from an expression
    pub(super) fn emit_statement(&mut self, expr: &TExpr) -> Result<(), String> {
        match &expr.kind {
            TExprKind::Call { func, args } => {
                match func {
                    crate::ir::FuncRef::Builtin(name) if name == "print" => {
                        // Special handling for print
                        if let Some(arg) = args.first() {
                            let arg_c = self.expr_to_c(arg)?;
                            if self.is_string_type(&arg.ty) {
                                self.emit_line(&format!("printf(\"%s\\n\", {}.data);", arg_c));
                            } else if matches!(arg.ty, Type::Float) {
                                self.emit_line(&format!("printf(\"%f\\n\", {});", arg_c));
                            } else {
                                self.emit_line(&format!("printf(\"%ld\\n\", (long){});", arg_c));
                            }
                        }
                        return Ok(());
                    }
                    _ => {}
                }
                let call = self.expr_to_c(expr)?;
                self.emit_line(&format!("{};", call));
            }

            TExprKind::For { binding, iter, body } => {
                // Emit a for loop
                let iter_c = self.expr_to_c(iter)?;
                self.emit_line(&format!(
                    "for (int __i = 0; __i < len({}); __i++) {{",
                    iter_c
                ));
                self.indent();
                self.emit_line(&format!("__local_{} = {}[__i];", binding.0, iter_c));
                self.emit_block(body)?;
                self.dedent();
                self.emit_line("}");
            }

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_c = self.expr_to_c(cond)?;
                self.emit_line(&format!("if ({}) {{", cond_c));
                self.indent();
                self.emit_block(then_branch)?;
                self.dedent();
                self.emit_line("} else {");
                self.indent();
                self.emit_block(else_branch)?;
                self.dedent();
                self.emit_line("}");
            }

            TExprKind::Assign { target, value } => {
                let val = self.expr_to_c(value)?;
                self.emit_line(&format!("__local_{} = {};", target.0, val));
            }

            _ => {
                let code = self.expr_to_c(expr)?;
                if !code.is_empty() {
                    self.emit_line(&format!("{};", code));
                }
            }
        }
        Ok(())
    }
}
