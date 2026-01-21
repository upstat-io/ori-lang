// Pretty printing for Sigil TIR
// Used for debugging and IR dumps

use super::expr::{FuncRef, TExpr, TExprKind, TMatch, TMatchPattern, TStmt};
use super::module::{TConfig, TFunction, TModule, TTest, TTypeDef, TTypeDefKind};
use super::patterns::TPattern;
use std::fmt;

/// Configuration for TIR display
pub struct DisplayConfig {
    pub show_types: bool,
    pub show_spans: bool,
    pub indent: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        DisplayConfig {
            show_types: true,
            show_spans: false,
            indent: 2,
        }
    }
}

/// Pretty printer for TIR
pub struct TIRPrinter {
    config: DisplayConfig,
    output: String,
    current_indent: usize,
}

impl TIRPrinter {
    pub fn new(config: DisplayConfig) -> Self {
        TIRPrinter {
            config,
            output: String::new(),
            current_indent: 0,
        }
    }

    pub fn print_module(&mut self, module: &TModule) -> &str {
        self.writeln(&format!("// Module: {}", module.name));
        self.writeln("");

        // Types
        for ty in &module.types {
            self.print_typedef(ty);
            self.writeln("");
        }

        // Configs
        for config in &module.configs {
            self.print_config(config);
            self.writeln("");
        }

        // Functions
        for func in &module.functions {
            self.print_function(func);
            self.writeln("");
        }

        // Tests
        for test in &module.tests {
            self.print_test(test);
            self.writeln("");
        }

        &self.output
    }

    fn print_typedef(&mut self, td: &TTypeDef) {
        let vis = if td.public { "pub " } else { "" };
        match &td.kind {
            TTypeDefKind::Alias(ty) => {
                self.writeln(&format!("{}type {} = {}", vis, td.name, ty));
            }
            TTypeDefKind::Struct(fields) => {
                self.writeln(&format!("{}type {} {{", vis, td.name));
                self.indent();
                for field in fields {
                    self.writeln(&format!("{}: {},", field.name, field.ty));
                }
                self.dedent();
                self.writeln("}");
            }
            TTypeDefKind::Enum(variants) => {
                self.writeln(&format!("{}type {} =", vis, td.name));
                self.indent();
                for (i, variant) in variants.iter().enumerate() {
                    let prefix = if i == 0 { "  " } else { "| " };
                    if variant.fields.is_empty() {
                        self.writeln(&format!("{}{}", prefix, variant.name));
                    } else {
                        let fields: Vec<_> = variant
                            .fields
                            .iter()
                            .map(|f| format!("{}: {}", f.name, f.ty))
                            .collect();
                        self.writeln(&format!("{}{} {{ {} }}", prefix, variant.name, fields.join(", ")));
                    }
                }
                self.dedent();
            }
        }
    }

    fn print_config(&mut self, config: &TConfig) {
        self.write(&format!("${}: {} = ", config.name, config.ty));
        self.print_expr(&config.value);
        self.writeln("");
    }

    fn print_function(&mut self, func: &TFunction) {
        let vis = if func.public { "pub " } else { "" };
        let params: Vec<_> = func
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();
        self.write(&format!(
            "{}@{}({}) -> {} = ",
            vis,
            func.name,
            params.join(", "),
            func.return_type
        ));
        self.print_expr(&func.body);
        self.writeln("");
    }

    fn print_test(&mut self, test: &TTest) {
        self.write(&format!("@{} tests @{} = ", test.name, test.target));
        self.print_expr(&test.body);
        self.writeln("");
    }

    fn print_expr(&mut self, expr: &TExpr) {
        match &expr.kind {
            TExprKind::Int(n) => self.write(&n.to_string()),
            TExprKind::Float(f) => self.write(&f.to_string()),
            TExprKind::String(s) => self.write(&format!("\"{}\"", s)),
            TExprKind::Bool(b) => self.write(&b.to_string()),
            TExprKind::Nil => self.write("nil"),

            TExprKind::Local(id) => self.write(&format!("local_{}", id.0)),
            TExprKind::Param(idx) => self.write(&format!("param_{}", idx)),
            TExprKind::Config(name) => self.write(&format!("${}", name)),

            TExprKind::List(elems) => {
                self.write("[");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(elem);
                }
                self.write("]");
            }

            TExprKind::MapLiteral(entries) => {
                self.write("{");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(k);
                    self.write(": ");
                    self.print_expr(v);
                }
                self.write("}");
            }

