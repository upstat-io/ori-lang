// Code formatter for Sigil
//
// Provides deterministic formatting of Sigil source code.
// The formatter parses source code, then pretty-prints it with
// consistent indentation and style.

use crate::ast::*;

/// Configuration for the pretty printer
#[derive(Clone, Debug)]
pub struct FormatConfig {
    /// Number of spaces for indentation
    pub indent_size: usize,
    /// Maximum line width before wrapping
    pub max_width: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig {
            indent_size: 4,
            max_width: 100,
        }
    }
}

/// Pretty printer for Sigil AST
pub struct PrettyPrinter {
    config: FormatConfig,
    output: String,
    indent: usize,
}

impl PrettyPrinter {
    /// Create a new pretty printer with default config
    pub fn new() -> Self {
        PrettyPrinter {
            config: FormatConfig::default(),
            output: String::new(),
            indent: 0,
        }
    }

    /// Create a pretty printer with custom config
    pub fn with_config(config: FormatConfig) -> Self {
        PrettyPrinter {
            config,
            output: String::new(),
            indent: 0,
        }
    }

    /// Set indentation size
    pub fn with_indent(mut self, size: usize) -> Self {
        self.config.indent_size = size;
        self
    }

    /// Format a module (list of items)
    pub fn format_module(&mut self, items: &[Item]) -> String {
        self.output.clear();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.format_item(item);
        }
        self.output.clone()
    }

    /// Format a single item
    fn format_item(&mut self, item: &Item) {
        match item {
            Item::Function(fd) => self.format_function(fd),
            Item::TypeDef(td) => self.format_type_def(td),
            Item::Config(cd) => self.format_config(cd),
            Item::Test(td) => self.format_test(td),
            Item::Use(ud) => self.format_use(ud),
            Item::Trait(td) => self.format_trait(td),
            Item::Impl(ib) => self.format_impl(ib),
            Item::Extend(eb) => self.format_extend(eb),
            Item::Extension(ei) => self.format_extension(ei),
        }
    }

    /// Format a function definition
    fn format_function(&mut self, fd: &FunctionDef) {
        // Function signature
        if fd.public {
            self.write("pub ");
        }
        self.write("@");
        self.write(&fd.name);

        // Type parameters
        if !fd.type_params.is_empty() {
            self.write("<");
            self.write(&fd.type_params.join(", "));
            self.write(">");
        }

        // Parameters
        self.write(" (");
        for (i, param) in fd.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&param.name);
            self.write(": ");
            self.format_type_expr(&param.ty);
        }
        self.write(")");

        // Return type
        self.write(" -> ");
        self.format_type_expr(&fd.return_type);

        // Uses clause
        if !fd.uses_clause.is_empty() {
            self.write(" uses ");
            self.write(&fd.uses_clause.join(", "));
        }

        // Where clause
        if !fd.where_clause.is_empty() {
            self.write(" where ");
            for (i, bound) in fd.where_clause.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&bound.type_param);
                self.write(": ");
                self.write(&bound.bounds.join(" + "));
            }
        }

        // Body
        self.write(" = ");
        self.format_expr(&fd.body.expr);
        self.newline();
    }

    /// Format a type definition
    fn format_type_def(&mut self, td: &TypeDef) {
        if td.public {
            self.write("pub ");
        }
        self.write("type ");
        self.write(&td.name);

        // Type parameters
        if !td.params.is_empty() {
            self.write("<");
            self.write(&td.params.join(", "));
            self.write(">");
        }

        match &td.kind {
            TypeDefKind::Alias(ty) => {
                self.write(" = ");
                self.format_type_expr(ty);
                self.newline();
            }
            TypeDefKind::Struct(fields) => {
                self.write(" {");
                self.newline();
                self.indent();
                for field in fields {
                    self.write_indent();
                    self.write(&field.name);
                    self.write(": ");
                    self.format_type_expr(&field.ty);
                    self.write(",");
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
                self.newline();
            }
            TypeDefKind::Enum(variants) => {
                self.write(" = ");
                for (i, variant) in variants.iter().enumerate() {
                    if i > 0 {
                        self.write(" | ");
                    }
                    self.write(&variant.name);
                    if !variant.fields.is_empty() {
                        self.write(" { ");
                        for (j, field) in variant.fields.iter().enumerate() {
                            if j > 0 {
                                self.write(", ");
                            }
                            self.write(&field.name);
                            self.write(": ");
                            self.format_type_expr(&field.ty);
                        }
                        self.write(" }");
                    }
                }
                self.newline();
            }
        }
    }

    /// Format a config definition
    fn format_config(&mut self, cd: &ConfigDef) {
        self.write("$");
        self.write(&cd.name);
        if let Some(ref ty) = cd.ty {
            self.write(": ");
            self.format_type_expr(ty);
        }
        self.write(" = ");
        self.format_expr(&cd.value.expr);
        self.newline();
    }

    /// Format a test definition
    fn format_test(&mut self, td: &TestDef) {
        self.write("@");
        self.write(&td.name);
        self.write(" tests @");
        self.write(&td.target);
        self.write(" () -> void = ");
        self.format_expr(&td.body.expr);
        self.newline();
    }

    /// Format a use statement
    fn format_use(&mut self, ud: &UseDef) {
        self.write("use ");
        self.write(&ud.path.join("."));
        if !ud.items.is_empty() {
            self.write(" { ");
            for (i, item) in ud.items.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&item.name);
                if let Some(alias) = &item.alias {
                    self.write(" as ");
                    self.write(alias);
                }
            }
            self.write(" }");
        }
        self.newline();
    }

    /// Format a trait definition
    fn format_trait(&mut self, td: &TraitDef) {
        if td.public {
            self.write("pub ");
        }
        self.write("trait ");
        self.write(&td.name);
        if !td.type_params.is_empty() {
            self.write("<");
            self.write(&td.type_params.join(", "));
            self.write(">");
        }
        if !td.supertraits.is_empty() {
            self.write(": ");
            self.write(&td.supertraits.join(" + "));
        }
        self.write(" {");
        self.newline();
        self.indent();

        // Associated types
        for at in &td.associated_types {
            self.write_indent();
            self.write("type ");
            self.write(&at.name);
            if !at.bounds.is_empty() {
                self.write(": ");
                self.write(&at.bounds.join(" + "));
            }
            if let Some(default) = &at.default {
                self.write(" = ");
                self.format_type_expr(default);
            }
            self.newline();
        }

        // Methods
        for method in &td.methods {
            self.write_indent();
            self.format_trait_method(method);
            self.newline();
        }
        self.dedent();
        self.write("}");
        self.newline();
    }

    /// Format a trait method
    fn format_trait_method(&mut self, m: &TraitMethodDef) {
        self.write("@");
        self.write(&m.name);
        if !m.type_params.is_empty() {
            self.write("<");
            self.write(&m.type_params.join(", "));
            self.write(">");
        }
        self.write(" (");
        for (i, param) in m.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&param.name);
            self.write(": ");
            self.format_type_expr(&param.ty);
        }
        self.write(") -> ");
        self.format_type_expr(&m.return_type);
        if let Some(body) = &m.default_body {
            self.write(" = ");
            self.format_expr(&body.expr);
        }
    }

    /// Format an impl block
    fn format_impl(&mut self, ib: &ImplBlock) {
        self.write("impl");
        if !ib.type_params.is_empty() {
            self.write("<");
            self.write(&ib.type_params.join(", "));
            self.write(">");
        }
        self.write(" ");
        if let Some(trait_name) = &ib.trait_name {
            self.write(trait_name);
            self.write(" for ");
        }
        self.format_type_expr(&ib.for_type);

        if !ib.where_clause.is_empty() {
            self.write(" where ");
            for (i, bound) in ib.where_clause.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&bound.type_param);
                self.write(": ");
                self.write(&bound.bounds.join(" + "));
            }
        }

        self.write(" {");
        self.newline();
        self.indent();

        // Associated types
        for at in &ib.associated_types {
            self.write_indent();
            self.write("type ");
            self.write(&at.name);
            self.write(" = ");
            self.format_type_expr(&at.ty);
            self.newline();
        }

        // Methods
        for func in &ib.methods {
            self.write_indent();
            self.format_function(func);
        }
        self.dedent();
        self.write("}");
        self.newline();
    }

    /// Format an extend block
    fn format_extend(&mut self, eb: &ExtendBlock) {
        self.write("extend ");
        self.write(&eb.trait_name);
        if !eb.where_clause.is_empty() {
            self.write(" where ");
            for (i, bound) in eb.where_clause.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&bound.type_param);
                self.write(": ");
                self.write(&bound.bounds.join(" + "));
            }
        }
        self.write(" {");
        self.newline();
        self.indent();
        for method in &eb.methods {
            self.write_indent();
            self.format_function(method);
        }
        self.dedent();
        self.write("}");
        self.newline();
    }

    /// Format an extension import
    fn format_extension(&mut self, ei: &ExtensionImport) {
        self.write("extension ");
        if ei.path.len() == 1 && ei.path[0].starts_with('"') {
            self.write(&ei.path[0]);
        } else {
            self.write(&ei.path.join("."));
        }
        self.write(" { ");
        for (i, item) in ei.items.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&item.trait_name);
            self.write(".");
            self.write(&item.method_name);
        }
        self.write(" }");
        self.newline();
    }

    /// Format a type expression
    fn format_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(name) => self.write(name),
            TypeExpr::Generic(name, args) => {
                self.write(name);
                self.write("<");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type_expr(arg);
                }
                self.write(">");
            }
            TypeExpr::List(inner) => {
                self.write("[");
                self.format_type_expr(inner);
                self.write("]");
            }
            TypeExpr::Optional(inner) => {
                self.write("?");
                self.format_type_expr(inner);
            }
            TypeExpr::Function(param, ret) => {
                self.write("(");
                self.format_type_expr(param);
                self.write(" -> ");
                self.format_type_expr(ret);
                self.write(")");
            }
            TypeExpr::Tuple(types) => {
                self.write("(");
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type_expr(t);
                }
                self.write(")");
            }
            TypeExpr::Map(k, v) => {
                self.write("{");
                self.format_type_expr(k);
                self.write(": ");
                self.format_type_expr(v);
                self.write("}");
            }
            TypeExpr::Record(fields) => {
                self.write("{ ");
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(name);
                    self.write(": ");
                    self.format_type_expr(ty);
                }
                self.write(" }");
            }
            TypeExpr::DynTrait(name) => {
                self.write("dyn ");
                self.write(name);
            }
        }
    }

    /// Format an expression
    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            // Literals
            Expr::Int(n) => self.write(&n.to_string()),
            Expr::Float(f) => self.write(&f.to_string()),
            Expr::String(s) => {
                self.write("\"");
                self.write(&escape_string(s));
                self.write("\"");
            }
            Expr::Bool(b) => self.write(if *b { "true" } else { "false" }),
            Expr::Nil => self.write("nil"),

            // References
            Expr::Ident(name) => self.write(name),
            Expr::Config(name) => {
                self.write("$");
                self.write(name);
            }
            Expr::LengthPlaceholder => self.write("#"),

            // Collections
            Expr::List(elems) => {
                self.write("[");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(e);
                }
                self.write("]");
            }
            Expr::Tuple(elems) => {
                self.write("(");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(e);
                }
                self.write(")");
            }
            Expr::MapLiteral(entries) => {
                self.write("{");
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(k);
                    self.write(": ");
                    self.format_expr(v);
                }
                self.write("}");
            }
            Expr::Struct { name, fields } => {
                self.write(name);
                self.write(" { ");
                for (i, (fname, val)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(fname);
                    self.write(": ");
                    self.format_expr(val);
                }
                self.write(" }");
            }
            Expr::Range { start, end } => {
                self.format_expr(start);
                self.write("..");
                self.format_expr(end);
            }

            // Access
            Expr::Field(obj, field) => {
                self.format_expr(obj);
                self.write(".");
                self.write(field);
            }
            Expr::Index(obj, idx) => {
                self.format_expr(obj);
                self.write("[");
                self.format_expr(idx);
                self.write("]");
            }

            // Calls
            Expr::Call { func, args } => {
                self.format_expr(func);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.format_expr(receiver);
                self.write(".");
                self.write(method);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }

            // Operations
            Expr::Binary { op, left, right } => {
                self.format_expr(left);
                self.write(" ");
                self.write(format_binary_op(*op));
                self.write(" ");
                self.format_expr(right);
            }
            Expr::Unary { op, operand } => {
                self.write(format_unary_op(*op));
                self.format_expr(operand);
            }

            // Lambda
            Expr::Lambda { params, body } => {
                if params.len() == 1 {
                    self.write(&params[0]);
                } else {
                    self.write("(");
                    self.write(&params.join(", "));
                    self.write(")");
                }
                self.write(" -> ");
                self.format_expr(body);
            }

            // Control flow
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.write("if ");
                self.format_expr(condition);
                self.write(" then ");
                self.format_expr(then_branch);
                if let Some(e) = else_branch {
                    self.newline();
                    self.write_indent();
                    self.write("else ");
                    self.format_expr(e);
                }
            }
            Expr::Match(m) => {
                self.write("match ");
                self.format_expr(&m.scrutinee);
                self.write(" {");
                self.newline();
                self.indent();
                for arm in &m.arms {
                    self.write_indent();
                    self.format_pattern(&arm.pattern);
                    self.write(" => ");
                    self.format_expr(&arm.body);
                    self.write(",");
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Expr::Block(exprs) => {
                self.write("run(");
                self.newline();
                self.indent();
                for (i, e) in exprs.iter().enumerate() {
                    self.write_indent();
                    self.format_expr(e);
                    if i < exprs.len() - 1 {
                        self.write(",");
                    }
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            Expr::For {
                binding,
                iterator,
                body,
            } => {
                self.write("for ");
                self.write(binding);
                self.write(" in ");
                self.format_expr(iterator);
                self.write(" do ");
                self.format_expr(body);
            }

            // Patterns
            Expr::Pattern(p) => self.format_pattern_expr(p),

            // Result/Option
            Expr::Ok(inner) => {
                self.write("Ok(");
                self.format_expr(inner);
                self.write(")");
            }
            Expr::Err(inner) => {
                self.write("Err(");
                self.format_expr(inner);
                self.write(")");
            }
            Expr::Some(inner) => {
                self.write("Some(");
                self.format_expr(inner);
                self.write(")");
            }
            Expr::None_ => self.write("None"),
            Expr::Coalesce { value, default } => {
                self.format_expr(value);
                self.write(" ?? ");
                self.format_expr(default);
            }
            Expr::Unwrap(inner) => {
                self.format_expr(inner);
                self.write(".unwrap()");
            }

            // Bindings
            Expr::Let {
                name,
                mutable,
                value,
            } => {
                self.write("let ");
                if *mutable {
                    self.write("mut ");
                }
                self.write(name);
                self.write(" = ");
                self.format_expr(value);
            }
            Expr::Reassign { target, value } => {
                self.write(target);
                self.write(" = ");
                self.format_expr(value);
            }

            // Capability
            Expr::With {
                capability,
                implementation,
                body,
            } => {
                self.write("with ");
                self.write(capability);
                self.write(" = ");
                self.format_expr(implementation);
                self.write(" in ");
                self.format_expr(body);
            }
        }
    }

    /// Format a match pattern
    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard => self.write("_"),
            Pattern::Literal(e) => self.format_expr(e),
            Pattern::Binding(name) => self.write(name),
            Pattern::Variant { name, fields } => {
                self.write(name);
                if !fields.is_empty() {
                    self.write(" { ");
                    for (i, (fname, pat)) in fields.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(fname);
                        self.write(": ");
                        self.format_pattern(pat);
                    }
                    self.write(" }");
                }
            }
            Pattern::Condition(e) => self.format_expr(e),
        }
    }

    /// Format a pattern expression
    fn format_pattern_expr(&mut self, p: &PatternExpr) {
        match p {
            PatternExpr::Fold {
                collection,
                init,
                op,
            } => {
                self.write("fold(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".over: ");
                self.format_expr(collection);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".init: ");
                self.format_expr(init);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".with: ");
                self.format_expr(op);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Map {
                collection,
                transform,
            } => {
                self.write("map(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".over: ");
                self.format_expr(collection);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".transform: ");
                self.format_expr(transform);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Filter {
                collection,
                predicate,
            } => {
                self.write("filter(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".over: ");
                self.format_expr(collection);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".where: ");
                self.format_expr(predicate);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Recurse {
                condition,
                base_value,
                step,
                memo,
                parallel_threshold,
            } => {
                self.write("recurse(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".cond: ");
                self.format_expr(condition);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".base: ");
                self.format_expr(base_value);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".step: ");
                self.format_expr(step);
                self.write(",");
                if *memo {
                    self.newline();
                    self.write_indent();
                    self.write(".memo: true,");
                }
                if *parallel_threshold > 0 {
                    self.newline();
                    self.write_indent();
                    self.write(".parallel: ");
                    self.write(&parallel_threshold.to_string());
                    self.write(",");
                }
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Collect { range, transform } => {
                self.write("collect(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".range: ");
                self.format_expr(range);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".into: ");
                self.format_expr(transform);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Count {
                collection,
                predicate,
            } => {
                self.write("count(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".over: ");
                self.format_expr(collection);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".where: ");
                self.format_expr(predicate);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Iterate {
                over,
                direction,
                into,
                with,
            } => {
                self.write("iterate(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".over: ");
                self.format_expr(over);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".direction: ");
                match direction {
                    IterDirection::Forward => self.write("forward"),
                    IterDirection::Backward => self.write("backward"),
                }
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".into: ");
                self.format_expr(into);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".with: ");
                self.format_expr(with);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Transform { input, steps } => {
                self.write("transform(");
                self.newline();
                self.indent();
                self.write_indent();
                self.format_expr(input);
                self.write(",");
                for step in steps {
                    self.newline();
                    self.write_indent();
                    self.format_expr(step);
                    self.write(",");
                }
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Parallel {
                branches,
                timeout,
                on_error,
            } => {
                self.write("parallel(");
                self.newline();
                self.indent();
                for (name, expr) in branches {
                    self.write_indent();
                    self.write(".");
                    self.write(name);
                    self.write(": ");
                    self.format_expr(expr);
                    self.write(",");
                    self.newline();
                }
                if let Some(t) = timeout {
                    self.write_indent();
                    self.write(".timeout: ");
                    self.format_expr(t);
                    self.write(",");
                    self.newline();
                }
                match on_error {
                    OnError::FailFast => {} // Default, don't output
                    OnError::CollectAll => {
                        self.write_indent();
                        self.write(".on_error: collect_all,");
                        self.newline();
                    }
                }
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Find {
                collection,
                predicate,
                default,
            } => {
                self.write("find(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".in: ");
                self.format_expr(collection);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".where: ");
                self.format_expr(predicate);
                self.write(",");
                if let Some(d) = default {
                    self.newline();
                    self.write_indent();
                    self.write(".default: ");
                    self.format_expr(d);
                    self.write(",");
                }
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Try { body, catch } => {
                self.write("try(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".body: ");
                self.format_expr(body);
                self.write(",");
                if let Some(c) = catch {
                    self.newline();
                    self.write_indent();
                    self.write(".catch: ");
                    self.format_expr(c);
                    self.write(",");
                }
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Retry {
                operation,
                max_attempts,
                backoff,
                delay_ms,
            } => {
                self.write("retry(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".op: ");
                self.format_expr(operation);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".times: ");
                self.format_expr(max_attempts);
                self.write(",");
                self.newline();
                self.write_indent();
                self.write(".backoff: ");
                match backoff {
                    RetryBackoff::None => self.write("none"),
                    RetryBackoff::Constant => self.write("constant"),
                    RetryBackoff::Linear => self.write("linear"),
                    RetryBackoff::Exponential => self.write("exponential"),
                }
                self.write(",");
                if let Some(d) = delay_ms {
                    self.newline();
                    self.write_indent();
                    self.write(".delay_ms: ");
                    self.format_expr(d);
                    self.write(",");
                }
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
            PatternExpr::Validate { rules, then_value } => {
                self.write("validate(");
                self.newline();
                self.indent();
                self.write_indent();
                self.write(".rules: [");
                self.newline();
                self.indent();
                for (cond, msg) in rules {
                    self.write_indent();
                    self.write("(");
                    self.format_expr(cond);
                    self.write(", ");
                    self.format_expr(msg);
                    self.write("),");
                    self.newline();
                }
                self.dedent();
                self.write_indent();
                self.write("],");
                self.newline();
                self.write_indent();
                self.write(".then: ");
                self.format_expr(then_value);
                self.write(",");
                self.newline();
                self.dedent();
                self.write_indent();
                self.write(")");
            }
        }
    }

    // Helper methods

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..(self.indent * self.config.indent_size) {
            self.output.push(' ');
        }
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }
}

impl Default for PrettyPrinter {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a binary operator
fn format_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::IntDiv => "//",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Pipe => "|>",
    }
}

/// Format a unary operator
fn format_unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

/// Escape special characters in a string
fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            _ => result.push(c),
        }
    }
    result
}

/// Format source code
pub fn format(source: &str) -> Result<String, String> {
    let tokens =
        crate::lexer::tokenize(source, "<format>").map_err(|e| format!("Lexer error: {:?}", e))?;
    let mut parser = crate::parser::Parser::new(tokens);
    let module = parser
        .parse_module("<format>")
        .map_err(|e| format!("Parser error: {}", e))?;

    let mut printer = PrettyPrinter::new();
    Ok(printer.format_module(&module.items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_function() {
        let source = "@add(a:int,b:int)->int=a+b";
        let formatted = format(source).unwrap();
        assert!(formatted.contains("@add (a: int, b: int) -> int = a + b"));
    }

    #[test]
    fn test_format_preserves_semantics() {
        let source = r#"
            @factorial (n: int) -> int = recurse(
                .cond: n <= 1,
                .base: 1,
                .step: n * self(n - 1),
            )
        "#;
        let formatted = format(source).unwrap();
        assert!(formatted.contains("@factorial"));
        assert!(formatted.contains("recurse"));
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("tab\there"), "tab\\there");
    }
}
