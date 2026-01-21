// Name resolution pass for the Sigil compiler
//
// Performs two-pass name resolution:
// 1. Collect all declarations (enables forward references)
// 2. Resolve all identifier references

use std::collections::HashMap;

use crate::ast::{ConfigDef, Expr, FunctionDef, Item, Module, TestDef, TypeDef, TypeDefKind, TypeExpr};
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticCollector, Span};

use super::id::{NodeId, SymbolId};
use super::scope::{ScopeKind, ScopeTree};
use super::symbol::{
    ConfigSymbol, FunctionSymbol, LocalSymbol, Symbol, SymbolKind,
    TypeDefKind as SymbolTypeDefKind, TypeSymbol,
};
use super::table::SymbolTable;

/// Result of name resolution.
pub struct ResolvedModule {
    /// The symbol table containing all definitions
    pub symbols: SymbolTable,
    /// The scope tree used during resolution
    pub scopes: ScopeTree,
    /// Mapping from AST node IDs to resolved symbols
    pub resolutions: HashMap<NodeId, SymbolId>,
    /// Any diagnostics produced during resolution
    pub diagnostics: DiagnosticCollector,
}

/// Name resolver for a module.
pub struct Resolver {
    /// Symbol table
    symbols: SymbolTable,
    /// Scope tree
    scopes: ScopeTree,
    /// AST node to symbol mappings
    resolutions: HashMap<NodeId, SymbolId>,
    /// Diagnostics collector
    diagnostics: DiagnosticCollector,
    /// The source filename (for error reporting)
    filename: String,
    /// Next node ID to assign
    next_node_id: u32,
}

impl Resolver {
    /// Create a new resolver.
    pub fn new(filename: &str) -> Self {
        Resolver {
            symbols: SymbolTable::new(),
            scopes: ScopeTree::new(),
            resolutions: HashMap::new(),
            diagnostics: DiagnosticCollector::new(),
            filename: filename.to_string(),
            next_node_id: 0,
        }
    }

    /// Resolve a module.
    pub fn resolve(mut self, module: &Module) -> ResolvedModule {
        // Pass 1: Collect all top-level declarations
        self.collect_declarations(module);

        // Pass 2: Resolve all references
        self.resolve_references(module);

        ResolvedModule {
            symbols: self.symbols,
            scopes: self.scopes,
            resolutions: self.resolutions,
            diagnostics: self.diagnostics,
        }
    }

    /// Generate a new node ID.
    #[allow(dead_code)]
    fn new_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    // =========================================================================
    // Pass 1: Declaration Collection
    // =========================================================================