            TExprKind::Tuple(elems) => {
                self.write("(");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(elem);
                }
                self.write(")");
            }

            TExprKind::Struct { name, fields } => {
                self.write(&format!("{} {{ ", name));
                for (i, (field_name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&format!("{}: ", field_name));
                    self.print_expr(value);
                }
                self.write(" }");
            }

            TExprKind::Binary { op, left, right } => {
                self.write("(");
                self.print_expr(left);
                self.write(&format!(" {:?} ", op));
                self.print_expr(right);
                self.write(")");
            }

            TExprKind::Unary { op, operand } => {
                self.write(&format!("{:?}", op));
                self.print_expr(operand);
            }

            TExprKind::Field(obj, field) => {
                self.print_expr(obj);
                self.write(&format!(".{}", field));
            }

            TExprKind::Index(obj, idx) => {
                self.print_expr(obj);
                self.write("[");
                self.print_expr(idx);
                self.write("]");
            }

            TExprKind::LengthOf(obj) => {
                self.write("len(");
                self.print_expr(obj);
                self.write(")");
            }

            TExprKind::Call { func, args } => {
                match func {
                    FuncRef::User(name) => self.write(name),
                    FuncRef::Builtin(name) => self.write(&format!("builtin:{}", name)),
                    FuncRef::Operator(op) => self.write(&format!("op:{:?}", op)),
                }
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(arg);
                }
                self.write(")");
            }

            TExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.print_expr(receiver);
                self.write(&format!(".{}(", method));
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(arg);
                }
                self.write(")");
            }

            TExprKind::Lambda {
                params,
                body,
                captures,
            } => {
                if !captures.is_empty() {
                    let caps: Vec<_> = captures.iter().map(|c| format!("l{}", c.0)).collect();
                    self.write(&format!("[{}] ", caps.join(", ")));
                }
                let ps: Vec<_> = params.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
                self.write(&format!("({}) -> ", ps.join(", ")));
                self.print_expr(body);
            }

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.write("if ");
                self.print_expr(cond);
                self.write(" then ");
                self.print_expr(then_branch);
                self.write(" else ");
                self.print_expr(else_branch);
            }

            TExprKind::Match(m) => {
                self.print_match(m);
            }

            TExprKind::Block(stmts, result) => {
                self.write("run(");
                self.indent();
                for stmt in stmts {
                    self.writeln("");
                    self.print_stmt(stmt);
                    self.write(",");
                }
                self.writeln("");
                self.print_expr(result);
                self.dedent();
                self.writeln("");
                self.write(")");
            }

            TExprKind::For { binding, iter, body } => {
                self.write(&format!("for local_{} in ", binding.0));
                self.print_expr(iter);
                self.write(" { ");
                self.print_expr(body);
                self.write(" }");
            }

            TExprKind::Assign { target, value } => {
                self.write(&format!("local_{} := ", target.0));
                self.print_expr(value);
            }

            TExprKind::Range { start, end } => {
                self.print_expr(start);
                self.write("..");
                self.print_expr(end);
            }

            TExprKind::Pattern(pattern) => {
                self.print_pattern(pattern);
            }

            TExprKind::Ok(inner) => {
                self.write("Ok(");
                self.print_expr(inner);
                self.write(")");
            }

            TExprKind::Err(inner) => {
                self.write("Err(");
                self.print_expr(inner);
                self.write(")");
            }

            TExprKind::Some(inner) => {
                self.write("Some(");
                self.print_expr(inner);
                self.write(")");
            }

            TExprKind::None_ => self.write("None"),

            TExprKind::Coalesce { value, default } => {
                self.print_expr(value);
                self.write(" ?? ");
                self.print_expr(default);
            }

            TExprKind::Unwrap(inner) => {
                self.print_expr(inner);
                self.write(".unwrap()");
            }
        }

        if self.config.show_types {
            self.write(&format!(" : {}", expr.ty));
        }
    }

    fn print_stmt(&mut self, stmt: &TStmt) {
        match stmt {
            TStmt::Expr(expr) => self.print_expr(expr),
            TStmt::Let { local, value } => {
                self.write(&format!("local_{} := ", local.0));
                self.print_expr(value);
            }
        }
    }

    fn print_match(&mut self, m: &TMatch) {
        self.write("match ");
        self.print_expr(&m.scrutinee);
        self.writeln(" {");
        self.indent();
        for arm in &m.arms {
            self.print_match_pattern(&arm.pattern);
            self.write(" => ");
            self.print_expr(&arm.body);
            self.writeln(",");
        }
        self.dedent();
        self.write("}");
    }

    fn print_match_pattern(&mut self, pattern: &TMatchPattern) {
        match pattern {
            TMatchPattern::Wildcard => self.write("_"),
            TMatchPattern::Literal(expr) => self.print_expr(expr),
            TMatchPattern::Binding(id, _ty) => self.write(&format!("local_{}", id.0)),
            TMatchPattern::Variant { name, bindings } => {
                self.write(name);
                if !bindings.is_empty() {
                    self.write(" { ");
                    for (i, (field, id, _)) in bindings.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(&format!("{}: local_{}", field, id.0));
                    }
                    self.write(" }");
                }
            }
            TMatchPattern::Condition(expr) => {
                self.write("if ");
                self.print_expr(expr);
            }
        }
    }

    fn print_pattern(&mut self, pattern: &TPattern) {
        match pattern {
            TPattern::Fold {
                collection,
                init,
                op,
                ..
            } => {
                self.write("fold(");
                self.print_expr(collection);
                self.write(", ");
                self.print_expr(init);
                self.write(", ");
                self.print_expr(op);
                self.write(")");
            }
            TPattern::Map {
                collection,
                transform,
                ..
            } => {
                self.write("map(");
                self.print_expr(collection);
                self.write(", ");
                self.print_expr(transform);
                self.write(")");
            }
            TPattern::Filter {
                collection,
                predicate,
                ..
            } => {
                self.write("filter(");
                self.print_expr(collection);
                self.write(", ");
                self.print_expr(predicate);
                self.write(")");
            }
            TPattern::Collect {
                range, transform, ..
            } => {
                self.write("collect(");
                self.print_expr(range);
                self.write(", ");
                self.print_expr(transform);
                self.write(")");
            }
            TPattern::Recurse {
                cond,
                base,
                step,
                memo,
                ..
            } => {
                self.write("recurse(");
                self.print_expr(cond);
                self.write(", ");
                self.print_expr(base);
                self.write(", ");
                self.print_expr(step);
                if *memo {
                    self.write(", .memo: true");
                }
                self.write(")");
            }
            TPattern::Iterate {
                over,
                direction,
                into,
                with,
                ..
            } => {
                self.write("iterate(.over: ");
                self.print_expr(over);
                self.write(&format!(", .direction: {:?}", direction));
                self.write(", .into: ");
                self.print_expr(into);
                self.write(", .with: ");
                self.print_expr(with);
                self.write(")");
            }
            TPattern::Transform { input, steps, .. } => {
                self.write("transform(");
                self.print_expr(input);
                for step in steps {
                    self.write(", ");
                    self.print_expr(step);
                }
                self.write(")");
            }
            TPattern::Count {
                collection,
                predicate,
                ..
            } => {
                self.write("count(");
                self.print_expr(collection);
                self.write(", ");
                self.print_expr(predicate);
                self.write(")");
            }
            TPattern::Parallel { branches, .. } => {
                self.write("parallel(");
                for (i, (name, expr, _)) in branches.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&format!(".{}: ", name));
                    self.print_expr(expr);
                }
                self.write(")");
            }
            TPattern::Find {
                collection,
                predicate,
                default,
                ..
            } => {
                self.write("find(.in: ");
                self.print_expr(collection);
                self.write(", .where: ");
                self.print_expr(predicate);
                if let Some(d) = default {
                    self.write(", .default: ");
                    self.print_expr(d);
                }
                self.write(")");
            }
            TPattern::Try { body, catch, .. } => {
                self.write("try(.body: ");
                self.print_expr(body);
                if let Some(c) = catch {
                    self.write(", .catch: ");
                    self.print_expr(c);
                }
                self.write(")");
            }
            TPattern::Retry {
                operation,
                max_attempts,
                backoff,
                delay_ms,
                ..
            } => {
                self.write("retry(.op: ");
                self.print_expr(operation);
                self.write(", .times: ");
                self.print_expr(max_attempts);
                self.write(&format!(", .backoff: {:?}", backoff));
                if let Some(d) = delay_ms {
                    self.write(", .delay: ");
                    self.print_expr(d);
                }
                self.write(")");
            }
            TPattern::Validate {
                rules,
                then_value,
                ..
            } => {
                self.write("validate(.rules: [");
                for (i, (cond, msg)) in rules.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.print_expr(cond);
                    self.write(" | ");
                    self.print_expr(msg);
                }
                self.write("], .then: ");
                self.print_expr(then_value);
                self.write(")");
            }
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        if !s.is_empty() {
            for _ in 0..self.current_indent {
                self.output.push(' ');
            }
            self.output.push_str(s);
        }
        self.output.push('\n');
    }

    fn indent(&mut self) {
        self.current_indent += self.config.indent;
    }

    fn dedent(&mut self) {
        self.current_indent = self.current_indent.saturating_sub(self.config.indent);
    }
}

/// Display trait implementation for TModule
impl fmt::Display for TModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printer = TIRPrinter::new(DisplayConfig {
            show_types: false,
            ..Default::default()
        });
        write!(f, "{}", printer.print_module(self))
    }
}

/// Dump TIR with full type information (for debugging)
pub fn dump_tir(module: &TModule) -> String {
    let mut printer = TIRPrinter::new(DisplayConfig::default());
    printer.print_module(module).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_config_default() {
        let config = DisplayConfig::default();
        assert!(config.show_types);
        assert!(!config.show_spans);
        assert_eq!(config.indent, 2);
    }
}
