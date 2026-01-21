// Declaration emission for C code generation
// Handles forward declarations, config variables, and function definitions

use super::CodeGen;
use crate::ast::*;

impl CodeGen {
    pub(super) fn emit_forward_decl(&mut self, f: &FunctionDef) -> Result<(), String> {
        let ret_type = self.type_to_c(&f.return_type);
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
            .collect();

        self.emit_line(&format!("{} {}({});", ret_type, f.name, params.join(", ")));
        Ok(())
    }

    pub(super) fn emit_config(&mut self, c: &ConfigDef) -> Result<(), String> {
        let ty = c
            .ty
            .as_ref()
            .map(|t| self.type_to_c(t))
            .unwrap_or_else(|| self.infer_c_type(&c.value));

        let value = self.expr_to_c(&c.value)?;

        // Use const for configs
        if ty == "String" {
            self.emit_line(&format!(
                "String {} = {{ .data = \"{}\", .len = {} }};",
                c.name,
                self.extract_string_literal(&c.value).unwrap_or_default(),
                self.extract_string_literal(&c.value)
                    .map(|s| s.len())
                    .unwrap_or(0)
            ));
        } else {
            self.emit_line(&format!("const {} {} = {};", ty, c.name, value));
        }
        Ok(())
    }

    pub(super) fn emit_function(&mut self, f: &FunctionDef) -> Result<(), String> {
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

    pub(super) fn emit_block(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Block(exprs) => {
                for e in exprs {
                    self.emit_statement(e)?;
                }
            }
            _ => {
                self.emit_statement(expr)?;
            }
        }
        Ok(())
    }

    pub(super) fn emit_statement(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Call { func, args } => {
                if let Expr::Ident(name) = func.as_ref() {
                    if name == "print" {
                        // Special handling for print
                        if let Some(arg) = args.first() {
                            let arg_c = self.expr_to_c(arg)?;
                            if self.is_string_expr(arg) {
                                self.emit_line(&format!("printf(\"%s\\n\", {}.data);", arg_c));
                            } else {
                                self.emit_line(&format!("printf(\"%ld\\n\", (long){});", arg_c));
                            }
                        }
                        return Ok(());
                    }
                }
                let call = self.expr_to_c(expr)?;
                self.emit_line(&format!("{};", call));
            }
            _ => {
                let code = self.expr_to_c(expr)?;
                self.emit_line(&format!("{};", code));
            }
        }
        Ok(())
    }
}