    fn collect_declarations(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Function(func) => self.collect_function(func),
                Item::Test(test) => self.collect_test(test),
                Item::Config(config) => self.collect_config(config),
                Item::TypeDef(typedef) => self.collect_typedef(typedef),
                Item::Use(_) => {} // Handle imports later
                Item::Trait(trait_def) => self.collect_trait(trait_def),
                Item::Impl(impl_block) => self.collect_impl(impl_block),
            }
        }
    }

    fn collect_function(&mut self, func: &FunctionDef) {
        let func_symbol = FunctionSymbol::new(
            func.type_params.clone(),
            func.params.iter().map(|p| p.name.clone()).collect(),
            func.params.iter().map(|p| p.ty.clone()).collect(),
            func.return_type.clone(),
        );

        let symbol = Symbol::new(func.name.clone(), SymbolKind::Function(func_symbol))
            .at_span(func.span.clone());

        let id = self.symbols.insert(symbol);
        self.scopes.define(func.name.clone(), id);
    }

    fn collect_test(&mut self, test: &TestDef) {
        let mut func_symbol = FunctionSymbol::test(test.target.clone());
        func_symbol.is_test = true;

        let symbol = Symbol::new(test.name.clone(), SymbolKind::Function(func_symbol));

        let id = self.symbols.insert(symbol);
        self.scopes.define(test.name.clone(), id);
    }

    fn collect_config(&mut self, config: &ConfigDef) {
        let config_symbol = ConfigSymbol {
            ty: config.ty.clone().unwrap_or_else(|| TypeExpr::Named("void".to_string())),
        };

        let symbol = Symbol::new(config.name.clone(), SymbolKind::Config(config_symbol));

        let id = self.symbols.insert(symbol);
        self.scopes.define(config.name.clone(), id);
    }

    fn collect_typedef(&mut self, typedef: &TypeDef) {
        let kind = match &typedef.kind {
            TypeDefKind::Struct(fields) => SymbolTypeDefKind::Struct {
                fields: fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
            },
            TypeDefKind::Enum(variants) => SymbolTypeDefKind::Enum {
                variants: variants
                    .iter()
                    .map(|v| super::symbol::EnumVariant {
                        name: v.name.clone(),
                        fields: v.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
                    })
                    .collect(),
            },
            TypeDefKind::Alias(target) => SymbolTypeDefKind::Alias {
                target: target.clone(),
            },
        };

        let type_symbol = TypeSymbol {
            type_params: typedef.params.clone(),
            kind,
        };

        let symbol = Symbol::new(typedef.name.clone(), SymbolKind::Type(type_symbol));

        let id = self.symbols.insert(symbol);
        self.scopes.define(typedef.name.clone(), id);
    }

    fn collect_trait(&mut self, trait_def: &crate::ast::TraitDef) {
        // Create trait symbol
        let trait_methods: Vec<super::symbol::TraitMethod> = trait_def
            .methods
            .iter()
            .map(|m| super::symbol::TraitMethod {
                name: m.name.clone(),
                param_types: m.params.iter().map(|p| p.ty.clone()).collect(),
                return_type: m.return_type.clone(),
                has_default: m.default_body.is_some(),
            })
            .collect();

        let trait_symbol = super::symbol::TraitSymbol {
            type_params: trait_def.type_params.clone(),
            methods: trait_methods,
            associated_types: trait_def.associated_types.iter().map(|at| at.name.clone()).collect(),
            supertraits: trait_def.supertraits.clone(),
        };

        let symbol = Symbol::new(trait_def.name.clone(), SymbolKind::Trait(trait_symbol))
            .at_span(trait_def.span.clone());

        let id = self.symbols.insert(symbol);
        self.scopes.define(trait_def.name.clone(), id);
    }

    fn collect_impl(&mut self, impl_block: &crate::ast::ImplBlock) {
        // Register functions from impl block
        for method in &impl_block.methods {
            // Impl methods are registered with qualified names or just function names
            let func_symbol = FunctionSymbol::new(
                method.type_params.clone(),
                method.params.iter().map(|p| p.name.clone()).collect(),
                method.params.iter().map(|p| p.ty.clone()).collect(),
                method.return_type.clone(),
            );

            let symbol = Symbol::new(method.name.clone(), SymbolKind::Function(func_symbol))
                .at_span(method.span.clone());

            let id = self.symbols.insert(symbol);
            // Note: We don't add to scope directly - method resolution happens differently
            let _ = id;
        }
    }

    // =========================================================================
    // Pass 2: Reference Resolution
    // =========================================================================

    fn resolve_references(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                Item::Function(func) => self.resolve_function(func),
                Item::Test(test) => self.resolve_test(test),
                Item::Config(config) => self.resolve_config(config),
                Item::TypeDef(typedef) => self.resolve_typedef(typedef),
                Item::Use(_) => {}
                Item::Trait(trait_def) => self.resolve_trait(trait_def),
                Item::Impl(impl_block) => self.resolve_impl(impl_block),
            }
        }
    }

    fn resolve_function(&mut self, func: &FunctionDef) {
        // Enter function scope
        self.scopes.enter(ScopeKind::Function);

        // Add type parameters to scope
        for type_param in &func.type_params {
            let symbol = Symbol::new(
                type_param.clone(),
                SymbolKind::TypeParam(super::symbol::TypeParamSymbol { bounds: vec![] }),
            );
            let id = self.symbols.insert(symbol);
            self.scopes.define(type_param.clone(), id);
        }

        // Add parameters to scope
        for param in &func.params {
            let local_symbol = LocalSymbol {
                ty: param.ty.clone(),
                mutable: false,
                scope: self.scopes.current(),
            };
            let symbol = Symbol::new(param.name.clone(), SymbolKind::Local(local_symbol));
            let id = self.symbols.insert(symbol);
            self.scopes.define(param.name.clone(), id);
        }

        // Resolve the body
        self.resolve_expr(&func.body.expr);

        self.scopes.exit();
    }

    fn resolve_test(&mut self, test: &TestDef) {
        // Verify target function exists
        if self.scopes.lookup(&test.target).is_none() {
            self.diagnostics.push(
                Diagnostic::error(
                    ErrorCode::E3011,
                    format!(
                        "test '{}' references unknown function '{}'",
                        test.name, test.target
                    ),
                )
                .with_label(
                    Span::new(&self.filename, 0..0),
                    format!("function '{}' not found", test.target),
                ),
            );
        }

        // Enter test scope and resolve body
        self.scopes.enter(ScopeKind::Function);
        self.resolve_expr(&test.body.expr);
        self.scopes.exit();
    }

    fn resolve_config(&mut self, config: &ConfigDef) {
        // Resolve type references in the config type
        if let Some(ty) = &config.ty {
            self.resolve_type(ty);
        }
        // Resolve the initializer expression
        self.resolve_expr(&config.value.expr);
    }

    fn resolve_typedef(&mut self, typedef: &TypeDef) {
        // Enter a scope for type parameters
        self.scopes.enter(ScopeKind::Block);

        for type_param in &typedef.params {
            let symbol = Symbol::new(
                type_param.clone(),
                SymbolKind::TypeParam(super::symbol::TypeParamSymbol { bounds: vec![] }),
            );
            let id = self.symbols.insert(symbol);
            self.scopes.define(type_param.clone(), id);
        }

        // Resolve type references in the definition
        match &typedef.kind {
            TypeDefKind::Struct(fields) => {
                for field in fields {
                    self.resolve_type(&field.ty);
                }
            }
            TypeDefKind::Enum(variants) => {
                for variant in variants {
                    for field in &variant.fields {
                        self.resolve_type(&field.ty);
                    }
                }
            }
            TypeDefKind::Alias(target) => {
                self.resolve_type(target);
            }
        }

        self.scopes.exit();
    }

    fn resolve_trait(&mut self, trait_def: &crate::ast::TraitDef) {
        // Enter a scope for type parameters
        self.scopes.enter(ScopeKind::Block);

        for type_param in &trait_def.type_params {
            let symbol = Symbol::new(
                type_param.clone(),
                SymbolKind::TypeParam(super::symbol::TypeParamSymbol { bounds: vec![] }),
            );
            let id = self.symbols.insert(symbol);
            self.scopes.define(type_param.clone(), id);
        }

        // Resolve supertrait references
        for supertrait in &trait_def.supertraits {
            if self.scopes.lookup(supertrait).is_none() {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E3003,
                    format!("cannot find trait '{}' in this scope", supertrait),
                ));
            }
        }

        // Resolve associated types
        for assoc_type in &trait_def.associated_types {
            for bound in &assoc_type.bounds {
                if self.scopes.lookup(bound).is_none() && !is_builtin_type(bound) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3003,
                        format!("cannot find trait '{}' in this scope", bound),
                    ));
                }
            }
            if let Some(default) = &assoc_type.default {
                self.resolve_type(default);
            }
        }

        // Resolve method signatures and default bodies
        for method in &trait_def.methods {
            self.scopes.enter(ScopeKind::Function);

            // Add method type parameters
            for type_param in &method.type_params {
                let symbol = Symbol::new(
                    type_param.clone(),
                    SymbolKind::TypeParam(super::symbol::TypeParamSymbol { bounds: vec![] }),
                );
                let id = self.symbols.insert(symbol);
                self.scopes.define(type_param.clone(), id);
            }

            // Add parameters
            for param in &method.params {
                self.resolve_type(&param.ty);
            }

            self.resolve_type(&method.return_type);

            // Resolve default body if present
            if let Some(body) = &method.default_body {
                self.resolve_expr(&body.expr);
            }

            self.scopes.exit();
        }

        self.scopes.exit();
    }

    fn resolve_impl(&mut self, impl_block: &crate::ast::ImplBlock) {
        // Enter a scope for type parameters
        self.scopes.enter(ScopeKind::Block);

        for type_param in &impl_block.type_params {
            let symbol = Symbol::new(
                type_param.clone(),
                SymbolKind::TypeParam(super::symbol::TypeParamSymbol { bounds: vec![] }),
            );
            let id = self.symbols.insert(symbol);
            self.scopes.define(type_param.clone(), id);
        }

        // Resolve trait name if present
        if let Some(trait_name) = &impl_block.trait_name {
            if self.scopes.lookup(trait_name).is_none() {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E3003,
                    format!("cannot find trait '{}' in this scope", trait_name),
                ));
            }
        }

        // Resolve for_type
        self.resolve_type(&impl_block.for_type);

        // Resolve where clause
        for bound in &impl_block.where_clause {
            for trait_name in &bound.bounds {
                if self.scopes.lookup(trait_name).is_none() && !is_builtin_type(trait_name) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3003,
                        format!("cannot find trait '{}' in this scope", trait_name),
                    ));
                }
            }
        }

        // Resolve associated type implementations
        for assoc_type in &impl_block.associated_types {
            self.resolve_type(&assoc_type.ty);
        }

        // Resolve methods
        for method in &impl_block.methods {
            self.resolve_function(method);
        }

        self.scopes.exit();
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                self.resolve_identifier(name);
            }

            Expr::Config(name) => {
                // Config references should exist
                if self.scopes.lookup(name).is_none() {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3002,
                        format!("cannot find config '{}' in this scope", name),
                    ));
                }
            }

            Expr::Call { func, args } => {
                self.resolve_expr(func);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }

            Expr::MethodCall { receiver, args, .. } => {
                self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }

            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }

            Expr::Unary { operand, .. } => {
                self.resolve_expr(operand);
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition);
                self.resolve_expr(then_branch);
                if let Some(else_br) = else_branch {
                    self.resolve_expr(else_br);
                }
            }

            Expr::Block(statements) => {
                self.scopes.enter(ScopeKind::Block);
                for stmt in statements {
                    self.resolve_expr(stmt);
                }
                self.scopes.exit();
            }

            Expr::Let { name, mutable, value } => {
                // Resolve the value first
                self.resolve_expr(value);

                // Then add the binding
                let local_symbol = LocalSymbol {
                    ty: TypeExpr::Named("_".to_string()), // Type inferred later
                    mutable: *mutable,
                    scope: self.scopes.current(),
                };
                let symbol = Symbol::new(name.clone(), SymbolKind::Local(local_symbol));
                let id = self.symbols.insert(symbol);
                self.scopes.define(name.clone(), id);
            }

            Expr::Reassign { target, value } => {
                // Check that the variable exists and is mutable
                if let Some((sym_id, _)) = self.scopes.lookup(target) {
                    if let Some(sym) = self.symbols.get(sym_id) {
                        if let Some(local) = sym.kind.as_local() {
                            if !local.mutable {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        ErrorCode::E3006,
                                        format!("cannot assign to immutable variable '{}'", target),
                                    )
                                    .with_help("consider declaring the variable with 'let mut'"),
                                );
                            }
                        }
                    }
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3002,
                        format!("cannot find variable '{}' in this scope", target),
                    ));
                }
                self.resolve_expr(value);
            }

            Expr::For {
                binding,
                iterator,
                body,
            } => {
                self.resolve_expr(iterator);

                self.scopes.enter(ScopeKind::Loop);
                let local_symbol = LocalSymbol {
                    ty: TypeExpr::Named("_".to_string()), // Will be inferred during type checking
                    mutable: false,
                    scope: self.scopes.current(),
                };
                let symbol = Symbol::new(binding.clone(), SymbolKind::Local(local_symbol));
                let id = self.symbols.insert(symbol);
                self.scopes.define(binding.clone(), id);

                self.resolve_expr(body);
                self.scopes.exit();
            }

            Expr::Lambda { params, body } => {
                self.scopes.enter(ScopeKind::Lambda);
                for param in params {
                    let local_symbol = LocalSymbol {
                        ty: TypeExpr::Named("_".to_string()), // Inferred
                        mutable: false,
                        scope: self.scopes.current(),
                    };
                    let symbol = Symbol::new(param.clone(), SymbolKind::Local(local_symbol));
                    let id = self.symbols.insert(symbol);
                    self.scopes.define(param.clone(), id);
                }
                self.resolve_expr(body);
                self.scopes.exit();
            }

            Expr::Match(match_expr) => {
                self.resolve_expr(&match_expr.scrutinee);
                for arm in &match_expr.arms {
                    self.scopes.enter(ScopeKind::Block);
                    // Pattern bindings would be added here
                    self.resolve_expr(&arm.body);
                    self.scopes.exit();
                }
            }

            Expr::Pattern(pattern) => {
                // Resolve expressions in pattern arguments
                self.resolve_pattern(pattern);
            }

            Expr::List(elements) => {
                for elem in elements {
                    self.resolve_expr(elem);
                }
            }

            Expr::Tuple(elements) => {
                for elem in elements {
                    self.resolve_expr(elem);
                }
            }

            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.resolve_expr(k);
                    self.resolve_expr(v);
                }
            }

            Expr::Struct { name, fields } => {
                self.resolve_identifier(name);
                for (_, value) in fields {
                    self.resolve_expr(value);
                }
            }

            Expr::Field(object, _) => {
                self.resolve_expr(object);
            }

            Expr::Index(object, index) => {
                self.resolve_expr(object);
                self.resolve_expr(index);
            }

            Expr::Range { start, end } => {
                self.resolve_expr(start);
                self.resolve_expr(end);
            }

            Expr::Ok(inner) | Expr::Err(inner) | Expr::Some(inner) => {
                self.resolve_expr(inner);
            }

            Expr::Unwrap(inner) => {
                self.resolve_expr(inner);
            }

            Expr::Coalesce { value, default } => {
                self.resolve_expr(value);
                self.resolve_expr(default);
            }

            Expr::With { implementation, body, .. } => {
                self.resolve_expr(implementation);
                self.resolve_expr(body);
            }

            Expr::Await(inner) => {
                self.resolve_expr(inner);
            }

            // Literals and others that don't need resolution
            Expr::Int(_)
            | Expr::Float(_)
            | Expr::String(_)
            | Expr::Bool(_)
            | Expr::Nil
            | Expr::None_
            | Expr::LengthPlaceholder => {}
        }
    }

    fn resolve_identifier(&mut self, name: &str) {
        if self.scopes.lookup(name).is_none() {
            // Check if it's a builtin
            if !is_builtin(name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E3002,
                        format!("cannot find value '{}' in this scope", name),
                    )
                    .with_label(
                        Span::new(&self.filename, 0..0),
                        format!("'{}' not found", name),
                    ),
                );
            }
        }
    }

    fn resolve_pattern(&mut self, pattern: &crate::ast::PatternExpr) {
        use crate::ast::PatternExpr;
        match pattern {
            PatternExpr::Fold { collection, init, op } => {
                self.resolve_expr(collection);
                self.resolve_expr(init);
                self.resolve_expr(op);
            }
            PatternExpr::Map { collection, transform } => {
                self.resolve_expr(collection);
                self.resolve_expr(transform);
            }
            PatternExpr::Filter { collection, predicate } => {
                self.resolve_expr(collection);
                self.resolve_expr(predicate);
            }
            PatternExpr::Collect { range, transform } => {
                self.resolve_expr(range);
                self.resolve_expr(transform);
            }
            PatternExpr::Recurse { condition, base_value, step, .. } => {
                self.resolve_expr(condition);
                self.resolve_expr(base_value);
                self.resolve_expr(step);
            }
            PatternExpr::Iterate { over, into, with, .. } => {
                self.resolve_expr(over);
                self.resolve_expr(into);
                self.resolve_expr(with);
            }
            PatternExpr::Transform { input, steps } => {
                self.resolve_expr(input);
                for step in steps {
                    self.resolve_expr(step);
                }
            }
            PatternExpr::Count { collection, predicate } => {
                self.resolve_expr(collection);
                self.resolve_expr(predicate);
            }
            PatternExpr::Parallel { branches, timeout, .. } => {
                for (_, expr) in branches {
                    self.resolve_expr(expr);
                }
                if let Some(timeout) = timeout {
                    self.resolve_expr(timeout);
                }
            }
            PatternExpr::Find { collection, predicate, default } => {
                self.resolve_expr(collection);
                self.resolve_expr(predicate);
                if let Some(d) = default {
                    self.resolve_expr(d);
                }
            }
            PatternExpr::Try { body, catch } => {
                self.resolve_expr(body);
                if let Some(c) = catch {
                    self.resolve_expr(c);
                }
            }
            PatternExpr::Retry { operation, max_attempts, delay_ms, .. } => {
                self.resolve_expr(operation);
                self.resolve_expr(max_attempts);
                if let Some(d) = delay_ms {
                    self.resolve_expr(d);
                }
            }
            PatternExpr::Validate { rules, then_value } => {
                for (cond, msg) in rules {
                    self.resolve_expr(cond);
                    self.resolve_expr(msg);
                }
                self.resolve_expr(then_value);
            }
        }
    }

    fn resolve_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(name) => {
                // Check if type exists
                if self.scopes.lookup(name).is_none() && !is_builtin_type(name) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3003,
                        format!("cannot find type '{}' in this scope", name),
                    ));
                }
            }
            TypeExpr::Generic(name, args) => {
                if self.scopes.lookup(name).is_none() && !is_builtin_type(name) {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3003,
                        format!("cannot find type '{}' in this scope", name),
                    ));
                }
                for arg in args {
                    self.resolve_type(arg);
                }
            }
            TypeExpr::List(inner) | TypeExpr::Optional(inner) | TypeExpr::Async(inner) => {
                self.resolve_type(inner);
            }
            TypeExpr::Tuple(elements) => {
                for elem in elements {
                    self.resolve_type(elem);
                }
            }
            TypeExpr::Map(key, value) => {
                self.resolve_type(key);
                self.resolve_type(value);
            }
            TypeExpr::Function(param, ret) => {
                self.resolve_type(param);
                self.resolve_type(ret);
            }
            TypeExpr::Record(fields) => {
                for (_, ty) in fields {
                    self.resolve_type(ty);
                }
            }
            TypeExpr::DynTrait(trait_name) => {
                // Check if the trait exists
                if self.scopes.lookup(trait_name).is_none() {
                    self.diagnostics.push(Diagnostic::error(
                        ErrorCode::E3003,
                        format!("cannot find trait '{}' in this scope", trait_name),
                    ));
                }
            }
        }
    }
}

/// Check if a name is a builtin function.
fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "println"
            | "len"
            | "push"
            | "pop"
            | "keys"
            | "values"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "assert_err"
            | "str"
            | "int"
            | "float"
            | "bool"
            | "type_of"
            | "range"
            | "enumerate"
            | "zip"
            | "all"
            | "any"
            | "sum"
            | "min"
            | "max"
            | "abs"
            | "sqrt"
            | "floor"
            | "ceil"
            | "round"
            | "run"
    )
}

/// Check if a name is a builtin type.
fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "int" | "float" | "str" | "bool" | "void" | "Result" | "Option" | "_"
    )
}

/// Resolve a module (convenience function).
pub fn resolve(module: &Module, filename: &str) -> ResolvedModule {
    let resolver = Resolver::new(filename);
    resolver.resolve(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_module(items: Vec<Item>) -> Module {
        Module {
            name: "test".to_string(),
            items,
        }
    }

    fn make_simple_function(name: &str) -> Item {
        Item::Function(FunctionDef {
            public: false,
            name: name.to_string(),
            type_params: vec![],
            type_param_bounds: vec![],
            where_clause: vec![],
            uses_clause: vec![],
            params: vec![],
            return_type: TypeExpr::Named("int".to_string()),
            body: SpannedExpr::no_span(Expr::Int(42)),
            span: 0..0,
        })
    }

    #[test]
    fn test_resolve_simple_function() {
        let module = make_module(vec![make_simple_function("foo")]);
        let result = resolve(&module, "test.si");

        assert!(!result.diagnostics.has_errors());
        assert!(result.symbols.lookup("foo").is_some());
    }

    #[test]
    fn test_resolve_function_with_params() {
        let module = make_module(vec![Item::Function(FunctionDef {
            public: false,
            name: "add".to_string(),
            type_params: vec![],
            type_param_bounds: vec![],
            where_clause: vec![],
            uses_clause: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: TypeExpr::Named("int".to_string()),
                },
                Param {
                    name: "b".to_string(),
                    ty: TypeExpr::Named("int".to_string()),
                },
            ],
            return_type: TypeExpr::Named("int".to_string()),
            body: SpannedExpr::no_span(Expr::Binary {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinaryOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            }),
            span: 0..0,
        })]);

        let result = resolve(&module, "test.si");
        assert!(!result.diagnostics.has_errors());
    }

    #[test]
    fn test_undefined_variable_error() {
        let module = make_module(vec![Item::Function(FunctionDef {
            public: false,
            name: "foo".to_string(),
            type_params: vec![],
            type_param_bounds: vec![],
            where_clause: vec![],
            uses_clause: vec![],
            params: vec![],
            return_type: TypeExpr::Named("int".to_string()),
            body: SpannedExpr::no_span(Expr::Ident("undefined_var".to_string())),
            span: 0..0,
        })]);

        let result = resolve(&module, "test.si");
        assert!(result.diagnostics.has_errors());
        assert_eq!(result.diagnostics.error_count(), 1);
    }

    #[test]
    fn test_forward_reference() {
        // foo calls bar, but bar is defined after foo
        let module = make_module(vec![
            Item::Function(FunctionDef {
                public: false,
                name: "foo".to_string(),
                type_params: vec![],
                type_param_bounds: vec![],
                where_clause: vec![],
                uses_clause: vec![],
                params: vec![],
                return_type: TypeExpr::Named("int".to_string()),
                body: SpannedExpr::no_span(Expr::Call {
                    func: Box::new(Expr::Ident("bar".to_string())),
                    args: vec![],
                }),
                span: 0..0,
            }),
            Item::Function(FunctionDef {
                public: false,
                name: "bar".to_string(),
                type_params: vec![],
                type_param_bounds: vec![],
                where_clause: vec![],
                uses_clause: vec![],
                params: vec![],
                return_type: TypeExpr::Named("int".to_string()),
                body: SpannedExpr::no_span(Expr::Int(42)),
                span: 0..0,
            }),
        ]);

        let result = resolve(&module, "test.si");
        // Forward references should work
        assert!(!result.diagnostics.has_errors());
    }

    #[test]
    fn test_shadowing() {
        let module = make_module(vec![Item::Function(FunctionDef {
            public: false,
            name: "foo".to_string(),
            type_params: vec![],
            type_param_bounds: vec![],
            where_clause: vec![],
            uses_clause: vec![],
            params: vec![Param {
                name: "x".to_string(),
                ty: TypeExpr::Named("int".to_string()),
            }],
            return_type: TypeExpr::Named("int".to_string()),
            body: SpannedExpr::no_span(Expr::Block(vec![
                Expr::Let {
                    name: "x".to_string(),
                    mutable: false,
                    value: Box::new(Expr::String("hello".to_string())),
                },
                Expr::Int(42),
            ])),
            span: 0..0,
        })]);

        let result = resolve(&module, "test.si");
        // Shadowing is allowed
        assert!(!result.diagnostics.has_errors());
    }
}
